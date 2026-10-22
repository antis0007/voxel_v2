use crate::*;
use engine_core::fast_hash::FastMap;
use glam::IVec3;
use proptest::prelude::*;

fn local() -> impl Strategy<Value = IVec3> {
    (0..32, 0..32, 0..32).prop_map(|(x, y, z)| IVec3::new(x, y, z))
}

fn chunk() -> impl Strategy<Value = IVec3> {
    (-2..2, -2..2, -2..2).prop_map(|(x, y, z)| IVec3::new(x, y, z))
}

/// Few distinct values, so repeated and `EMPTY` writes happen often.
fn cell() -> impl Strategy<Value = Cell> {
    (0u8..3, 0u8..3, 0u8..2).prop_map(|(material, volume, hot)| Cell {
        material,
        volume,
        temperature: if hot == 0 { AMBIENT_DECIKELVIN } else { 3000 },
        pressure: 0,
        charge: 0,
    })
}

#[test]
fn every_local_cell_has_one_brick_and_cell_index_and_back() {
    let mut seen = vec![false; CHUNK_CELLS];
    for y in 0..32 {
        for z in 0..32 {
            for x in 0..32 {
                let p = IVec3::new(x, y, z);
                let (brick, c) = brick_and_cell(p);
                assert!((brick as usize) < BRICKS_PER_CHUNK && c < BRICK_CELLS);
                let flat = brick as usize * BRICK_CELLS + c;
                assert!(!seen[flat], "{p} collides");
                seen[flat] = true;
                assert_eq!(local_of(brick, c), p);
            }
        }
    }
}

#[test]
fn a_chunk_has_64_bricks_of_4_kib() {
    assert_eq!(BRICKS_PER_CHUNK, 64, "one dirty bit per brick in a u64");
    assert_eq!(BRICK_CELLS * size_of::<Cell>(), 4096);
}

proptest! {
    #[test]
    fn split_voxel_floors_and_reassembles(x in -(1 << 30)..(1 << 30), y in -(1 << 30)..(1 << 30), z in -(1 << 30)..(1 << 30)) {
        let v = IVec3::new(x, y, z);
        let (chunk, local) = split_voxel(v);
        prop_assert!(local.cmpge(IVec3::ZERO).all() && local.cmplt(IVec3::splat(32)).all());
        prop_assert_eq!(chunk * 32 + local, v);
    }

    /// Whatever sequence of writes, reads agree with a plain map, the pool
    /// bookkeeping holds, and exactly the bricks whose cells changed are dirty.
    #[test]
    fn writes_match_a_map_model(ops in prop::collection::vec((chunk(), local(), cell()), 0..200)) {
        let mut store = BrickStore::new(1000);
        let mut model: FastMap<(IVec3, IVec3), Cell> = FastMap::default();
        let mut expected_dirty: FastMap<IVec3, u64> = FastMap::default();
        for (c, l, value) in &ops {
            let before = model.get(&(*c, *l)).copied().unwrap_or(Cell::EMPTY);
            if before != *value {
                *expected_dirty.entry(*c).or_default() |= 1 << brick_and_cell(*l).0;
            }
            model.insert((*c, *l), *value);
            store.set(*c, *l, *value).unwrap();
        }
        store.check_invariants();
        for ((c, l), value) in &model {
            prop_assert_eq!(store.get(*c, *l), *value);
        }
        for (c, mask) in expected_dirty {
            prop_assert_eq!(store.take_dirty(c), mask);
            prop_assert_eq!(store.take_dirty(c), 0, "taking clears");
        }
        prop_assert_eq!(store.active_bricks(), 0);
    }

    /// Freeing what is empty never loses a value, and leaves nothing empty stored.
    #[test]
    fn trim_keeps_values_and_frees_empty_bricks(ops in prop::collection::vec((local(), cell()), 0..100)) {
        let mut store = BrickStore::new(100);
        let origin = IVec3::new(-1, 0, 3);
        for (l, v) in &ops {
            store.set(origin, *l, *v).unwrap();
        }
        let before: Vec<Cell> = ops.iter().map(|(l, _)| store.get(origin, *l)).collect();
        let stored = store.bricks_in_use();
        let freed = store.trim(origin);
        store.check_invariants();
        prop_assert_eq!(store.bricks_in_use(), stored - freed);
        for ((l, _), want) in ops.iter().zip(before) {
            prop_assert_eq!(store.get(origin, *l), want);
        }
        prop_assert_eq!(store.trim(origin), 0, "nothing left to free");
    }
}

#[test]
fn writing_the_current_value_wakes_nothing_and_empty_into_nothing_allocates_nothing() {
    let mut store = BrickStore::new(4);
    let at = IVec3::new(5, 6, 7);
    store.set(IVec3::ZERO, at, Cell::EMPTY).unwrap();
    assert_eq!((store.bricks_in_use(), store.chunk_count(), store.active_bricks()), (0, 0, 0));
    store.set(IVec3::ZERO, at, Cell::filled(3)).unwrap();
    assert_eq!(store.take_dirty(IVec3::ZERO).count_ones(), 1);
    store.set(IVec3::ZERO, at, Cell::filled(3)).unwrap();
    assert_eq!(store.active_bricks(), 0, "same value: the brick stays idle");
}

#[test]
fn a_full_pool_refuses_and_changes_nothing() {
    let mut store = BrickStore::new(2);
    store.set(IVec3::ZERO, IVec3::new(0, 0, 0), Cell::filled(1)).unwrap();
    store.set(IVec3::ZERO, IVec3::new(8, 0, 0), Cell::filled(1)).unwrap();
    let refused = store.set(IVec3::ZERO, IVec3::new(16, 0, 0), Cell::filled(1));
    assert_eq!(refused, Err(PoolFull));
    assert_eq!(store.get(IVec3::ZERO, IVec3::new(16, 0, 0)), Cell::EMPTY);
    store.check_invariants();
    // A fill that does not fit leaves the chunk as it was.
    let half = |p: IVec3| if p.x < 24 { Cell::filled(2) } else { Cell::EMPTY };
    assert_eq!(store.fill_chunk(IVec3::ZERO, half), Err(PoolFull));
    assert_eq!(store.get(IVec3::ZERO, IVec3::ZERO), Cell::filled(1));
    // Freeing a chunk returns its slots for reuse.
    store.remove_chunk(IVec3::ZERO);
    store.fill_chunk(IVec3::ZERO, |p| if p.y < 8 && p.z < 8 && p.x < 16 { Cell::filled(2) } else { Cell::EMPTY }).unwrap();
    assert_eq!(store.bricks_in_use(), 2);
    store.check_invariants();
}

#[test]
fn fill_chunk_stores_only_bricks_with_content_and_marks_them_dirty() {
    let mut store = BrickStore::new(64);
    let ground = |p: IVec3| if p.y < 12 { Cell::filled(1) } else { Cell::EMPTY };
    store.fill_chunk(IVec3::new(2, -1, 0), ground).unwrap();
    // y 0..12 touches brick rows 0 and 1: 2 of 4 rows, 16 bricks each.
    assert_eq!(store.bricks_in_use(), 32);
    assert_eq!(store.active_bricks(), 32);
    let slots = store.chunk_slots(IVec3::new(2, -1, 0)).unwrap();
    assert_eq!(slots.iter().filter(|s| **s != NO_SLOT).count(), 32);
    assert_eq!(store.pool_bytes().len(), 32 * 4096);
    let mut cells = 0;
    store.for_each_cell(IVec3::new(2, -1, 0), |_, c| cells += (c != Cell::EMPTY) as usize);
    assert_eq!(cells, 32 * 32 * 12);
    store.check_invariants();
}

#[test]
fn dirty_bricks_come_out_in_a_fixed_order() {
    let mut store = BrickStore::new(16);
    for chunk in [IVec3::new(1, 0, 0), IVec3::new(0, 0, 1), IVec3::new(0, 0, 0)] {
        store.set(chunk, IVec3::new(9, 0, 0), Cell::filled(1)).unwrap();
        store.set(chunk, IVec3::new(0, 0, 0), Cell::filled(1)).unwrap();
    }
    let order: Vec<(i32, i32, i32, u8)> =
        store.dirty_bricks().iter().map(|b| (b.chunk.z, b.chunk.y, b.chunk.x, b.brick)).collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted);
    assert_eq!(order.len(), 6);
    assert!(store.dirty_bricks().iter().all(|b| b.scale == Scale::FINEST));
}

#[test]
#[should_panic(expected = "outside the chunk")]
fn a_local_cell_past_the_chunk_edge_panics_instead_of_aliasing() {
    let _ = BrickStore::new(1).get(IVec3::ZERO, IVec3::new(32, 0, 0));
}

#[test]
#[should_panic(expected = "outside the chunk")]
fn a_negative_local_cell_panics_instead_of_aliasing() {
    let _ = BrickStore::new(1).get(IVec3::ZERO, IVec3::new(-1, 0, 1));
}
