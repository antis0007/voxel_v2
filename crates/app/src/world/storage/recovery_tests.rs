use super::*;

#[test]
fn recovery_checkpoints_survive_save_and_pre_41_load_uses_world_spawn() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "voxel-recovery-save-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::HalfMeter, 8181);
    world.adventure.respawn_point = Some(Vec3::new(12.5, 8.0, -9.25));
    world.adventure.last_death_pos = Some(Vec3::new(-30.0, -12.0, 4.5));

    let current = tests::synth_save(&world, SAVE_VERSION, &dir, "recovery-current.vxw");
    let loaded = load_exact(&current, &current).unwrap();
    assert_eq!(
        loaded.world.adventure.respawn_point,
        world.adventure.respawn_point
    );
    assert_eq!(
        loaded.world.adventure.last_death_pos,
        world.adventure.last_death_pos
    );

    let previous = tests::synth_save(&world, 40, &dir, "recovery-v40.vxw");
    let loaded = load_exact(&previous, &previous).unwrap();
    assert_eq!(loaded.world.adventure.respawn_point, None);
    assert_eq!(loaded.world.adventure.last_death_pos, None);

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn absent_recovery_checkpoints_remain_absent() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "voxel-recovery-empty-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    let world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 99);
    let path = tests::synth_save(&world, SAVE_VERSION, &dir, "recovery-empty.vxw");
    let loaded = load_exact(&path, &path).unwrap();
    assert_eq!(loaded.world.adventure.respawn_point, None);
    assert_eq!(loaded.world.adventure.last_death_pos, None);
    std::fs::remove_dir_all(dir).unwrap();
}
