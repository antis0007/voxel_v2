use super::*;

fn limited(
    world: &mut World,
    staged: Vec<StagedChunk>,
    byte_limit: usize,
    dirty_limit: usize,
) -> anyhow::Result<()> {
    publish_with_admission(world, staged, &mut HashSet::new(), &mut HashSet::new(),
        byte_limit, |world, additional| {
            // These fixtures retain every dirty chunk and never dispatch writeback.
            let dirty = world.chunks.keys().filter(|&&c| world.regions.is_dirty(c)).count();
            additional == 0 || dirty + additional <= dirty_limit
        })
}

#[test]
fn successive_frames_cannot_exceed_aggregate_resident_limit() {
    let mut world = world();
    let unit = chunk_bytes(&air());
    let first = IVec3::new(0, 2, 0);
    limited(&mut world, vec![StagedChunk::new(first, air())], unit, usize::MAX).unwrap();
    assert_eq!(world.edited_resident_bytes(), unit);
    for x in 1..=3 {
        let c = first + IVec3::new(x, 0, 0);
        let error = limited(&mut world, vec![StagedChunk::new(c, air())], unit, usize::MAX).unwrap_err();
        assert!(error.to_string().contains("resident allocation"));
        assert!(!world.chunks.contains_key(&c));
        assert_eq!(world.edited_resident_bytes(), unit);
    }
    assert_eq!(world.chunks.len(), 1);
}

#[test]
fn aggregate_memory_refusal_preserves_every_chunk_and_publication_set() {
    let mut world = world();
    let coord = IVec3::new(0, 2, 0);
    let base = air();
    let unit = chunk_bytes(&base);
    world.chunks.insert(coord, Arc::clone(&base));
    let p = coord * CHUNK_SIZE;
    world.water_levels.insert(p, 3);
    world.environment.water_deficit.insert(environment::key(p), 7);
    world.snapshot();
    let limit = world.edited_resident_bytes();
    let mut edited = StagedChunk::new(coord, Arc::clone(&base));
    edited.edit(p, BlockId::Stone);
    let new_coord = coord + IVec3::X;
    let staged = vec![edited, StagedChunk::new(new_coord, air())];
    let sentinel = IVec3::splat(-10);
    let mut blocks = HashSet::from([sentinel]);
    let mut chunks = HashSet::from([sentinel]);
    assert!(publish_with_admission(&mut world, staged, &mut blocks, &mut chunks,
        limit, |_, _| true).is_err());
    assert_eq!(blocks, HashSet::from([sentinel]));
    assert_eq!(chunks, HashSet::from([sentinel]));
    assert!(Arc::ptr_eq(world.chunks.get(&coord).unwrap(), &base));
    assert!(!world.chunks.contains_key(&new_coord));
    assert_eq!(world.chunks.allocation_bytes(), unit);
    assert_eq!(world.water_levels.get(&p), Some(&3));
    assert_eq!(world.environment.water_deficit.get(&environment::key(p)), Some(&7));
    assert_eq!(world.revision(), 0);
    assert!(world.changed_cells.is_empty());
    assert!(world.full_snapshot_cache.is_some());
    assert!(!world.regions.is_dirty(coord));
}

#[test]
fn replacements_receive_allocation_credit_without_charging_staged_metadata() {
    let mut world = world();
    let coord = IVec3::new(0, 2, 0);
    let base = air();
    let unit = chunk_bytes(&base);
    world.chunks.insert(coord, Arc::clone(&base));
    let mut staged = StagedChunk::new(coord, base);
    staged.edit(coord * CHUNK_SIZE, BlockId::Water);
    assert!(staged.bytes() > unit);
    limited(&mut world, vec![staged], unit, usize::MAX).unwrap();
    assert_eq!(world.edited_resident_bytes(), unit);
    assert_eq!(world.get(coord * CHUNK_SIZE), BlockId::Water);
}

#[test]
fn over_budget_world_allows_no_growth_but_refuses_torch_allocation_growth() {
    let mut world = world();
    let coord = IVec3::new(0, 2, 0);
    let base = air();
    let unit = chunk_bytes(&base);
    world.chunks.insert(coord, Arc::clone(&base));
    world.chunks.insert(coord + IVec3::X, Arc::clone(&base));
    let p = coord * CHUNK_SIZE;
    let mut staged = StagedChunk::new(coord, base);
    staged.edit(p, BlockId::Water);
    limited(&mut world, vec![staged], unit, usize::MAX).unwrap();
    let current = Arc::clone(world.chunks.get(&coord).unwrap());
    let mut staged = StagedChunk::new(coord, Arc::clone(&current));
    staged.edit(p, BlockId::Torch);
    assert!(limited(&mut world, vec![staged], unit, usize::MAX).is_err());
    assert!(Arc::ptr_eq(world.chunks.get(&coord).unwrap(), &current));
    assert_eq!(world.get(p), BlockId::Water);
    assert_eq!(world.revision(), 1);
}

#[test]
fn whole_batch_dirty_admission_refuses_partial_and_successive_growth() {
    let mut world = world();
    let a = IVec3::new(0, 2, 0);
    let b = a + IVec3::X;
    let error = limited(&mut world, vec![StagedChunk::new(a, air()), StagedChunk::new(b, air())],
        usize::MAX, 1).unwrap_err();
    assert!(error.to_string().contains("2 additional chunks"));
    assert!(world.chunks.is_empty());
    limited(&mut world, vec![StagedChunk::new(a, air())], usize::MAX, 1).unwrap();
    assert!(limited(&mut world, vec![StagedChunk::new(b, air())], usize::MAX, 1).is_err());
    assert!(!world.chunks.contains_key(&b));
    let mut staged = StagedChunk::new(a, Arc::clone(world.chunks.get(&a).unwrap()));
    staged.edit(a * CHUNK_SIZE, BlockId::Water);
    limited(&mut world, vec![staged], usize::MAX, 0).unwrap();
    assert_eq!(world.get(a * CHUNK_SIZE), BlockId::Water);
}

#[test]
fn editing_a_clean_resident_chunk_consumes_new_dirty_capacity() {
    let mut world = world();
    let coord = IVec3::new(0, 2, 0);
    let base = air();
    world.chunks.insert(coord, Arc::clone(&base));
    let mut staged = StagedChunk::new(coord, Arc::clone(&base));
    staged.edit(coord * CHUNK_SIZE, BlockId::Water);
    assert!(limited(&mut world, vec![staged], usize::MAX, 0).is_err());
    assert!(Arc::ptr_eq(world.chunks.get(&coord).unwrap(), &base));
    assert!(!world.regions.is_dirty(coord));
    assert_eq!(world.revision(), 0);
}

#[test]
fn no_op_insertion_gets_writeback_ownership_but_resident_no_op_needs_no_credit() {
    let mut world = world();
    let coord = IVec3::new(0, 2, 0);
    let base = air();
    limited(&mut world, vec![StagedChunk::new(coord, Arc::clone(&base))], usize::MAX, 1).unwrap();
    assert!(world.regions.is_dirty(coord));
    assert!(world.simulation_writeback_pending());
    let version = world.regions.source_version(coord);
    limited(&mut world, vec![StagedChunk::new(coord, base)], 0, 0).unwrap();
    assert_eq!(world.regions.source_version(coord), version);
    assert_eq!(world.revision(), 0);
    assert!(world.changed_cells.is_empty());
}

#[test]
fn relighting_neighbours_consume_dirty_capacity_before_publication() {
    let mut world = world();
    let a = IVec3::new(0, 2, 0);
    let b = a + IVec3::X;
    let base = air();
    world.chunks.insert(a, Arc::clone(&base));
    world.chunks.insert(b, Arc::clone(&base));
    world.regions.mark_dirty(a);
    let torch = a * CHUNK_SIZE + IVec3::new(31, 16, 16);
    let mut staged = StagedChunk::new(a, Arc::clone(&base));
    staged.edit(torch, BlockId::Torch);
    assert!(limited(&mut world, vec![staged], usize::MAX, 1).is_err());
    assert!(Arc::ptr_eq(world.chunks.get(&a).unwrap(), &base));
    assert_eq!(world.get(torch), BlockId::Air);
    assert!(!world.regions.is_dirty(b));
    assert_eq!(block_light::level_at(&world, torch + IVec3::X), 0);
}
