//! Initial ecological conditions in metres. Placement uses independent niche
//! constraints and bounded local dispersal, not a universal winner score.
use super::{WorldSampleConfig, hash_u32, landscape};
use engine_content::environment::{Species, definition};
use glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Conditions {
    pub moisture_per_mille: u32,
    pub temperature_c: i32,
    pub shade_per_mille: u32,
}

/// Runtime moisture belongs to soil state. Querying temperature must not
/// recompute a regional drainage plan whose moisture result is discarded.
pub fn temperature_c(cfg: &WorldSampleConfig, p: Vec3) -> i32 {
    let (heat, _, _) = landscape::visual_ecology(cfg, p.x, p.z, p.y);
    (heat * 50.0 - 15.0).round() as i32
}

pub fn conditions(cfg: &WorldSampleConfig, p: Vec3, shade: u32) -> Conditions {
    let (heat, wet, _) = landscape::visual_ecology(cfg, p.x, p.z, p.y);
    Conditions {
        moisture_per_mille: ((wet + super::drainage::sample(cfg, p.x, p.z).bank * 0.24).min(1.0) * 1_000.0)
            .round() as u32,
        temperature_c: (heat * 50.0 - 15.0).round() as i32,
        shade_per_mille: shade,
    }
}

pub fn can_establish(species: &Species, c: Conditions) -> bool {
    (species.moisture_min..=species.moisture_max).contains(&c.moisture_per_mille)
        && (species.temperature_min_c..=species.temperature_max_c).contains(&c.temperature_c)
        && c.shade_per_mille <= species.shade_tolerance
}

pub fn colonist(cfg: &WorldSampleConfig, p: Vec3, c: Conditions) -> u32 {
    let Some(definition) = definition() else {
        return 0;
    };
    let local = (p / 2.0).floor().as_ivec3();
    let source = (p / 48.0).floor().as_ivec3();
    definition
        .species
        .iter()
        .filter(|s| can_establish(s, c))
        .filter(|s| {
            !hash_u32(cfg.seed ^ s.id.wrapping_mul(719) ^ source.x as u32 ^ (source.z as u32).rotate_left(16))
                .is_multiple_of(4)
        })
        .min_by_key(|s| {
            hash_u32(
                cfg.seed
                    ^ s.id.wrapping_mul(179)
                    ^ (local.x as u32).wrapping_mul(73856093)
                    ^ (local.y as u32).rotate_left(7)
                    ^ (local.z as u32).wrapping_mul(19349663),
            )
        })
        .map_or(0, |s| s.id)
}
