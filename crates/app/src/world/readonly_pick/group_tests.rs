use super::*;
use crate::{fluid::VoxelScalePreset, world::{regions, ChunkData}};
use std::{path::PathBuf, time::{Duration, Instant}};
fn fixture() -> World {
    let root=std::env::var_os("AXIOM_PICK_TEST_ROOT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
        .join(format!("axiom-group-read-{}-{}",std::process::id(),regions::checkpoint_id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut world=World::with_seed_in(WorldType::Superflat,VoxelScalePreset::OneMeter,7,&root);
    world.generator_version=287;world
}
fn fake_lane(picker:&mut ReadOnlyPicker, world:&World)
    -> (mpsc::Receiver<Job>,mpsc::SyncSender<Done>) {
    picker.prepare(world).unwrap();
    let (tx,jobs)=mpsc::sync_channel(1);let (done,rx)=mpsc::sync_channel(1);
    picker.tx=Some(tx);picker.rx=rx;(jobs,done)
}
#[test]
fn groups_preserve_order_duplicates_and_current_resident_authority() {
    let mut world=fixture();let mut picker=ReadOnlyPicker::default();picker.prepare(&world).unwrap();
    let a=IVec3::new(-1,8,0);let b=IVec3::new(32,8,0);
    picker.cache.insert(Key::at_position(&world,a),Arc::from([BlockId::Stone]));
    picker.cache.insert(Key::at_position(&world,b),Arc::from([BlockId::Dirt]));
    assert_eq!(picker.read_group(&world,&[b,a,b]),Readiness::Ready(vec![BlockId::Dirt,BlockId::Stone,BlockId::Dirt]));
    world.chunks.insert(chunk_coord_from_world_block(a),Arc::new(ChunkData {
        blocks:Box::new([BlockId::Water;32*32*32]),light:Box::new([0;32*32*32]),
        torches:Default::default(),modified_bounds:None,structural_bounds:None}));
    assert_eq!(picker.read_group(&world,&[a,b]),Readiness::Ready(vec![BlockId::Water,BlockId::Dirt]));
    assert_eq!(picker.read_group(&world,&[]),Readiness::Ready(Vec::new()));
}
#[test]
fn stable_union_deduplicates_and_unrelated_consumers_do_not_cancel() {
    let world=fixture();let mut picker=ReadOnlyPicker::default();let (jobs,_done)=fake_lane(&mut picker,&world);
    let a=IVec3::new(0,8,0);let b=IVec3::new(1,8,0);let c=IVec3::new(80,8,0);
    assert_eq!(picker.lookup_ready(&world,c),Readiness::Pending);assert!(jobs.try_recv().is_err());
    assert_eq!(picker.read_group(&world,&[b,a,b]),Readiness::Pending);
    let job=jobs.try_recv().unwrap();assert_eq!(job.keys,vec![Key::at_position(&world,b),Key::at_position(&world,a)]);
    assert_eq!(picker.read_ready(&world,c),Readiness::Pending);
    assert_eq!(picker.poll(&world,DVec3::new(80.5,8.5,0.5),-Vec3::Y,6.0),Readiness::Pending);
    assert_eq!(picker.read_group(&world,&[c,a]),Readiness::Pending);
    assert!(!job.cancel.load(Ordering::Relaxed));assert!(jobs.try_recv().is_err());
    picker.reset();assert!(job.cancel.load(Ordering::Relaxed));assert!(picker.pending.is_some());
}
#[test]
fn speculative_failure_does_not_block_missing_work_or_unrelated_reads() {
    let world=fixture();let mut picker=ReadOnlyPicker::default();let (jobs,_done)=fake_lane(&mut picker,&world);
    let a=IVec3::new(0,8,0);let bad=IVec3::new(1,8,0);let known=IVec3::new(2,8,0);
    picker.failure=Some((Key::at_position(&world,bad),"corrupt".into()));
    picker.cache.insert(Key::at_position(&world,known),Arc::from([BlockId::Stone]));
    assert_eq!(picker.prepare_group(&world,&[bad,a,bad]),Ok(()));
    let job=jobs.try_recv().unwrap();assert_eq!(job.keys,vec![Key::at_position(&world,a)]);
    assert_eq!(picker.lookup_ready(&world,known),Readiness::Ready(BlockId::Stone));
    assert_eq!(picker.lookup_ready(&world,bad),Readiness::Failed("corrupt".into()));
    assert_eq!(picker.read_group(&world,&[known,bad]),Readiness::Failed("corrupt".into()));
    assert!(!job.cancel.load(Ordering::Relaxed));
}
#[test]
fn group_budgets_reject_before_enqueue_and_stored_cells_share_one_source() {
    let mut world=fixture();let mut picker=ReadOnlyPicker::default();let (jobs,_done)=fake_lane(&mut picker,&world);
    assert!(matches!(picker.read_group(&world,&vec![IVec3::ZERO;513]),Readiness::Failed(_)));
    let mut points=Vec::new();
    for x in 0..65 {let c=IVec3::new(x,0,0);Arc::make_mut(&mut world.regions.view.records).insert(c,regions::controlled_source_record(x as u64+1));points.push(c*32+IVec3::Y*8);}
    assert!(picker.prepare_group(&world,&points).unwrap_err().contains("footprint"));assert!(jobs.try_recv().is_err());
    assert_eq!(picker.prepare_group(&world,&[points[0],points[0]+IVec3::X,points[1]]),Ok(()));
    let job=jobs.try_recv().unwrap();assert_eq!(job.keys.len(),2);assert!(job.keys.iter().all(|k|k.record.is_some()));
}
#[test]
fn stale_group_completion_is_rejected_after_configuration_change() {
    let mut world=fixture();let mut picker=ReadOnlyPicker::default();let (jobs,done)=fake_lane(&mut picker,&world);
    let a=IVec3::new(0,8,0);picker.prepare_group(&world,&[a]).unwrap();let job=jobs.try_recv().unwrap();
    world.seed+=1;
    done.send(Done {generation:job.generation,items:vec![(job.keys[0].clone(),Ok(Arc::from([BlockId::Stone])))],work:Default::default()}).unwrap();
    assert_eq!(picker.lookup_ready(&world,a),Readiness::Pending);assert!(picker.cache.is_empty());
    assert!(job.cancel.load(Ordering::Relaxed));
}
#[test]
fn actual_alternating_groups_finish_exactly_without_installation() {
    let world=fixture();let mut picker=ReadOnlyPicker::default();
    let groups=[vec![IVec3::new(-1,8,0),IVec3::new(0,0,0)],vec![IVec3::new(40,8,0),IVec3::new(40,0,0)]];
    let expected:Vec<Vec<BlockId>>=groups.iter().map(|g|g.iter().map(|&p|world.get(p)).collect()).collect();
    let revision=world.revision();let records=world.regions.view.records.len();let resident=world.chunks.len();
    let end=Instant::now()+Duration::from_secs(15);let mut ready=[false;2];
    while !ready.iter().all(|x|*x) {
        for (i,g) in groups.iter().enumerate() {match picker.read_group(&world,g) {
            Readiness::Ready(values)=>{assert_eq!(values,expected[i]);ready[i]=true;},
            Readiness::Pending=>{},Readiness::Failed(e)=>panic!("{e}"),
        }}
        assert!(Instant::now()<end);std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(world.revision(),revision);assert_eq!(world.chunks.len(),resident);
    assert_eq!(world.regions.view.records.len(),records);assert!(!world.simulation_writeback_pending());
}

#[test]
fn actual_alternating_rays_and_point_reads_make_progress_with_current_hits() {
    let world=fixture();let mut picker=ReadOnlyPicker::default();
    let origins=[DVec3::new(0.5,8.5,0.5),DVec3::new(80.5,8.5,0.5)];
    let expected:Vec<_>=origins.iter().map(|&o|super::super::collision::raycast_block_precise(&world,o,-Vec3::Y,6.0)
        .map(|(cell,previous)|PickHit{cell,previous,block:world.get(cell)})).collect();
    let point=IVec3::new(-40,0,0);let block=world.get(point);
    let end=Instant::now()+Duration::from_secs(15);let mut ready=[false;3];
    while !ready.iter().all(|x|*x) {
        for (i,&origin) in origins.iter().enumerate() {match picker.poll(&world,origin,-Vec3::Y,6.0) {
            Readiness::Ready(hit)=>{assert_eq!(hit,expected[i]);ready[i]=true;},
            Readiness::Pending=>{},Readiness::Failed(e)=>panic!("{e}"),
        }}
        match picker.read_ready(&world,point) {
            Readiness::Ready(value)=>{assert_eq!(value,block);ready[2]=true;},
            Readiness::Pending=>{},Readiness::Failed(e)=>panic!("{e}"),
        }
        assert!(Instant::now()<end);std::thread::sleep(Duration::from_millis(1));
    }
    // Return to the first ray after the second and the point query, not the last hit.
    assert_eq!(picker.poll(&world,origins[0],-Vec3::Y,6.0),Readiness::Ready(expected[0]));
    assert!(world.chunks.is_empty());assert!(!world.simulation_writeback_pending());
}

impl ReadOnlyPicker {
    /// Holds the lane until the returned test guard is dropped.
    pub(crate) fn hold_group_for_test(&mut self, world:&World, p:IVec3) -> impl Sized + use<> {
        self.prepare(world).unwrap();
        assert!(self.pending.is_none(), "test lane must start idle");
        let (done,rx)=mpsc::sync_channel::<Done>(1);
        self.rx=rx;self.tx=None;
        self.pending=Some(Pending {keys:vec![Key::at_position(world,p)],generation:self.generation,
            cancel:Arc::new(AtomicBool::new(false))});
        done
    }
}
