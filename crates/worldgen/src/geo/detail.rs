//! Mid-scale relief: gullies and spurs from a local erosion solve per tile.
//!
//! A tile starts from the planet surface plus seeded roughness and erodes it.
//! Only the high-passed, clamped difference is kept, so detail cuts gullies
//! and leaves spurs without moving the planet-scale surface or adding peaks.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

use super::erosion::{self, Params};
use super::grid::Grid;
use super::noise::{fbm, smoothstep};
use super::planet::Planet;
use super::SEA_M;

pub const TILE_M: f64 = 1024.0;
pub const CELL_M: f64 = 16.0;
/// Overlap past the tile core: half a tile for blending plus the fixed rim.
const PAD_M: f64 = TILE_M * 0.5 + 2.0 * CELL_M;
const CELLS: usize = ((TILE_M + 2.0 * PAD_M) / CELL_M) as usize;
const BLUR: usize = 4;

pub struct Tile {
    grid: Grid,
    delta: Vec<f32>,
}

/// Slope of the planet surface in metres per metre.
pub fn planet_slope(planet: &Planet, x: f64, z: f64) -> f64 {
    let d = planet.grid.cell_m * 0.5;
    let g = &planet.grid;
    let sx = (g.bilinear(&planet.height, x + d, z) - g.bilinear(&planet.height, x - d, z)) / (2.0 * d);
    let sz = (g.bilinear(&planet.height, x, z + d) - g.bilinear(&planet.height, x, z - d)) / (2.0 * d);
    (sx * sx + sz * sz).sqrt()
}

fn build(planet: &Planet, seed: u32, tx: i32, tz: i32) -> Tile {
    let grid = Grid { n: CELLS, cell_m: CELL_M, min_m: [tx as f64 * TILE_M - PAD_M, tz as f64 * TILE_M - PAD_M] };
    let len = grid.len();
    let mut base = vec![0.0; len];
    let mut reach = vec![0.0; len];
    let mut fixed = vec![false; len];
    let mut h = vec![0.0; len];
    for i in 0..len {
        let [x, z] = grid.at(i);
        base[i] = planet.grid.bilinear(&planet.height, x, z);
        let slope = planet_slope(planet, x, z);
        reach[i] = (4.0 + slope * 40.0).min(36.0);
        fixed[i] = grid.on_border(i) || base[i] < SEA_M + 0.5;
        let rough = (1.5 + slope * 45.0).min(28.0);
        let n = fbm(x / 180.0, z / 180.0, seed ^ 0x5a17, 4) - 0.5;
        h[i] = base[i] + if fixed[i] { 0.0 } else { rough * 2.0 * n };
    }
    let zero = vec![0.0; len];
    let ones = vec![1.0; len];
    let p = Params { k_dt: 0.08, dt: 1.0, kappa_dt: 6.0, steps: 24 };
    erosion::evolve(&grid, &mut h, &fixed, &zero, &ones, p, |_, _| {});
    let raw: Vec<f64> = (0..len).map(|i| (h[i] - base[i]).clamp(-reach[i], reach[i])).collect();
    let smooth = box_blur(&raw, CELLS, BLUR);
    let delta = (0..len)
        .map(|i| {
            if fixed[i] {
                return 0.0;
            }
            (raw[i] - smooth[i]).clamp(-reach[i], 0.2 * reach[i]) as f32
        })
        .collect();
    Tile { grid, delta }
}

fn box_blur(f: &[f64], n: usize, r: usize) -> Vec<f64> {
    let pass = |src: &[f64], step_x: bool| {
        let mut out = vec![0.0; src.len()];
        for z in 0..n {
            for x in 0..n {
                let (mut sum, mut count) = (0.0, 0.0);
                let c = if step_x { x } else { z };
                for k in c.saturating_sub(r)..=(c + r).min(n - 1) {
                    sum += if step_x { src[z * n + k] } else { src[k * n + x] };
                    count += 1.0;
                }
                out[z * n + x] = sum / count;
            }
        }
        out
    };
    pass(&pass(f, true), false)
}

type Key = (u32, u64, i32, i32);
type Slot = Arc<OnceLock<Arc<Tile>>>;

#[derive(Default)]
struct Cache {
    slots: HashMap<Key, Slot>,
    recent: VecDeque<Key>,
}

static TILES: OnceLock<Mutex<Cache>> = OnceLock::new();

thread_local! {
    static HOT: RefCell<VecDeque<(Key, Arc<Tile>)>> = const { RefCell::new(VecDeque::new()) };
}

fn tile(planet: &Planet, seed: u32, tx: i32, tz: i32) -> Arc<Tile> {
    let key = (seed, planet.checksum, tx, tz);
    if let Some(t) = HOT.with(|hot| hot.borrow().iter().find(|(k, _)| *k == key).map(|(_, t)| Arc::clone(t))) {
        return t;
    }
    let slot = {
        let mut cache = TILES.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = cache.slots.get(&key) {
            Arc::clone(s)
        } else {
            if cache.slots.len() >= 256
                && let Some(old) = cache.recent.pop_front()
            {
                cache.slots.remove(&old);
            }
            let s: Slot = Arc::new(OnceLock::new());
            cache.slots.insert(key, Arc::clone(&s));
            cache.recent.push_back(key);
            s
        }
    };
    let t = Arc::clone(slot.get_or_init(|| Arc::new(build(planet, seed, tx, tz))));
    HOT.with(|hot| {
        let mut hot = hot.borrow_mut();
        if hot.len() >= 8 {
            hot.pop_front();
        }
        hot.push_back((key, Arc::clone(&t)));
    });
    t
}

/// Mid-scale height offset at a world point, blended across four overlapping tiles.
pub fn offset(planet: &Planet, seed: u32, x: f64, z: f64) -> f64 {
    let (u, v) = (x / TILE_M - 0.5, z / TILE_M - 0.5);
    let (tx, tz) = (u.floor() as i32, v.floor() as i32);
    let fx = smoothstep(0.0, 1.0, u - u.floor());
    let fz = smoothstep(0.0, 1.0, v - v.floor());
    let mut sum = 0.0;
    for dz in 0..2 {
        for dx in 0..2 {
            let w = if dx == 0 { 1.0 - fx } else { fx } * if dz == 0 { 1.0 - fz } else { fz };
            if w <= 0.0 {
                continue;
            }
            let t = tile(planet, seed, tx + dx, tz + dz);
            sum += t.grid.bilinear(&t.delta, x, z) * w;
        }
    }
    sum
}
