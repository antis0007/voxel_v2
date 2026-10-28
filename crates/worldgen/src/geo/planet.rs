//! Whole-face terrain solve, its progress reporting and its process-wide cache.

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use super::climate;
use super::erosion::{self, Params};
use super::grid::Grid;
use super::noise::{fbm, smoothstep, unit, value};
use super::tectonics;
use super::{PlanetParams, SEA_M};

/// Solve stages in order; the loading screen names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Stage {
    Plates = 0,
    Uplift = 1,
    Climate = 2,
    Carving = 3,
    Rivers = 4,
    Done = 5,
}

const UPLIFT_STEPS: u32 = 60;
const CARVING_STEPS: u32 = 60;

/// Snapshot of the solve for display: land heights in [0, 1], ocean as -1.
pub struct Preview {
    pub n: usize,
    pub height: Vec<f32>,
    /// Precipitation in [0, 1), once the climate stage has run.
    pub wet: Option<Vec<f32>>,
    /// Upstream area in square kilometres, once rivers are routed.
    pub discharge_km2: Option<Vec<f32>>,
}

/// Shared progress of one planet solve; every count is real work done.
#[derive(Default)]
pub struct Progress {
    stage: AtomicU8,
    step: AtomicU32,
    preview: Mutex<Option<Arc<Preview>>>,
}

impl Progress {
    pub fn stage(&self) -> Stage {
        match self.stage.load(Ordering::Relaxed) {
            0 => Stage::Plates,
            1 => Stage::Uplift,
            2 => Stage::Climate,
            3 => Stage::Carving,
            4 => Stage::Rivers,
            _ => Stage::Done,
        }
    }

    /// Erosion steps finished out of the total the solve runs.
    pub fn steps(&self) -> (u32, u32) {
        (self.step.load(Ordering::Relaxed), UPLIFT_STEPS + CARVING_STEPS)
    }

    pub fn preview(&self) -> Option<Arc<Preview>> {
        self.preview.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set_stage(&self, s: Stage) {
        self.stage.store(s as u8, Ordering::Relaxed);
    }

    fn publish(&self, p: Preview) {
        *self.preview.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(p));
    }
}

pub struct Planet {
    pub grid: Grid,
    /// Ground top in metres.
    pub height: Vec<f32>,
    /// Lake surface in metres where a depression holds water, else `f32::MIN`.
    pub lake: Vec<f32>,
    /// Upstream area weighted by rainfall, square metres.
    pub discharge: Vec<f32>,
    pub receiver: Vec<u32>,
    pub land: Vec<bool>,
    pub maat: Vec<f32>,
    pub wet: Vec<f32>,
    pub season: Vec<f32>,
    /// River node per cell: the cell centre moved by a hashed offset, so channels never follow the grid.
    pub node: Vec<[f32; 2]>,
    pub checksum: u64,
}

fn preview_of(h: &[f64], fixed: &[bool], n: usize) -> Preview {
    let top = h.iter().zip(fixed).filter(|(_, f)| !**f).map(|(v, _)| *v).fold(1e-9, f64::max);
    let height = h.iter().zip(fixed).map(|(v, f)| if *f { -1.0 } else { (v / top) as f32 }).collect();
    Preview { n, height, wet: None, discharge_km2: None }
}

/// Solve units to metres along a concave hypsometric curve: most land stays
/// low and the upper slopes of ranges steepen, as on real continents.
fn to_metres(h: &[f64], fixed: &[bool], relief_m: f64) -> Vec<f64> {
    let top = h.iter().zip(fixed).filter(|(_, f)| !**f).map(|(v, _)| *v).fold(1e-9, f64::max);
    h.iter()
        .map(|v| {
            let t = v.max(0.0) / top;
            SEA_M + 0.6 + t * (0.3 + 0.7 * t) * relief_m
        })
        .collect()
}

pub fn build(seed: u32, p: &PlanetParams, progress: &Progress) -> Planet {
    let n = p.cells;
    let half = p.face_edge_m * 0.5;
    let grid = Grid { n, cell_m: p.face_edge_m / n as f64, min_m: [-half, -half] };
    progress.set_stage(Stage::Plates);
    let tect = tectonics::build(&grid, seed, p);
    let fixed: Vec<bool> = (0..grid.len()).map(|i| !tect.land[i] || grid.on_border(i)).collect();
    // Small initial roughness seeds the channel network; the solve does the rest.
    let mut h: Vec<f64> = (0..grid.len())
        .map(|i| {
            let [x, z] = grid.at(i);
            if fixed[i] { 0.0 } else { 5.0 * fbm(x / 1500.0, z / 1500.0, seed ^ 31, 3) + 3.0 * value(x / 400.0, z / 400.0, seed ^ 32) }
        })
        .collect();
    progress.publish(preview_of(&h, &fixed, n));

    // Diffusion scales with cell area so the landscape keeps its shape at any resolution.
    let kappa_dt = 100.0 * (grid.cell_m / 128.0) * (grid.cell_m / 128.0);
    let params = |steps| Params { k_dt: 0.05, dt: 25.0, kappa_dt, steps };
    let mut done = 0;
    let mut tick = |h: &[f64]| {
        done += 1;
        progress.step.store(done, Ordering::Relaxed);
        if done % 4 == 0 {
            progress.publish(preview_of(h, &fixed, n));
        }
    };
    progress.set_stage(Stage::Uplift);
    let ones = vec![1.0; grid.len()];
    erosion::evolve(&grid, &mut h, &fixed, &tect.uplift, &ones, params(UPLIFT_STEPS), |_, h| tick(h));

    progress.set_stage(Stage::Climate);
    let first = climate::solve(&grid, &to_metres(&h, &fixed, p.relief_m), &tect.land, p);
    let weight: Vec<f64> = first.wet.iter().map(|&w| 0.3 + 1.4 * w as f64).collect();

    progress.set_stage(Stage::Carving);
    erosion::evolve(&grid, &mut h, &fixed, &tect.uplift, &weight, params(CARVING_STEPS), |_, h| tick(h));

    progress.set_stage(Stage::Rivers);
    let mut hm = to_metres(&h, &fixed, p.relief_m);
    let coast = coast_distance(&grid, &fixed);
    // Continents stand clear of the sea inland; the rise grows away from every
    // coast, so it keeps each flow path running downhill.
    for i in 0..grid.len() {
        if !fixed[i] {
            hm[i] += 22.0 * smoothstep(0.0, 24.0, coast[i] as f64);
        }
    }
    erosion::relax_talus(&grid, &mut hm, &fixed, 0.85, 4);
    sink_ocean(&grid, &mut hm, &fixed);
    let flow = erosion::route(&grid, &hm, &fixed, &weight);
    // Hollows too shallow to hold a lake fill to their spill level; rift lakes
    // keep sinking during the solve, so their depth is capped.
    for i in 0..grid.len() {
        let depth = flow.spill[i] - hm[i];
        hm[i] = if depth <= 0.5 { hm[i].max(flow.spill[i]) } else { hm[i].max(flow.spill[i] - 30.0) };
    }
    let lake: Vec<f32> = (0..grid.len())
        .map(|i| if !fixed[i] && flow.spill[i] > hm[i] + 0.5 { flow.spill[i] as f32 } else { f32::MIN })
        .collect();
    let climate = climate::solve(&grid, &hm, &tect.land, p);
    let node = (0..grid.len())
        .map(|i| {
            let [x, z] = grid.at(i);
            let (cx, cz) = ((i % n) as i64, (i / n) as i64);
            let j = |salt| (unit(cx, cz, seed.wrapping_add(salt)) - 0.5) * 0.7 * grid.cell_m;
            [(x + j(41)) as f32, (z + j(42)) as f32]
        })
        .collect();
    let height: Vec<f32> = hm.iter().map(|&v| v as f32).collect();
    let mut planet = Planet {
        grid,
        height,
        lake,
        discharge: flow.area.iter().map(|&a| a as f32).collect(),
        receiver: flow.receiver,
        land: tect.land,
        maat: climate.maat,
        wet: climate.wet,
        season: climate.season,
        node,
        checksum: 0,
    };
    planet.checksum = checksum(&planet);
    let mut last = preview_of(&h, &fixed, n);
    last.wet = Some(planet.wet.clone());
    last.discharge_km2 = Some(planet.discharge.iter().map(|a| a / 1.0e6).collect());
    progress.publish(last);
    progress.set_stage(Stage::Done);
    planet
}

/// Cells from each cell to the nearest cell where `source` is true.
fn distance_to(grid: &Grid, source: impl Fn(usize) -> bool) -> Vec<u32> {
    let mut dist = vec![u32::MAX; grid.len()];
    let mut queue = std::collections::VecDeque::new();
    for i in 0..grid.len() {
        if source(i) {
            dist[i] = 0;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        for (j, _) in grid.neighbours(i) {
            if dist[j] == u32::MAX {
                dist[j] = dist[i] + 1;
                queue.push_back(j);
            }
        }
    }
    dist
}

fn coast_distance(grid: &Grid, fixed: &[bool]) -> Vec<u32> {
    distance_to(grid, |i| fixed[i])
}

/// Ocean floor deepens away from the coast.
fn sink_ocean(grid: &Grid, hm: &mut [f64], fixed: &[bool]) {
    let dist = distance_to(grid, |i| !fixed[i]);
    for i in 0..grid.len() {
        if fixed[i] {
            let d = if dist[i] == u32::MAX { 64.0 } else { dist[i] as f64 };
            hm[i] = SEA_M - 4.0 - 36.0 * smoothstep(0.0, 6.0, d);
        }
    }
}

fn checksum(p: &Planet) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let fields = [&p.height, &p.lake, &p.discharge, &p.maat, &p.wet];
    for f in fields {
        for v in f.iter() {
            for b in v.to_bits().to_le_bytes() {
                h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
            }
        }
    }
    h
}

/// One planet's cache slot: its solve progress and, once finished, the planet.
#[derive(Default)]
pub struct Entry {
    pub progress: Progress,
    planet: OnceLock<Arc<Planet>>,
    prewarmed: std::sync::atomic::AtomicBool,
}

impl Entry {
    pub fn ready(&self) -> bool {
        self.planet.get().is_some()
    }

    /// True for the first caller only.
    pub fn claim_prewarm(&self) -> bool {
        !self.prewarmed.swap(true, Ordering::Relaxed)
    }
}

static PLANETS: OnceLock<Mutex<Vec<((u32, u64), Arc<Entry>)>>> = OnceLock::new();

/// The cache slot for a planet; whichever thread builds it reports into its progress.
pub fn entry(seed: u32, p: &PlanetParams) -> Arc<Entry> {
    let key = (seed, p.fingerprint());
    let mut all = PLANETS.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, e)) = all.iter().find(|(k, _)| *k == key) {
        return Arc::clone(e);
    }
    if all.len() >= 3 {
        all.remove(0);
    }
    let e = Arc::new(Entry::default());
    all.push((key, Arc::clone(&e)));
    e
}

/// The solved planet for a seed, built once per process.
///
/// Concurrent callers for the same planet wait for one build instead of repeating it.
pub fn get(seed: u32, p: &PlanetParams) -> Arc<Planet> {
    let e = entry(seed, p);
    Arc::clone(e.planet.get_or_init(|| Arc::new(build(seed, p, &e.progress))))
}
