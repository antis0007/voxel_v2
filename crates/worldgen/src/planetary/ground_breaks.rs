//! Generator 293: breaks in the ground of a Planet world, carved in 3D below
//! the solved surface through `cave_carves`.
//!
//! - Ravines: meandering slots 100 to 240 m long and 20 to 58 m deep. Depth
//!   and width fall to nothing at both ends, so each end is a ramp in; the
//!   wider ones keep a bench a third of the way down. More common, and
//!   aligned, where the plates are pulling apart.
//! - Fissure swarms: short narrow cracks parallel to a spreading boundary.
//! - Sinkholes: bell-shaped shafts, mostly on carbonate ground.
//!
//! Every feature lies inside its own grid cell, so a voxel tests one
//! candidate per layer. A feature is dropped when water stands over any
//! probe along it, nothing is carved within 3 m of sea level, and a break
//! stops 2 m under the water table, so a wet one holds a pool, not a lake
//! standing over the caves below. The far
//! horizon draws the solved surface, so these stay narrow enough to read as
//! a crack until the chunks arrive.
use super::{SEA, column_with_water, direction, get};
use crate::WorldSampleConfig;
use crate::{hash2, value_noise2};
use glam::IVec3;
use std::cell::RefCell;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI};

/// Deepest any break reaches below the surface, metres.
pub(super) const MAX_DEPTH: f32 = 60.0;
/// Rock left over sea level, metres.
pub(super) const FREEBOARD: f32 = 3.0;

/// Deepest standing water in a break, metres.
const POOL: f32 = 2.0;

const RAVINE_CELL: f32 = 320.0;
const FISSURE_CELL: f32 = 96.0;
const SINK_CELL: f32 = 176.0;
const PROVINCE_CELL: f32 = 1024.0;
/// Extension stress above which fissures open and ravines follow the strike.
const RIFTING: f32 = 0.25;

/// Plate state shared by every feature in a kilometre cell.
#[derive(Clone, Copy)]
struct Province {
    /// Extension stress, 0 to 1.
    rift: f32,
    carbonate: f32,
    /// Direction along the nearest plate boundary, radians.
    strike: f32,
}

#[derive(Clone, Copy)]
struct Slot {
    cx: f32,
    cz: f32,
    cos: f32,
    sin: f32,
    half_len: f32,
    half_w: f32,
    depth: f32,
    meander: f32,
    phase: f32,
    bench: bool,
}

#[derive(Clone, Copy)]
struct Sink {
    cx: f32,
    cz: f32,
    r: f32,
    depth: f32,
}

/// A cell map with the last cell kept in hand: neighbouring voxels ask for
/// the same cell, so most lookups are one comparison.
struct Memo<V> {
    last: Option<((i32, i32), V)>,
    map: HashMap<(i32, i32), V>,
}

impl<V: Copy> Memo<V> {
    fn new() -> Self {
        Self { last: None, map: HashMap::new() }
    }
    fn get(&mut self, key: (i32, i32)) -> Option<V> {
        if let Some((k, v)) = self.last
            && k == key
        {
            return Some(v);
        }
        let v = *self.map.get(&key)?;
        self.last = Some((key, v));
        Some(v)
    }
    fn put(&mut self, key: (i32, i32), v: V) {
        if self.map.len() >= 8192 {
            self.map.clear();
        }
        self.map.insert(key, v);
        self.last = Some((key, v));
    }
}

struct Cache {
    cfg: WorldSampleConfig,
    provinces: Memo<Province>,
    ravines: Memo<Option<Slot>>,
    fissures: Memo<Option<Slot>>,
    sinks: Memo<Option<Sink>>,
}

thread_local! {
    static CACHE: RefCell<Option<Cache>> = const { RefCell::new(None) };
}

/// Look a cell up, building it outside the borrow: a build queries the
/// terrain, which must be free to come back here.
fn cached<V: Copy>(cfg: &WorldSampleConfig, key: (i32, i32), pick: impl Fn(&mut Cache) -> &mut Memo<V>, build: impl FnOnce() -> V) -> V {
    let hit = CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.as_ref().is_none_or(|c| c.cfg != *cfg) {
            *c = Some(Cache { cfg: *cfg, provinces: Memo::new(), ravines: Memo::new(), fissures: Memo::new(), sinks: Memo::new() });
        }
        c.as_mut().and_then(|c| pick(c).get(key))
    });
    if let Some(v) = hit {
        return v;
    }
    let v = build();
    CACHE.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut().filter(|c| c.cfg == *cfg) {
            pick(c).put(key, v);
        }
    });
    v
}

fn cell(x: f32, z: f32, size: f32) -> (i32, i32) {
    ((x / size).floor() as i32, (z / size).floor() as i32)
}

fn province(cfg: &WorldSampleConfig, x: f32, z: f32) -> Province {
    let key = cell(x, z, PROVINCE_CELL);
    cached(cfg, key, |c| &mut c.provinces, || {
        let world = get(cfg);
        let (cx, cz) = ((key.0 as f32 + 0.5) * PROVINCE_CELL, (key.1 as f32 + 0.5) * PROVINCE_CELL);
        let at = |x: f32, z: f32| world.plates.at(direction(cfg, f64::from(x), f64::from(z)));
        let here = at(cx, cz);
        let gx = (at(cx + 400.0, cz).boundary_m - at(cx - 400.0, cz).boundary_m) as f32;
        let gz = (at(cx, cz + 400.0).boundary_m - at(cx, cz - 400.0).boundary_m) as f32;
        // The boundary runs across the gradient of the distance to it.
        let strike = if gx.hypot(gz) > 1.0 { gz.atan2(gx) + FRAC_PI_2 } else { hash2(key.0, key.1, cfg.seed.wrapping_add(29_311)) * PI };
        Province { rift: (here.rift as f32).clamp(0.0, 1.0), carbonate: (here.carbonate as f32).clamp(0.0, 1.0), strike }
    })
}

/// Dry land at a point, high enough over the sea to carve into.
fn dry(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    let (column, water) = column_with_water(cfg, cfg.blocks_for_height(x), cfg.blocks_for_height(z), cfg.voxel_meters());
    water.is_none() && f64::from(cfg.world_y(column.height)) > SEA + 8.0
}

fn slot(cfg: &WorldSampleConfig, key: (i32, i32), fissure: bool) -> Option<Slot> {
    let size = if fissure { FISSURE_CELL } else { RAVINE_CELL };
    let salt = if fissure { 29_400 } else { 29_300 };
    let h = |k: u32| hash2(key.0, key.1, cfg.seed.wrapping_add(salt + k));
    let (mx, mz) = ((key.0 as f32 + 0.5) * size, (key.1 as f32 + 0.5) * size);
    let p = province(cfg, mx, mz);
    let rifting = p.rift > RIFTING;
    let chance = match (fissure, rifting) {
        (true, false) => 0.0,
        (true, true) => 0.2 + 0.6 * p.rift,
        (false, _) => 0.35 + 0.45 * p.rift,
    };
    if h(1) >= chance {
        return None;
    }
    let jitter = if fissure { 6.0 } else { 20.0 };
    let (cx, cz) = (mx + (h(2) - 0.5) * jitter, mz + (h(3) - 0.5) * jitter);
    let angle = match (fissure, rifting) {
        (true, _) => p.strike + (h(4) - 0.5) * 0.3,
        (false, true) => p.strike + (h(4) - 0.5) * 0.5,
        (false, false) => h(4) * PI,
    };
    // Sizes keep the whole feature inside its cell, and the depth shallow
    // enough for its length that the end ramps can be walked.
    let s = if fissure {
        let half_len = 18.0 + h(5) * 20.0;
        Slot { cx, cz, cos: angle.cos(), sin: angle.sin(), half_len, half_w: 1.2 + h(6) * 1.4, depth: (10.0 + h(7) * 24.0).min(half_len * 0.9), meander: 2.0, phase: h(8) * 6.0, bench: false }
    } else {
        let half_len = 50.0 + h(5) * 70.0;
        let half_w = 3.5 + h(6) * 5.5;
        Slot { cx, cz, cos: angle.cos(), sin: angle.sin(), half_len, half_w, depth: (20.0 + h(7) * 38.0).min(half_len * 0.55), meander: h(9) * 14.0, phase: h(8) * 6.0, bench: half_w > 6.5 }
    };
    let probes: &[f32] = if fissure { &[-1.0, 0.0, 1.0] } else { &[-1.0, -0.5, 0.0, 0.5, 1.0] };
    probes.iter().all(|t| dry(cfg, s.cx + s.cos * s.half_len * t, s.cz + s.sin * s.half_len * t)).then_some(s)
}

fn sink(cfg: &WorldSampleConfig, key: (i32, i32)) -> Option<Sink> {
    let h = |k: u32| hash2(key.0, key.1, cfg.seed.wrapping_add(29_500 + k));
    let (mx, mz) = ((key.0 as f32 + 0.5) * SINK_CELL, (key.1 as f32 + 0.5) * SINK_CELL);
    if h(1) >= 0.05 + 0.4 * province(cfg, mx, mz).carbonate {
        return None;
    }
    let s = Sink { cx: mx + (h(2) - 0.5) * 100.0, cz: mz + (h(3) - 0.5) * 100.0, r: 4.0 + h(4) * 6.0, depth: 18.0 + h(5) * 26.0 };
    let reach = s.r * 1.6;
    [(0.0, 0.0), (reach, 0.0), (-reach, 0.0), (0.0, reach), (0.0, -reach)].iter().all(|(dx, dz)| dry(cfg, s.cx + dx, s.cz + dz)).then_some(s)
}

impl Slot {
    fn carves(&self, x: f32, y: f32, z: f32, depth: f32) -> bool {
        let (dx, dz) = (x - self.cx, z - self.cz);
        let along = dx * self.cos + dz * self.sin;
        let t = along / self.half_len;
        if t.abs() >= 1.0 {
            return false;
        }
        // Full depth through the middle, easing to the surface at both ends.
        let e = 1.0 - t * t;
        let floor = self.depth * e * e * (3.0 - 2.0 * e);
        if depth >= floor {
            return false;
        }
        let across = dz * self.cos - dx * self.sin - self.meander * (along * 0.035 + self.phase).sin() * e;
        // Narrowing downward; the bench is a step in that at a third depth.
        let u = depth / floor.max(1.0);
        let shape = 0.45 + 0.55 * (1.0 - u).powf(0.8) + if self.bench && u < 0.35 { 0.6 } else { 0.0 };
        let wobble = (value_noise2(along * 0.08, y * 0.08, 29_601) - 0.5) * 0.5;
        across.abs() < self.half_w * (shape + wobble) * (0.5 + 0.5 * e)
    }
}

impl Sink {
    fn carves(&self, x: f32, y: f32, z: f32, depth: f32) -> bool {
        if depth >= self.depth {
            return false;
        }
        let (dx, dz) = (x - self.cx, z - self.cz);
        let u = depth / self.depth;
        let wobble = (value_noise2(dz.atan2(dx) * 1.5, y * 0.1, 29_603) - 0.5) * 0.3;
        dx.hypot(dz) < self.r * (0.8 + 0.8 * u * u + wobble)
    }
}

/// True where a break opens this voxel. `depth` is metres below the column's
/// surface and `y` its height; the caller has already bounded both.
pub(super) fn carves(cfg: &WorldSampleConfig, p: IVec3, surface: i32, y: f32, depth: f32) -> bool {
    if !opens(cfg, p, y, depth) {
        return false;
    }
    // The same table the carved cell is filled from (`aquifer_pocket`).
    let table = crate::water_table(cfg, p.x, p.z).min(surface - cfg.blocks_for_span(7.0));
    p.y >= table - cfg.blocks_for_span(POOL)
}

fn opens(cfg: &WorldSampleConfig, p: IVec3, y: f32, depth: f32) -> bool {
    let (x, z) = (cfg.world_x(p.x), cfg.world_z(p.z));
    let key = cell(x, z, RAVINE_CELL);
    if cached(cfg, key, |c| &mut c.ravines, || slot(cfg, key, false)).is_some_and(|s| s.carves(x, y, z, depth)) {
        return true;
    }
    let key = cell(x, z, FISSURE_CELL);
    if cached(cfg, key, |c| &mut c.fissures, || slot(cfg, key, true)).is_some_and(|s| s.carves(x, y, z, depth)) {
        return true;
    }
    let key = cell(x, z, SINK_CELL);
    cached(cfg, key, |c| &mut c.sinks, || sink(cfg, key)).is_some_and(|s| s.carves(x, y, z, depth))
}

/// What a square of ground holds, for review: (ravines, fissures, sinkholes).
pub fn census(cfg: &WorldSampleConfig, x0: f32, z0: f32, side: f32) -> (usize, usize, usize) {
    let count = |size: f32, has: &dyn Fn((i32, i32)) -> bool| {
        let (a, b) = (cell(x0, z0, size), cell(x0 + side, z0 + side, size));
        (a.0..=b.0).flat_map(|ix| (a.1..=b.1).map(move |iz| (ix, iz))).filter(|&k| has(k)).count()
    };
    (
        count(RAVINE_CELL, &|k| cached(cfg, k, |c| &mut c.ravines, || slot(cfg, k, false)).is_some()),
        count(FISSURE_CELL, &|k| cached(cfg, k, |c| &mut c.fissures, || slot(cfg, k, true)).is_some()),
        count(SINK_CELL, &|k| cached(cfg, k, |c| &mut c.sinks, || sink(cfg, k)).is_some()),
    )
}

/// The first ravine in a square of ground, for review: (centre x, centre z,
/// unit direction x, unit direction z, half length, depth).
pub fn first_ravine(cfg: &WorldSampleConfig, x0: f32, z0: f32, side: f32) -> Option<[f32; 6]> {
    let (a, b) = (cell(x0, z0, RAVINE_CELL), cell(x0 + side, z0 + side, RAVINE_CELL));
    (a.0..=b.0).flat_map(|ix| (a.1..=b.1).map(move |iz| (ix, iz))).find_map(|k| {
        cached(cfg, k, |c| &mut c.ravines, || slot(cfg, k, false)).map(|s| [s.cx, s.cz, s.cos, s.sin, s.half_len, s.depth])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockId, WorldType, sample_block};

    /// `GB_SEED` (default 74). Prints what a 6 km square round the spawn
    /// holds, then two sections through its first ravine, one character per
    /// metre: across the middle, and along the axis.
    #[test]
    #[ignore = "review dump; set VOXEL_HISTORY_MYR=20 for a quick planet"]
    fn dump_ground_breaks() {
        let seed = std::env::var("GB_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(74);
        let cfg = WorldSampleConfig { generator_version: crate::planetary::GROUND_BREAK_GENERATOR, seed, world_type: WorldType::Planet, voxel_scale: engine_core::VoxelScalePreset::OneMeter, planet: None };
        let spawn = crate::planetary::spawn_point(&cfg);
        let (x0, z0, side) = (spawn.x as f32 - 3000.0, spawn.z as f32 - 3000.0, 6000.0);
        println!("spawn {spawn:?} census (ravines, fissures, sinkholes) {:?}", census(&cfg, x0, z0, side));
        // Where the plates pull apart, within 80 km: how much ground rifts, and
        // what 2 km round the strongest spot holds.
        let mut best = (0.0f32, 0.0f32, 0.0f32);
        let mut rifting = 0;
        for i in -20..=20 {
            for j in -20..=20 {
                let (x, z) = (spawn.x as f32 + i as f32 * 4000.0, spawn.z as f32 + j as f32 * 4000.0);
                let p = province(&cfg, x, z);
                rifting += usize::from(p.rift > RIFTING);
                if p.rift > best.0 && dry(&cfg, x, z) {
                    best = (p.rift, x, z);
                }
            }
        }
        println!("rifting provinces {rifting}/1681, strongest on dry land {:.2} at ({:.0}, {:.0}): census {:?}", best.0, best.1, best.2, census(&cfg, best.1 - 1000.0, best.2 - 1000.0, 2000.0));
        if std::env::var("GB_CENSUS").is_ok() {
            return;
        }
        let Some([cx, cz, cos, sin, half_len, depth]) = first_ravine(&cfg, x0, z0, side) else {
            println!("no ravine in the square");
            return;
        };
        println!("ravine at ({cx:.0}, {cz:.0}) half length {half_len:.0} depth {depth:.0}");
        let top = cfg.world_y(crate::planetary::column(&cfg, cx as i32, cz as i32, 1.0).height) as i32 + 6;
        let glyph = |x: f32, y: i32, z: f32| match sample_block(&cfg, IVec3::new(x.round() as i32, y, z.round() as i32)) {
            BlockId::Air => ' ',
            BlockId::Water => '~',
            b if b.is_solid() => '#',
            _ => '+',
        };
        for (name, reach, ux, uz) in [("across", 28.0, -sin, cos), ("along", half_len + 6.0, cos, sin)] {
            println!("{name}");
            for y in (top - depth as i32 - 12..=top).rev() {
                let n = reach as i32;
                let line: String = (-n..=n).map(|k| glyph(cx + ux * k as f32, y, cz + uz * k as f32)).collect();
                println!("{y:>5} {line}");
            }
        }
    }
}
