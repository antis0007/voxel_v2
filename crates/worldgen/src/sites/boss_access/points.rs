//! Treasure caches of every boss complex near a point, for loot placement.
use super::{Shrine, WorldSampleConfig, cavern_shrine, site_shrine};
use glam::{IVec3, Vec3};

/// One treasure cache of a boss complex.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TreasurePoint {
    /// Encounter id of the complex (`z == 0` ruin site, `z == 1` cavern).
    pub site: IVec3,
    /// Stable per complex: 0 hidden room, 1 lever room, 2 boss vault.
    pub index: u8,
    /// 1-3 (3 = boss vault).
    pub tier: u8,
    /// Cache cell centre, world metres.
    pub pos: Vec3,
}

/// Every treasure cache of the boss complexes whose cache lies within
/// `radius` of `center`. Pure; `(site, index)` is a stable key, so the app can
/// place loot exactly once per cache.
pub fn treasure_points(cfg: &WorldSampleConfig, center: Vec3, radius: f32) -> Vec<TreasurePoint> {
    let mut out = Vec::new();
    let mut push = |site: IVec3, s: Shrine| {
        for t in s.treasures() {
            let c = t.cache;
            let p = s.world_of(Vec3::new(c[0] as f32 + 0.5, c[2] as f32 + 0.5, c[4] as f32 + 0.5));
            if (p - center).length() <= radius {
                out.push(TreasurePoint { site, index: t.index, tier: t.tier, pos: p });
            }
        }
    };
    let span = super::super::region_span(cfg);
    let (rx, rz) = ((center.x / span).round() as i32, (center.z / span).round() as i32);
    let reach = (radius / span).ceil() as i32 + 1;
    for x in rx - reach..=rx + reach {
        for z in rz - reach..=rz + reach {
            if let Some(s) = site_shrine(cfg, x, z) {
                push(IVec3::new(x, z, 0), s);
            }
        }
    }
    let (gx, gz) = ((center.x / 72.0).round() as i32, (center.z / 72.0).round() as i32);
    let reach = (radius / 72.0).ceil() as i32 + 1;
    for x in gx - reach..=gx + reach {
        for z in gz - reach..=gz + reach {
            if let Some(s) = cavern_shrine(cfg, x, z) {
                push(IVec3::new(x, z, 1), s);
            }
        }
    }
    out
}

