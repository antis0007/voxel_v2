use super::*;

#[test]
fn wildlife_save_37_round_trip_and_36_migration() {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("voxel-wildlife-save-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::HalfMeter, 424242);
    let mut words = vec![2];
    for (slot, health) in [(0, 17.0_f32), (1, 0.0)] {
        words.extend([2, 3, slot, 16]);
        words.extend([100.0_f32, 12.0, 150.0, 102.0, 12.0, 152.0, health, 0.8].map(f32::to_bits));
    }
    let mut words = words.into_iter();
    world.adventure.wildlife = super::super::adventure::wildlife::State::decode(
        &mut || Ok(words.next().unwrap())).unwrap();
    let mut expected = Vec::new();
    world.adventure.wildlife.encode(&[], &mut expected);
    for version in [36, SAVE_VERSION] {
        let path = tests::synth_save(&world, version, &dir, &format!("v{version}.vxw"));
        let loaded = load_exact(&path, &path).unwrap();
        let mut actual = Vec::new();
        loaded.world.adventure.wildlife.encode(&[], &mut actual);
        if version == 36 { assert_eq!(actual, 0_u32.to_le_bytes()); }
        else { assert_eq!(actual, expected); }
        assert_eq!(loaded.world.seed, world.seed);
        assert_eq!(loaded.world.voxel_scale, world.voxel_scale);
        assert_eq!(loaded.world.generator_version, world.generator_version);
        std::fs::remove_file(path).unwrap();
    }
    assert!(dir.canonicalize().unwrap().starts_with(std::env::temp_dir().canonicalize().unwrap()));
    std::fs::remove_dir_all(dir).unwrap();
}
