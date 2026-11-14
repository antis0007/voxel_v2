//! Source-footprint regressions updated from spatial bricks to exact points.
use super::*;
use crate::{fluid::VoxelScalePreset,world::{regions,WorldType}};
use std::time::Duration;
fn fixture()->World{
    let root=std::env::var_os("AXIOM_PICK_TEST_ROOT").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir)
        .join(format!("axiom-exact-point-{}-{}",std::process::id(),regions::checkpoint_id()));
    let mut world=World::with_seed_in(WorldType::Superflat,VoxelScalePreset::OneMeter,7,&root);world.generator_version=287;world
}
fn put(picker:&mut ReadOnlyPicker,world:&World,key:Key,block:BlockId){
    picker.prepare(world).unwrap();let count=key.voxels();let generation=picker.generation;
    let (tx,rx)=mpsc::sync_channel(1);picker.rx=rx;
    tx.send(Done{generation,items:vec![(key,Ok(Arc::from(vec![block;count])))],work:Default::default()}).unwrap();
    picker.prepare(world).unwrap();
}
#[test]
fn exact_footprints_preserve_negative_coordinates_and_integer_extremes(){
    let world=fixture();
    for x in [i32::MIN,-33,-32,-9,-8,-1,0,7,8,31,32,i32::MAX]{for p in [IVec3::splat(x),IVec3::new(x,8,x)]{
        let key=Key::at_position(&world,p);assert_eq!(key.edge(),1);assert_eq!(key.voxels(),1);
        assert_eq!(key.coord,chunk_coord_from_world_block(p));assert_eq!(key.origin(),p);
        assert_eq!(key.point,Some(p));assert_eq!(key.local_index(p),0);assert!(key.is_current(&world));
        if let Some(x)=p.x.checked_add(1){assert_ne!(key,Key::at_position(&world,IVec3::new(x,p.y,p.z)));}
    }}
}
#[test]
fn actual_worker_batch_samples_only_requested_points_with_scalar_equality(){
    let world=fixture();let (tx,rx)=source::start().unwrap();let mut keys=Vec::new();
    for x in [-33,-9,-8,-1,0,7,8,31,32]{for y in [0,8,32]{keys.push(Key::at_position(&world,IVec3::new(x,y,7-x)));}}
    tx.send(Job{keys:keys.clone(),generation:0,view:world.regions.view.clone(),cancel:Arc::new(AtomicBool::new(false)),review_gate:None}).unwrap();
    let done=rx.recv_timeout(Duration::from_secs(15)).unwrap();assert_eq!(done.items.len(),keys.len());
    assert_eq!(done.work.procedural_samples,keys.len() as u64);assert_eq!(done.work.stored_reads,0);
    for ((key,data),expected) in done.items.into_iter().zip(keys){assert_eq!(key,expected);
        let data=data.unwrap();assert_eq!(data.len(),1);assert_eq!(data[0],engine_worldgen::sample_block(&world.sample_config(),key.origin()));}
    assert!(world.chunks.is_empty());assert!(!world.simulation_writeback_pending());
}
#[test]
fn neighboring_points_do_not_alias_and_saved_record_invalidates_them(){
    let mut world=fixture();let a=IVec3::new(0,8,0);let b=IVec3::new(1,8,0);
    let ka=Key::at_position(&world,a);let kb=Key::at_position(&world,b);assert_eq!(ka.coord,kb.coord);assert_ne!(ka,kb);
    let mut picker=ReadOnlyPicker::default();put(&mut picker,&world,ka.clone(),BlockId::Dirt);put(&mut picker,&world,kb.clone(),BlockId::Stone);
    assert_eq!(picker.read_ready(&world,a),Readiness::Ready(BlockId::Dirt));assert_eq!(picker.read_ready(&world,b),Readiness::Ready(BlockId::Stone));
    Arc::make_mut(&mut world.regions.view.records).insert(ka.coord,regions::controlled_source_record(1));
    assert!(!ka.is_current(&world));assert!(!kb.is_current(&world));let stored=Key::at_position(&world,a);
    assert_eq!(stored,Key::at_position(&world,b));assert!(stored.point.is_none());assert_eq!(stored.voxels(),32768);
    put(&mut picker,&world,stored,BlockId::Bedrock);assert_eq!(picker.read_ready(&world,a),Readiness::Ready(BlockId::Bedrock));
    assert_eq!(picker.read_ready(&world,b),Readiness::Ready(BlockId::Bedrock));
}
#[test]
fn stored_and_point_entries_obey_both_memory_and_entry_limits(){
    let mut world=fixture();let mut picker=ReadOnlyPicker::default();let stored_bytes=32768*std::mem::size_of::<BlockId>();
    let max_stored=(CACHE_BYTES/stored_bytes).min(CACHE_CHUNKS);let inserted=CACHE_CHUNKS+8;
    for x in 0..inserted{let c=IVec3::new(x as i32,0,0);Arc::make_mut(&mut world.regions.view.records).insert(c,regions::controlled_source_record(1));
        put(&mut picker,&world,Key::at_position(&world,c*32),BlockId::Dirt);
        assert!(picker.cache.len()<=CACHE_CHUNKS);assert!(picker.cache.iter().map(|(_,v)|v.len()*std::mem::size_of::<BlockId>()).sum::<usize>()<=CACHE_BYTES);}
    assert_eq!(picker.cache.len(),max_stored);assert_eq!(picker.cache.front().unwrap().0.coord.x,(inserted-max_stored) as i32);
    for x in 0..CACHE_CHUNKS+16{put(&mut picker,&world,Key::at_position(&world,IVec3::new(100000+x as i32,8,0)),BlockId::Stone);
        assert!(picker.cache.len()<=CACHE_CHUNKS);assert!(picker.cache.iter().map(|(_,v)|v.len()*std::mem::size_of::<BlockId>()).sum::<usize>()<=CACHE_BYTES);}
    assert_eq!(picker.cache.len(),CACHE_CHUNKS);assert!(picker.cache.iter().all(|(key,_)|key.point.is_some()));
}
#[test]
fn maximum_ray_plan_is_bounded_and_matches_original_read_enumeration(){
    let world=fixture();let mut worst=0;
    for origin in [DVec3::splat(0.01),DVec3::splat(7.99),DVec3::splat(-0.01)]{
        for dir in [Vec3::ONE,Vec3::new(1.0,0.99,0.98),Vec3::new(-1.0,0.37,0.71),Vec3::X]{
            let mut original=Vec::new();
            // Independent call to the unchanged original reader: synthetic all-air
            // enumeration defines potential reads, never an actual game result.
            super::super::collision::raycast_loaded_block_precise(&world,origin,dir,MAX_REACH_BLOCKS,|p|{original.push(p);Some(BlockId::Air)});
            let mut picker=ReadOnlyPicker::default();let plan=picker.plan(&world,origin,dir,MAX_REACH_BLOCKS).unwrap();
            let expected:Vec<_>=original.into_iter().filter(|p|p.y>=world.sample_config().min_world_y() && p.y<world.sample_config().max_world_y()).collect();
            assert_eq!(plan.iter().map(Key::origin).collect::<Vec<_>>(),expected);
            worst=worst.max(plan.len());assert!(plan.len()<=MAX_BATCH && plan.len()<=CACHE_CHUNKS);
            assert!(plan.iter().map(|key|key.voxels()*std::mem::size_of::<BlockId>()).sum::<usize>()<=CACHE_BYTES);
        }
    }
    assert!(worst>400,"fixture must exercise the maximum diagonal source count");
}
#[test]
fn nonuniform_point_payloads_preserve_every_negative_coordinate(){
    let world=fixture();let anchor=IVec3::new(-8,8,-8);let palette=[BlockId::Air,BlockId::Stone,BlockId::Dirt,BlockId::Water,BlockId::Bedrock];
    let mut picker=ReadOnlyPicker::default();
    for y in 0..8{for z in 0..8{for x in 0..8{let p=anchor+IVec3::new(x,y,z);put(&mut picker,&world,Key::at_position(&world,p),palette[((x+8*z+64*y)%5) as usize]);}}}
    for y in 0..8{for z in 0..8{for x in 0..8{let p=anchor+IVec3::new(x,y,z);assert_eq!(picker.read_ready(&world,p),Readiness::Ready(palette[((x+8*z+64*y)%5) as usize]));}}}
    assert_eq!(picker.cache.len(),512);assert!(picker.pending.is_none());
}
#[test]
fn planning_air_is_never_a_result_and_known_occluders_bound_the_plan(){
    let world=fixture();let mut picker=ReadOnlyPicker::default();let origin=DVec3::new(0.5,9.5,0.5);
    put(&mut picker,&world,Key::at_position(&world,IVec3::new(0,8,0)),BlockId::Air);
    put(&mut picker,&world,Key::at_position(&world,IVec3::new(0,7,0)),BlockId::Stone);
    let plan=picker.plan(&world,origin,-Vec3::Y,6.0).unwrap();
    assert_eq!(plan.iter().map(Key::origin).collect::<Vec<_>>(),vec![IVec3::new(0,9,0)]);
    assert!(picker.pending.is_none()); // Enumeration alone did not start a worker.
    assert!(matches!(picker.poll(&world,origin,-Vec3::Y,6.0),Readiness::Pending));
    assert_eq!(picker.pending.as_ref().unwrap().keys,plan);
    picker.reset();let end=std::time::Instant::now()+Duration::from_secs(15);
    while picker.pending.is_some(){picker.prepare(&world).unwrap();assert!(std::time::Instant::now()<end);std::thread::sleep(Duration::from_millis(1));}
}

#[test]
fn nearer_exact_hit_wins_over_a_deeper_completed_source_failure(){
    let world=fixture();let mut picker=ReadOnlyPicker::default();picker.prepare(&world).unwrap();
    let near=IVec3::new(0,8,0);let deep=near+IVec3::X;
    let (tx,rx)=mpsc::sync_channel(1);picker.rx=rx;
    tx.send(Done{generation:picker.generation,items:vec![(Key::at_position(&world,near),Ok(Arc::from([BlockId::Stone]))),
        (Key::at_position(&world,deep),Err("injected deeper source failure".into()))],work:Default::default()}).unwrap();
    assert_eq!(picker.poll(&world,near.as_dvec3()+DVec3::splat(0.5),Vec3::X,6.0),
        Readiness::Ready(Some(PickHit{cell:near,previous:near,block:BlockId::Stone})));
    assert!(matches!(picker.poll(&world,deep.as_dvec3()+DVec3::splat(0.5),Vec3::X,6.0),Readiness::Failed(_)));
}
#[test]
fn maximal_mixed_source_plan_deduplicates_stored_chunks_and_keeps_only_ray_points(){
    let mut world=fixture();let origin=DVec3::new(0.01,32.01,-0.01);let dir=Vec3::new(0.99,0.51,-0.61);
    let mut original=Vec::new();super::super::collision::raycast_loaded_block_precise(&world,origin,dir,MAX_REACH_BLOCKS,|p|{original.push(p);Some(BlockId::Air)});
    let mut chunks=Vec::new();for p in &original{let c=chunk_coord_from_world_block(*p);if !chunks.contains(&c){chunks.push(c);}}
    for (i,c) in chunks.iter().enumerate(){if i%2==0{Arc::make_mut(&mut world.regions.view.records).insert(*c,regions::controlled_source_record(1));}}
    let mut expected=Vec::new();for p in &original{let key=Key::at_position(&world,*p);if !expected.contains(&key){expected.push(key);}}
    let mut picker=ReadOnlyPicker::default();let plan=picker.plan(&world,origin,dir,MAX_REACH_BLOCKS).unwrap();assert_eq!(plan,expected);
    assert!(plan.iter().any(|k|k.point.is_some()) && plan.iter().any(|k|k.point.is_none()));
    assert!(plan.len()<=MAX_BATCH);assert!(plan.iter().map(|k|k.voxels()*std::mem::size_of::<BlockId>()).sum::<usize>()<=CACHE_BYTES);
    for key in plan{if let Some(p)=key.point{assert!(original.contains(&p));}else{assert!(original.iter().any(|p|chunk_coord_from_world_block(*p)==key.coord));}}
}
