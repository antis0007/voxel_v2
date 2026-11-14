use super::*;
use crate::{fluid::VoxelScalePreset, world::{ChunkData, regions, WorldType}};
use std::{path::PathBuf, time::{Duration, Instant}};
fn fixture() -> (World, PathBuf) {
    let root = std::env::var_os("AXIOM_PICK_TEST_ROOT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
        .join(format!("axiom-readonly-pick-{}-{}", std::process::id(), regions::checkpoint_id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut world = World::with_seed_in(WorldType::Superflat, VoxelScalePreset::OneMeter, 7, &root);
    world.generator_version = 287;
    (world, root)
}
fn blocks(block: BlockId) -> Arc<SourceBlocks> { Arc::from(vec![block; 1]) }
pub(super) fn chunk(block: BlockId) -> Arc<ChunkData> {
    Arc::new(ChunkData { blocks: Box::new([block; 32*32*32]), light: Box::new([0; 32*32*32]),
        torches: Default::default(), modified_bounds: None, structural_bounds: None })
}
fn feed(picker: &mut ReadOnlyPicker, world: &World, key: Key, generation: u64,
    data: Result<Option<Arc<SourceBlocks>>, String>) {
    let (tx, rx) = mpsc::sync_channel(1); picker.rx = rx;
    let items=match data {Ok(Some(data))=>vec![(key,Ok(data))],Ok(None)=>Vec::new(),Err(error)=>vec![(key,Err(error))]};
    tx.send(Done { generation, items, work:Default::default() }).unwrap(); picker.prepare(world).unwrap();
}
#[test]
fn authority_ca_resident_stored_and_procedural_precedence() {
    let (mut world, _) = fixture(); let p = IVec3::new(0,8,0); let c = chunk_coord_from_world_block(p);
    let mut picker = ReadOnlyPicker::default(); picker.prepare(&world).unwrap();
    picker.cache.push_back((Key::at_position(&world,p), blocks(BlockId::Stone)));
    assert_eq!(picker.read_ready(&world,p), Readiness::Ready(BlockId::Stone));
    Arc::make_mut(&mut world.regions.view.records).insert(c, regions::controlled_source_record(1));
    picker.cache.push_back((Key::at_position(&world,p), Arc::from(vec![BlockId::Dirt;32*32*32])));
    assert_eq!(picker.read_ready(&world,p), Readiness::Ready(BlockId::Dirt));
    world.chunks.insert(c, chunk(BlockId::Bedrock));
    assert_eq!(picker.read_ready(&world,p), Readiness::Ready(BlockId::Bedrock));
    world.ca_view = Some(Arc::new(crate::simulation::cpu_region::Snapshot {
        origin:p,tick:1,applied_sequence:0,cells:Box::new([engine_sim::Cell::filled(BlockId::Water as u8);512]),
        hash:0,stats:Default::default(),
    }));
    assert_eq!(picker.read_ready(&world,p), Readiness::Ready(BlockId::Water));
    world.ca_view = None;
    assert_eq!(picker.read_ready(&world,p), Readiness::Ready(world.get(p)));
}
#[test]
fn current_source_and_generation_reject_stale_completions() {
    let (mut world, _) = fixture(); let c = IVec3::ZERO;
    let mut picker = ReadOnlyPicker::default(); picker.prepare(&world).unwrap();
    let key = Key::at(&world,c); let generation = picker.generation;
    world.seed += 1;
    feed(&mut picker,&world,key,generation,Ok(Some(blocks(BlockId::Dirt)))); assert!(picker.cache.is_empty());
    let key = Key::at(&world,c); let generation = picker.generation;
    picker.reset(); picker.prepare(&world).unwrap();
    feed(&mut picker,&world,key,generation,Ok(Some(blocks(BlockId::Dirt)))); assert!(picker.cache.is_empty());
    Arc::make_mut(&mut world.regions.view.records).insert(c,regions::controlled_source_record(1));
    let key = Key::at(&world,c); let generation = picker.generation;
    Arc::make_mut(&mut world.regions.view.records).insert(c,regions::controlled_source_record(2));
    assert_ne!(key,Key::at(&world,c)); // Same version, different immutable record generation.
    feed(&mut picker,&world,key,generation,Ok(Some(blocks(BlockId::Dirt)))); assert!(picker.cache.is_empty());
    let (other, _) = fixture(); assert_ne!(Key::at(&world,c).instance,Key::at(&other,c).instance);
    let old = Key::at(&world,c); let generation = picker.generation;
    feed(&mut picker,&other,old,generation,Ok(Some(blocks(BlockId::Dirt)))); assert!(picker.cache.is_empty());
}
#[test]
fn full_config_identity_and_source_local_failures() {
    let (mut world, _) = fixture(); let mut picker = ReadOnlyPicker::default();
    let c=IVec3::ZERO; let original=Key::at(&world,c);
    world.voxel_scale=VoxelScalePreset::QuarterMeter;assert_ne!(original,Key::at(&world,c));
    world.voxel_scale=VoxelScalePreset::OneMeter;world.seed+=1;assert_ne!(original,Key::at(&world,c));
    world.seed-=1;world.world_type=WorldType::Hills;assert_ne!(original,Key::at(&world,c));
    world.world_type=WorldType::Superflat;world.generator_version=286;assert_ne!(original,Key::at(&world,c));
    assert!(matches!(picker.read_ready(&world,IVec3::ZERO),Readiness::Failed(_)));
    world.generator_version=287;world.planet_settings=Some(Default::default());assert_ne!(original,Key::at(&world,c));
    world.planet_settings=None;picker.prepare(&world).unwrap();
    let key=Key::at_position(&world,IVec3::new(0,8,0));let generation=picker.generation;
    feed(&mut picker,&world,key,generation,Err("injected corrupt source".into()));
    assert!(matches!(picker.read_ready(&world,IVec3::new(0,8,0)),Readiness::Failed(_)));
    world.chunks.insert(IVec3::X,chunk(BlockId::Dirt));
    assert_eq!(picker.read_ready(&world,IVec3::new(32,8,0)),Readiness::Ready(BlockId::Dirt));
    assert!(matches!(picker.poll(&world,DVec3::ZERO,Vec3::Y,257.0),Readiness::Failed(_)));
}
#[test]
fn bounded_completed_cache_pins_survive_eviction() {
    let (world, _) = fixture(); let mut picker=ReadOnlyPicker::default();picker.prepare(&world).unwrap();
    let pinned=blocks(BlockId::Dirt);
    for x in 0..(CACHE_CHUNKS+3) {
        let key=Key::at(&world,IVec3::new(x as i32,0,0));let generation=picker.generation;
        feed(&mut picker,&world,key,generation,Ok(Some(pinned.clone())));
        assert!(picker.cache.len()<=CACHE_CHUNKS);
    }
    assert_eq!(picker.cache.len(),CACHE_CHUNKS);assert_eq!(picker.cache.front().unwrap().0.coord.x,3);
    picker.reset();assert_eq!(pinned[0],BlockId::Dirt);assert!(picker.cache.is_empty());
}
#[test]
fn reset_cancels_but_does_not_release_active_occupancy() {
    let (world, _) = fixture();let mut picker=ReadOnlyPicker::default();picker.prepare(&world).unwrap();
    let cancel=Arc::new(AtomicBool::new(false));
    picker.pending=Some(Pending{keys:vec![Key::at(&world,IVec3::ZERO)],generation:picker.generation,cancel:cancel.clone()});
    picker.reset();assert!(cancel.load(Ordering::Relaxed));assert!(picker.pending.is_some());
    assert!(picker.cache.is_empty());assert!(picker.identity.is_none());
}
#[test]
fn asynchronous_scalar_ready_matches_original_and_never_installs() {
    let (world, root)=fixture();let mut picker=ReadOnlyPicker::default();
    let origin=DVec3::new(0.5,8.5,0.5);let dir=-Vec3::Y;let reach=6.0;
    let expected=super::super::collision::raycast_block_precise(&world,origin,dir,reach)
        .map(|(cell,previous)|PickHit{cell,previous,block:world.get(cell)});
    let before=std::fs::read_dir(&root).unwrap().count();let revision=world.revision();
    let deadline=Instant::now()+Duration::from_secs(30);let mut pending=0;
    let actual=loop {match picker.poll(&world,origin,dir,reach) {
        Readiness::Ready(hit)=>break hit,Readiness::Failed(error)=>panic!("{error}"),
        Readiness::Pending=>{pending+=1;assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(1));}
    }};
    assert!(pending>0);assert_eq!(actual,expected);assert!(world.chunks.is_empty());
    assert!(!world.simulation_writeback_pending());assert_eq!(world.revision(),revision);
    assert_eq!(std::fs::read_dir(&root).unwrap().count(),before);
    for (key,data) in picker.cache.iter() {
        let n=key.edge();
        for z in 0..n {for x in 0..n {for y in 0..n {
            let p=key.origin()+IVec3::new(x,y,z);
            assert_eq!(data[(x+n*z+n*n*y) as usize],engine_worldgen::sample_block(&world.sample_config(),p));
        }}}
    }
}

#[test]
fn missing_stored_record_fails_without_stone_or_installation() {
    let (mut world, _) = fixture();let c=IVec3::ZERO;let p=IVec3::new(0,8,0);
    Arc::make_mut(&mut world.regions.view.records).insert(c,regions::controlled_source_record(1));
    let mut picker=ReadOnlyPicker::default();let deadline=Instant::now()+Duration::from_secs(10);
    loop { match picker.read_ready(&world,p) {
        Readiness::Failed(error)=>{assert!(error.contains("stored source"));break;},
        Readiness::Ready(block)=>panic!("failed stored source became a block: {block:?}"),
        Readiness::Pending=>{assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(1));}
    }}
    assert!(world.chunks.is_empty());assert!(!world.simulation_writeback_pending());
}
