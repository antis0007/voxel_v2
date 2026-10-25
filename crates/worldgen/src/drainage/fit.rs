//! Generator 53 channels fitted to the current terrain. A water surface never
//! rises above the ground it crosses: node heads come from the lowest ground
//! around each node, each reach stays under the lowest ground met so far along
//! its centreline, and a node's level drops to its lowest incoming reach so
//! surfaces join where reaches meet.
use super::{CACHE_CAP, Channel, FIT_REACH_M, Key, Node, Outlet, bend_offset, harmonics, landscape, node, outlet};
use crate::WorldSampleConfig;
use engine_core::VoxelScalePreset;
use engine_core::fast_hash::FastMap;
use glam::Vec2;
use std::cell::RefCell;
use std::sync::{OnceLock, RwLock};

/// Water surface below the lowest ground around a node or along a reach.
const FREEBOARD_M: f32 = 0.5;
/// Ring radius around a node whose lowest ground bounds its water surface.
const NODE_RING_M: f32 = 12.0;
/// Centreline samples per reach, both nodes included.
const STATIONS: usize = 9;
/// Water depth at the edge of the wetted channel.
const LIP_M: f32 = 0.4;
/// Bank crest height above the water surface.
const CREST_M: f32 = 0.3;
/// Bank crest width beyond the water edge; water stops at its outer side.
const CREST_WIDTH_M: f32 = 2.5;

thread_local! {
    static REACHES: RefCell<FastMap<Key, [f32; STATIONS]>> = RefCell::new(FastMap::default());
    static LEVELS: RefCell<FastMap<Key, f32>> = RefCell::new(FastMap::default());
}

fn cached<V: Copy>(
    map: &'static std::thread::LocalKey<RefCell<FastMap<Key, V>>>, key: Key, make: impl FnOnce() -> V,
) -> V {
    if let Some(v) = map.with(|m| m.borrow().get(&key).copied()) {
        return v;
    }
    let v = make();
    map.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= CACHE_CAP {
            m.clear();
        }
        m.insert(key, v);
    });
    v
}

type Shared<V> = OnceLock<RwLock<FastMap<Key, V>>>;
static SHARED_HEADS: Shared<Node> = OnceLock::new();
static SHARED_REACHES: Shared<[f32; STATIONS]> = OnceLock::new();

/// Cross-thread memo for results that cost dozens of terrain evaluations.
/// Horizon workers each walk the whole far field, so thread-local caches alone
/// replan every node once per worker. Results are pure, so a race only
/// computes the same value twice.
fn shared<V: Copy>(map: &'static Shared<V>, key: Key, make: impl FnOnce() -> V) -> V {
    let map = map.get_or_init(|| RwLock::new(FastMap::default()));
    if let Some(v) = map.read().unwrap_or_else(|e| e.into_inner()).get(&key).copied() {
        return v;
    }
    let v = make();
    let mut m = map.write().unwrap_or_else(|e| e.into_inner());
    if m.len() >= 4 * CACHE_CAP {
        m.clear();
    }
    m.insert(key, v);
    v
}

/// Pre-carve ground top at one metre, so every voxel size sees the same plan.
fn ground(cfg: &WorldSampleConfig, p: Vec2) -> f32 {
    let base = WorldSampleConfig { voxel_scale: VoxelScalePreset::OneMeter, ..*cfg };
    landscape::pre_drainage_preview_top(&base, p.x, p.y)
}

/// Node head from the lowest pre-carve ground on and around its centre.
pub(super) fn head(cfg: &WorldSampleConfig, (x, z): (i32, i32), center: Vec2) -> Node {
    shared(&SHARED_HEADS, (cfg.seed, cfg.world_type as u8, cfg.generator_version, x, z), || head_uncached(cfg, center))
}

fn head_uncached(cfg: &WorldSampleConfig, center: Vec2) -> Node {
    let ring = (0..8).map(|k| {
        let a = k as f32 * std::f32::consts::FRAC_PI_4;
        ground(cfg, center + Vec2::new(a.cos(), a.sin()) * NODE_RING_M)
    });
    let head_m = ring.fold(ground(cfg, center), f32::min) - FREEBOARD_M;
    let base = WorldSampleConfig { voxel_scale: VoxelScalePreset::OneMeter, ..*cfg };
    let (_, rainfall, _) = landscape::visual_ecology(&base, center.x, center.y, head_m);
    Node { center_m: center, head_m, rainfall }
}

/// Ground minus freeboard at evenly spaced points on the reach's centreline.
fn stations(cfg: &WorldSampleConfig, from: (i32, i32), to: (i32, i32)) -> [f32; STATIONS] {
    let key = (cfg.seed, cfg.world_type as u8, cfg.generator_version, from.0, from.1);
    cached(&REACHES, key, || shared(&SHARED_REACHES, key, || {
        let (source, target) = (node(cfg, from.0, from.1), node(cfg, to.0, to.1));
        let delta = target.center_m - source.center_m;
        let length = delta.length().max(1.0);
        let across = Vec2::new(-delta.y, delta.x) / length;
        let weights = harmonics(cfg, from);
        std::array::from_fn(|k| {
            let t = k as f32 / (STATIONS - 1) as f32;
            let p = source.center_m + delta * t + across * bend_offset(weights, length, t);
            // Both bank crests too: a reach along a side slope has one low bank.
            let side = (2.5 + source.rainfall * 3.5) * (0.85 + 0.3 * t) + CREST_WIDTH_M;
            let low = ground(cfg, p).min(ground(cfg, p + across * side)).min(ground(cfg, p - across * side));
            low - FREEBOARD_M
        })
    }))
}

/// Node water level: its head, lowered to the lowest ground on any reach that
/// drains into it and to its own outlet's first station, so inflowing and
/// outflowing surfaces start from one level. Only immediate inflows count, so
/// the query stays bounded.
fn level(cfg: &WorldSampleConfig, x: i32, z: i32) -> f32 {
    cached(&LEVELS, (cfg.seed, cfg.world_type as u8, cfg.generator_version, x, z), || {
        let mut l = node(cfg, x, z).head_m;
        if let Outlet::Downhill(c, d) = outlet(cfg, x, z) {
            l = l.min(stations(cfg, (x, z), (c, d))[0]);
        }
        for dz in -1..=1 {
            for dx in -1..=1 {
                let from = (x + dx, z + dz);
                if (dx, dz) != (0, 0) && outlet(cfg, from.0, from.1) == Outlet::Downhill(x, z) {
                    l = stations(cfg, from, (x, z)).into_iter().fold(l, f32::min);
                }
            }
        }
        l
    })
}

/// Water surface at reach parameter `t`: the line between node levels, kept
/// under the lowest station passed so far.
fn reach_surface(cfg: &WorldSampleConfig, from: (i32, i32), to: (i32, i32), t: f32) -> f32 {
    let (start, end) = (level(cfg, from.0, from.1), level(cfg, to.0, to.1));
    let s = stations(cfg, from, to);
    let f = t * (STATIONS - 1) as f32;
    let k = (f.floor() as usize).min(STATIONS - 2);
    let low = |i: usize| s[..=i].iter().copied().fold(start, f32::min);
    let envelope = low(k) + (low(k + 1) - low(k)) * (f - k as f32);
    (start + (end - start) * t).min(envelope)
}

/// Cross-section in ground-top metres. Inside the water edge the bed is a
/// parabola below the surface; outside it the bank rises and steepens until it
/// meets the terrain. The nearest wet channel also sets a floor: its banks
/// reach just above the water before falling back to the terrain, so no water
/// column stands beside lower dry ground. Ceilings of every channel combine by
/// minimum, so confluences merge instead of leaving ridges.
pub(super) fn sample(
    cfg: &WorldSampleConfig, p: Vec2, (ix, iz): (i32, i32), window: std::ops::RangeInclusive<i32>,
) -> Channel {
    let mut bank = 0.0f32;
    let mut ceiling = f32::MAX;
    // (distance past the water edge, surface, bank floor)
    let mut nearest: Option<(f32, f32, f32)> = None;
    for dz in window.clone() {
        for dx in window.clone() {
            let (a, b) = (ix + dx, iz + dz);
            let source = node(cfg, a, b);
            let (distance, surface, wet, width) = match outlet(cfg, a, b) {
                Outlet::Ocean => continue,
                Outlet::ClosedBasin => (
                    (p - source.center_m).length(),
                    level(cfg, a, b),
                    source.rainfall > 0.35,
                    9.0 + source.rainfall * 9.0,
                ),
                Outlet::Downhill(c, d) => {
                    let target = node(cfg, c, d);
                    let (distance, t, width) = super::meander(cfg, (a, b), source, target, p);
                    if distance - width > FIT_REACH_M {
                        continue;
                    }
                    (distance, reach_surface(cfg, (a, b), (c, d), t), source.rainfall > 0.3, width)
                }
            };
            let edge = distance - width;
            if edge > FIT_REACH_M {
                continue;
            }
            bank = bank.max(1.0 - crate::smoothstep(width, width + 8.0, distance));
            let depth = (0.9 + 0.18 * width).clamp(1.6, 3.2);
            let profile = if edge <= 0.0 {
                let r = distance / width;
                surface - LIP_M - (depth - LIP_M) * (1.0 - r * r)
            } else {
                surface - LIP_M + 0.7 * edge + 0.5 * (edge - 6.0).max(0.0).powi(2)
            };
            ceiling = ceiling.min(profile);
            if wet && nearest.is_none_or(|(e, _, _)| edge < e) {
                let floor = profile.min(surface + CREST_M - 0.8 * (edge - CREST_WIDTH_M).max(0.0));
                nearest = Some((edge, surface, floor));
            }
        }
    }
    Channel {
        bank,
        bed_m: ceiling,
        water_m: nearest.filter(|&(e, _, _)| e <= CREST_WIDTH_M).map(|(_, s, _)| s),
        levee_m: nearest.map(|(_, _, floor)| floor.min(ceiling)),
    }
}

#[cfg(test)]
mod tests;
