use super::*;

fn fixture(world: &World, version: u8, directory: &Path) -> PathBuf {
    let mut bytes = session_identity(world, &Player::new(), &DayNightCycle::default(), version).unwrap();
    if version < 49 { bytes.truncate(bytes.len() - 8); }
    bytes[..8].copy_from_slice(format!("VXWSAV{version:02}").as_bytes());
    let name = b"environment-test";
    push(&mut bytes, name.len() as u32);
    bytes.extend_from_slice(name);
    push(&mut bytes, 0);
    bytes.extend_from_slice(&session_footer(world, version));
    world.activity.snapshot().encode(&mut bytes);
    bytes.extend_from_slice(&1u64.to_le_bytes());
    let crc = checksum(&bytes);
    bytes.extend_from_slice(&crc.to_le_bytes());
    let path = directory.join(format!("world{version}.vxw"));
    fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn save_environment_stocks_work_rates_and_v30_compatibility() {
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let directory = std::env::temp_dir().join(format!("voxel_environment_{}_{}", std::process::id(), nonce));
    fs::create_dir(&directory).unwrap();
    fs::create_dir_all(directory.join("regions/environment-test")).unwrap();
    let mut world = World::new(WorldType::Superflat, VoxelScalePreset::QuarterMeter);
    let p = IVec3::new(-3, 4, -5);
    world.environment.create_patch(world.sample_config(), p, BlockId::Clay, true, true);
    world.environment.fixed_steps = 600;
    world.environment.target_tick = 5;
    world.environment.tick = 3;
    world.environment.biological_rate = 75;
    world.environment.erosion_rate = 35;
    world.environment.ledger.imported_litter_mg = 98_765;
    world.environment.ledger.imported_litter_nutrient_mg = 2_469;
    world.environment.ledger.edited_water_added_ul = 1_239_803;
    world.environment.ledger.edited_water_removed_ul = 347;
    world
        .environment
        .sediment
        .insert((p.x, p.y + 1, p.z), engine_sim::environment::Sediment { suspended: 12345, bed: 6789 });
    world.environment.hydraulic_work.insert((p.x, p.y + 1, p.z), 45678);
    world.environment.water_deficit.insert((p.x, p.y + 1, p.z), 9876);
    world.environment.wake(p);
    world.environment.wake(p + IVec3::Y);
    let mut expected = Vec::new();
    world.environment.encode(&mut expected);
    let path = fixture(&world, SAVE_VERSION, &directory);
    let loaded = load_exact(&path, &path).unwrap();
    let mut actual = Vec::new();
    loaded.world.environment.encode(&mut actual);
    assert_eq!(expected, actual);
    assert_eq!(loaded.world.voxel_scale, world.voxel_scale);
    drop(loaded);
    world.generator_version = 41;
    let legacy = fixture(&world, 30, &directory);
    let loaded = load_exact(&legacy, &legacy).unwrap();
    assert_eq!(loaded.world.generator_version, 41);
    assert!(loaded.world.environment.patches.is_empty());
    assert!(!crate::world::environment::State::enabled(loaded.world.generator_version));
    assert_eq!(loaded.world.inventory.hotbar, world.inventory.hotbar);
    drop(loaded);
    drop(world);
    fs::remove_dir_all(directory).unwrap();
}
