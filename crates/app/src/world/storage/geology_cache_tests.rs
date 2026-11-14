//! Two fresh processes; the external driver must verify cold miss then real warm hit.
use super::*;
use crate::world::inventory::{Item, ItemStack, SurvivalInventory};
use crate::simulation::{WorldSimulation, cpu_region::Region};

#[test]
#[ignore = "explicit private root and write/read stages; performs full planet generation"]
fn geology_cache_preserves_saved_edits_across_processes() {
    let root = PathBuf::from(std::env::var_os("AXIOM_GEOLOGY_SAVE_TEST_ROOT").expect("private root required"));
    assert!(root.is_absolute());
    let root = root.canonicalize().unwrap();
    assert_eq!(fs::read(root.join(".axiom-geology-save-test")).unwrap(), b"axiom-private-geology-save-test-v1");
    let saves = root.join("saves");
    let cache = root.join("geology");
    assert_eq!(PathBuf::from(std::env::var_os("VOXEL_SAVE_DIR").expect("isolated save env required"))
        .canonicalize().unwrap(), saves.canonicalize().unwrap());
    assert!(saves.is_dir() && cache.is_dir());
    for directory in [&root, &saves, &cache] {
        for path in directory.ancestors() {
            let meta = fs::symlink_metadata(path).unwrap();
            assert!(!meta.file_type().is_symlink());
            #[cfg(windows)] {
                use std::os::windows::fs::MetadataExt;
                assert_eq!(meta.file_attributes() & 0x400, 0, "reparse path forbidden");
            }
        }
    }
    let stage = std::env::var("AXIOM_GEOLOGY_SAVE_TEST_STAGE").unwrap();
    assert!(matches!(stage.as_str(), "write" | "read"));
    assert!(engine_worldgen::planetary::configure_geology_cache(cache.clone()));
    let cfg = engine_voxel::WorldSampleConfig { generator_version: 287, seed: 74,
        world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter, planet: None };
    let expected_path = root.join("expected.bin");
    if stage == "write" {
        assert_eq!(fs::read_dir(&saves).unwrap().count(), 0, "write stage requires empty saves");
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0, "write stage requires cold cache");
        assert!(!expected_path.exists());
        let mut world = World::with_seed_in(WorldType::Planet, VoxelScalePreset::OneMeter, 74, &saves);
        world.generator_version = 287;
        assert_eq!(world.sample_config(), cfg);
        let _view = engine_worldgen::planetary::regional_view(&cfg);
        // Select dry editable space near the real procedural spawn, without fallback edits.
        let spawn = crate::world::suggested_spawn(&cfg).expect("planet must have procedural spawn").floor().as_ivec3();
        let p = (0..16).map(|dy| spawn + IVec3::new(3, dy, 3)).find(|p| {
            p.y > cfg.min_world_y() && p.y < cfg.max_world_y() - 1
                && engine_worldgen::sample_block(&cfg, *p) == BlockId::Air
                && engine_worldgen::sample_block(&cfg, *p + IVec3::X) == BlockId::Air
        }).expect("dry in-bounds placement near spawn required");
        let adjacent = p + IVec3::X;
        let original = engine_worldgen::sample_block(&cfg, p);
        let neighbor = engine_worldgen::sample_block(&cfg, adjacent);
        let marker = if original == BlockId::Stone { BlockId::Bedrock } else { BlockId::Stone };
        assert_eq!(original, BlockId::Air, "fixture must not displace ocean water");
        // Production admitted edits require caller-prepared chunk residency.
        world.resident_edit(crate::world::chunk_coord_from_world_block(p)).unwrap();
        assert!(world.chunks.contains_key(&crate::world::chunk_coord_from_world_block(p)));
        let mut committed = Vec::new();
        world.set_many_committed(&[(p, marker)], |at, old, new| committed.push((at, old, new)));
        assert_eq!(committed, vec![(p, original, marker)], "production edit must commit exactly once");
        assert_eq!(world.get(p), marker);
        assert_eq!(world.get(adjacent), neighbor);
        world.inventory = SurvivalInventory::empty();
        world.inventory.pack[3] = Some(ItemStack::new(Item::Block(BlockId::Stone), 73));
        let player = Player::new();
        let day = DayNightCycle::default();
        let mut sim = WorldSimulation::new();
        sim.reset_for_world(world.voxel_scale);
        // Isolated inert authority well above procedural terrain; edit remains in region storage.
        let origin = IVec3::new(0, (cfg.max_world_y() + 16).div_euclid(8) * 8, 0);
        let region = Region::import(&world, origin).unwrap();
        world.publish_ca_view(region.published.clone(), &mut HashSet::new(), &mut HashSet::new());
        region.publish_reservations(&mut world);
        sim.cpu_region = Some(region);
        assert!(session::save(&saves, &mut world, &player, &day, &sim, &crate::player::Input::default()).unwrap());
        let mut evidence = OpenOptions::new().write(true).create_new(true).open(&expected_path).unwrap();
        for coordinate in [p.x, p.y, p.z] { evidence.write_all(&coordinate.to_le_bytes()).unwrap(); }
        for value in [original as u32, marker as u32, neighbor as u32] {
            evidence.write_all(&value.to_le_bytes()).unwrap();
        }
        evidence.sync_all().unwrap();
        assert!(saves.join("simulation.vxs").is_file());
        eprintln!("[geology-save-test] WRITE_OK seed=74 generator=287 edit={p:?}");
    } else {
        let evidence = fs::read(&expected_path).unwrap();
        assert_eq!(evidence.len(), 24);
        let value = |offset| BlockId::from_u32(u32::from_le_bytes(evidence[offset..offset+4].try_into().unwrap()));
        let coordinate = |offset| i32::from_le_bytes(evidence[offset..offset+4].try_into().unwrap());
        let p = IVec3::new(coordinate(0), coordinate(4), coordinate(8));
        let adjacent = p + IVec3::X;
        assert!(p.y > cfg.min_world_y() && p.y < cfg.max_world_y() - 1);
        let original = value(12); let marker = value(16); let neighbor = value(20);
        assert_ne!(original, marker);
        // This must emit the real cache hit in the external driver's captured stderr.
        let _view = engine_worldgen::planetary::regional_view(&cfg);
        let restored = session::load(&saves).unwrap();
        let world = restored.saved.world;
        assert!(restored.saved.recovery_notice.is_none());
        assert_eq!(world.sample_config(), cfg);
        assert_eq!(world.get(p), marker);
        assert_eq!(world.get(adjacent), neighbor);
        assert_eq!(engine_worldgen::sample_block(&cfg, p), original);
        assert_eq!(engine_worldgen::sample_block(&cfg, adjacent), neighbor);
        let mut expected_inventory = SurvivalInventory::empty();
        expected_inventory.pack[3] = Some(ItemStack::new(Item::Block(BlockId::Stone), 73));
        assert_eq!(world.inventory.pack, expected_inventory.pack);
        assert_eq!(world.inventory.hotbar, expected_inventory.hotbar);
        assert_eq!(world.inventory.cursor, expected_inventory.cursor);
        eprintln!("[geology-save-test] READ_OK seed=74 generator=287 saved-edit-preserved");
    }
}