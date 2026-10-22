//! Generator 27's connected teaching site. Historical worlds retain their terrain.
use crate::{BlockId as B, WorldSampleConfig, WorldType};
use glam::Vec3;

pub const LANTERN: Vec3 = Vec3::new(294.5, 0.0, 38.5);
pub const SOURCES: [Vec3; 3] = [
    Vec3::new(296.5, 0.0, 48.5),
    Vec3::new(342.5, 0.0, 48.5),
    Vec3::new(372.5, 0.0, 48.5),
];

pub fn sample(cfg: &WorldSampleConfig, m: Vec3) -> Option<B> {
    if cfg.generator_version < 27
        || cfg.world_type == WorldType::Superflat
        || !(274.0..390.0).contains(&m.x)
        || !(27.0..178.0).contains(&m.z)
    {
        return None;
    }
    // Broad stair to daylight. The one-metre steps remain the same at fine voxel scales.
    if (289.0..296.0).contains(&m.x) && (62.0..178.0).contains(&m.z) {
        let floor = ((m.z - 62.0) * 0.5).floor();
        if m.y >= floor - 1.0 && m.y < floor {
            return Some(B::Limestone);
        }
        if m.y >= floor && m.y < floor + 5.0 {
            return Some(B::Air);
        }
    }
    let hollow = ((m.x - 294.0) / 18.0).powi(2) + ((m.z - 48.0) / 18.0).powi(2) < 1.0;
    let cistern = (310.0..333.0).contains(&m.x) && (34.0..63.0).contains(&m.z);
    let silk = ((m.x - 345.0) / 17.0).powi(2) + ((m.z - 48.0) / 13.0).powi(2) < 1.0;
    let nursery = ((m.x - 373.0) / 13.0).powi(2) + ((m.z - 48.0) / 12.0).powi(2) < 1.0;
    let passage = (302.0..375.0).contains(&m.x) && (46.0..51.0).contains(&m.z);
    if !(hollow || cistern || silk || nursery || passage) {
        return None;
    }
    if !(-2.0..48.0).contains(&m.y) {
        return None;
    }
    if m.y < -1.0 {
        return Some(B::Limestone);
    }
    let pool = ((m.x - 298.5).abs() < 1.0 && (m.z - 48.5).abs() < 1.0)
        || ((m.x - 344.5).abs() < 1.0 && (m.z - 48.5).abs() < 1.0)
        || ((m.x - 374.5).abs() < 1.0 && (m.z - 48.5).abs() < 1.0)
        || (cistern && (m.z - 39.0).abs() < 2.0)
        || (cfg.generator_version >= 29
            && ((m.x - 298.5).abs() < 0.5 && (43.0..54.0).contains(&m.z)
                || (m.x - 374.5).abs() < 0.5 && (44.0..53.0).contains(&m.z)));
    if m.y < 0.0 {
        return Some(if pool {
            B::Water
        } else if hollow || nursery {
            B::FungalSoil
        } else {
            B::Limestone
        });
    }
    // A dry, low anchor above the spider source; walkable hunting space below.
    if (m.x - 342.5).abs() < 1.0 && (m.z - 48.5).abs() < 1.0 && (2.0..3.0).contains(&m.y) {
        return Some(B::MushroomStem);
    }
    if cfg.generator_version >= 29
        && let Some(block) = garden_details(m, hollow, nursery) {
            return Some(block);
        }
    if hollow && (m.x - 294.0).abs() < 5.0 && (m.z - 48.0).abs() < 5.0 {
        return Some(B::Air); // Open sinkhole, including a visible distant landmark.
    }
    if m.y
        < if hollow {
            12.0
        } else if nursery {
            7.0
        } else {
            8.0
        }
    {
        // Sparse roots and cistern columns leave the connecting route open.
        if (m.z - 48.0).abs() > 4.0 && cistern && (m.x - 315.0).rem_euclid(8.0) < 1.0 {
            return Some(B::StoneBricks);
        }
        if (m.x - LANTERN.x).abs() < 0.5 && (m.z - LANTERN.z).abs() < 0.5 && m.y < 1.0 {
            return Some(B::StoneBricks);
        }
        return Some(B::Air);
    }
    if m.y < 14.0 { Some(B::Limestone) } else { None }
}

/// Authored growth, cultivation remnants and ordinary torch light. Gate 29
/// preserves already-created 27/28 worlds exactly.
fn garden_details(m: Vec3, hollow: bool, nursery: bool) -> Option<B> {
    for (x, z) in [
        (284.5, 48.5),
        (305.5, 54.5),
        (316.5, 55.5),
        (331.5, 54.5),
        (347.5, 55.5),
        (360.5, 53.5),
        (378.5, 52.5),
    ] {
        if (m.x - x).abs() < 0.5 && (m.z - z).abs() < 0.5 {
            if (0.0..1.0).contains(&m.y) {
                return Some(B::StoneBricks);
            }
            if (1.0..2.0).contains(&m.y) {
                return Some(B::Torch);
            }
        }
    }
    for (x, z, h) in [
        (284.5, 38.5, 7.0),
        (304.5, 37.5, 9.0),
        (366.5, 40.5, 5.0),
        (380.5, 54.5, 4.0),
    ] {
        let dx = (m.x - x).abs();
        let dz = (m.z - z).abs();
        if dx < 0.6 && dz < 0.6 && m.y < h {
            return Some(B::MushroomStem);
        }
        if dx < 2.5 && dz < 2.0 && (h - 1.0..h + 1.0).contains(&m.y) {
            return Some(if dx + dz < 2.0 { B::GlowCap } else { B::Leaves });
        }
        if dx < 2.0 && dz < 1.5 && m.y < 0.6 {
            return Some(B::Bush);
        }
    }
    if hollow && (m.x - 294.0).abs() > 14.0 && (m.z - 48.0).abs() < 9.0 && m.y < 1.0 {
        return Some(B::FungalSoil);
    }
    if nursery && (m.x - 373.0).abs() > 9.0 && (m.z - 48.0).abs() < 7.0 && m.y < 1.0 {
        return Some(B::FungalSoil);
    }
    None
}
