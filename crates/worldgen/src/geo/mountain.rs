//! Weather the final, metre-scale mountain mass, not its normalized precursor.
//!
//! Relief remapping steepens the original drainage solve. Coupling conservative
//! hillslope transport to its rain-fed drainage network lets divides retreat,
//! builds their footslopes and keeps connected incised valley floors. No radial
//! peak masks or independent noise trenches are added at sampling time.

use super::{erosion, grid::Grid, noise::smoothstep};

const CYCLES: usize = 8;
const TALUS_PASSES: usize = 4;
const CRITICAL_SLOPE: f64 = 0.75;
// Effective erodibility times geological step duration, as in erosion::Params.
const STREAM_POWER_K_DT: f64 = 0.12;

/// Measured outputs of this bake, not an invented calendar chronology.
pub(crate) struct HistoryRecord {
    pub steps: u32,
    pub before_m: Vec<f32>,
    pub hillslope_net_m: Vec<f32>,
    pub fluvial_incision_m: Vec<f32>,
    pub fracture_damage: Vec<f32>,
    pub boundary_export_m3: f64,
    pub fluvial_removal_m3: f64,
}

pub(super) fn weather(grid: &Grid, h: &mut [f64], fixed: &[bool], rain: &[f64], erodibility: f64, damage: Option<&[f64]>) -> Option<HistoryRecord> {
    let started = std::time::Instant::now();
    let before = damage.map(|_| h.to_vec());
    let pairs = pairs(grid, fixed);
    let setup_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut delta = vec![0.0; h.len()];
    let mut exposed = if damage.is_some() { vec![0.0; h.len()] } else { Vec::new() };
    let mut transported = 0.0;
    let mut exported = 0.0;
    let mut incised = 0.0;
    let (mut transport_ms, mut route_ms, mut incision_ms) = (0.0, 0.0, 0.0);
    for _ in 0..CYCLES {
        let phase = std::time::Instant::now();
        for _ in 0..TALUS_PASSES {
            let (moved, lost) = transport_pairs(&pairs, h, fixed, CRITICAL_SLOPE, &mut delta, damage, &exposed);
            transported += moved;
            exported += lost;
        }
        transport_ms += phase.elapsed().as_secs_f64() * 1000.0;
        let phase = std::time::Instant::now();
        let flow = erosion::route_compact(grid, h, fixed, rain);
        route_ms += phase.elapsed().as_secs_f64() * 1000.0;
        let phase = std::time::Instant::now();
        // Receivers are solved first: the implicit stream-power step stays
        // downhill even where contributing area and incision become large.
        for &cell in &flow.order {
            let i = cell as usize;
            let r = flow.receiver[i] as usize;
            if fixed[i] || i == r || h[i] < flow.spill[i] {
                continue;
            }
            let diagonal = i % grid.n != r % grid.n && i / grid.n != r / grid.n;
            let distance = grid.cell_m * if diagonal { super::grid::SQRT2 } else { 1.0 };
            // Wet contributing area, in m², sets channel initiation. Divides
            // receive slope transport; rainfall converges into ravines below.
            let channel = smoothstep(4096.0, 16384.0, flow.area[i]);
            let resistance = damage.map_or(1.0, |d| 0.85 + 1.4 * d[i]);
            let f = STREAM_POWER_K_DT * erodibility * channel * flow.area[i].sqrt() / distance * resistance;
            let next = (h[i] + f * h[r]) / (1.0 + f);
            let cut = (h[i] - next).max(0.0);
            h[i] -= cut;
            incised += cut;
            if damage.is_some() { exposed[i] += cut; }
        }
        incision_ms += phase.elapsed().as_secs_f64() * 1000.0;
    }
    let cell_area = grid.cell_m * grid.cell_m;
    let history = damage.zip(before).map(|(damage, before)| HistoryRecord {
        steps: CYCLES as u32,
        hillslope_net_m: (0..h.len()).map(|i| (h[i] - before[i] + exposed[i]) as f32).collect(),
        before_m: before.iter().map(|v| *v as f32).collect(),
        fluvial_incision_m: exposed.iter().map(|v| *v as f32).collect(),
        fracture_damage: damage.iter().map(|v| *v as f32).collect(),
        boundary_export_m3: exported * cell_area,
        fluvial_removal_m3: incised * cell_area,
    });
    eprintln!("[worldgen] mountain_erosion cycles={CYCLES} transport_m3={:.1} boundary_export_m3={:.1} fluvial_removal_m3={:.1} ms={:.1}",
        transported * cell_area, exported * cell_area, incised * cell_area, started.elapsed().as_secs_f64() * 1000.0);
    eprintln!("[worldgen] mountain_profile pairs={} setup_ms={setup_ms:.1} transport_ms={transport_ms:.1} route_ms={route_ms:.1} incision_ms={incision_ms:.1}", pairs.len());
    history
}

struct Pair { a: usize, b: usize, distance: f64 }

fn pairs(grid: &Grid, fixed: &[bool]) -> Vec<Pair> {
    let mut pairs = Vec::new();
    for i in 0..grid.len() {
        // Preserve the original summation order exactly. Coast/ocean pairs
        // that could never move material are omitted once, not 32 times.
        for (j, distance) in grid.neighbours(i).filter(|(j, _)| *j > i) {
            if !fixed[i] || !fixed[j] { pairs.push(Pair { a:i, b:j, distance }); }
        }
    }
    pairs
}

/// Simultaneous transfers across each undirected neighbour pair. Unlike an
/// in-place steepest-neighbour sweep, results have no scan-direction bias.
/// Every transfer is deposited in the lower cell or accounted at a fixed outlet.
fn transport_pairs(pairs: &[Pair], h: &mut [f64], fixed: &[bool], critical: f64, delta: &mut [f64], damage: Option<&[f64]>, exposed: &[f64]) -> (f64, f64) {
    delta.fill(0.0);
    let (mut moved, mut exported) = (0.0, 0.0);
    for &Pair { a:i, b:j, distance } in pairs {
            let (high, low) = if h[i] > h[j] { (i, j) } else { (j, i) };
            if fixed[high] {
                continue;
            }
            // At most eight transfers leave a cell. The 1/16 factor keeps
            // this explicit update stable even on an isolated sharp summit.
            // Resistant rock can hold a gorge wall only where runoff has
            // actually exposed it. Unincised summits retain the talus slope.
            let critical = damage.map_or(critical, |d| {
                critical + 0.8 * (1.0 - d[high]) * smoothstep(1.0, 8.0, exposed[low])
            });
            let amount = (h[high] - h[low] - critical * distance).max(0.0) / 16.0;
            delta[high] -= amount;
            moved += amount;
            if fixed[low] {
                exported += amount;
            } else {
                delta[low] += amount;
            }
    }
    for (height, change) in h.iter_mut().zip(delta) {
        *height += *change;
    }
    (moved, exported)
}

#[cfg(test)]
fn transport(grid: &Grid, h: &mut [f64], fixed: &[bool], critical: f64, delta: &mut [f64]) -> (f64, f64) {
    transport_pairs(&pairs(grid, fixed), h, fixed, critical, delta, None, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slope_transport_conserves_mass_and_has_no_scan_direction_bias() {
        let grid = Grid { n: 17, cell_m: 12.0, min_m: [-102.0; 2] };
        let mut h: Vec<f64> = (0..grid.len()).map(|i| {
            let [x, z] = grid.at(i);
            (180.0 - 3.5 * x.hypot(z)).max(10.0)
        }).collect();
        let before: f64 = h.iter().sum();
        let fixed = vec![false; grid.len()];
        let mut delta = vec![0.0; h.len()];
        for _ in 0..32 {
            let (_, exported) = transport(&grid, &mut h, &fixed, 0.75, &mut delta);
            assert_eq!(exported, 0.0);
        }
        assert!((h.iter().sum::<f64>() - before).abs() < 1e-8);
        assert!(h[8 * 17 + 8] < 130.0, "unsupported needle must retreat");
        assert!(h.iter().all(|h| *h >= 10.0));
        for z in 0..17 { for x in 0..17 {
            assert!((h[z * 17 + x] - h[x * 17 + z]).abs() < 1e-10);
        }}
    }

    #[test]
    fn fixed_outlet_export_is_accounted_and_bed_is_unchanged() {
        let grid = Grid { n: 5, cell_m: 2.0, min_m: [0.0; 2] };
        let fixed: Vec<bool> = (0..grid.len()).map(|i| grid.on_border(i)).collect();
        let mut h: Vec<f64> = fixed.iter().map(|f| if *f { 0.0 } else { 20.0 }).collect();
        let before: f64 = h.iter().sum();
        let (_, exported) = transport(&grid, &mut h, &fixed, 0.75, &mut vec![0.0; grid.len()]);
        assert!(exported > 0.0);
        assert!((before - h.iter().sum::<f64>() - exported).abs() < 1e-10);
        assert!(h.iter().zip(fixed).all(|(h, fixed)| !fixed || *h == 0.0));
    }

    #[test]
    fn rain_fed_channels_remain_connected_and_dry_weather_does_not_incise() {
        let grid = Grid { n: 33, cell_m: 12.0, min_m: [-198.0; 2] };
        let fixed: Vec<bool> = (0..grid.len()).map(|i| grid.on_border(i)).collect();
        let base: Vec<f64> = (0..grid.len()).map(|i| {
            let [x, z] = grid.at(i);
            10.0 + 0.1 * (z + 198.0) + 0.2 * x.abs()
        }).collect();
        let mut wet = base.clone();
        let mut dry = base.clone();
        let _ = weather(&grid, &mut wet, &fixed, &vec![1.0; grid.len()], 1.0, None);
        let _ = weather(&grid, &mut dry, &fixed, &vec![0.0; grid.len()], 1.0, None);
        assert_eq!(dry, base);
        assert!(wet.iter().zip(&base).any(|(a, b)| b - a > 1.0));
        let flow = erosion::route(&grid, &wet, &fixed, &vec![1.0; grid.len()]);
        for i in 0..grid.len() {
            assert!((wet[i] - base[i]).abs() < 1e-10 || !fixed[i]);
            if !fixed[i] {
                assert!(wet[flow.receiver[i] as usize] <= wet[i] + 1e-8);
                assert!(flow.spill[i] - wet[i] < 1e-8);
            }
        }
    }
}
