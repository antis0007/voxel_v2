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

fn device() -> wgpu::Device {
    let adapter = engine_gpu::test_adapter();
    pollster::block_on(adapter.request_device(&Default::default())).unwrap().0
}

#[test]
#[ignore = "needs a GPU adapter; run with just test-gpu"]
fn gpu_arena_respects_budget() {
    let device = device();
    let mut arena = super::MeshArena::new((super::PAGE_VERTEX_BYTES + super::PAGE_INDEX_BYTES) as usize);
    let first = arena.alloc(&device, super::PAGE_VERTEX_BYTES, 4).expect("one page fits the budget");
    assert_eq!(first.range().page, 0);
    assert!(arena.alloc(&device, 56, 4).is_none(), "a second page would exceed the budget");
    assert!(arena.alloc(&device, super::PAGE_VERTEX_BYTES + 56, 4).is_none(), "larger than a page");
}

#[test]
#[ignore = "needs a GPU adapter; run with just test-gpu"]
fn gpu_freed_range_not_reused_same_frame() {
    let device = device();
    let mut arena = super::MeshArena::new(1 << 30);
    let a = arena.alloc(&device, 56, 4).unwrap();
    let a_range = *a.range();
    drop(a);
    let b = arena.alloc(&device, 56, 4).unwrap();
    assert_ne!(b.range().vertex_offset, a_range.vertex_offset, "freed range reused before begin_frame");
    assert_ne!(b.range().index_offset, a_range.index_offset);
    arena.begin_frame();
    let c = arena.alloc(&device, 56, 4).unwrap();
    assert_eq!((c.range().vertex_offset, c.range().index_offset), (a_range.vertex_offset, a_range.index_offset));
    drop((b, c));
}

#[test]
#[ignore = "needs a GPU adapter; run with just test-gpu"]
fn gpu_arena_frees_everything_after_leases_drop() {
    let device = device();
    let mut arena = super::MeshArena::new(1 << 30);
    let leases: Vec<_> = (0..3).map(|i| arena.alloc(&device, 56 * (i + 1), 12).unwrap()).collect();
    assert!(arena.used_bytes() > 0);
    drop(leases);
    arena.begin_frame();
    assert_eq!(arena.used_bytes(), 0);
}
