//! Linked channel incision and trail surveys on the finished regional bed.
use super::{Budget, Frame, Grid, SPAN_M, Stage, erosion};
use crate::planetary::tectonics::smooth;
use glam::DVec2;
use std::{cmp::Reverse, collections::BinaryHeap};

pub(super) fn dissect(stage: &mut Stage, flow: &erosion::Flow, budget: &mut Budget) {
    let grid = stage.grid;
    let cell_area = grid.cell_m * grid.cell_m;
    let span = (28.0 / grid.cell_m).round() as usize;
    let mut cut = vec![[0.0_f32; 2]; grid.len()];
    for &i in &flow.order {
        let i = i as usize;
        let r = flow.receiver[i] as usize;
        if i == r {
            continue;
        }
        let [x, z] = grid.at(i);
        let margin = x.min(z).min(SPAN_M - x).min(SPAN_M - z);
        if margin <= 28.0 {
            continue;
        }
        let area = flow.area[i];
        let [down, breadth] = cut[r].map(f64::from);
        // Priority-flood can assign flow on flats. Require opposing valley
        // sides before incision so a receiver on a flat leaves no grid trace.
        let bank = [
            span,
            grid.n * span,
            (grid.n - 1) * span,
            (grid.n + 1) * span,
        ]
        .into_iter()
        .map(|d| stage.height[i - d].min(stage.height[i + d]))
        .fold(stage.height[i], f64::max)
            - stage.height[i];
        let reach = smooth(6000.0, 34000.0, area) * smooth(0.25, 2.0, bank);
        let distance = DVec2::from_array(grid.at(i)).distance(DVec2::from_array(grid.at(r)));
        let fade = smooth(0.0, 380.0, margin);
        // Tributaries have a narrower adjustment corridor than the trunk.
        // Between channels the adjustment decays along the actual downslope
        // route, leaving drainage divides above their connected valley sides.
        let width = 24.0 + 22.0 * smooth(35_000.0, 440_000.0, area);
        let lateral = down * breadth / (breadth + distance);
        // Preserve the established trunk floor. Most of the adjustment
        // belongs at the transition from slope wash to tributary incision.
        let depth = 5.4 - 3.6 * smooth(48_000.0, 400_000.0, area);
        let change = (lateral + reach * (depth - lateral)) * fade;
        let grade_room = (stage.height[i] - stage.height[r] + down - 0.0004 * distance).max(0.0);
        let change = change.min(grade_room);
        cut[i] = [change as f32, (breadth + reach * (width - breadth)) as f32];
    }
    for (i, &[bed_cut, _]) in cut.iter().enumerate() {
        if stage.fixed[i] {
            continue;
        }
        // Relax the new banks into adjacent similar-elevation cells once.
        // Large elevation jumps reject the transfer and preserve divides.
        let mut sum = f64::from(bed_cut);
        let mut weights = 1.0;
        for (j, distance) in grid.neighbours(i) {
            let contrast = (stage.height[i] - stage.height[j]).abs();
            let w = (1.0 - contrast / distance).max(0.0) * 0.22;
            sum += w * f64::from(cut[j][0]);
            weights += w;
        }
        let change = sum / weights;
        if change == 0.0 {
            continue;
        }
        let soil = f64::from(stage.soil[i]);
        let rock = f64::from(stage.rock[i]);
        stage.soil[i] = (soil - change).max(0.0) as f32;
        stage.rock[i] = (rock - (change - soil).max(0.0)) as f32;
        let detached = rock - f64::from(stage.rock[i]);
        budget.eroded_rock_m3 += detached * cell_area;
        budget.exported_sediment_m3 += (detached + soil - f64::from(stage.soil[i])) * cell_area;
    }
}

const STEP: f64 = 8.0;
const SIDE: usize = 101;
const GRADE: f64 = 0.30;
const DRY: f64 = 15.0;

pub(super) struct Trails {
    pub paths: [Vec<DVec2>; 2],
    // Sparse per-cell segment index. A typical query sees no segments.
    bins: std::collections::BTreeMap<(i32, i32), Vec<(DVec2, DVec2)>>,
}

impl Trails {
    pub fn survey(grid: Grid, terrain: &[f32], centre: DVec2, keepouts: &[(DVec2, f64)]) -> Option<Self> {
        let half = STEP * (SIDE / 2) as f64;
        let sample = Grid {
            n: SIDE,
            cell_m: STEP,
            min_m: (centre - DVec2::splat(half)).to_array(),
        };
        let at = |i| DVec2::from_array(sample.at(i));
        let ground = |p: DVec2| grid.bilinear(terrain, p.x, p.y);
        let valid = (0..sample.len())
            .map(|i| {
                let p = at(i);
                let margin = p.x.min(p.y).min(SPAN_M - p.x).min(SPAN_M - p.y);
                margin >= 440.0
                    && ground(p) >= DRY
                    && keepouts
                        .iter()
                    .all(|&(centre, radius)| p.distance_squared(centre) >= radius * radius)
            })
            .collect::<Vec<_>>();
        let mut edges = vec![[u16::MAX; 8]; sample.len()];
        for (i, links) in edges.iter_mut().enumerate() {
            let a = at(i);
            if !valid[i] {
                continue;
            }
            for (slot, (j, length)) in sample.neighbours(i).enumerate() {
                let b = at(j);
                if !valid[j] {
                    continue;
                }
                let steps = (length / 2.0).ceil() as usize;
                let mut last = ground(a);
                let mut rise = 0.0;
                for k in 1..=steps {
                    let h = ground(a.lerp(b, k as f64 / steps as f64));
                    rise += (h - last).abs();
                    if h < DRY || (h - last).abs() > GRADE * length / steps as f64 {
                        rise = f64::INFINITY;
                        break;
                    }
                    last = h;
                }
                if rise.is_finite() {
                    links[slot] = (100.0 * (length + rise * 2.5)) as u16;
                }
            }
        }
        let start = (0..sample.len())
            .filter(|&i| edges[i].iter().filter(|&&v| v != u16::MAX).count() >= 3)
            .min_by(|&a, &b| {
                at(a)
                    .distance_squared(centre)
                    .total_cmp(&at(b).distance_squared(centre))
            })?;
        let (cost, parent) = search(sample, &edges, start, &[]);
        let mut goals: Vec<_> = (0..sample.len())
            .filter(|&i| {
                cost[i] != u32::MAX && (200.0..=365.0).contains(&at(i).distance(at(start)))
            })
            .collect();
        let score = |i| {
            at(i).distance(at(start)) - 0.004 * f64::from(cost[i])
                + (ground(at(i)) - ground(at(start))).clamp(-25.0, 45.0)
        };
        goals.sort_unstable_by(|&a, &b| score(b).total_cmp(&score(a)).then(a.cmp(&b)));
        for goal in goals.into_iter().step_by(9).take(8) {
            let primary = unwind(&parent, start, goal);
            let mut near = vec![false; sample.len()];
            for &i in &primary {
                let (x, z) = (i % SIDE, i / SIDE);
                for b in z.saturating_sub(3)..=(z + 3).min(SIDE - 1) {
                    for a in x.saturating_sub(3)..=(x + 3).min(SIDE - 1) {
                        near[b * SIDE + a] = true;
                    }
                }
            }
            let (_, alternative) = search(sample, &edges, start, &near);
            let second = unwind(&alternative, start, goal);
            let different = second.iter().filter(|&&i| !near[i]).count();
            if second.is_empty() || different * 3 < second.len() || second.len() > primary.len() * 2
            {
                continue;
            }
            let paths = [primary, second].map(|p| p.into_iter().map(at).collect::<Vec<_>>());
            let mut bins = std::collections::BTreeMap::<_, Vec<_>>::new();
            for path in &paths {
                for segment in path.windows(2) {
                    let (a, b) = (segment[0], segment[1]);
                    let lo = key(a.min(b) - DVec2::splat(2.0));
                    let hi = key(a.max(b) + DVec2::splat(2.0));
                    for z in lo.1..=hi.1 {
                        for x in lo.0..=hi.0 {
                            bins.entry((x, z)).or_default().push((a, b));
                        }
                    }
                }
            }
            return Some(Self { paths, bins });
        }
        None
    }

    pub fn covers(&self, at: DVec2) -> bool {
        self.bin_covers(key(at), at, 1.8_f64.powi(2))
    }

    pub fn overlaps(&self, at: DVec2, radius: f64) -> bool {
        let lo = key(at - DVec2::splat(radius));
        let hi = key(at + DVec2::splat(radius));
        let limit = radius * radius;
        (lo.1..=hi.1).any(|z| (lo.0..=hi.0).any(|x| self.bin_covers((x, z), at, limit)))
    }

    fn bin_covers(&self, bin: (i32, i32), at: DVec2, limit: f64) -> bool {
        self.bins.get(&bin).is_some_and(|legs| {
            legs.iter().any(|&(a, b)| {
                let t = ((at - a).dot(b - a) / a.distance_squared(b)).clamp(0.0, 1.0);
                at.distance_squared(a.lerp(b, t)) < limit
            })
        })
    }

    pub fn chart(&self, frame: Frame) -> [Vec<DVec2>; 2] {
        self.paths
            .each_ref()
            .map(|p| p.iter().map(|v| frame.chart(v.x, v.y)).collect())
    }
}

fn key(at: DVec2) -> (i32, i32) {
    ((at.x / 16.0).floor() as i32, (at.y / 16.0).floor() as i32)
}

fn search(grid: Grid, edges: &[[u16; 8]], start: usize, near: &[bool]) -> (Vec<u32>, Vec<usize>) {
    let mut cost = vec![u32::MAX; edges.len()];
    let mut parent = vec![usize::MAX; edges.len()];
    let mut work = BinaryHeap::from([Reverse((0, start))]);
    cost[start] = 0;
    while let Some(Reverse((g, i))) = work.pop() {
        if g != cost[i] {
            continue;
        }
        for (slot, (j, _)) in grid.neighbours(i).enumerate() {
            let edge = edges[i][slot];
            if edge == u16::MAX {
                continue;
            }
            let next = g + u32::from(edge) * if near.get(j) == Some(&true) { 5 } else { 1 };
            if next < cost[j] {
                cost[j] = next;
                parent[j] = i;
                work.push(Reverse((next, j)));
            }
        }
    }
    (cost, parent)
}

fn unwind(parent: &[usize], start: usize, mut i: usize) -> Vec<usize> {
    let mut path = vec![i];
    while i != start {
        i = parent[i];
        if i == usize::MAX {
            return Vec::new();
        }
        path.push(i);
    }
    path.reverse();
    path
}
