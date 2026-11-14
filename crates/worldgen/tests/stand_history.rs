use engine_worldgen::{WorldSampleConfig, hash2, planetary, smoothstep, value_noise2};
// The implementation needs only stable identity and metre-space anchors.
struct TreeProfile { cell_x: i32, cell_z: i32, anchor_x: i32, anchor_z: i32 }
#[path = "../src/trees/stands.rs"]
mod stands;

#[test]
fn legacy_age_and_voxel_scale_contract() {
    use engine_core::VoxelScalePreset;
    let mut cfg = WorldSampleConfig { planet: None, seed: 74, generator_version: 139,
        world_type: engine_worldgen::WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter };
    let t = TreeProfile { cell_x: -3, cell_z: 7, anchor_x: -36, anchor_z: 84 };
    assert_eq!(stands::age(&cfg, &t), hash2(-3, 7, 74u32.wrapping_add(12011)));
    cfg.generator_version = stands::VERSION;
    let a = stands::age(&cfg, &t);
    cfg.voxel_scale = VoxelScalePreset::QuarterMeter;
    let fine = TreeProfile { anchor_x: -144, anchor_z: 336, ..t };
    assert_eq!(a, stands::age(&cfg, &fine));
}
