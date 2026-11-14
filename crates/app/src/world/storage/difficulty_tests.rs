use super::*;
use crate::world::adventure::difficulty::Difficulty;

#[test]
fn difficulty_40_round_trip_and_39_loads_as_normal() {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("voxel-difficulty-save-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::HalfMeter, 424242);
    world.adventure.set_difficulty(Difficulty::Hard);
    for version in [39, SAVE_VERSION] {
        let path = tests::synth_save(&world, version, &dir, &format!("difficulty-v{version}.vxw"));
        let loaded = load_exact(&path, &path).unwrap();
        let expected = if version == 39 { Difficulty::Normal } else { Difficulty::Hard };
        assert_eq!(loaded.world.adventure.difficulty, expected, "save version {version}");
        assert_eq!(loaded.world.adventure.hunger_mode, expected.hunger_mode(), "save version {version}");
        assert_eq!(loaded.world.seed, world.seed);
        std::fs::remove_file(path).unwrap();
    }
    assert!(dir.canonicalize().unwrap().starts_with(std::env::temp_dir().canonicalize().unwrap()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn every_level_survives_a_save() {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("voxel-difficulty-levels-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    for (index, level) in Difficulty::ALL.into_iter().enumerate() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::HalfMeter, 424242);
        world.adventure.set_difficulty(level);
        let path = tests::synth_save(&world, SAVE_VERSION, &dir, &format!("level-{index}.vxw"));
        let loaded = load_exact(&path, &path).unwrap();
        assert_eq!(loaded.world.adventure.difficulty, level);
        std::fs::remove_file(path).unwrap();
    }
    std::fs::remove_dir_all(dir).unwrap();
}
