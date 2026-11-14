use super::*;
use crate::fluid::VoxelScalePreset;

fn world(tag: &str) -> World {
    let base=std::env::temp_dir().join(format!("axiom-interest-memo-{}-{}-{tag}",std::process::id(),checkpoint_id()));
    let mut w=World::with_seed_in(super::super::WorldType::Superflat,VoxelScalePreset::OneMeter,31,&base);
    w.regions.retry_after=Instant::now()+Duration::from_secs(3600);w
}
// Frozen original setter, including overflow and set-equality invalidation rules.
fn original(w: &mut World,sources: &[IVec3]) {
    let mut wanted=HashSet::new();
    for &q in sources {
        for y in -1..=1 {for z in -1..=1 {for x in -1..=1 {
            if let (Some(px),Some(py),Some(pz))=(q.x.checked_add(x),q.y.checked_add(y),q.z.checked_add(z)) {
                wanted.insert(IVec3::new(px,py,pz));
            }
        }}}
    }
    if w.regions.visual_interest!=wanted {
        w.regions.visual_interest=wanted;w.regions.last_plan=None;w.regions.last_eviction=None;w.snapshot_revision=u64::MAX;
    }
}
fn markers(w:&mut World) {
    w.regions.last_plan=Some((IVec3::ZERO,2,3,4));
    w.regions.last_eviction=Some((IVec3::ZERO,2,3,4,5));w.snapshot_revision=17;
}
#[test]
fn exact_input_matches_frozen_setter_and_invalidation() {
    let mut a=world("a");let mut b=world("b");
    assert!(a.regions.visual_interest_input.is_none());
    let x=IVec3::new(8,2,-9);let y=IVec3::new(-13,4,6);
    let cases=vec![vec![],vec![],vec![x],vec![x],vec![x,x],vec![x,y],vec![y,x],vec![y,x],
        vec![IVec3::splat(i32::MAX),IVec3::splat(i32::MIN),IVec3::new(i32::MAX,0,i32::MIN)],vec![]];
    for sources in cases {
        markers(&mut a);markers(&mut b);a.set_visual_region_interest(&sources);original(&mut b,&sources);
        assert_eq!(a.regions.visual_interest,b.regions.visual_interest);
        assert_eq!(a.regions.last_plan,b.regions.last_plan);assert_eq!(a.regions.last_eviction,b.regions.last_eviction);
        assert_eq!(a.snapshot_revision,b.snapshot_revision);
        assert_eq!(a.regions.visual_interest_input.as_deref(),Some(sources.as_slice()));
        let ptr=a.regions.visual_interest_input.as_ref().unwrap().as_ptr();
        a.set_visual_region_interest(&sources);assert_eq!(a.regions.visual_interest_input.as_ref().unwrap().as_ptr(),ptr);
    }
    // Opening another stream cannot inherit a previous world's memo.
    let reopened=RegionStream::open(std::env::temp_dir().join(format!("axiom-interest-reopen-{}",checkpoint_id())),HashMap::new());
    assert!(reopened.visual_interest_input.is_none());assert!(reopened.visual_interest.is_empty());
}
#[test]
fn interest_memo_keeps_old_snapshot_readers_and_releases_removed_sources() {
    let mut w=world("snapshots");let q=IVec3::new(90,2,90);
    w.chunks.insert(q,Arc::new(ChunkData {modified_bounds:None,structural_bounds:None,
        blocks:Box::new([BlockId::Air;VOXELS]),light:Box::new([0;VOXELS]),torches:HashSet::new()}));
    let center=IVec3::new(0,2,0);let before=w.snapshot_near(center,2);assert!(!before.chunks.contains_key(&q));
    w.set_visual_region_interest(&[q]);let present=w.snapshot_near(center,2);assert!(present.chunks.contains_key(&q));
    w.set_visual_region_interest(&[q]);let repeated=w.snapshot_near(center,2);assert!(Arc::ptr_eq(&present.chunks,&repeated.chunks));
    w.set_visual_region_interest(&[]);let removed=w.snapshot_near(center,2);
    assert!(!removed.chunks.contains_key(&q));assert!(present.chunks.contains_key(&q));assert!(!before.chunks.contains_key(&q));
}
