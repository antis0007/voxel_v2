//! Shared watershed graph in physical metres. Every node chooses the same
//! downhill neighbor regardless of the requesting chunk, LOD or voxel size.
//! Strictly decreasing head prevents cycles; minima are explicit closed basins.
//! This is an initial drainage plan, not a continuously replenished water source.
use super::{WorldSampleConfig, WorldType, hash_u32, landscape};
use engine_core::fast_hash::FastMap;
use engine_core::{VoxelScalePreset, environment::ENVIRONMENT_GENERATOR};
use glam::Vec2;
use std::cell::RefCell;
const SPACING: f32 = 192.0;
/// Generator 50: jittered nodes, eight-way steepest descent and meandering,
/// width-varying channels. Earlier environment worlds keep the axis lattice.
pub const MEANDER_GENERATOR: u32 = 50;
/// Node jitter as a fraction of `SPACING` from generator 50. With the meander
/// amplitude and bank below, every channel stays inside `sample`'s 4x4 window.
const MEANDER_JITTER: f32 = 0.38;
const MEANDER_AMPLITUDE: f32 = 0.14;
/// Generator 53: node heads come from the current terrain, and wet channels
/// get a cross-section whose banks contain the water surface.
pub const RIVER_FIT_GENERATOR: u32 = 53;
/// Most a bank may be raised above the terrain to hold the water surface.
pub const LEVEE_CAP_M: f32 = 2.5;
/// Bank distance beyond the water edge that a fitted channel can still carve.
const FIT_REACH_M: f32 = 16.0;
type Key = (u32, u8, u32, i32, i32);
/// Entries per thread-local plan cache. Horizon workers walk kilometres, and a
/// fitted node costs 36 terrain evaluations, so a full clear must stay rare.
const CACHE_CAP: usize = 32_768;

fn meanders(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= MEANDER_GENERATOR
}

fn fits(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= RIVER_FIT_GENERATOR
}

/// Cache identity of a plan: fitted plans read the current terrain, so they
/// key by generator version; older plans key by lattice or meander.
fn plan_id(cfg: &WorldSampleConfig) -> u32 {
    if fits(cfg) { cfg.generator_version } else { u32::from(meanders(cfg)) }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    pub center_m: Vec2,
    pub head_m: f32,
    pub rainfall: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outlet {
    Ocean,
    Downhill(i32, i32),
    ClosedBasin,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Channel {
    pub bank: f32,
    /// Before generator 53 a bed height blended in by `bank`; from 53 the
    /// highest ground top allowed here.
    pub bed_m: f32,
    pub water_m: Option<f32>,
    /// Generator 53: the lowest ground top allowed here, so banks hold water.
    pub levee_m: Option<f32>,
}
thread_local! {
    static NODES: RefCell<FastMap<Key, Node>> = RefCell::new(FastMap::default());
    static OUTLETS: RefCell<FastMap<Key, Outlet>> = RefCell::new(FastMap::default());
}

pub fn node(cfg: &WorldSampleConfig, x: i32, z: i32) -> Node {
    let key = (cfg.seed, cfg.world_type as u8, plan_id(cfg), x, z);
    if let Some(n) = NODES.with(|nodes| nodes.borrow().get(&key).copied()) {
        return n;
    }
    let h = hash_u32(cfg.seed ^ (x as u32).wrapping_mul(73856093) ^ (z as u32).wrapping_mul(19349663));
    let jitter = if meanders(cfg) { MEANDER_JITTER * 2.0 * SPACING } else { 64.0 };
    let center = Vec2::new(
        x as f32 * SPACING + ((h & 255) as f32 / 255.0 - 0.5) * jitter,
        z as f32 * SPACING + (((h >> 8) & 255) as f32 / 255.0 - 0.5) * jitter,
    );
    let value = if fits(cfg) {
        fit::head(cfg, (x, z), center)
    } else {
        // Pin the pre-drainage generator to avoid recursion and preserve this
        // model's initial conditions when future terrain versions are introduced.
        let base = WorldSampleConfig { generator_version: 41, voxel_scale: VoxelScalePreset::OneMeter, ..*cfg };
        let head_m = landscape::preview_height_lod(&base, center.x.round() as i32, center.y.round() as i32, 16.0);
        let (_, rainfall, _) = landscape::visual_ecology(&base, center.x, center.y, head_m);
        Node { center_m: center, head_m, rainfall }
    };
    NODES.with(|nodes| {
        let mut nodes = nodes.borrow_mut();
        if nodes.len() >= CACHE_CAP {
            nodes.clear();
        }
        nodes.insert(key, value);
    });
    value
}

pub fn outlet(cfg: &WorldSampleConfig, x: i32, z: i32) -> Outlet {
    let current = node(cfg, x, z);
    if current.head_m <= 12.0 {
        return Outlet::Ocean;
    }
    if meanders(cfg) {
        let key = (cfg.seed, cfg.world_type as u8, plan_id(cfg), x, z);
        if let Some(o) = OUTLETS.with(|outlets| outlets.borrow().get(&key).copied()) {
            return o;
        }
        // Steepest descent over all eight neighbours: drop per metre, so a
        // diagonal must fall further to win and the network has no grid bias.
        let mut best: Option<(f32, (i32, i32))> = None;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let n = node(cfg, x + dx, z + dz);
                if (dx, dz) == (0, 0) || n.head_m >= current.head_m - 0.01 {
                    continue;
                }
                let slope = (current.head_m - n.head_m) / (n.center_m - current.center_m).length().max(1.0);
                // Ties keep the first in scan order, so the choice is order-independent of callers.
                if best.is_none_or(|(s, _)| slope > s) {
                    best = Some((slope, (x + dx, z + dz)));
                }
            }
        }
        let o = best.map_or(Outlet::ClosedBasin, |(_, (a, b))| Outlet::Downhill(a, b));
        OUTLETS.with(|outlets| {
            let mut outlets = outlets.borrow_mut();
            if outlets.len() >= CACHE_CAP {
                outlets.clear();
            }
            outlets.insert(key, o);
        });
        return o;
    }
    [(x - 1, z), (x, z - 1), (x, z + 1), (x + 1, z)]
        .into_iter()
        .filter(|&(a, b)| node(cfg, a, b).head_m < current.head_m - 0.01)
        .min_by(|&(a, b), &(c, d)| {
            node(cfg, a, b).head_m.total_cmp(&node(cfg, c, d).head_m).then((a, b).cmp(&(c, d)))
        })
        .map_or(Outlet::ClosedBasin, |(a, b)| Outlet::Downhill(a, b))
}

pub fn sample(cfg: &WorldSampleConfig, x: f32, z: f32) -> Channel {
    sample_window(cfg, x, z, -1..=2)
}

fn sample_window(cfg: &WorldSampleConfig, x: f32, z: f32, window: std::ops::RangeInclusive<i32>) -> Channel {
    if cfg.generator_version < ENVIRONMENT_GENERATOR || cfg.world_type == WorldType::Superflat {
        return Channel::default();
    }
    let p = Vec2::new(x, z);
    let ix = (x / SPACING).floor() as i32;
    let iz = (z / SPACING).floor() as i32;
    if fits(cfg) {
        return fit::sample(cfg, p, (ix, iz), window);
    }
    let mut best = Channel::default();
    for dz in window.clone() {
        for dx in window.clone() {
            let (a, b) = (ix + dx, iz + dz);
            let source = node(cfg, a, b);
            let (distance, head, wet, width) = match outlet(cfg, a, b) {
                Outlet::Ocean => continue,
                Outlet::ClosedBasin => (
                    (p - source.center_m).length(),
                    source.head_m + 0.6,
                    source.rainfall > 0.35,
                    9.0 + source.rainfall * 9.0,
                ),
                Outlet::Downhill(c, d) if meanders(cfg) => {
                    let target = node(cfg, c, d);
                    let (distance, t, width) = meander(cfg, (a, b), source, target, p);
                    (distance, source.head_m + (target.head_m - source.head_m) * t, source.rainfall > 0.3, width)
                }
                Outlet::Downhill(c, d) => {
                    let target = node(cfg, c, d);
                    let delta = target.center_m - source.center_m;
                    let t = ((p - source.center_m).dot(delta) / delta.length_squared()).clamp(0.0, 1.0);
                    let distance = (p - (source.center_m + delta * t)).length();
                    (
                        distance,
                        source.head_m + (target.head_m - source.head_m) * t,
                        source.rainfall > 0.3,
                        2.5 + source.rainfall * 3.5,
                    )
                }
            };
            let bank = 1.0 - super::smoothstep(width, width + 8.0, distance);
            if bank > best.bank {
                best =
                    Channel { bank, bed_m: head - 1.5, water_m: (wet && distance <= width).then_some(head), levee_m: None };
            }
        }
    }
    best
}

/// Distance from `p` to the meandering channel from `source` to `target`, the
/// channel parameter there, and the local half-width. The centreline bends
/// perpendicular to the chord by `A (s1 sin(pi t) + s2 sin(2 pi t) + s3 sin(3 pi t))`, which is
/// zero at both nodes so tributaries still meet their outlet. Shape comes from
/// a per-edge hash, so every chunk, LOD and voxel size sees the same river.
fn meander(cfg: &WorldSampleConfig, edge: (i32, i32), source: Node, target: Node, p: Vec2) -> (f32, f32, f32) {
    use std::f32::consts::PI;
    let delta = target.center_m - source.center_m;
    let length = delta.length().max(1.0);
    let along = delta / length;
    let across = Vec2::new(-along.y, along.x);
    let local = p - source.center_m;
    let t = (local.dot(along) / length).clamp(0.0, 1.0);
    // Far from the chord the bend (at most 0.2 x chord, slope factor below
    // 1.9) cannot bring the bank here, so skip the trigonometry.
    let max_width = (2.5 + source.rainfall * 3.5) * 1.15 * 1.3;
    let chord = (p - (source.center_m + delta * t)).length();
    let reach = if fits(cfg) { FIT_REACH_M } else { 8.0 };
    if chord > 0.2 * length + 1.9 * (max_width + reach) {
        return (chord, t, max_width);
    }
    let weights = harmonics(cfg, edge);
    let [s1, s2, s3, w] = weights;
    let amplitude = MEANDER_AMPLITUDE * length;
    let wave = |k: f32| (k * PI * t).sin();
    let dwave = |k: f32| k * PI * (k * PI * t).cos();
    let offset = bend_offset(weights, length, t);
    // Slope correction fades to zero at the nodes so the bent reach and the
    // round caps agree there (no seam where they meet).
    let slope = amplitude * (s1 * dwave(1.0) + s2 * dwave(2.0) + s3 * dwave(3.0)) / length * wave(1.0);
    // Perpendicular distance to the bent centreline, corrected for its slope;
    // past either end the plain distance to that node keeps round caps.
    let distance = if local.dot(along) < 0.0 || local.dot(along) > length {
        (p - (source.center_m + delta * t)).length()
    } else {
        (local.dot(across) - offset).abs() / (1.0 + slope * slope).sqrt()
    };
    // Widen downstream and pulse along the reach so banks never run ruler-straight.
    let pulse = 1.0 + 0.3 * (PI * (2.0 * t + w)).sin() * (PI * t).sin();
    let width = (2.5 + source.rainfall * 3.5) * (0.85 + 0.3 * t) * pulse;
    (distance, t, width)
}

/// Per-edge meander weights `[s1, s2, s3, pulse phase]`.
fn harmonics(cfg: &WorldSampleConfig, edge: (i32, i32)) -> [f32; 4] {
    let h = hash_u32(cfg.seed ^ 0x6d65_616e ^ (edge.0 as u32).wrapping_mul(83492791) ^ (edge.1 as u32).wrapping_mul(2971215073));
    let unit = |shift: u32| ((h >> shift) & 255) as f32 / 255.0 * 2.0 - 1.0;
    [unit(0), unit(8) * 0.7, unit(16) * 0.4, unit(24)]
}

/// Centreline offset across the chord at parameter `t`.
fn bend_offset([s1, s2, s3, _]: [f32; 4], length: f32, t: f32) -> f32 {
    use std::f32::consts::PI;
    let amplitude = MEANDER_AMPLITUDE * length;
    let wave = |k: f32| (k * PI * t).sin();
    // The clamp bounds the reach `sample`'s window relies on (0.2 x chord).
    (amplitude * (s1 * wave(1.0) + s2 * wave(2.0) + s3 * wave(3.0))).clamp(-0.2 * length, 0.2 * length)
}

mod fit;
#[cfg(test)]
mod tests;
