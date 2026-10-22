use super::{ArenaLease, ArenaRange, RangeAllocator};

fn range(page: u32, vertex_offset: u64, index_offset: u64) -> ArenaRange {
    ArenaRange { page, vertex_offset, vertex_bytes: 56, index_offset, index_bytes: 4 }
}

#[test]
fn alloc_rounds_to_alignment() {
    let mut a = RangeAllocator::new(1024, 56);
    assert_eq!(a.alloc(1), Some(0));
    assert_eq!(a.alloc(1), Some(56));
    assert_eq!(a.used(), 112);
}

#[test]
fn free_coalesces_both_sides() {
    let mut a = RangeAllocator::new(56 * 3, 56);
    let (x, y, z) = (a.alloc(56).unwrap(), a.alloc(56).unwrap(), a.alloc(56).unwrap());
    a.free(x, 56);
    a.free(z, 56);
    assert_eq!(a.largest_free(), 56);
    a.free(y, 56);
    assert_eq!(a.largest_free(), a.capacity());
    assert_eq!(a.used(), 0);
}

#[test]
fn exhaustion_returns_none() {
    let mut a = RangeAllocator::new(112, 56);
    assert!(a.alloc(56).is_some());
    assert!(a.alloc(56).is_some());
    assert_eq!(a.alloc(56), None);
    assert_eq!(a.alloc(0), None, "a zero-size allocation has no range to own");
}

#[test]
fn fragmentation_first_fit_reuses_hole() {
    let mut a = RangeAllocator::new(56 * 8, 56);
    let offsets: Vec<u64> = (0..4).map(|_| a.alloc(56).unwrap()).collect();
    a.free(offsets[1], 56);
    assert_eq!(a.alloc(56), Some(offsets[1]));
    assert_eq!(a.alloc(56 * 2), Some(56 * 4), "a larger request skips the filled hole");
}

#[test]
fn lease_drop_returns_range_once() {
    let (tx, rx) = std::sync::mpsc::channel();
    let lease = ArenaLease::new(range(2, 112, 8), tx);
    assert_eq!(lease.range().page, 2);
    assert!(rx.try_recv().is_err(), "nothing returned while the lease lives");
    drop(lease);
    assert_eq!(rx.try_recv().unwrap(), range(2, 112, 8));
    assert!(rx.try_recv().is_err());
}

#[test]
fn lease_drop_after_arena_is_gone_is_silent() {
    let (tx, rx) = std::sync::mpsc::channel();
    let lease = ArenaLease::new(range(0, 0, 0), tx);
    drop(rx);
    drop(lease);
}

#[test]
fn base_vertex_and_first_index() {
    let r = range(0, 56 * 3, 4 * 5);
    assert_eq!(r.base_vertex(), 3);
    assert_eq!(r.first_index(), 5);
}
