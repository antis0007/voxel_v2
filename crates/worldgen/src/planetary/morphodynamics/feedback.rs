//! Refit the next flood's inlet using the first flood's high-water marks and
//! the bed it left behind. This pass estimates escape resistance only. The
//! hydraulic solver still determines velocity and moves all water and grains.
use super::{Bed, FINE_SETTLING, Grid, SEA, Water};
use std::{cmp::Reverse, collections::{BinaryHeap, VecDeque}};

pub(super) fn handoff(grid: Grid, bed: &mut [Bed], water: &mut [Water], crest: &[f32],
    inundation: (&[u16], usize), inlets: &mut [(usize, f64)]) {
    let started = std::time::Instant::now();
    let outlets = sea_edges(grid, bed);
    let resistance = escape_resistance(grid, bed, crest, inundation, &outlets);
    let best = inlets.iter().map(|&(i, _)| resistance[i]).min().unwrap_or(u32::MAX);
    if best == u32::MAX {
        eprintln!("[worldgen] delta_feedback connected_inlets=0/{} pass_ms={:.3}",
            inlets.len(), started.elapsed().as_secs_f64() * 1000.0);
        return;
    }
    let mut connected = 0;
    let mut smallest = 1.0_f64;
    for (i, taper) in inlets.iter_mut() {
        let work = resistance[*i];
        connected += usize::from(work != u32::MAX);
        // A wet but poorly drained lane still gets some water. The smaller
        // second flood can then cut a new route through a recently formed bar.
        let factor = if work == u32::MAX { 0.22 } else { (f64::from(best) / f64::from(work)).sqrt().max(0.22) };
        smallest = smallest.min(factor);
        *taper *= factor;
    }
    let draining = ebb_connections(grid, bed, water, &outlets);
    let retained = settle_disconnected(&draining, bed, water) * grid.dx * grid.dx;
    eprintln!("[worldgen] delta_feedback connected_inlets={connected}/{} minimum_inlet_factor={smallest:.3} retained_fines_m3={retained:.3} pass_ms={:.3}",
        inlets.len(), started.elapsed().as_secs_f64() * 1000.0);
}

fn sea_edges(grid: Grid, bed: &[Bed]) -> Vec<usize> {
    let Grid { nx, ny, dx, inland, .. } = grid;
    (0..bed.len()).filter(|&i| {
        let (x, y) = (i % nx, i / nx);
        let ocean_edge = x + 1 == nx || ((y == 0 || y + 1 == ny) && x as f64 * dx > inland + 24.0);
        ocean_edge && bed[i].height() < SEA - 0.75
    }).collect()
}

pub(super) fn neighbours(grid: Grid, i: usize) -> impl Iterator<Item = usize> {
    let (x, y) = (i % grid.nx, i / grid.nx);
    [(x > 0).then(|| i - 1), (x + 1 < grid.nx).then(|| i + 1),
        (y > 0).then(|| i - grid.nx), (y + 1 < grid.ny).then(|| i + grid.nx)].into_iter().flatten()
}

fn escape_resistance(grid: Grid, bed: &[Bed], crest: &[f32], (wet_steps, samples): (&[u16], usize), outlets: &[usize]) -> Vec<u32> {
    let mut path = vec![u32::MAX; bed.len()];
    let mut pending = BinaryHeap::new();
    // Shallow sills consume more head than open water. Briefly flooded paths
    // are less reliable even if their peak depth was high.
    let cell_work: Vec<_> = bed.iter().zip(crest).zip(wet_steps).map(|((cell, &head), &wet)| {
        let depth = f64::from(head).max(SEA) - cell.height();
        let interruption = 1.0 - f64::from(wet) / samples.max(1) as f64;
        (depth >= 0.12).then(|| 1 + ((1.0 + 2.0 * interruption) * 16.0 / (depth + 0.15).powi(2)).round() as u32)
    }).collect();
    for &i in outlets {
        path[i] = 1;
        pending.push(Reverse((1, i)));
    }
    while let Some(Reverse((work, i))) = pending.pop() {
        if work != path[i] { continue; }
        for j in neighbours(grid, i) {
            if let Some(local) = cell_work[j] {
                let next = work + local;
                if next < path[j] {
                    path[j] = next;
                    pending.push(Reverse((next, j)));
                }
            }
        }
    }
    path
}

/// A pool reached at flood crest can be cut off on the ebb. Follow wet
/// connections at the handoff stage, with both surfaces above each sill.
fn ebb_connections(grid: Grid, bed: &[Bed], water: &[Water], outlets: &[usize]) -> Vec<bool> {
    let mut draining = vec![false; bed.len()];
    let mut pending = VecDeque::new();
    for &i in outlets {
        if water[i].depth >= 0.10 {
            draining[i] = true;
            pending.push_back(i);
        }
    }
    while let Some(i) = pending.pop_front() {
        let a = bed[i].height();
        for j in neighbours(grid, i) {
            if draining[j] || water[j].depth < 0.10 { continue; }
            let b = bed[j].height();
            if (a + water[i].depth).min(b + water[j].depth) - a.max(b) >= 0.10 {
                draining[j] = true;
                pending.push_back(j);
            }
        }
    }
    draining
}

/// Resolve a short slack-water interval analytically in shallow, isolated
/// overbank pools. The next flood encounters the deposited mud as terrain.
fn settle_disconnected(draining: &[bool], bed: &mut [Bed], water: &mut [Water]) -> f64 {
    const SLACK_SECONDS: f64 = 8.0;
    let mut retained = 0.0;
    for ((&connected, b), w) in draining.iter().zip(bed).zip(water) {
        if connected || !(0.10..=1.5).contains(&w.depth) || b.height() < SEA - 0.2 {
            continue;
        }
        let available = w.suspended[1];
        let [qx, qy] = w.momentum;
        if available <= 0.0 || qx * qx + qy * qy > (0.25 * w.depth).powi(2) { continue; }
        let fraction = 1.0 - (-SLACK_SECONDS * FINE_SETTLING / w.depth).exp();
        let settled = (available * fraction).min(0.18);
        w.suspended[1] -= settled;
        w.river_fines = w.river_fines.clamp(0.0, available) * (1.0 - settled / available);
        b.sediment[1] += settled;
        retained += settled;
    }
    retained
}
