use super::*;
use crate::world::WorldType;
use engine_core::{CHUNK_SIZE, VoxelScalePreset};
use glam::Vec2;

fn world() -> World {
    World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 31)
}

fn air() -> Arc<ChunkData> {
    Arc::new(ChunkData {
        modified_bounds: None,
        structural_bounds: None,
        blocks: Box::new([BlockId::Air; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize]),
        light: Box::new([0; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize]),
        torches: HashSet::new(),
    })
}

fn commit(world: &mut World, staged: Vec<StagedChunk>) -> (HashSet<IVec3>, HashSet<IVec3>) {
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    publish(world, staged, &mut blocks, &mut chunks).unwrap();
    (blocks, chunks)
}

#[test]
fn staging_leaves_world_and_existing_snapshot_unchanged() {
    let mut world = world();
    let p = IVec3::new(-1, 95, -1);
    let coord = chunk_coord_from_world_block(p);
    let base = air();
    world.chunks.insert(coord, Arc::clone(&base));
    let snapshot = world.snapshot();
    let revision = world.revision();
    let mut staged = StagedChunk::new(coord, Arc::clone(&base));
    staged.edit(p, BlockId::Stone);
    staged.edit(p - IVec3::X, BlockId::Torch);

    assert_eq!(world.get(p), BlockId::Air);
    assert_eq!(snapshot.get(p), BlockId::Air);
    assert!(Arc::ptr_eq(world.chunks.get(&coord).unwrap(), &base));
    assert!(base.torches.is_empty());
    assert!(base.modified_bounds.is_none());
    assert!(world.changed_cells.is_empty());
    assert_eq!(world.revision(), revision);
    assert_eq!(world.regions.source_version(coord), None);
    assert!(world.full_snapshot_cache.is_some());

    let (blocks, chunks) = commit(&mut world, vec![staged]);
    assert_eq!(world.get(p), BlockId::Stone);
    assert_eq!(world.get(p - IVec3::X), BlockId::Torch);
    assert_eq!(snapshot.get(p), BlockId::Air);
    assert_eq!(blocks, HashSet::from([p, p - IVec3::X]));
    assert_eq!(chunks.len(), 8, "the corner includes its complete halo");
    assert_eq!(world.changed_cells, vec![p, p - IVec3::X]);
    assert_eq!(world.revision(), revision + 2);
    assert!(world.regions.source_version(coord).is_some());
    assert!(world.full_snapshot_cache.is_none());
    let data = world.chunks.get(&coord).unwrap();
    assert!(data.torches.contains(&(p - IVec3::X)));
    assert_eq!(data.modified_bounds, Some((IVec3::new(30, 31, 31), IVec3::splat(32))));
    assert_eq!(data.structural_bounds, data.modified_bounds);
}

#[test]
fn every_replacement_is_installed_before_cross_chunk_relighting() {
    let mut world = world();
    let left = IVec3::new(0, 2, 0);
    let right = left + IVec3::X;
    let torch = left * CHUNK_SIZE + IVec3::new(31, 16, 16);
    let near = torch + IVec3::X;
    let base = air();
    world.chunks.insert(left, Arc::clone(&base));
    world.chunks.insert(right, Arc::clone(&base));
    let mut a = StagedChunk::new(left, Arc::clone(&base));
    let mut b = StagedChunk::new(right, base);
    a.edit(torch, BlockId::Torch);
    b.edit(near + IVec3::new(10, 10, 10), BlockId::Water);
    commit(&mut world, vec![a, b]);
    assert_eq!(block_light::level_at(&world, torch), 14);
    assert_eq!(block_light::level_at(&world, near), 13);
    assert_eq!(world.revision(), 2);
}

#[test]
fn publish_cleans_removed_water_and_preserves_unchanged_authoritative_water() {
    let mut world = world();
    let coord = IVec3::new(0, 2, 0);
    let p = coord * CHUNK_SIZE + IVec3::splat(8);
    let q = p + IVec3::X;
    let mut base = air();
    for cell in [p, q] {
        Arc::make_mut(&mut base).blocks[ChunkData::local_index(cell.x, cell.y, cell.z)] = BlockId::Water;
        world.water_levels.insert(cell, 3);
        world.water_levels.set_velocity(cell, Vec2::ONE);
        world.environment.water_deficit.insert(environment::key(cell), 7);
    }
    world.chunks.insert(coord, Arc::clone(&base));
    let mut staged = StagedChunk::new(coord, base);
    staged.edit(p, BlockId::Torch);
    staged.edit(q, BlockId::Water);
    assert_eq!(world.water_levels.get(&p), Some(&3));
    commit(&mut world, vec![staged]);

    assert_eq!(world.water_levels.get(&p), None);
    assert_eq!(world.water_levels.velocity(p), Vec2::ZERO);
    assert!(!world.environment.water_deficit.contains_key(&environment::key(p)));
    assert_eq!(world.water_levels.get(&q), Some(&3));
    assert_eq!(world.water_levels.velocity(q), Vec2::ONE);
    assert_eq!(world.environment.water_deficit.get(&environment::key(q)), Some(&7));
    assert!(world.chunks.get(&coord).unwrap().torches.contains(&p));

    let mut staged = StagedChunk::new(coord, Arc::clone(world.chunks.get(&coord).unwrap()));
    staged.edit(p, BlockId::Air);
    commit(&mut world, vec![staged]);
    assert!(!world.chunks.get(&coord).unwrap().torches.contains(&p));
    assert_eq!(block_light::level_at(&world, p), 0);
}

#[test]
fn unchanged_resident_chunk_preserves_all_bookkeeping() {
    let mut world = world();
    let coord = IVec3::new(0, 2, 0);
    let base = air();
    world.chunks.insert(coord, Arc::clone(&base));
    world.snapshot();
    let mut staged = StagedChunk::new(coord, Arc::clone(&base));
    staged.edit(coord * CHUNK_SIZE, BlockId::Air);
    let (blocks, chunks) = commit(&mut world, vec![staged]);
    assert!(blocks.is_empty() && chunks.is_empty());
    assert!(world.changed_cells.is_empty());
    assert_eq!(world.revision(), 0);
    assert!(Arc::ptr_eq(world.chunks.get(&coord).unwrap(), &base));
    assert!(world.full_snapshot_cache.is_some());
    assert_eq!(world.regions.source_version(coord), None);
}

#[test]
fn missing_unchanged_chunk_invalidates_snapshot_without_importing_water() {
    let mut world = world();
    let coord = IVec3::new(0, 2, 0);
    let p = coord * CHUNK_SIZE;
    world.water_levels.insert(p, 2);
    world.environment.water_deficit.insert(environment::key(p), 5);
    let mut base = air();
    Arc::make_mut(&mut base).blocks[0] = BlockId::Water;
    world.snapshot();
    let mut staged = StagedChunk::new(coord, base);
    staged.edit(p, BlockId::Water);
    let (blocks, chunks) = commit(&mut world, vec![staged]);
    assert!(world.chunks.contains_key(&coord));
    assert!(world.full_snapshot_cache.is_none());
    assert_eq!(world.snapshot().get(p), BlockId::Water);
    assert_eq!(world.water_levels.get(&p), Some(&2));
    assert_eq!(world.environment.water_deficit.get(&environment::key(p)), Some(&5));
    assert!(blocks.is_empty());
    assert_eq!(chunks, HashSet::from([coord]));
    assert_eq!(world.revision(), 0);
    assert!(world.regions.is_dirty(coord), "inserted chunks need recoverable writeback ownership");
    assert!(world.regions.source_version(coord).is_some());
    assert!(world.simulation_writeback_pending());
}

#[test]
fn repeated_edits_keep_intermediate_water_cleanup_and_revision_count() {
    let mut world = world();
    let coord = IVec3::new(0, 2, 0);
    let p = coord * CHUNK_SIZE;
    let mut base = air();
    Arc::make_mut(&mut base).blocks[0] = BlockId::Water;
    world.chunks.insert(coord, Arc::clone(&base));
    world.water_levels.insert(p, 2);
    world.environment.water_deficit.insert(environment::key(p), 5);
    let mut staged = StagedChunk::new(coord, base);
    staged.edit(p, BlockId::Air);
    staged.edit(p, BlockId::Water);
    staged.edit(p, BlockId::Water);
    let (blocks, _) = commit(&mut world, vec![staged]);
    assert_eq!(world.get(p), BlockId::Water);
    assert_eq!(world.water_levels.get(&p), None);
    assert!(!world.environment.water_deficit.contains_key(&environment::key(p)));
    assert_eq!(world.changed_cells, vec![p, p]);
    assert_eq!(world.revision(), 2);
    assert_eq!(blocks, HashSet::from([p]));
}

#[test]
fn byte_estimate_includes_private_cell_light_torch_and_edit_allocations() {
    let coord = IVec3::new(0, 2, 0);
    let base = air();
    let payload = base.blocks.len() * size_of::<BlockId>() + base.light.len();
    let mut staged = StagedChunk::new(coord, base);
    let before = staged.bytes();
    assert!(before > payload);
    staged.edit(coord * CHUNK_SIZE, BlockId::Torch);
    let extra = staged.data.torches.capacity() * (size_of::<IVec3>() + 1) * 2
        + staged.changed.capacity() * size_of::<(IVec3, BlockId)>();
    assert_eq!(staged.bytes(), before + extra);
    assert!(extra > 0);
}

#[path = "admission_tests.rs"]
mod admission;
