use super::*;
use crate::fluid::VoxelScalePreset;
use crate::world::{World, WorldType};

#[test]
fn visible_set_is_empty_at_radius_zero_and_full_at_view_distance() {
    let mut cache = ChunkCache::new(IVec3::ZERO);
    let snapshot = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 74).snapshot();
    cache.update_center_chunk_with_snapshot_radius(IVec3::ZERO, &snapshot, 0);
    assert!(cache.visible_keys.is_empty());
    cache.update_center_chunk_with_snapshot_radius(IVec3::ZERO, &snapshot, 4);
    assert!(cache.visible_keys.iter().all(|k| (k.coord - IVec3::ZERO).abs().max_element() <= 4));
    assert!(!cache.visible_keys.is_empty());
}

#[test]
fn frozen_cache_keeps_meshes_under_pressure() {
    let mut cache = ChunkCache::new(IVec3::ZERO);
    let far = IVec3::new(9, 0, 9);
    cache.freeze(true);
    assert!(cache.frozen());
    cache.residency_limits.1 = 1;
    cache.install_test_mesh(far, 4096);
    cache.evict_meshes_over_budget();
    assert!(cache.gpu_meshes.contains_key(&far));
    cache.center_chunk = IVec3::new(500, 0, 500);
    cache.retain_visible();
    assert!(cache.gpu_meshes.contains_key(&far), "a frozen cache keeps out-of-range meshes");
    cache.freeze(false);
    cache.evict_meshes_over_budget();
    assert!(!cache.gpu_meshes.contains_key(&far));
}

#[test]
fn cap_rebuilds_only_on_two_ring_steps_or_the_ends() {
    assert!(!cap_changes(28, 27, 32));
    assert!(cap_changes(28, 26, 32));
    assert!(cap_changes(1, 0, 32));
    assert!(cap_changes(27, 28, 28), "reaching the view distance applies at once");
    assert!(!cap_changes(28, 28, 32));
}

#[test]
fn one_ring_cap_step_keeps_the_visible_set() {
    let mut cache = ChunkCache::new(IVec3::ZERO);
    let snapshot = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 74).snapshot();
    // Caps at or below the minimum view distance (4) are never clamped.
    assert!(cache.update_center_chunk_with_snapshot_radius(IVec3::ZERO, &snapshot, 3));
    let before = cache.visible.clone();
    assert!(!cache.update_center_chunk_with_snapshot_radius(IVec3::ZERO, &snapshot, 2));
    assert_eq!(cache.visible, before);
    assert!(cache.update_center_chunk_with_snapshot_radius(IVec3::ZERO, &snapshot, 1));
    assert!(cache.update_center_chunk_with_snapshot_radius(IVec3::ZERO, &snapshot, 0));
    assert!(cache.visible_keys.is_empty());
}

#[test]
fn frozen_cache_keeps_lod_meshes() {
    let mut cache = ChunkCache::new(IVec3::ZERO);
    cache.freeze(true);
    cache.residency_limits.1 = 1;
    let before = cache.lod_meshes.bytes();
    cache.lod_meshes.remember(IVec3::new(9, 0, 9), ChunkGpuMesh::test_mesh(3, 1, false, 4096));
    cache.trim_lod_budget();
    assert_eq!(cache.lod_meshes.bytes(), before + 4096);
    cache.freeze(false);
    cache.trim_lod_budget();
    assert_eq!(cache.lod_meshes.bytes(), 0);
}
