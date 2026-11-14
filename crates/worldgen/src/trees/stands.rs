//! Stand history changes age composition, not the number of occupied tree cells.
//! Metre-space fields are independent of chunks, terrain LOD and voxel scale.
use super::{TreeProfile, WorldSampleConfig, hash2};
use crate::{smoothstep, value_noise2};

pub const VERSION: u32 = 140;

pub(super) fn age(cfg: &WorldSampleConfig, tree: &TreeProfile) -> f32 {
    let individual = hash2(tree.cell_x, tree.cell_z, cfg.seed.wrapping_add(12011));
    if cfg.generator_version < VERSION { return individual; }
    if crate::planetary::active(cfg) && crate::planetary::landscape_cycles(cfg.generator_version) {
        let at = glam::DVec3::new(f64::from(cfg.world_x(tree.anchor_x)), 0.0,
            f64::from(cfg.world_z(tree.anchor_z)));
        let dir = engine_core::cube_sphere::atlas::to_body(at).normalize();
        // The same disturbance field that opens the canopy also produces a
        // young cohort. No terrain resampling on the tree/LOD hot path.
        let stand = crate::planetary::ecoregion::stand(cfg.seed, dir, 16.0, 0.7, 0.0, 0.0);
        let recruit = 0.08 + 0.30 * individual;
        let mature = (stand.maturity + (individual - 0.5) * 0.24).clamp(0.0, 1.0);
        return if individual < 0.22 { recruit }
            else { mature + (recruit - mature) * stand.regeneration };
    }
    composition(cfg.seed, cfg.world_x(tree.anchor_x), cfg.world_z(tree.anchor_z), individual)
}

fn composition(seed: u32, x: f32, z: f32, individual: f32) -> f32 {
    let history = value_noise2(x / 160.0, z / 160.0, seed.wrapping_add(140_011));
    // This is the same gap field that opens the regional stand canopy at 136.
    let gap = smoothstep(0.74, 0.80,
        value_noise2(x / 41.0, z / 41.0, seed.wrapping_add(136_037)));
    let mature = 0.48 + 0.50 * history;
    // Recruits survive below older crowns; disturbance shifts a whole patch
    // toward regeneration rather than scattering unrelated tree sizes.
    let recruit = 0.10 + 0.30 * individual;
    let cohort = if individual < 0.25 { recruit } else {
        (mature + (individual - 0.5) * 0.22).clamp(0.0, 1.0)
    };
    cohort + (recruit - cohort) * gap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_continuous_across_chunk_and_negative_coordinate_boundaries() {
        for boundary in [-32.0, 0.0, 32.0] {
            let a = composition(74, boundary - 0.001, -16.0, 0.7);
            let b = composition(74, boundary + 0.001, -16.0, 0.7);
            assert!((a - b).abs() < 0.001);
            assert_eq!(a, composition(74, boundary - 0.001, -16.0, 0.7));
        }
    }

    #[test]
    fn stands_retain_recruits_and_multiple_canopy_ages() {
        let mut min = 1.0f32;
        let mut max = 0.0f32;
        for x in -20..20 {
            for z in -20..20 {
                let mature = composition(74, x as f32 * 20.0, z as f32 * 20.0, 0.8);
                let recruit = composition(74, x as f32 * 20.0, z as f32 * 20.0, 0.1);
                assert!(mature >= recruit && mature <= 1.0 && recruit >= 0.0);
                min = min.min(mature);
                max = max.max(mature);
            }
        }
        assert!(max - min > 0.3);
    }
}
