//! The unbreakable floor under the world (generator 31 and later).
use super::{WorldSampleConfig, fbm2, hash3};

/// First generator with a bedrock floor. Older versions keep their flat basalt.
pub const VERSION: u32 = 31;

/// Highest bedrock cell in a column: 2 to 6 m thick, rolling over about 12 m
/// with a one-voxel hash fringe, so caves meet a rough floor and not a plane.
pub fn top(cfg: &WorldSampleConfig, x: i32, z: i32) -> i32 {
    let swell = fbm2(cfg.world_x(x) * 0.08, cfg.world_z(z) * 0.08, cfg.seed.wrapping_add(911));
    let fringe = i32::from(hash3(x, 0, z, cfg.seed.wrapping_add(913)) > 0.6);
    cfg.min_world_y() + cfg.blocks_for_span(2.0 + swell * 4.0) - 1 + fringe
}

/// Highest cell `top` can ever return. `fbm2` sums four octaves (0.5 + 0.25 +
/// 0.125 + 0.0625) of a [0, 1] noise, so `swell` stays under 0.94 and a column
/// is at most 5.75 m thick. Callers test `y <= ceiling` first so the noise is
/// evaluated only for the bottom few metres, not for every sampled cell.
pub fn ceiling(cfg: &WorldSampleConfig) -> i32 {
    cfg.min_world_y() + cfg.blocks_for_span(6.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceiling_bounds_every_column() {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
            let c = cfg(VERSION, scale);
            for x in (-4000..4000).step_by(37) {
                for z in (-4000..4000).step_by(41) {
                    assert!(top(&c, x, z) <= ceiling(&c), "{scale:?} {x},{z}");
                }
            }
        }
    }
    use crate::{BlockId, WorldType, sample_block};
    use engine_core::VoxelScalePreset;
    use glam::IVec3;

    fn cfg(version: u32, scale: VoxelScalePreset) -> WorldSampleConfig {
        WorldSampleConfig { generator_version: version, seed: 424242, world_type: WorldType::Hills, voxel_scale: scale }
    }

    #[test]
    fn floor_is_solid_bedrock_and_uneven_with_no_water_inside() {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter] {
            let c = cfg(VERSION, scale);
            let mut tops = std::collections::BTreeSet::new();
            for x in (-300..300).step_by(11) {
                for z in (-300..300).step_by(13) {
                    let top = top(&c, x, z);
                    tops.insert(top);
                    assert!(top > c.min_world_y(), "at least two cells at {x},{z}");
                    for y in c.min_world_y()..=top {
                        assert_eq!(sample_block(&c, IVec3::new(x, y, z)), BlockId::Bedrock, "{x},{y},{z}");
                    }
                    assert_ne!(sample_block(&c, IVec3::new(x, top + 1, z)), BlockId::Bedrock);
                }
            }
            assert!(tops.len() >= 4, "floor should roll, saw {tops:?}");
        }
    }

    #[test]
    fn older_generators_have_no_bedrock() {
        let c = cfg(VERSION - 1, VoxelScalePreset::OneMeter);
        for x in (-300..300).step_by(17) {
            for z in (-300..300).step_by(19) {
                for y in c.min_world_y()..c.min_world_y() + 12 {
                    assert_ne!(sample_block(&c, IVec3::new(x, y, z)), BlockId::Bedrock);
                }
            }
        }
    }
}
