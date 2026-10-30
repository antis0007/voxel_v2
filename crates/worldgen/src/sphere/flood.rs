//! Priority-Flood drainage (Barnes et al. 2014) on the sphere cell graph.
//!
//! Each cell's receiver is the neighbour whose pop discovered it, so every
//! receiver precedes its donors in pop order and chains cannot cycle. Cells
//! raised to a spill level go through a FIFO before the heap, which routes
//! flats breadth first from their outlet instead of by cell index.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use super::class::{PlanetParams, ocean_mask};
use super::grid::SphereGrid;
use super::noise::elevation;

/// Heap key ordered by level, then cell index; levels are never NaN.
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

/// Per-cell drainage of one planet, by dense cell index.
#[derive(Clone, Debug, PartialEq)]
pub struct Drainage {
    /// Ground height in metres.
    pub height: Vec<f64>,
    /// Downstream cell; ocean cells and retained basin floors are their own receiver.
    pub receiver: Vec<u32>,
    /// Water surface in metres: spill level in lakes, sea level on ocean, else ground.
    pub filled: Vec<f64>,
    /// Land cell under a filled lake that drains out through its spill point.
    pub lake: Vec<bool>,
    /// Cell of a connected ocean.
    pub ocean: Vec<bool>,
    /// Upstream contributing area in square metres, including the cell itself.
    pub discharge_m2: Vec<f64>,
    /// Cells from base level upstream: every receiver precedes its donors.
    pub order: Vec<u32>,
    /// Floors of retained basins, ascending by index.
    pub basins: Vec<u32>,
}

/// Solves drainage for the class's own height field.
pub fn solve(grid: &SphereGrid, params: &PlanetParams) -> Drainage {
    let h = elevation(grid, params);
    route(grid, h, params)
}

/// Solves drainage for given heights; `None` unless there is one per cell.
pub fn solve_heights(grid: &SphereGrid, h: &[f64], params: &PlanetParams) -> Option<Drainage> {
    let usable = h.len() == grid.len() && !grid.is_empty() && h.iter().all(|x| x.is_finite());
    usable.then(|| route(grid, h.to_vec(), params))
}

fn route(grid: &SphereGrid, h: Vec<f64>, params: &PlanetParams) -> Drainage {
    let ocean = ocean_mask(grid, &h, params);
    let ocean_cells: Vec<u32> = (0..grid.len() as u32).filter(|&i| ocean[i as usize]).collect();
    let basins = retained_basins(grid, &h, &ocean, &ocean_cells, params);
    let mut seeds: Vec<(u32, f64)> =
        ocean_cells.iter().map(|&i| (i, params.sea_level_m.max(h[i as usize]))).collect();
    seeds.extend(basins.iter().map(|&i| (i, h[i as usize])));
    let flood = flood(grid, &h, &seeds);
    let mut discharge_m2 = grid.areas_m2().to_vec();
    for &i in flood.order.iter().rev() {
        let r = flood.receiver[i as usize];
        if r != i {
            discharge_m2[r as usize] += discharge_m2[i as usize];
        }
    }
    let lake = (0..grid.len()).map(|i| !ocean[i] && flood.spill[i] > h[i]).collect();
    Drainage {
        height: h,
        receiver: flood.receiver,
        filled: flood.spill,
        lake,
        ocean,
        discharge_m2,
        order: flood.order,
        basins,
    }
}

/// Picks the strict local minima that stay terminal instead of filling.
///
/// A first flood from the ocean (or, with none, the lowest cell) gives each
/// minimum its depression depth: spill level minus floor.
fn retained_basins(
    grid: &SphereGrid,
    h: &[f64],
    ocean: &[bool],
    ocean_cells: &[u32],
    params: &PlanetParams,
) -> Vec<u32> {
    let key = |i: usize| Key(h[i], i as u32);
    let minima = (0..grid.len()).filter(|&i| {
        !ocean[i] && grid.neighbours(i).iter().all(|&j| key(i) < key(j as usize))
    });
    let lowest = (0..grid.len()).min_by_key(|&i| key(i)).unwrap_or(0) as u32;
    let Some(share) = params.class.retain_share() else {
        return minima.map(|i| i as u32).collect();
    };
    let seeds: Vec<(u32, f64)> = if ocean_cells.is_empty() {
        vec![(lowest, h[lowest as usize])]
    } else {
        ocean_cells.iter().map(|&i| (i, params.sea_level_m.max(h[i as usize]))).collect()
    };
    let spill = flood(grid, h, &seeds).spill;
    let depth = share * params.relief_m;
    let mut kept: Vec<u32> = minima.filter(|&i| spill[i] - h[i] > depth).map(|i| i as u32).collect();
    // A closed sphere with no ocean still needs one base level.
    if ocean_cells.is_empty() && !kept.contains(&lowest) {
        kept.push(lowest);
        kept.sort_unstable();
    }
    kept
}

struct Flood {
    receiver: Vec<u32>,
    spill: Vec<f64>,
    order: Vec<u32>,
}

/// Floods inward from `seeds`, each given as `(cell, starting level)`.
fn flood(grid: &SphereGrid, h: &[f64], seeds: &[(u32, f64)]) -> Flood {
    let len = grid.len();
    let mut receiver: Vec<u32> = (0..len as u32).collect();
    let mut spill = h.to_vec();
    let mut seen = vec![false; len];
    let mut order = Vec::with_capacity(len);
    let mut heap = BinaryHeap::with_capacity(seeds.len());
    let mut pit = VecDeque::new();
    for &(i, level) in seeds {
        if !seen[i as usize] {
            seen[i as usize] = true;
            spill[i as usize] = level;
            heap.push(Reverse(Key(level, i)));
        }
    }
    loop {
        let i = match pit.pop_front() {
            Some(i) => i,
            None => match heap.pop() {
                Some(Reverse(Key(_, i))) => i,
                None => break,
            },
        };
        order.push(i);
        let level = spill[i as usize];
        for j in grid.neighbours(i as usize) {
            let ju = j as usize;
            if seen[ju] {
                continue;
            }
            seen[ju] = true;
            receiver[ju] = i;
            if h[ju] <= level {
                spill[ju] = level;
                pit.push_back(j);
            } else {
                heap.push(Reverse(Key(h[ju], j)));
            }
        }
    }
    Flood { receiver, spill, order }
}
