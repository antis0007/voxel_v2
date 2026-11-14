//! Opt-in measurements of the baked topology and the second flood.
use super::{Bed, Grid, SEA, Water, feedback::neighbours};
use glam::DVec3;
use std::collections::VecDeque;

pub(super) struct Branches {
    visits: Vec<[u8; 2]>,
    snapshots: u8,
}

impl Branches {
    pub(super) fn new(cells: usize) -> Self {
        Self {
            visits: vec![[0; 2]; cells],
            snapshots: 0,
        }
    }

    pub(super) fn observe(
        &mut self,
        grid: Grid,
        bed: &[Bed],
        water: &[Water],
        inlets: &[(usize, f64)],
    ) {
        let wet = |i: usize| water[i].depth > 0.16 && water[i].river_fines > 1e-7;
        let link = |i: usize, j: usize| {
            let a = bed[i].height();
            let b = bed[j].height();
            wet(j) && (a + water[i].depth).min(b + water[j].depth) - a.max(b) > 0.10
        };
        let river = flood(
            grid,
            water.len(),
            inlets.iter().map(|&(i, _)| i).filter(|&i| wet(i)),
            link,
        );
        let sinks = (0..water.len()).filter(|&i| {
            i % grid.nx > ((grid.inland + 28.0) / grid.dx) as usize
                && bed[i].height() < SEA - 1.7
                && wet(i)
        });
        let sea = flood(grid, water.len(), sinks, link);
        let furthest = river
            .iter()
            .enumerate()
            .filter(|&(_, hit)| *hit)
            .map(|(i, _)| (i % grid.nx) as f64 * grid.dx - grid.inland)
            .reduce(f64::max)
            .unwrap_or(-grid.inland);
        let connected = river.iter().zip(&sea).filter(|&(a, b)| *a && *b).count();
        eprintln!(
            "[worldgen] delta_snapshot river_front_m={furthest:.0} shelf_connected_cells={connected}"
        );
        for (i, [connected, outward]) in self.visits.iter_mut().enumerate() {
            if river[i] && sea[i] {
                *connected += 1;
                *outward += u8::from(water[i].momentum[0] > 0.015);
            }
        }
        self.snapshots += 1;
    }

    pub(super) fn report(
        self,
        grid: Grid,
        bed: &[Bed],
        water: &[Water],
        initial: &[f64],
        position: impl Fn(usize, usize) -> DVec3,
    ) {
        let dry = |i: usize| bed[i].height() >= SEA + 0.20 && water[i].depth < 0.12;
        // The tide can reverse a channel briefly. Require river connectivity
        // in all three phases and outward flow in at least two.
        let flowing = |i: usize| {
            self.snapshots == 3
                && self.visits[i][0] == self.snapshots
                && self.visits[i][1] >= 2
                && bed[i].height() < SEA - 0.10
                && water[i].depth > 0.12
        };
        let mut seen = vec![false; bed.len()];
        let mut body = Vec::new();
        let mut coast_islets = 0;
        let mut splits = 0;
        for i in 0..bed.len() {
            if seen[i] || !dry(i) {
                continue;
            }
            body.clear();
            body.push(i);
            seen[i] = true;
            let mut p = 0;
            let mut edge = false;
            while let Some(&k) = body.get(p) {
                let (x, y) = (k % grid.nx, k / grid.nx);
                edge |= x == 0 || x + 1 == grid.nx || y == 0 || y + 1 == grid.ny;
                for j in neighbours(grid, k) {
                    if !seen[j] && dry(j) {
                        seen[j] = true;
                        body.push(j);
                    }
                }
                p += 1;
            }
            let area = body.len() as f64 * grid.dx * grid.dx;
            if edge || !(32.0..=6000.0).contains(&area) {
                continue;
            }
            let centre = (
                body.iter().map(|i| i % grid.nx).sum::<usize>() / body.len(),
                body.iter().map(|i| i / grid.nx).sum::<usize>() / body.len(),
            );
            if ((centre.0 as f64) * grid.dx - grid.inland).abs() > 160.0 {
                continue;
            }
            coast_islets += 1;
            let mut sections = vec![(usize::MAX, 0); grid.nx];
            for &j in &body {
                let (x, y) = (j % grid.nx, j / grid.nx);
                sections[x] = (sections[x].0.min(y), sections[x].1.max(y));
            }
            let search = (24.0 / grid.dx) as usize;
            let paired = sections
                .iter()
                .enumerate()
                .filter(|&(x, &(lo, hi))| {
                    lo != usize::MAX
                        && (lo.saturating_sub(search)..lo).any(|y| flowing(y * grid.nx + x))
                        && (hi + 1..=(hi + search).min(grid.ny - 1))
                            .any(|y| flowing(y * grid.nx + x))
                })
                .count();
            let new_land =
                body.iter().filter(|&&i| initial[i] < SEA).count() as f64 * grid.dx * grid.dx;
            if paired >= 2 {
                splits += 1;
            }
            let at = position(centre.0, centre.1);
            eprintln!(
                "[worldgen] delta_islet area_m2={area:.0} new_land_m2={new_land:.0} paired_sections={paired} grid_x={} grid_y={} atlas_x={:.1} atlas_z={:.1}",
                centre.0, centre.1, at.x, at.z
            );
        }
        eprintln!(
            "[worldgen] delta_branch_audit snapshots={} coastal_islets={coast_islets} persistent_splits={splits}",
            self.snapshots
        );
    }
}

fn flood(
    grid: Grid,
    cells: usize,
    start: impl Iterator<Item = usize>,
    link: impl Fn(usize, usize) -> bool,
) -> Vec<bool> {
    let mut reached = vec![false; cells];
    let mut queue = VecDeque::new();
    for i in start {
        reached[i] = true;
        queue.push_back(i);
    }
    while let Some(i) = queue.pop_front() {
        for j in neighbours(grid, i) {
            if !reached[j] && link(i, j) {
                reached[j] = true;
                queue.push_back(j);
            }
        }
    }
    reached
}
