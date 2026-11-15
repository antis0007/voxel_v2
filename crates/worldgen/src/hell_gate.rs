//! Rare buried thresholds, shared by collision, meshing and realm travel.
use crate::{BlockId as B, WorldSampleConfig, WorldType};
use glam::{IVec3, Vec3};
pub mod approach;
pub mod sanctuary;
#[cfg(test)]
mod tests;
pub const VERSION: u32 = 83;
pub use approach::VERSION as APPROACH_VERSION;
pub const INSCRIPTION: &str = "Abandon all hope, ye who enter here.";
const SPACING: f32 = 1024.0;
pub fn enabled(cfg: &WorldSampleConfig) -> bool {
    // New Hell planets are reached through player-built surface thresholds.
    // Historical sanctuary links have a different saved destination and must
    // not appear as unusable Return gates in this world.
    if cfg.is_hell() { return false; }
    cfg.generator_version >= VERSION && crate::features::enabled(cfg, crate::features::Feature::HellGate)
}
/// One sanctuary per square kilometre, with repeatable seed offsets.
pub fn center(cfg: &WorldSampleConfig, p: Vec3) -> Vec3 {
    let spacing = if cfg.generator_version >= 90 { 640.0 } else { SPACING };
    let tx = ((p.x - 240.0) / spacing).round() as i32;
    let tz = ((p.z + 64.0) / spacing).round() as i32;
    let key = cfg.seed ^ (tx as u32).wrapping_mul(0x9e3779b9) ^ (tz as u32).wrapping_mul(0x85ebca6b);
    Vec3::new(240.0 + tx as f32 * spacing + (crate::hash_u32(key) % 65) as f32 - 32.0,
        0.0, -64.0 + tz as f32 * spacing + (crate::hash_u32(key ^ 71) % 65) as f32 - 32.0)
}
pub fn floor(cfg: &WorldSampleConfig, c: Vec3) -> f32 {
    type Key = (u32, u32, u32, i32, i32);
    thread_local! { static FLOORS: std::cell::RefCell<Vec<(Key, f32)>> = const { std::cell::RefCell::new(Vec::new()) }; }
    let key = (cfg.seed, cfg.generator_version, cfg.voxel_meters().to_bits(), c.x as i32, c.z as i32);
    if let Some(y) = FLOORS.with(|cache| cache.borrow().iter().find(|(k, _)| *k == key).map(|(_, y)| *y)) { return y; }
    let parent = WorldSampleConfig { world_type: WorldType::Hills, ..*cfg };
    let nether = WorldSampleConfig { world_type: WorldType::Nether, ..*cfg };
    let cell = (c / cfg.voxel_meters()).floor().as_ivec3();
    let far_surface = crate::nether::column(&nether, cell.x, cell.z).height as f32 * cfg.voxel_meters();
    // The paired chamber must be below BOTH surfaces, never a floating arrival.
    let y = (crate::geo::site_floor(&parent, c.x, c.z) - 32.0)
        .min(far_surface - 12.0).max(crate::WORLD_MIN_Y_METERS + 9.0).floor();
    FLOORS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= 32 { cache.clear(); }
        cache.push((key, y));
    });
    y
}
pub fn focus(cfg: &WorldSampleConfig, feet: Vec3) -> bool {
    if !enabled(cfg) { return false; }
    let c = center(cfg, feet);
    let p = feet - c - Vec3::Y * floor(cfg, c);
    p.x.abs() < 2.4 && p.z.abs() < 4.0 && (0.0..3.0).contains(&p.y)
}
/// The porter's record stands beside the arch, clear of the approach.
pub fn record_near(cfg: &WorldSampleConfig, p: Vec3) -> Option<Vec3> {
    if cfg.generator_version < crate::dungeon::v2::sprawl::VERSION || !enabled(cfg) { return None; }
    let c = center(cfg, p);
    if (p.x - c.x).abs() > 16.0 || (p.z - c.z).abs() > 16.0 { return None; }
    let q = c + Vec3::new(-5.5, floor(cfg, c), 4.5);
    (p.distance_squared(q) < 64.0).then_some(q)
}
pub fn block(cfg: &WorldSampleConfig, p: IVec3, column: crate::ColumnSample) -> Option<B> {
    if !enabled(cfg) { return None; }
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    let c = center(cfg, m);
    let d = m - c;
    let radius = d.x.hypot(d.z);
    if radius > 15.0 {
        if let Some(arch) = crate::hall::departure_block(cfg, c, p, column) { return Some(arch); }
        return if approach::enabled(cfg) { approach::outer(cfg, c, p, d, column) } else { None };
    }
    let ruins = approach::enabled(cfg);
    let base = floor(cfg, c);
    let y = m.y - base;
    let surface = (column.height as f32 * cfg.voxel_meters() + 1.0)
        .max(crate::WATER_LEVEL_METERS + 1.0);
    if y < -1.0 { return None; }
    if y >= 7.0 && m.y > surface + 2.0 {
        return if ruins && radius >= 13.0 { sanctuary::tower(cfg, d, m, surface) } else { None };
    }
    // Broken watchtower over a continuous, dry stair shaft.
    if radius >= 13.0 {
        if ruins && m.y > surface - 1.0 { return sanctuary::tower(cfg, d, m, surface); }
        if m.y > surface + 2.0 { return None; }
        if m.y > surface - 1.0 && (d.x.abs() < 3.0 || d.z.abs() < 3.0) { return Some(B::Air); }
        return Some(if m.y > surface - 3.0 { B::WeatheredBricks } else { B::Basalt });
    }
    if y < 0.0 {
        if ruins && let Some(b) = sanctuary::floor_inlay(d, y, cfg.voxel_meters()) { return Some(b); }
        return Some(B::StoneBricks);
    }
    if radius >= 9.0 {
        // An entrance gallery meets the surrounding ground. Walk along it to
        // the matching stair turn instead of dropping into the shaft.
        if radius >= 11.5 && m.y >= surface - 1.0 && m.y < surface { return Some(B::WeatheredBricks); }
        if radius >= 11.5 && m.y >= surface && m.y < surface + 3.0 { return Some(B::Air); }
        // Broad helix: eight metres per turn, half-metre treads, seven metres headroom.
        let phase = d.x.atan2(-d.z).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
        let tread = (phase * 16.0).floor() * 0.5;
        let above = (y - tread).rem_euclid(8.0);
        if above >= 7.0 { return Some(B::StoneBricks); }
        if radius > 12.0 && above < 1.0 && (phase * 8.0).fract() < 0.1 { return Some(B::GlowCap); }
        return Some(B::Air);
    }
    if y >= 7.0 { return Some(if m.y < surface { B::Basalt } else { B::Air }); }
    if radius >= 8.0 && !(d.x.abs() < 2.5 && d.z < 0.0 && y < 3.0) { return Some(B::Basalt); }
    if ruins && let Some(b) = sanctuary::sanctuary(c, base, p, d, cfg.voxel_meters()) { return Some(b); }
    let (x, z) = (d.x.abs(), d.z.abs());
    if cfg.generator_version >= crate::dungeon::v2::sprawl::VERSION
        && (d.x + 5.5).abs() <= 0.6 && (d.z - 4.5).abs() <= 0.6 && y < 1.8 {
        return Some(if y < 0.5 { B::Basalt } else { B::Bookcase });
    }
    if ((2.5..3.5).contains(&x) && z < 0.75 && y < 6.5)
        || (x < 3.5 && z < 0.75 && (5.0..6.5).contains(&y)) { return Some(B::Basalt); }
    if (4.5..5.5).contains(&x) && (3.5..4.5).contains(&z) {
        return Some(if y < 1.0 { B::Basalt } else if y < 2.0 { B::GlowCap } else { B::Air });
    }
    if (6.0..7.0).contains(&x) && z < 1.0 && y < 4.0 { return Some(B::WeatheredBricks); }
    if d.x >= 5.0 && d.x < 6.0 && d.z >= 4.0 && d.z < 5.0 && y < 1.0 { return Some(B::SalvageCrate); }
    if cfg.world_type == WorldType::Nether && z > 6.0 && x < 3.0 && y < 2.0 { return Some(B::Crystal); }
    Some(B::Air)
}
