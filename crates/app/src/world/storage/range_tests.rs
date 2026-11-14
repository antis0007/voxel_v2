use super::max_player_coordinate;
use crate::fluid::VoxelScalePreset;
use crate::world::WorldType;

#[test]
fn flat_worlds_keep_the_million_voxel_bound() {
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
        assert_eq!(max_player_coordinate(WorldType::Hills, scale), 1_000_000.0);
    }
}

#[test]
fn planet_bound_covers_every_face_at_every_scale() {
    // Face 5 ends at 5 * PITCH + HALF in metres; a save must hold any point on it.
    let far_face_metres = 5.0 * engine_core::cube_sphere::atlas::PITCH_M + engine_core::cube_sphere::atlas::HALF_M;
    for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        let voxels = far_face_metres / f64::from(scale.meters());
        assert!(f64::from(max_player_coordinate(WorldType::Planet, scale)) >= voxels, "{scale:?}");
    }
}
