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
