//! Brick store throughput: filling a chunk and writing cells into a resident one.
//! `just bench-check` compares both with `baselines.json`.
use criterion::{Criterion, criterion_group, criterion_main};
use engine_sim::{BrickStore, Cell};
use glam::IVec3;
use std::time::Duration;

/// Ground to y = 12: 32 of the chunk's 64 bricks hold something.
fn ground(p: IVec3) -> Cell {
    if p.y < 12 { Cell::filled(1) } else { Cell::EMPTY }
}

fn bricks(c: &mut Criterion) {
    c.bench_function("brick_fill_chunk", |b| {
        let mut store = BrickStore::new(64);
        b.iter(|| store.fill_chunk(IVec3::ZERO, ground).expect("pool holds one chunk"))
    });
    c.bench_function("brick_set_32k_cells", |b| {
        let mut store = BrickStore::new(64);
        store.fill_chunk(IVec3::ZERO, ground).expect("pool holds one chunk");
        let mut flip = false;
        b.iter(|| {
            // Alternate the value so every write changes the cell and wakes its brick.
            flip = !flip;
            let value = Cell::filled(if flip { 2 } else { 1 });
            for y in 0..32 {
                for z in 0..32 {
                    for x in 0..32 {
                        store.set(IVec3::ZERO, IVec3::new(x, y, z), value).expect("pool holds one chunk");
                    }
                }
            }
            std::hint::black_box(store.take_dirty(IVec3::ZERO))
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10).warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2));
    targets = bricks
}
criterion_main!(benches);
