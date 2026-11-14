//! Matched terrain measurements for generated and observed elevation grids.
use super::{OUTLET_HALF_WIDTH_M, SPAN_M};
use crate::geo::{erosion, grid::Grid};
use glam::DVec2;

const CHANNEL_AREA_M2: f64 = 30_000.0;
pub const WIDTH_BINS_KM2: [f64; 3] = [0.03, 0.10, 0.40];

pub struct Measurements {
    pub basin_area_km2: f64,
    pub head_density_km2: f64,
    pub ridge_spacing_m: Option<f64>,
    pub width_m: [Option<f64>; 3],
    pub width_samples: [usize; 3],
    pub prominence_m: Option<f64>,
    pub prominent_peaks: usize,
    pub main_stem_m: f64,
    pub profile_breaks: usize,
}

/// Measure a 4 km square in metres. Without a supplied outlet mask, use the
/// dominant outlet and the same 320 m outlet span as the generated basins.
pub fn measure(height: &[f32], spacing_m: f64, selected: Option<&[bool]>) -> Option<Measurements> {
    measure_region(height, spacing_m, selected, None).map(|(metrics, _)| metrics)
}

/// Select an interior outlet by area, or reuse the exact outlet from a paired run.
/// Reject any basin whose contributing cells reach the sampled window boundary.
pub fn measure_whole(
    height: &[f32], spacing_m: f64, target_km2: f64, outlet: Option<usize>,
) -> Option<(Measurements, usize)> {
    measure_region(height, spacing_m, None, Some((target_km2, outlet)))
}

fn measure_region(
    height: &[f32], spacing_m: f64, selected: Option<&[bool]>,
    whole: Option<(f64, Option<usize>)>,
) -> Option<(Measurements, usize)> {
    if ![2.0, 4.0].contains(&spacing_m) {
        return None;
    }
    let grid = Grid {
        n: (SPAN_M / spacing_m) as usize,
        cell_m: spacing_m,
        min_m: [0.0; 2],
    };
    let len = grid.len();
    if height.len() != len
        || height.iter().any(|h| !h.is_finite())
        || selected.is_some_and(|mask| mask.len() != len)
    {
        return None;
    }
    let fixed: Vec<_> = (0..len).map(|i| grid.on_border(i)).collect();
    let height64: Vec<_> = height.iter().map(|&h| f64::from(h)).collect();
    let flow = erosion::route(&grid, &height64, &fixed, &vec![1.0; len]);
    let mut interior_outlet = None;
    let automatic;
    let selected = if let Some(mask) = selected {
        mask
    } else if let Some((area, outlet)) = whole {
        let outlet = enclosed_outlet(grid, &flow, area, outlet)?;
        interior_outlet = Some(outlet);
        automatic = {
            let mut mask = vec![false; len];
            for &i in &flow.order {
                let i = i as usize;
                mask[i] = i == outlet || mask[flow.receiver[i] as usize];
            }
            mask
        };
        &automatic
    } else {
        let edge = (0..len)
            .filter(|&i| fixed[i])
            .max_by(|&a, &b| flow.area[a].total_cmp(&flow.area[b]))?;
        let [x, z] = grid.at(edge);
        let same_edge = |i: usize| {
            let [a, b] = grid.at(i);
            (x == a || z == b) && (x - a).abs().max((z - b).abs()) <= OUTLET_HALF_WIDTH_M
        };
        automatic = {
            let mut mask = vec![false; len];
            for &i in &flow.order {
                let i = i as usize;
                let r = flow.receiver[i] as usize;
                mask[i] = if i == r { same_edge(i) } else { mask[r] };
            }
            mask
        };
        &automatic
    };
    let basin_area_km2 =
        selected.iter().filter(|&&inside| inside).count() as f64 * spacing_m.powi(2) / 1e6;
    if basin_area_km2 == 0.0 {
        return None;
    }

    let mut donor = vec![u32::MAX; len];
    let mut widths: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::new());
    let channel = |i: usize| selected[i] && flow.area[i] >= CHANNEL_AREA_M2;
    for i in (0..len).filter(|&i| channel(i)) {
        let r = flow.receiver[i] as usize;
        if i == r || !selected[r] {
            continue;
        }
        if donor[r] == u32::MAX || flow.area[donor[r] as usize] < flow.area[i] {
            donor[r] = i as u32;
        }
        let p = DVec2::from_array(grid.at(i));
        let mut end = r;
        while DVec2::from_array(grid.at(end)).distance_squared(p) < 24.0 * 24.0 {
            let next = flow.receiver[end] as usize;
            if next == end {
                break;
            }
            end = next;
        }
        let across = (DVec2::from_array(grid.at(end)) - p).normalize().perp();
        let width =
            bank_distance(grid, height, p, across).zip(bank_distance(grid, height, p, -across));
        if let Some((left, right)) = width {
            let bin = WIDTH_BINS_KM2.partition_point(|&edge| edge * 1e6 <= flow.area[i]) - 1;
            widths[bin].push(left + right);
        }
    }
    let heads = (0..len)
        .filter(|&i| channel(i) && !fixed[i] && donor[i] == u32::MAX)
        .count();
    let outlet = interior_outlet.or_else(|| (0..len)
        .filter(|&i| selected[i] && fixed[i])
        .max_by(|&a, &b| flow.area[a].total_cmp(&flow.area[b])))?;
    let (main_stem_m, profile_breaks) = profile(grid, height, outlet, &donor);
    let mut ridges = ridge_spacing(grid, height, selected);
    let mut peaks = prominence(grid, height, selected);
    Some((Measurements {
        basin_area_km2,
        head_density_km2: heads as f64 / basin_area_km2,
        ridge_spacing_m: median(&mut ridges),
        width_samples: std::array::from_fn(|i| widths[i].len()),
        width_m: widths.each_mut().map(|v| median(v)),
        prominence_m: median(&mut peaks),
        prominent_peaks: peaks.len(),
        main_stem_m,
        profile_breaks,
    }, outlet))
}

pub(super) fn enclosed_outlet(grid: Grid, flow: &erosion::Flow, area: f64, outlet: Option<usize>) -> Option<usize> {
    // The border cells themselves are fixed outlets. Flag the first
    // interior ring as well so clipped feeder valleys cannot qualify.
    let mut boundary: Vec<_> = (0..grid.len()).map(|i| {
        let (x, z) = (i % grid.n, i / grid.n);
        x <= 1 || z <= 1 || x + 2 >= grid.n || z + 2 >= grid.n
    }).collect();
    for &i in flow.order.iter().rev() {
        let i = i as usize;
        boundary[flow.receiver[i] as usize] |= boundary[i];
    }
    let goal = area * 1e6;
    let outlet = outlet.or_else(|| (0..grid.len())
        .filter(|&i| !boundary[i] && flow.area[i] >= 0.15 * goal)
        .min_by(|&a, &b| (flow.area[a] - goal).abs().total_cmp(&(flow.area[b] - goal).abs())))?;
    (!*boundary.get(outlet)? && flow.area[outlet] >= 0.15 * goal).then_some(outlet)
}

fn bank_distance(grid: Grid, height: &[f32], p: DVec2, across: DVec2) -> Option<f64> {
    let bank = grid.bilinear(height, p.x, p.y) + 5.0;
    let mut last = bank - 5.0;
    for n in 1..=60 {
        let d = f64::from(n) * 4.0;
        let p = p + across * d;
        if !grid.contains(p.x, p.y) {
            break;
        }
        let h = grid.bilinear(height, p.x, p.y);
        if h >= bank {
            return Some(d - 4.0 + 4.0 * (bank - last) / (h - last));
        }
        last = h;
    }
    None
}

fn profile(grid: Grid, height: &[f32], outlet: usize, donor: &[u32]) -> (f64, usize) {
    let mut i = outlet;
    let mut length = 0.0;
    let mut samples = vec![f64::from(height[i])];
    let mut next_m = 48.0;
    while donor[i] != u32::MAX {
        let j = donor[i] as usize;
        let edge = DVec2::from_array(grid.at(i)).distance(DVec2::from_array(grid.at(j)));
        if length + edge >= next_m {
            let t = (next_m - length) / edge;
            samples.push(f64::from(height[i]) * (1.0 - t) + f64::from(height[j]) * t);
            next_m += 48.0;
        }
        length += edge;
        i = j;
    }
    let breaks = samples
        .windows(3)
        .filter(|v| {
            let a = (v[1] - v[0]) / 48.0;
            let b = (v[2] - v[1]) / 48.0;
            (a - b).abs() >= 0.08 && a.max(b) >= 2.0 * a.min(b).max(0.0)
        })
        .count();
    (length, breaks)
}

fn ridge_spacing(grid: Grid, height: &[f32], selected: &[bool]) -> Vec<f64> {
    let mut distances = Vec::new();
    for z in (80..4_000).step_by(160) {
        let z = f64::from(z);
        let row = (z / grid.cell_m) as usize * grid.n;
        let profile: Vec<_> = (4..4_000)
            .step_by(8)
            .map(|x| grid.bilinear(height, f64::from(x), z))
            .collect();
        let mut last = None;
        for (i, &h) in profile.iter().enumerate().skip(10).take(profile.len() - 20) {
            let x = 4.0 + i as f64 * 8.0;
            if !selected[row + (x / grid.cell_m) as usize] {
                last = None;
                continue;
            }
            let low =
                |range: std::ops::Range<usize>| profile[range].iter().copied().fold(h, f64::min);
            if h > profile[i - 1]
                && h >= profile[i + 1]
                && h - low(i - 10..i).max(low(i + 1..i + 11)) >= 6.0
            {
                if let Some(a) = last {
                    distances.push(x - a);
                }
                last = Some(x);
            }
        }
    }
    distances
}

fn prominence(grid: Grid, height: &[f32], selected: &[bool]) -> Vec<f64> {
    let analysis = Grid {
        n: 500,
        cell_m: 8.0,
        min_m: [0.0; 2],
    };
    let len = analysis.len();
    let elevation: Vec<_> = (0..len)
        .map(|i| {
            let [x, z] = analysis.at(i);
            grid.bilinear(height, x, z)
        })
        .collect();
    let mut order: Vec<_> = (0..len).collect();
    order.sort_unstable_by(|&a, &b| elevation[b].total_cmp(&elevation[a]).then(a.cmp(&b)));
    let mut roots = vec![usize::MAX; len];
    let mut peaks = Vec::new();
    for i in order {
        roots[i] = i;
        for (j, _) in analysis.neighbours(i) {
            if roots[j] == usize::MAX {
                continue;
            }
            let a = root(&mut roots, i);
            let b = root(&mut roots, j);
            if a == b {
                continue;
            }
            let (high, low) =
                if elevation[a] > elevation[b] || (elevation[a] == elevation[b] && a < b) {
                    (a, b)
                } else {
                    (b, a)
                };
            roots[low] = high;
            let drop = elevation[low] - elevation[i];
            let [x, z] = analysis.at(low);
            let source = (z / grid.cell_m) as usize * grid.n + (x / grid.cell_m) as usize;
            if drop >= 10.0 && !analysis.on_border(low) && selected[source] {
                peaks.push(drop);
            }
        }
    }
    peaks
}

fn root(roots: &mut [usize], mut i: usize) -> usize {
    while roots[i] != i {
        roots[i] = roots[roots[i]];
        i = roots[i];
    }
    i
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable_by(f64::total_cmp);
    let mid = values.len() / 2;
    Some((values[(values.len() - 1) / 2] + values[mid]) * 0.5)
}
