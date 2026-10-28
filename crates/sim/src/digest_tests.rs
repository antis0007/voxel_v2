//! The incremental digest always equals one computed from scratch, and it localises a difference.
use crate::digest::{RegionDigest, brick_hash, step_tracked};
use crate::test_support::*;
use crate::*;
use engine_core::hash::hash_u32;
use glam::IVec3;
use proptest::prelude::*;

/// Sand, water and stone over two chunks along x, on a floor.
fn scene(seed: u32) -> BrickStore {
    let mut s = BrickStore::new(4000);
    fill(&mut s, IVec3::new(0, 0, 0), IVec3::new(48, 1, 24), Cell::filled(STONE));
    for i in 0..400u32 {
        let r = hash_u32(seed ^ hash_u32(i));
        let p = IVec3::new((r % 48) as i32, 1 + ((r >> 8) % 20) as i32, ((r >> 16) % 24) as i32);
        let cell = match (r >> 28) % 4 {
            0 | 1 => Cell::filled(SAND),
            2 => water(1 + (r >> 20) as u8 % VOLUME_FULL),
            _ => Cell::filled(STONE),
        };
        put(&mut s, p, cell);
    }
    s
}

#[test]
fn an_empty_brick_hashes_to_zero_and_a_full_one_never_does() {
    let mut cells = [Cell::EMPTY; BRICK_CELLS];
    assert_eq!(brick_hash(IVec3::ZERO, 3, &cells), 0);
    cells[7] = Cell::filled(SAND);
    assert_ne!(brick_hash(IVec3::ZERO, 3, &cells), 0);
    assert_eq!(RegionDigest::of(&BrickStore::new(4)).total(), 0);
}

#[test]
fn the_same_cells_in_another_pool_layout_give_the_same_digest() {
    let a = scene(1);
    // Allocate the bricks in another order: a throwaway chunk first, removed afterwards.
    let mut b = BrickStore::new(4000);
    b.fill_chunk(IVec3::new(5, 5, 5), |_| Cell::filled(STONE)).unwrap();
    for chunk in [IVec3::new(1, 0, 0), IVec3::ZERO] {
        a.for_each_cell(chunk, |local, cell| b.set(chunk, local, cell).unwrap());
    }
    b.remove_chunk(IVec3::new(5, 5, 5));
    assert_eq!(RegionDigest::of(&a).total(), RegionDigest::of(&b).total());
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn the_same_content_somewhere_else_is_a_different_world() {
    let cells: Vec<Cell> = (0..BRICK_CELLS).map(|i| if i % 3 == 0 { Cell::filled(SAND) } else { Cell::EMPTY }).collect();
    let cells = &cells[..];
    let here = brick_hash(IVec3::ZERO, 0, cells);
    assert_ne!(here, brick_hash(IVec3::ZERO, 1, cells));
    assert_ne!(here, brick_hash(IVec3::new(0, 1, 0), 0, cells));
    assert_ne!(here, brick_hash(IVec3::new(-1, 0, 0), 0, cells));
}

#[test]
fn one_changed_cell_narrows_to_its_chunk_and_brick() {
    let t = table();
    let (mut a, mut b) = (scene(9), scene(9));
    for tick in 0..30 {
        step(&mut a, &t, 4, tick);
        step(&mut b, &t, 4, tick);
    }
    let mut db = RegionDigest::of(&b);
    let p = IVec3::new(40, 0, 3);
    put(&mut b, p, Cell::filled(SAND));
    db.observe(&b);
    db.refresh(&b);
    let da = RegionDigest::of(&a);
    assert_ne!(da.total(), db.total());
    let (chunk, local) = split_voxel(p);
    assert_eq!(da.differing_chunks(&db.chunk_sums()), vec![chunk]);
    let (ha, hb) = (da.brick_hashes(chunk), db.brick_hashes(chunk));
    let differing: Vec<usize> = (0..BRICKS_PER_CHUNK).filter(|&i| ha[i] != hb[i]).collect();
    assert_eq!(differing, vec![brick_and_cell(local).0 as usize]);
}

#[test]
fn whole_chunk_operations_are_caught_by_mark_chunk() {
    let mut s = scene(3);
    let mut d = RegionDigest::of(&s);
    s.fill_chunk(IVec3::new(1, 0, 0), |l| if l.y < 4 { Cell::filled(DIRT) } else { Cell::EMPTY }).unwrap();
    d.mark_chunk(IVec3::new(1, 0, 0));
    d.refresh(&s);
    assert_eq!(d.total(), RegionDigest::of(&s).total());
    s.remove_chunk(IVec3::ZERO);
    d.mark_chunk(IVec3::ZERO);
    d.refresh(&s);
    assert_eq!(d.total(), RegionDigest::of(&s).total());
    assert_eq!(d.chunk(IVec3::ZERO), 0);
    assert_eq!(d.chunk_sums().len(), 1);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(12))]

    /// Edits between ticks and the kernel's own writes all reach the digest, whether it refreshes every
    /// tick or only every `every` ticks.
    #[test]
    fn the_incremental_digest_matches_a_full_rehash(seed in any::<u32>(), game in any::<u32>(), every in 1u32..8) {
        let t = table();
        let mut s = scene(seed);
        let mut d = RegionDigest::of(&s);
        for tick in 0..60u32 {
            let r = hash_u32(seed ^ tick.wrapping_mul(0x9e37_79b9));
            if r.is_multiple_of(3) {
                let p = IVec3::new((r % 48) as i32, 1 + ((r >> 8) % 20) as i32, ((r >> 16) % 24) as i32);
                let cell = if r & 1 << 30 == 0 { Cell::filled(SAND) } else { Cell::EMPTY };
                put(&mut s, p, cell);
            }
            step_tracked(&mut s, &mut d, &t, game, tick);
            if tick % every == 0 {
                d.refresh(&s);
                prop_assert_eq!(d.total(), RegionDigest::of(&s).total(), "tick {}", tick);
            }
        }
        d.refresh(&s);
        prop_assert_eq!(d.chunk_sums(), RegionDigest::of(&s).chunk_sums());
    }
}

/// An edit the kernel then leaves alone, on a heat tick, is asleep again after the step: only the
/// observation before the step sees it.
#[test]
fn an_edit_that_never_moves_is_still_tracked() {
    let t = table();
    let mut s = BrickStore::new(64);
    fill(&mut s, IVec3::new(0, 0, 0), IVec3::new(24, 1, 24), Cell::filled(STONE));
    fill(&mut s, IVec3::new(2, 1, 2), IVec3::new(6, 5, 6), Cell::filled(SAND));
    settle(&mut s, &t, 2, 2000);
    let mut d = RegionDigest::of(&s);
    let tick = (10_000..).find(|&k| is_heat_tick(k)).unwrap_or(0);
    put(&mut s, IVec3::new(20, 30, 10), Cell::filled(STONE));
    step_tracked(&mut s, &mut d, &t, 2, tick);
    assert_eq!(s.active_bricks(), 0, "the floating stone went back to sleep");
    d.refresh(&s);
    assert_eq!(d.total(), RegionDigest::of(&s).total());
}
