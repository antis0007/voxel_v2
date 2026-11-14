use super::key;
use crate::{WorldSampleConfig, WorldType};
use engine_core::VoxelScalePreset;
#[test]
fn canonical_cache_key_includes_every_configuration_field() {
    let cfg = WorldSampleConfig { seed: 74, generator_version: 287,
        world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter,
        planet: None };
    let base = key::canonical(&cfg, true, &[1; 32]);
    assert_ne!(base, key::canonical(&cfg, false, &[1; 32]));
    assert_ne!(base, key::canonical(&cfg, true, &[2; 32]));
    let mut changed = cfg; changed.seed += 1;
    assert_ne!(base, key::canonical(&changed, true, &[1; 32]));
    changed = cfg; changed.generator_version += 1;
    assert_ne!(base, key::canonical(&changed, true, &[1; 32]));
    for world_type in [WorldType::Superflat, WorldType::Hills, WorldType::Ridged,
        WorldType::Nether, WorldType::Manyfold, WorldType::Hall] {
        changed = cfg; changed.world_type = world_type;
        assert_ne!(base, key::canonical(&changed, true, &[1; 32]));
    }
    for voxel_scale in [VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
        changed = cfg; changed.voxel_scale = voxel_scale;
        assert_ne!(base, key::canonical(&changed, true, &[1; 32]));
    }
    changed = cfg; changed.planet = Some(engine_voxel::PlanetSettings::default());
    let recipe = key::canonical(&changed, true, &[1; 32]);
    assert_ne!(base, recipe);
    for i in 0..engine_voxel::PLANET_DIALS {
        let mut custom = changed;
        custom.planet.as_mut().unwrap().values[i] += 1;
        assert_ne!(recipe, key::canonical(&custom, true, &[1; 32]));
    }
    changed.planet.as_mut().unwrap().civilization = engine_voxel::CivilizationMode::Wilderness;
    assert_ne!(recipe, key::canonical(&changed, true, &[1; 32]));
}
