use super::*;
use crate::{
    fluid::VoxelScalePreset,
    world::{BlockId, World, WorldType},
};

fn world() -> World {
    World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7)
}

#[test]
fn quality_plan_reuses_unchanged_snapshot_without_retaining_voxels() {
    let mut world = world();
    let snapshot = world.snapshot();
    let counts = (
        Arc::strong_count(&snapshot.chunks),
        Arc::strong_count(&snapshot.water_levels),
    );
    let mut inputs = Inputs::default();
    let first = inputs.observe(&snapshot);
    for _ in 0..10 {
        assert_eq!(inputs.observe(&snapshot.clone()), first);
    }
    assert_eq!(
        (
            Arc::strong_count(&snapshot.chunks),
            Arc::strong_count(&snapshot.water_levels)
        ),
        counts
    );
}

#[test]
fn quality_plan_invalidates_on_edits_outside_previous_vertical_membership() {
    let mut world = world();
    let snapshot = world.snapshot();
    let mut inputs = Inputs::default();
    let before = inputs.observe(&snapshot);
    world.set(IVec3::new(-33, 255, -65), BlockId::Stone);
    let edited = world.snapshot();
    assert_ne!(inputs.observe(&edited), before);
    let after = inputs.observe(&edited);
    let mut evicted = edited.clone();
    Arc::make_mut(&mut evicted.chunks).clear();
    assert_ne!(inputs.observe(&evicted), after);
}

#[test]
fn quality_plan_invalidates_on_cold_record_and_partial_water_publication() {
    let mut world = world();
    let mut snapshot = world.snapshot();
    let mut inputs = Inputs::default();
    let before = inputs.observe(&snapshot);
    Arc::make_mut(&mut snapshot.regions.records).insert(
        IVec3::new(-2, 12, -3),
        crate::world::regions::controlled_source_record(1),
    );
    assert_ne!(inputs.observe(&snapshot), before);
    let before_water = inputs.observe(&snapshot);
    snapshot.water_levels = Arc::new(WaterSnapshot::default());
    assert_ne!(inputs.observe(&snapshot), before_water);
}

#[test]
fn quality_plan_invalidates_equal_path_world_replacement_and_configuration() {
    let mut world = world();
    let mut snapshot = world.snapshot();
    let mut inputs = Inputs::default();
    let before = inputs.observe(&snapshot);
    snapshot.regions.root = Arc::new((*snapshot.regions.root).clone());
    assert_ne!(inputs.observe(&snapshot), before);
    let before_cfg = inputs.observe(&snapshot);
    snapshot.cfg.seed += 1;
    assert_ne!(inputs.observe(&snapshot), before_cfg);
}

#[test]
fn quality_plan_weak_roots_do_not_reuse_an_expired_snapshot() {
    let mut world = world();
    let mut snapshot = world.snapshot();
    snapshot.chunks = Arc::new((*snapshot.chunks).clone());
    let old = SnapshotIdentity::capture(&snapshot);
    snapshot.chunks = Arc::new((*snapshot.chunks).clone());
    assert!(old.chunks.upgrade().is_none());
    assert!(!old.matches(&snapshot));
}
