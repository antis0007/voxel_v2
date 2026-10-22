//! Source packet only. Native tests run by Root, never implicitly at preparation.
use super::*;
use crate::mesh::Vertex3;
use std::sync::atomic::AtomicU8;

fn requested_adapter() -> wgpu::Adapter {
    let instance=wgpu::Instance::new(&wgpu::InstanceDescriptor{backends:wgpu::Backends::VULKAN,..Default::default()});
    let adapter=pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:wgpu::PowerPreference::HighPerformance,..Default::default()
    })).unwrap();
    let info=adapter.get_info();println!("[pending-gpu-test] adapter={info:?}");
    if let Ok(name)=std::env::var("VOXEL_TEST_ADAPTER_NAME") {
        assert!(info.name.contains(&name),"Required adapter {name:?}, got {:?}",info.name);
    }
    adapter
}
fn setup() -> (wgpu::Device,wgpu::Queue,ChunkCache,ChunkStreamer) {
    let adapter=requested_adapter();
    let(device,queue)=pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut cache=ChunkCache::new(IVec3::ZERO);
    cache.visible=vec![ChunkKey{coord:IVec3::ZERO,lod:0}];cache.rebuild_visible_sets();
    let streamer=ChunkStreamer::new(&device,1).unwrap();
    (device,queue,cache,streamer)
}
fn cpu(streamer:&ChunkStreamer,lod:u8,revision:u64,job_id:u64,vertex_count:usize)->CpuChunkMesh {
    let vertices=(0..vertex_count).map(|i|Vertex3{pos:[i as f32,revision as f32,0.],
        color:[0.5;3],normal:[0.,1.,0.],surface:[0.9,0.05,3.,0.],ao_strength:1.}).collect::<Vec<_>>();
    let mesh=ChunkMesh{vertices,indices:vec![0,1,2]};
    let bytes=mesh.vertices.capacity()*std::mem::size_of::<Vertex3>()+mesh.indices.capacity()*4;
    let lease=streamer.mesh_budget.acquire(bytes,&AtomicBool::new(false)).unwrap();
    CpuChunkMesh{failed:false,lod_invariant:false,key:ChunkKey{coord:IVec3::ZERO,lod},revision,
        job_id,world_epoch:streamer.upload_epoch,cache_epoch:0,mesh,shadow_casters:Default::default(),boundary_profile:Default::default(),_lease:Some(lease)}
}
fn uploaded(device:&wgpu::Device,queue:&wgpu::Queue,s:&ChunkStreamer,c:&CpuChunkMesh)->ChunkGpuMesh {
    let(mesh,check)=gpu_upload::start(device,queue,&c.mesh,c.key,c.lod_invariant,&s.empty_vertex,&s.empty_index);
    let submission=queue.submit([]);
    device.poll(wgpu::PollType::Wait{submission_index:Some(submission),timeout:Some(Duration::from_secs(10))}).unwrap();
    assert!(pollster::block_on(check).is_none());mesh.unwrap()
}
fn controlled_check(state:Arc<AtomicU8>)->gpu_upload::Check {
    Box::pin(std::future::poll_fn(move |_|match state.load(Ordering::Relaxed) {
        0=>std::task::Poll::Pending,1=>std::task::Poll::Ready(None),
        _=>std::task::Poll::Ready(Some("controlled delayed validation error".into()))
    }))
}
fn seed_old(device:&wgpu::Device,queue:&wgpu::Queue,cache:&mut ChunkCache,s:&ChunkStreamer,lod:u8) {
    let c=cpu(s,lod,1,1,3);let gpu=uploaded(device,queue,s,&c);cache.install_mesh(c.key.coord,gpu);
}
fn stage(device:&wgpu::Device,queue:&wgpu::Queue,cache:&mut ChunkCache,s:&mut ChunkStreamer,
    c:CpuChunkMesh,state:Arc<AtomicU8>) {
    let gpu=uploaded(device,queue,s,&c);let lease=cache.reserve_pending_gpu(c.key,gpu.bytes).unwrap();
    s.begin_pending_upload(c,Ok(gpu),controlled_check(state),lease,cache.gpu_world_epoch);
}

#[test]
#[ignore="native production keyed GPU publication Pending/Ready contract; Root slot only"]
fn pending_gpu_success_and_error_keep_old_cover_until_validation() {
    for completion in [1,2] {
        let(device,queue,mut cache,mut s)=setup();seed_old(&device,&queue,&mut cache,&s,2);
        let old_revision=cache.gpu_mesh_revision;let old_owners=cache.published_coords.clone();
        let state=Arc::new(AtomicU8::new(0));let c=cpu(&s,0,10,0,3); // synthetic job_id 0 still has an epoch
        stage(&device,&queue,&mut cache,&mut s,c,state.clone());let held=s.mesh_budget.used();
        assert!(held>0&&cache.pending_gpu.bytes()>0);
        assert_eq!(s.poll_pending_uploads(&device,&AtomicBool::new(false),&mut cache,4,Instant::now()).0,0);
        assert_eq!(cache.gpu_meshes[&IVec3::ZERO].lod,2);assert_eq!(cache.gpu_mesh_revision,old_revision);
        assert_eq!(cache.published_coords,old_owners);assert_eq!(cache.lod_meshes.bytes(),0);
        assert_eq!(s.mesh_budget.used(),held);assert_eq!(s.pending_uploads.len(),1);
        assert!(cache.reserve_pending_gpu(ChunkKey{coord:IVec3::ZERO,lod:0},180).is_none(),"same-coordinate reservation cannot overwrite");
        state.store(completion,Ordering::Relaxed);
        let committed=s.poll_pending_uploads(&device,&AtomicBool::new(false),&mut cache,4,Instant::now()).0;
        assert_eq!(cache.pending_gpu.bytes(),0);assert!(s.pending_uploads.is_empty());
        if completion==1 {
            assert_eq!(committed,1);assert_eq!(cache.gpu_meshes[&IVec3::ZERO].lod,0);
            assert!(cache.gpu_mesh_revision>old_revision);assert_eq!(s.mesh_budget.used(),0);
        }else{
            assert_eq!(committed,0);assert_eq!(cache.gpu_meshes[&IVec3::ZERO].lod,2);
            assert_eq!(cache.gpu_mesh_revision,old_revision);assert_eq!(s.ready_meshes.len(),1);
            assert_eq!(s.mesh_budget.used(),held);assert!(s.requested_coords.contains(&IVec3::ZERO));
        }
    }
}

#[test]
#[ignore="native production edit/interest/LOD revalidation; Root slot only"]
fn pending_gpu_edit_interest_and_lod_changes_revalidate_before_commit() {
    for change in 0..3 {
        let(device,queue,mut cache,mut s)=setup();seed_old(&device,&queue,&mut cache,&s,0);
        if change==2 {cache.center_chunk=IVec3::new(1000,0,1000);}
        let lod=cache.desired_lod(IVec3::ZERO);if change==2{assert_ne!(lod,0);}
        let state=Arc::new(AtomicU8::new(0));let c=cpu(&s,lod,10,7,3);
        stage(&device,&queue,&mut cache,&mut s,c,state.clone());let revision=cache.gpu_mesh_revision;
        match change {
            0=>{s.dirty_revisions.insert(IVec3::ZERO,11);cache.dirty_meshes.insert(IVec3::ZERO);},
            1=>{cache.visible_coords.clear();cache.published_coords.clear();cache.published_columns.clear();},
            _=>{cache.center_chunk=IVec3::ZERO;}
        }
        state.store(1,Ordering::Relaxed);
        assert_eq!(s.poll_pending_uploads(&device,&AtomicBool::new(false),&mut cache,4,Instant::now()).0,0);
        assert_eq!(cache.gpu_meshes[&IVec3::ZERO].lod,0);assert_eq!(cache.gpu_mesh_revision,revision);
        assert_eq!(cache.pending_gpu.bytes(),0);assert_eq!(s.mesh_budget.used(),0);
        if change==0 {assert_eq!(s.dirty_revisions.get(&IVec3::ZERO),Some(&11));assert!(s.pending_keys.contains(&ChunkKey{coord:IVec3::ZERO,lod:0}));}
    }
}

#[test]
#[ignore="native production world reset and upload-token ownership; Root slot only"]
fn pending_gpu_world_reset_and_newer_tokens_cannot_publish_or_clear_new_work() {
    for reset in [true,false] {
        let(device,queue,mut cache,mut s)=setup();seed_old(&device,&queue,&mut cache,&s,2);
        let state=Arc::new(AtomicU8::new(0));let mut c=cpu(&s,0,10,0,3);
        c.mesh=ChunkMesh::empty();c._lease=None;c.lod_invariant=true; // actual uniform-empty shape/job0
        stage(&device,&queue,&mut cache,&mut s,c,state.clone());
        let key=ChunkKey{coord:IVec3::ZERO,lod:0};let old_epoch=s.upload_epoch;
        if reset {
            cache.reset(IVec3::ZERO);s.reset();assert_ne!(s.upload_epoch,old_epoch);
            s.requested.insert(key);s.requested_coords.insert(key.coord);
        }else{
            let newer=s.next_upload_token+1;s.pending_gpu_coords.insert(key.coord,newer);
            s.requested.insert(key);s.requested_coords.insert(key.coord);
        }
        state.store(1,Ordering::Relaxed);
        assert_eq!(s.poll_pending_uploads(&device,&AtomicBool::new(false),&mut cache,4,Instant::now()).0,0);
        assert!(s.requested.contains(&key)&&s.requested_coords.contains(&key.coord));
        assert_eq!(cache.pending_gpu.bytes(),0);assert_eq!(s.mesh_budget.used(),0);
        if reset {assert!(cache.gpu_meshes.is_empty());}else{assert_eq!(cache.gpu_meshes[&key.coord].lod,2);}
    }
    // Cache-only generation reset also invalidates an already allocated result,
    // even before a separate streamer reset has run.
    {
        let(device,queue,mut cache,mut s)=setup();seed_old(&device,&queue,&mut cache,&s,2);
        let state=Arc::new(AtomicU8::new(0));let c=cpu(&s,0,10,0,3);
        stage(&device,&queue,&mut cache,&mut s,c,state.clone());cache.reset(IVec3::ZERO);
        assert!(cache.pending_gpu.bytes()>0,"old-generation overlap remains charged until dropped");
        state.store(1,Ordering::Relaxed);assert_eq!(s.poll_pending_uploads(&device,&AtomicBool::new(false),&mut cache,4,Instant::now()).0,0);
        assert!(cache.gpu_meshes.is_empty());assert_eq!(cache.pending_gpu.bytes(),0);assert_eq!(s.mesh_budget.used(),0);
    }
    // Old synthesized CPU result with job_id=0 is rejected after epoch change.
    let(device,_,mut cache,mut s)=setup();let old=cpu(&s,0,1,0,3);s.reset();
    assert!(!s.cpu_world_current(&cache,&old));drop(old);
    assert_eq!(s.poll_pending_uploads(&device,&AtomicBool::new(false),&mut cache,4,Instant::now()).0,0);
}

#[test]
#[ignore="actual small-limit GPU scoped validation failure, no OOM; Root slot only"]
fn pending_gpu_actual_scoped_error_never_installs_invalid_destination() {
    let(device,queue,mut cache,mut s)=setup();seed_old(&device,&queue,&mut cache,&s,2);
    let old_revision=cache.gpu_mesh_revision;
    let adapter=requested_adapter();
    let(small,small_queue)=pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor{
        required_limits:wgpu::Limits{max_buffer_size:4096,..Default::default()},..Default::default()})).unwrap();
    let c=cpu(&s,0,10,9,128); // 7168 vertex bytes: small, but above the requested device limit.
    assert!(std::mem::size_of_val(c.mesh.vertices.as_slice()) as u64>small.limits().max_buffer_size);
    let bytes=std::mem::size_of_val(c.mesh.vertices.as_slice())+c.mesh.indices.len()*4;
    let lease=cache.reserve_pending_gpu(c.key,bytes).unwrap();
    // Deliberately bypass supported() ONLY in this fixture to exercise the real
    // production error scopes; no production failure injection or huge allocation.
    let(attempt,check)=gpu_upload::start(&small,&small_queue,&c.mesh,c.key,false,&s.empty_vertex,&s.empty_index);
    s.begin_pending_upload(c,attempt,check,lease,cache.gpu_world_epoch);
    small.poll(wgpu::PollType::Wait{submission_index:None,timeout:Some(Duration::from_secs(10))}).unwrap();
    assert_eq!(s.poll_pending_uploads(&small,&AtomicBool::new(false),&mut cache,4,Instant::now()).0,0);
    assert_eq!(cache.gpu_meshes[&IVec3::ZERO].lod,2);assert_eq!(cache.gpu_mesh_revision,old_revision);
    assert!(s.upload_error.is_some());assert_eq!(s.ready_meshes.len(),1);
    assert!(s.mesh_budget.used()>0);assert_eq!(cache.pending_gpu.bytes(),0);
}

#[test]
#[ignore="actual nonblocking validation progress without capture/profiler; Root slot only"]
fn pending_gpu_real_success_progresses_without_capture_or_profiler() {
    let(device,queue,mut cache,mut s)=setup();seed_old(&device,&queue,&mut cache,&s,2);
    let c=cpu(&s,0,10,0,3);
    let lease=cache.reserve_pending_gpu(c.key,std::mem::size_of_val(c.mesh.vertices.as_slice())+c.mesh.indices.len()*4).unwrap();
    let(attempt,check)=gpu_upload::start(&device,&queue,&c.mesh,c.key,false,&s.empty_vertex,&s.empty_index);
    s.begin_pending_upload(c,attempt,check,lease,cache.gpu_world_epoch);
    queue.submit([]);
    let deadline=Instant::now()+Duration::from_secs(5);
    while !s.pending_uploads.is_empty() && Instant::now()<deadline {
        s.poll_pending_uploads(&device,&AtomicBool::new(false),&mut cache,4,Instant::now());
        std::thread::yield_now();
    }
    assert!(s.pending_uploads.is_empty(),"production nonblocking poll must progress without renderer/capture/profiler");
    assert_eq!(cache.gpu_meshes[&IVec3::ZERO].lod,0);
    assert!(s.upload_error.is_none());assert_eq!(cache.pending_gpu.bytes(),0);
    assert_eq!(s.mesh_budget.used(),0);
}

#[test]
#[ignore="native handles with synthetic budget metadata; not physical VRAM pressure; Root slot only"]
fn pending_gpu_accounting_includes_overlap_and_dynamic_supplemental_membership() {
    let(device,queue,mut cache,s)=setup();seed_old(&device,&queue,&mut cache,&s,0);
    let q=IVec3::ZERO;let key=ChunkKey{coord:q,lod:0};
    cache.horizon_pin_coords.insert(q);
    // Budget metadata intentionally models a near-full 1GiB owner set without
    // allocating 1GiB on the device. Exercise the actual reservation functions.
    cache.gpu_meshes.get_mut(&q).unwrap().bytes=edit_proxies::BYTE_LIMIT-100;
    cache.gpu_bytes=edit_proxies::BYTE_LIMIT-100;
    cache.residency_limits.1=edit_proxies::BYTE_LIMIT*2;
    assert!(cache.reserve_pending_gpu(key,180).is_none(),"old buffers cannot be subtracted before validation");
    cache.gpu_meshes.get_mut(&q).unwrap().bytes=edit_proxies::BYTE_LIMIT-400;
    cache.gpu_bytes=edit_proxies::BYTE_LIMIT-400;
    let lease=cache.reserve_pending_gpu(key,180).unwrap();assert_eq!(cache.pending_gpu.bytes(),180);
    let other=cache.pending_gpu.reserve(IVec3::X,180).unwrap();assert_eq!(cache.pending_gpu.bytes(),360);
    cache.horizon_pin_coords.insert(IVec3::X);
    assert!(cache.pending_publication_fits(key,180));
    cache.gpu_meshes.get_mut(&q).unwrap().bytes=edit_proxies::BYTE_LIMIT-300;
    cache.gpu_bytes=edit_proxies::BYTE_LIMIT-300;
    assert!(!cache.pending_publication_fits(key,180),"new supplemental membership must be rechecked");
    drop(other);drop(lease);assert_eq!(cache.pending_gpu.bytes(),0);
}

#[test]
#[ignore="actual GPU destinations plus controlled device-health race; Root native slot"]
fn pending_gpu_health_signal_changes_during_ready_poll_cannot_publish() {
    let(device,queue,mut cache,mut s)=setup();
    seed_old(&device,&queue,&mut cache,&s,2);
    let old_revision=cache.gpu_mesh_revision;
    let old_owners=cache.published_coords.clone();
    let c=cpu(&s,0,10,1,3);
    stage(&device,&queue,&mut cache,&mut s,c,Arc::new(AtomicU8::new(0)));
    let lost=Arc::new(AtomicBool::new(false));let callback=Arc::clone(&lost);
    // Controlled callback timing, not a claim of reproducing a driver failure.
    s.pending_uploads[0].check=Some(Box::pin(async move {
        callback.store(true,Ordering::Release);None
    }));
    assert_eq!(s.poll_pending_uploads(&device,&lost,&mut cache,4,Instant::now()).0,0);
    assert_eq!(cache.gpu_meshes[&IVec3::ZERO].lod,2);
    assert_eq!(cache.gpu_mesh_revision,old_revision);
    assert_eq!(cache.published_coords,old_owners);
    assert_eq!(cache.pending_gpu.bytes(),0);
    assert!(s.pending_uploads.is_empty());assert_eq!(s.ready_meshes.len(),1);
    assert_eq!(cache.lod_meshes.bytes(),0);
}

fn exact_test_sources(cache:&mut ChunkCache,coords:HashSet<IVec3>) {
    let columns=coords.iter().map(|q|(q.x,q.z)).collect();
    cache.edit_proxies.set_quality_sources(coords.clone(),columns,HashSet::new());
    cache.visible_coords.extend(coords);
}

#[test]
#[ignore="actual RTX4090 destinations; equal-revision COW and water invalidation"]
fn local_source_changes_cancel_old_gpu_candidates_but_not_unrelated_publication() {
    use crate::{fluid::VoxelScalePreset,world::{World,WorldType}};
    for change in 0..5 {
        let(device,queue,mut cache,mut s)=setup();seed_old(&device,&queue,&mut cache,&s,2);
        let mut world=World::with_seed(WorldType::Superflat,VoxelScalePreset::OneMeter,7);
        if change==2 {world.water_levels.insert(IVec3::ZERO,4);}
        let revision=world.revision();let mut original=world.snapshot();
        if change==4 {Arc::make_mut(&mut original.regions.records).insert(IVec3::ZERO,crate::world::regions::controlled_source_record(1));}
        let far=IVec3::new(10,0,0);
        exact_test_sources(&mut cache,HashSet::from([IVec3::ZERO,far]));
        s.invalidate_changed_sources(&original,&mut cache);
        let old=cache.gpu_mesh_revision;
        let state=Arc::new(AtomicU8::new(0));
        let old_cpu=cpu(&s,0,revision,42,3);
        stage(&device,&queue,&mut cache,&mut s,old_cpu,state.clone());
        let mut unrelated=cpu(&s,0,revision,43,3);unrelated.key.coord=far;
        let far_state=Arc::new(AtomicU8::new(0));
        stage(&device,&queue,&mut cache,&mut s,unrelated,far_state.clone());
        let mut changed=original.clone();
        match change {
            0=>{Arc::make_mut(&mut changed.chunks).insert(IVec3::ZERO,
                Arc::new(engine_worldgen::generate(&changed.cfg,IVec3::ZERO)));}
            1=>{world.water_levels.insert(IVec3::ZERO,4);changed.water_levels=Arc::new(world.water_levels.snapshot_all());}
            2=>{world.water_levels.remove(&IVec3::ZERO);changed.water_levels=Arc::new(world.water_levels.snapshot_all());}
            _=>{Arc::make_mut(&mut changed.regions.records).insert(IVec3::ZERO,crate::world::regions::controlled_source_record(2));}
        }
        assert_eq!(world.revision(),revision,"source observation must handle equal simulation revision");
        let invalid=s.invalidate_changed_sources(&changed,&mut cache);
        assert!(invalid.contains(&IVec3::ZERO));assert!(!invalid.contains(&far));
        assert_eq!(s.pending_uploads.len(),1);assert_eq!(cache.pending_gpu.bytes(),180);
        assert_eq!(cache.gpu_mesh_revision,old);assert_eq!(cache.gpu_meshes[&IVec3::ZERO].lod,2);
        state.store(1,Ordering::Relaxed);far_state.store(1,Ordering::Relaxed);
        assert_eq!(s.poll_pending_uploads(&device,&AtomicBool::new(false),&mut cache,4,Instant::now()).0,1);
        assert_eq!(cache.gpu_meshes[&IVec3::ZERO].lod,2);assert_eq!(cache.gpu_meshes[&far].lod,0);
        let fresh=cpu(&s,0,revision,44,3);
        stage(&device,&queue,&mut cache,&mut s,fresh,Arc::new(AtomicU8::new(1)));
        assert!(s.invalidate_changed_sources(&changed,&mut cache).is_empty());
        assert_eq!(s.poll_pending_uploads(&device,&AtomicBool::new(false),&mut cache,4,Instant::now()).0,1);
        assert_eq!(cache.gpu_meshes[&IVec3::ZERO].lod,0);
        println!("[local-source-contract] change={change} same_revision={revision} old rejected/unrelated and fresh accepted");
    }
}

#[test]
#[ignore="actual RTX4090 fixtures; late worker and uniform source cancellation"]
fn local_source_cancellation_discards_uniform_ready_and_late_old_worker_result() {
    use crate::{fluid::VoxelScalePreset,world::{World,WorldType}};
    let(device,queue,mut cache,mut s)=setup();seed_old(&device,&queue,&mut cache,&s,2);
    let mut world=World::with_seed(WorldType::Superflat,VoxelScalePreset::OneMeter,7);
    let original=world.snapshot();exact_test_sources(&mut cache,HashSet::from([IVec3::ZERO]));
    s.invalidate_changed_sources(&original,&mut cache);
    let key=ChunkKey{coord:IVec3::ZERO,lod:0};
    let mut empty=cpu(&s,0,world.revision(),0,3);empty.mesh=ChunkMesh::empty();empty.lod_invariant=true;
    s.ready_meshes.push_back(empty);
    let old_cancel=Arc::new(AtomicBool::new(false));s.meshing.insert(key,(42,old_cancel.clone()));
    let mut changed=original.clone();Arc::make_mut(&mut changed.chunks).insert(IVec3::ZERO,
        Arc::new(engine_worldgen::generate(&changed.cfg,IVec3::ZERO)));
    s.invalidate_changed_sources(&changed,&mut cache);
    assert!(s.ready_meshes.is_empty());assert!(old_cancel.load(Ordering::Relaxed));assert!(!s.meshing.contains_key(&key));
    let late=cpu(&s,0,world.revision(),42,3);
    let (tx,rx)=mpsc::channel();s.mesh_result_rx=rx;
    let new_cancel=Arc::new(AtomicBool::new(false));s.meshing.insert(key,(43,new_cancel.clone()));
    assert!(tx.send(late).is_ok());s.collect_finished_meshes();
    assert!(s.ready_meshes.is_empty());assert_eq!(s.meshing[&key].0,43);assert!(!new_cancel.load(Ordering::Relaxed));
    assert_eq!(cache.gpu_meshes[&IVec3::ZERO].lod,2);
}

#[test]
#[ignore="actual RTX4090 fixture; first exact owner must not use old cached handles"]
fn local_source_enrollment_precedes_first_quality_column_admission() {
    use crate::{fluid::VoxelScalePreset,world::{World,WorldType}};
    let(device,queue,mut cache,mut s)=setup();
    let mut world=World::with_seed(WorldType::Superflat,VoxelScalePreset::OneMeter,7);let snapshot=world.snapshot();
    let column=(5,0);cache.column_y_cache.begin(snapshot.cfg);
    cache.column_y_cache.set_supplemental_interest(HashSet::from([column]));
    let deadline=Instant::now()+Duration::from_secs(5);
    let range=loop {
        cache.column_y_cache.begin(snapshot.cfg);
        if let Some(range)=cache.column_y_cache.range(column){break range;}
        assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(1));
    };
    let members:Vec<_>=(range.0..=range.1).map(|y|IVec3::new(column.0,y,column.1)).collect();
    for &q in &members {
        let mut c=cpu(&s,0,world.revision(),1,3);c.key.coord=q;
        cache.install_mesh(q,uploaded(&device,&queue,&s,&c));
    }
    assert!(!cache.published_columns.contains_key(&column));
    cache.set_quality_columns(&snapshot,&[column],false,&mut s);
    assert!(!cache.published_columns.contains_key(&column),"cached handles cannot establish new source ownership before observation");
    let desired:Vec<_>=cache.quality_proxies.coords.iter().copied().collect();assert!(!desired.is_empty());
    assert!(desired.iter().all(|q|cache.dirty_meshes.contains(q)));
    for &q in &desired {
        let mut c=cpu(&s,0,world.revision(),2,3);c.key.coord=q;
        cache.install_mesh(q,uploaded(&device,&queue,&s,&c));
    }
    cache.set_quality_columns(&snapshot,&[column],false,&mut s);
    assert!(cache.published_columns.contains_key(&column),"fresh complete column must make progress");
}
