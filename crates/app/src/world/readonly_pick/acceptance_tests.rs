//! Additive acceptance tests. Fixture writes are deliberate; picker never installs.
use super::*;
use crate::{fluid::VoxelScalePreset,world::{regions,WorldType}};
use std::{collections::HashMap,io::{Read,Seek,SeekFrom,Write},path::{Path,PathBuf},time::{Duration,Instant}};
fn fixture() -> (World,PathBuf) {
    let root=std::env::var_os("AXIOM_PICK_TEST_ROOT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
        .join(format!("axiom-readonly-acceptance-{}-{}",std::process::id(),regions::checkpoint_id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut world=World::with_seed_in(WorldType::Superflat,VoxelScalePreset::OneMeter,7,&root);
    world.generator_version=287;(world,root)
}
type FileState=(PathBuf,u64,std::time::SystemTime,Option<Vec<u8>>);
fn files(root:&Path)->Vec<FileState> {
    fn collect(path:&Path,out:&mut Vec<FileState>){
        if !path.exists(){return;}
        for entry in std::fs::read_dir(path).unwrap(){let path=entry.unwrap().path();
            if path.is_dir(){collect(&path,out);}else{
                let metadata=std::fs::metadata(&path).expect("fixture file metadata");
                // The live region writer intentionally locks this exact file on Windows.
                // Retain its path/length/mtime; every other payload must remain readable
                // and is compared byte-for-byte. Do not ignore general IO failures.
                let payload=if path.file_name()==Some(std::ffi::OsStr::new(".owner-lock")){None}
                    else{Some(std::fs::read(&path).expect("fixture payload fingerprint"))};
                out.push((path,metadata.len(),metadata.modified().expect("fixture modified time"),payload));
            }
        }
    }
    let mut out=Vec::new();collect(root,&mut out);out.sort_by(|a,b|a.0.cmp(&b.0));out
}
fn result(picker:&mut ReadOnlyPicker,world:&World,p:IVec3)->Readiness<BlockId>{
    let end=Instant::now()+Duration::from_secs(15);
    loop{let result=picker.read_ready(world,p);if !matches!(result,Readiness::Pending){return result;}
        assert!(Instant::now()<end,"read-only source timeout");std::thread::sleep(Duration::from_millis(1));}
}
#[test]
fn genuine_saved_source_roundtrip_and_corruption_do_not_write_or_install(){
    let (mut writer,root)=fixture();let p=IVec3::new(70,40,-50);let c=chunk_coord_from_world_block(p);
    writer.set(p,BlockId::Dirt);writer.request_simulation_writeback();
    let deadline=Instant::now()+Duration::from_secs(10);
    while !writer.regions.view.records.contains_key(&c){
        assert!(Instant::now()<deadline,"fixture save timeout");writer.request_simulation_writeback();
        writer.stream_regions(c,1);std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!writer.simulation_writeback_pending());
    let saved_root=(*writer.regions.view.root).clone();
    let record=writer.regions.view.records.get(&c).unwrap().clone();
    let records=HashMap::from([(c,record.clone())]);
    let (mut reader,_)=fixture();reader.regions=regions::RegionStream::open(saved_root.clone(),records.clone());
    assert!(reader.chunks.is_empty());let revision=reader.revision();let before=files(&root);
    let mut picker=ReadOnlyPicker::default();
    assert_eq!(result(&mut picker,&reader,p),Readiness::Ready(BlockId::Dirt));
    let loaded=reader.regions.view.load(c).unwrap().unwrap();
    let cached=&picker.cache.iter().find(|(key,_)|key.coord==c).unwrap().1;
    assert_eq!(&cached[..],&loaded.chunk.blocks[..],"every saved block must roundtrip exactly");
    assert!(reader.chunks.is_empty());assert_eq!(reader.revision(),revision);
    assert!(!reader.simulation_writeback_pending());assert_eq!(files(&root),before);
    drop(picker);drop(reader);
    // Corrupt the actual immutable saved bytes. A fresh reader avoids a valid
    // decoded result surviving in the existing RegionView memory cache.
    let r=IVec3::new(c.x.div_euclid(8),c.y.div_euclid(8),c.z.div_euclid(8));
    let name=if record.generation==0{format!("r.{}.{}.{}.vxr",r.x,r.y,r.z)}
        else{format!("r.{}.{}.{}.{}.vxr",r.x,r.y,r.z,record.generation)};
    let mut file=std::fs::OpenOptions::new().read(true).write(true).open(saved_root.join(name)).unwrap();
    file.seek(SeekFrom::Start(record.offset)).unwrap();let mut byte=[0];file.read_exact(&mut byte).unwrap();
    byte[0]^=1;file.seek(SeekFrom::Start(record.offset)).unwrap();file.write_all(&byte).unwrap();file.sync_all().unwrap();drop(file);
    let (mut corrupt,_)=fixture();corrupt.regions=regions::RegionStream::open(saved_root,records);
    let before=files(&root);let mut picker=ReadOnlyPicker::default();
    assert!(matches!(result(&mut picker,&corrupt,p),Readiness::Failed(_)),"corruption must not become Stone or Air");
    assert!(corrupt.chunks.is_empty());assert!(!corrupt.simulation_writeback_pending());assert_eq!(files(&root),before);
}
fn gated(picker:&mut ReadOnlyPicker,world:&World,c:IVec3)->(mpsc::SyncSender<()>,Arc<AtomicBool>){
    picker.prepare(world).unwrap();let key=Key::at_position(world,c*32+IVec3::new(0,8,0));
    let (tx,rx)=source::start().unwrap();picker.tx=Some(tx.clone());picker.rx=rx;
    let (entered,started)=mpsc::sync_channel(1);let (release,gate)=mpsc::sync_channel(1);
    let cancel=Arc::new(AtomicBool::new(false));
    picker.pending=Some(Pending{keys:vec![key.clone()],generation:picker.generation,cancel:cancel.clone()});
    tx.send(Job{keys:vec![key],generation:picker.generation,view:world.regions.view.clone(),cancel:cancel.clone(),
        review_gate:Some(source::ReviewGate{entered,release:gate})}).unwrap();
    started.recv_timeout(Duration::from_secs(5)).expect("actual worker must acknowledge the gate");(release,cancel)
}
fn drain(picker:&mut ReadOnlyPicker,world:&World){
    let end=Instant::now()+Duration::from_secs(15);
    while picker.pending.is_some(){picker.prepare(world).unwrap();assert!(Instant::now()<end);std::thread::sleep(Duration::from_millis(1));}
}
#[test]
fn current_edit_and_ca_override_a_real_inflight_source(){
    let (mut world,_)=fixture();let p=IVec3::new(0,8,0);let c=IVec3::ZERO;
    let mut picker=ReadOnlyPicker::default();let (release,_)=gated(&mut picker,&world,c);
    // The real worker cannot complete until released, so these Ready results
    // prove current authority does not wait for an obsolete procedural source.
    world.chunks.insert(c,super::tests::chunk(BlockId::Dirt));
    assert_eq!(picker.read_ready(&world,p),Readiness::Ready(BlockId::Dirt));
    world.ca_view=Some(Arc::new(crate::simulation::cpu_region::Snapshot{origin:p,tick:2,applied_sequence:0,
        cells:Box::new([engine_sim::Cell::filled(BlockId::Water as u8);512]),hash:0,stats:Default::default()}));
    assert_eq!(picker.read_ready(&world,p),Readiness::Ready(BlockId::Water));
    release.send(()).unwrap();drain(&mut picker,&world);
    assert_eq!(picker.read_ready(&world,p),Readiness::Ready(BlockId::Water));
    world.ca_view=None;assert_eq!(picker.read_ready(&world,p),Readiness::Ready(BlockId::Dirt));
    world.chunks.remove(&c);
    assert_eq!(picker.read_ready(&world,p),Readiness::Ready(engine_worldgen::sample_block(&world.sample_config(),p)));
}
#[test]
fn real_worker_cancellation_never_joins_and_discards_completion(){
    let (world,_)=fixture();let mut picker=ReadOnlyPicker::default();
    let (release,cancel)=gated(&mut picker,&world,IVec3::ZERO);
    let start=Instant::now();picker.reset();let reset_ms=start.elapsed().as_secs_f64()*1000.0;
    // This line can only execute if reset did not join the deliberately blocked
    // actual worker. No fragile wall-clock deadline is the correctness proof.
    assert!(cancel.load(Ordering::Relaxed));assert!(picker.pending.is_some());
    assert!(matches!(picker.rx.try_recv(),Err(mpsc::TryRecvError::Empty)),"worker must still be gated, not joined");
    release.send(()).unwrap();drain(&mut picker,&world);assert!(picker.cache.is_empty());
    println!("[readonly-cancel] reset_ms={reset_ms:.6} completion_drained=true");
}
#[test]
fn every_raw_planet_dial_and_civilization_binds_the_source(){
    let (mut world,_)=fixture();world.planet_settings=Some(Default::default());let original=Key::at(&world,IVec3::ZERO);
    for dial in 0..12{for value in [0,51,100,255]{
        let mut settings=engine_voxel::PlanetSettings::default();settings.values[dial]=value;
        world.planet_settings=Some(settings);assert_ne!(original,Key::at(&world,IVec3::ZERO));
    }}
    let mut settings=engine_voxel::PlanetSettings::default();settings.civilization=engine_voxel::CivilizationMode::Wilderness;
    world.planet_settings=Some(settings);assert_ne!(original,Key::at(&world,IVec3::ZERO));
    world.planet_settings=None;assert_ne!(original,Key::at(&world,IVec3::ZERO));
}
#[test]
fn invalid_empty_ray_is_not_a_pending_unknown_source(){
    let (world,_)=fixture();let mut picker=ReadOnlyPicker::default();
    for (origin,dir,reach) in [(DVec3::ZERO,Vec3::ZERO,6.0),(DVec3::splat(f64::NAN),Vec3::Y,6.0),
        (DVec3::ZERO,Vec3::splat(f32::NAN),6.0),(DVec3::ZERO,Vec3::Y,0.0),
        (DVec3::ZERO,Vec3::Y,f32::NAN),(DVec3::ZERO,Vec3::Y,f32::INFINITY)]{
        assert_eq!(super::super::collision::raycast_block_precise(&world,origin,dir,reach),None);
        assert_eq!(picker.poll(&world,origin,dir,reach),Readiness::Ready(None));
        assert!(picker.pending.is_none());
    }
    assert!(matches!(picker.poll(&world,DVec3::new(0.5,8.5,0.5),Vec3::Y,6.0),Readiness::Pending));
    picker.reset();drain(&mut picker,&world);
}
