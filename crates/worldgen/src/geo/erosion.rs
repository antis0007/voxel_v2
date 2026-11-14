//! Stream-power landscape evolution (Braun and Willett 2013) on a cell grid.
//!
//! Each step routes flow over a priority-flood surface, so a depression drains
//! through its spill point instead of trapping flow, and the flood's pop order
//! is already a valid upstream-to-downstream stack.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::grid::Grid;

/// Fill increment that makes every flooded flat drain toward its outlet.
const EPS: f64 = 1e-4;

#[derive(Clone, Copy, PartialEq)]
struct Key(f64, u32);

impl Eq for Key {}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0).then(self.1.cmp(&other.1))
    }
}

pub struct Flow {
    /// Downstream cell; a fixed cell is its own receiver.
    pub receiver: Vec<u32>,
    /// Cells from outlets upstream: every receiver precedes its donors.
    pub order: Vec<u32>,
    /// Water level a depression fills to before it spills; equals height outside lakes.
    pub spill: Vec<f64>,
    /// Upstream area times the weight field, in square metres.
    pub area: Vec<f64>,
}

/// Routes flow from every cell to a fixed cell.
pub fn route(grid: &Grid, h: &[f64], fixed: &[bool], weight: &[f64]) -> Flow {
    route_impl(grid, h, fixed, weight, false)
}

/// Same drainage fields as `route`, without putting fixed cells that cannot
/// receive land runoff through the heap. Their own discharge stays unchanged.
pub fn route_compact(grid: &Grid, h: &[f64], fixed: &[bool], weight: &[f64]) -> Flow {
    route_impl(grid, h, fixed, weight, true)
}

fn route_impl(grid: &Grid, h: &[f64], fixed: &[bool], weight: &[f64], compact: bool) -> Flow {
    let len = grid.len();
    let mut filled = vec![0.0; len];
    let mut spill = vec![0.0; len];
    let mut seen = vec![false; len];
    let mut order = Vec::with_capacity(len);
    let mut heap = BinaryHeap::new();
    for i in 0..len {
        if fixed[i] {
            filled[i] = h[i];
            spill[i] = h[i];
            seen[i] = true;
            if compact && grid.neighbours(i).all(|(j, _)| fixed[j]) {
                // No donor can reach this outlet. Moving it ahead of the
                // heap order cannot change any accumulation or spill level.
                order.push(i as u32);
            } else {
                heap.push(Reverse(Key(h[i], i as u32)));
            }
        }
    }
    while let Some(Reverse(Key(_, i))) = heap.pop() {
        let i = i as usize;
        order.push(i as u32);
        for (j, _) in grid.neighbours(i) {
            if seen[j] {
                continue;
            }
            seen[j] = true;
            filled[j] = h[j].max(filled[i] + EPS);
            spill[j] = h[j].max(spill[i]);
            heap.push(Reverse(Key(filled[j], j as u32)));
        }
    }
    let mut receiver: Vec<u32> = (0..len as u32).collect();
    for i in 0..len {
        if fixed[i] {
            continue;
        }
        let mut best = 0.0;
        for (j, d) in grid.neighbours(i) {
            let drop = (filled[i] - filled[j]) / d;
            if drop > best {
                best = drop;
                receiver[i] = j as u32;
            }
        }
    }
    let cell = grid.cell_m * grid.cell_m;
    let mut area: Vec<f64> = weight.iter().map(|w| w * cell).collect();
    for &i in order.iter().rev() {
        let r = receiver[i as usize] as usize;
        if r != i as usize {
            area[r] += area[i as usize];
        }
    }
    Flow { receiver, order, spill, area }
}

#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// Erodibility times step length; the stream-power exponents are m = 0.5, n = 1.
    pub k_dt: f64,
    /// Uplift multiplier per step.
    pub dt: f64,
    /// Hillslope diffusivity times step length, square metres.
    pub kappa_dt: f64,
    pub steps: u32,
}

/// Runs `p.steps` uplift, incision and diffusion steps; heights of fixed cells never change.
pub fn evolve(
    grid: &Grid,
    h: &mut [f64],
    fixed: &[bool],
    uplift: &[f64],
    weight: &[f64],
    p: Params,
    mut on_step: impl FnMut(u32, &[f64]),
) {
    let mut scratch = vec![0.0; h.len()];
    for step in 0..p.steps {
        let flow = route(grid, h, fixed, weight);
        for &i in &flow.order {
            let i = i as usize;
            if fixed[i] {
                continue;
            }
            // A lake bed only gains uplift; it is not incised while submerged.
            if h[i] < flow.spill[i] {
                h[i] += p.dt * uplift[i];
                continue;
            }
            let r = flow.receiver[i] as usize;
            let (rx, rz) = ((r % grid.n) as i64, (r / grid.n) as i64);
            let (ix, iz) = ((i % grid.n) as i64, (i / grid.n) as i64);
            let dist = if rx != ix && rz != iz { grid.cell_m * super::grid::SQRT2 } else { grid.cell_m };
            let f = p.k_dt * flow.area[i].sqrt() / dist;
            h[i] = (h[i] + p.dt * uplift[i] + f * h[r]) / (1.0 + f);
        }
        diffuse(grid, h, fixed, p.kappa_dt, &mut scratch);
        on_step(step, h);
    }
}

/// Explicit hillslope diffusion, sub-stepped to stay stable.
pub fn diffuse(grid: &Grid, h: &mut [f64], fixed: &[bool], kappa_dt: f64, scratch: &mut [f64]) {
    let limit = 0.2 * grid.cell_m * grid.cell_m;
    let subs = (kappa_dt / limit).ceil().max(1.0);
    let c = kappa_dt / subs / (grid.cell_m * grid.cell_m);
    let n = grid.n;
    for _ in 0..subs as u32 {
        scratch.copy_from_slice(h);
        for z in 1..n - 1 {
            for x in 1..n - 1 {
                let i = z * n + x;
                if fixed[i] {
                    continue;
                }
                let lap = scratch[i - 1] + scratch[i + 1] + scratch[i - n] + scratch[i + n] - 4.0 * scratch[i];
                h[i] = scratch[i] + c * lap;
            }
        }
    }
}

/// Lowers any cell steeper than `max_slope` toward its lowest neighbour.
pub fn relax_talus(grid: &Grid, h: &mut [f64], fixed: &[bool], max_slope: f64, passes: u32) {
    for _ in 0..passes {
        for i in 0..grid.len() {
            if fixed[i] {
                continue;
            }
            let mut low = (i, 0.0);
            for (j, d) in grid.neighbours(i) {
                let s = (h[i] - h[j]) / d;
                if s > low.1 {
                    low = (j, s);
                }
            }
            if low.1 > max_slope {
                let (j, d) = (low.0, if low.1 > 0.0 { (h[i] - h[low.0]) / low.1 } else { grid.cell_m });
                let excess = (h[i] - h[j] - max_slope * d) * 0.5;
                h[i] -= excess;
                if !fixed[j] {
                    h[j] += excess;
                }
            }
        }
    }
}
