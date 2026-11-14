//! Connected excavation spaces. Stay inside the mine/site ownership envelope.
use super::*;

pub(super) fn mine(q: Vec3, v: f32, version: u32) -> Option<B> {
    // Crosscut off the existing decline, then two galleries reconnect to its
    // fungal chamber. Both branches share the decline's elevation at 26 m.
    let depth = -q.z;
    // The original decline owns its complete centre strip and support frames.
    if q.x.abs() < 2.6 { return None; }
    let cross = (depth - 26.0).abs() < 1.8 && q.x.abs() < 13.5;
    let branch = (q.x.abs() - 12.0).abs() < 1.5 && (26.0..=41.0).contains(&depth);
    // Working faces widen away from the haulage lane, giving the galleries
    // ore chambers rather than ending in indistinguishable narrow tubes.
    let working = version >= 44 && (31.0..37.0).contains(&depth)
        && (10.5..16.5).contains(&q.x.abs());
    if !cross && !branch && !working { return None; }
    let run = (depth - 26.0).max(0.0);
    let floor = ((-11.7 - run.min(15.0) * (7.3 / 15.0)) / v).floor() * v;
    let y = q.y - floor;
    let roof = if version >= 54 && working {
        6.2 + 1.6 * (1.0 - ((q.x.abs() - 13.5) / 3.0).abs()).max(0.0)
    } else { 4.5 };
    if y < -v || y >= roof + v { return None; }
    if y < v {
        let haul = version >= 54 && branch && (q.x.abs() - 12.0).abs() < v.max(0.35);
        return Some(if haul { B::Basalt } else { B::Cobblestone });
    }
    if y >= roof { return Some(B::Limestone); }
    if version >= 54 && working {
        let frame = (depth - 31.0).rem_euclid(4.0) < v;
        if frame && (q.x.abs() > 14.8 || y >= roof - v) { return Some(B::Log); }
        if (34.0..34.0 + v).contains(&depth) && q.x.abs() >= 14.5 {
            if (2.5..3.0).contains(&y) { return Some(B::Log); }
            if q.x.abs() < 14.5 + v && (3.0..3.0 + v).contains(&y) { return Some(B::Torch); }
        }
    }
    if working && q.x.abs() >= 15.5 {
        return Some(if y < 2.5 {
            if q.x < 0.0 { B::CopperOre } else { B::TinOre }
        } else { B::Limestone });
    }
    // Timber frames never cross the central walking strip.
    let edge = if cross { (depth - 26.0).abs() > 1.8 - v } else { (q.x.abs() - 12.0).abs() > 1.5 - v };
    let bay = if cross { q.x.abs() } else { depth - 26.0 };
    if version >= 44 && !working && edge && bay.rem_euclid(5.0) >= v
        && bay.rem_euclid(5.0) < 2.0 * v && (2.5..2.5 + v).contains(&y) {
        return Some(B::Torch);
    }
    if !(version >= 54 && working) && bay.rem_euclid(5.0) < v
        && (edge || y >= 3.5) { return Some(B::Log); }
    // Ore at the outside of the crosscut gives each branch a readable purpose.
    if cross && q.x.abs() > 12.5 && y < 2.5 {
        return Some(if q.x < 0.0 { B::CopperOre } else { B::TinOre });
    }
    Some(B::Air)
}

pub(super) fn pit(s: Site, cfg: &WorldSampleConfig, q: Vec3) -> Option<B> {
    if s.starter || s.kind == 0 || s.kind == 1 { return None; }
    let levels = ((s.ground - crate::WORLD_MIN_Y_METERS - 3.0) / 22.0).floor().clamp(0.0, 3.0) as i32;
    if levels == 0 { return None; }
    let v = cfg.voxel_meters();
    let (x, z) = (q.x, q.z + 6.0);
    let below = -q.y;
    let bottom = levels as f32 * 22.0;
    let expansion = if cfg.generator_version >= 54 { 3.0 } else { 0.0 };
    if x.abs() > 7.8 + expansion || z.abs() > 6.8 + expansion
        || below < -v || below > bottom + v { return None; }
    let radius = x.hypot(z);
    if radius < 2.6 {
        if below >= bottom { return Some(B::StoneBricks); }
        if radius < 0.65 { return Some(B::StoneBricks); }
        // Four complete turns per landing: all side rooms meet the same east
        // landing. Annular treads have a central support and full headroom.
        let angle = z.atan2(x).rem_euclid(std::f32::consts::TAU);
        let riser = v.max(0.5);
        let phase = (angle / std::f32::consts::TAU * 5.5 / riser).floor() * riser;
        if below >= 0.0 && (below - phase).rem_euclid(5.5) < v {
            return Some(B::StoneBricks);
        }
        return Some(B::Air);
    }
    for level in 1..=levels {
        let y = q.y + level as f32 * 22.0;
        let roof = if cfg.generator_version >= 54 { 7.2 } else { 5.2 };
        if y < -v || y >= roof + v { continue; }
        if cfg.generator_version >= 44 && y < 0.0
            && (x.abs() > 6.0 || z.abs() > 5.0) {
            return Some(B::Cobblestone);
        }
        if !(0.0..roof).contains(&y) || x.abs() > 7.2 + expansion || z.abs() > 6.2 + expansion {
            return Some(B::WeatheredBricks);
        }
        // Rooms open onto the east side of each full-turn landing only.
        if radius < 3.3 && !(x > 0.0 && z.abs() < 1.5 && y < 3.5) {
            if cfg.generator_version >= 54 && x < 0.0 && z.abs() < 1.0
                && (1.5..4.5).contains(&y) { return Some(B::Air); }
            return Some(B::StoneBricks);
        }
        if cfg.generator_version >= 54 {
            // Arched side bays and ceiling ribs expose the excavation's depth.
            let rib = (x.abs() - 4.5).abs() < v.max(0.6);
            let spring = 4.2 + (8.8 - z.abs()).max(0.0) * 0.32;
            if rib && y >= spring { return Some(B::Limestone); }
            if z.abs() > 8.2 && rib && y < 4.2 { return Some(B::StoneBricks); }
        }
        if x > 5.5 && z.abs() < 1.0 && y < v {
            return Some(match level { 1 => B::CultivatorCache, 2 => B::SurveyCache, _ => B::SalvageCrate });
        }
        if cfg.generator_version >= 44 {
            // Keep the east landing-to-cache aisle open. Different side bays
            // tell a storage / survey / excavation story down the shaft.
            let bay = (x.abs() - 4.5).abs() < 1.0 && z.abs() > 4.5;
            if bay && y < v.max(0.8) {
                return Some(match level {
                    1 => B::Planks,
                    2 => B::StoneBricks,
                    _ => B::CopperOre,
                });
            }
            if z.abs() > 5.2 + expansion && x.abs() > 3.5 && x.abs() < 4.5
                && (2.5..2.5 + v).contains(&y) {
                return Some(B::Torch);
            }
        }
        if cfg.generator_version >= 55
            && (x.abs() - 4.5).abs() < v * 0.5
            && (z.abs() - 5.5).abs() < v * 0.5 && y < 2.5 + v {
            return Some(if y < 2.5 { B::StoneBricks } else { B::Torch });
        }
        if cfg.generator_version >= 55 && y < v {
            // Flooded side bays, with a dry cross and perimeter circulation.
            // Breakable stone contains actual water cells; no scripted flood.
            let dry = x.abs() < 1.5 || z.abs() < 1.5 || x.abs() > 8.5 || z.abs() > 7.5;
            return Some(if dry { B::StoneBricks } else { B::Water });
        }
        if z > 5.2 && x.abs() < 1.0 && y > 2.0 && y < 2.0 + v { return Some(B::GlowCap); }
        return Some(B::Air);
    }
    if radius < 3.3 { return Some(B::StoneBricks); }
    None
}
