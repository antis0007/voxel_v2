//! Brick store throughput: filling a chunk and writing cells into a resident one.
//! `just bench-check` compares both with `baselines.json`.
use criterion::{Criterion, criterion_group, criterion_main};
use engine_core::material::{MovementClass, SIM_MATERIAL_ROWS, SimMaterial};
use engine_core::hash::hash_u32;
use engine_core::reaction::{ReactionTable, SimRules};
use engine_core::Budget;
use engine_sim::{
    AMBIENT_DECIKELVIN, BRICKS_PER_CHUNK, BrickStore, Cell, FineRegion, MatterStore, RegionDigest, VOLUME_FULL, step,
};
use glam::IVec3;
use std::time::Duration;

/// Air, stone, sand and water, with the thermal data the real table packs, so heat has work to do.
fn table() -> [SimMaterial; SIM_MATERIAL_ROWS] {
    let row = |class, kg: f32, flow, k: f32, cp: u16| {
        let density = (kg * 100.0) as u32;
        SimMaterial {
            class,
            density,
            flow,
            conductivity: (k * 100.0).round() as u16,
            capacity: cp,
            thermal_mass: ((density as u64 * cp as u64) >> 10) as u32,
            ..SimMaterial::INERT
        }
    };
    let mut t = [SimMaterial::INERT; SIM_MATERIAL_ROWS];
    t[0] = row(MovementClass::Void, 1.2, 0, 0.026, 1005);
    t[1] = row(MovementClass::Solid, 2600.0, 0, 2.5, 790);
    t[2] = row(MovementClass::Powder, 1700.0, 0, 0.3, 830);
    t[3] = row(MovementClass::Liquid, 1000.0, 255, 0.6, 4186);
    t
}

/// 64^3 cells (262,144) in 2x2x2 chunks: a stone floor, then a third each of sand, water and air.
fn mixed_region(gradient: bool) -> BrickStore {
    let mut s = BrickStore::new(1024);
    for cy in 0..2 {
        for cz in 0..2 {
            for cx in 0..2 {
                let chunk = IVec3::new(cx, cy, cz);
                s.fill_chunk(chunk, |l| {
                    let p = chunk * 32 + l;
                    if p.y == 0 {
                        return Cell::filled(1);
                    }
                    let h = hash_u32((p.x as u32) ^ hash_u32((p.y as u32) ^ hash_u32(p.z as u32)));
                    // A spread of temperatures makes every block conduct; ambient everywhere makes none.
                    let temperature = if gradient { AMBIENT_DECIKELVIN + (h >> 8) as u16 % 400 } else { AMBIENT_DECIKELVIN };
                    let cell = match h % 3 {
                        0 => Cell::filled(2),
                        1 => Cell { volume: VOLUME_FULL, ..Cell::filled(3) },
                        _ => return Cell::EMPTY,
                    };
                    Cell { temperature, ..cell }
                })
                .expect("pool holds 8 chunks");
            }
        }
    }
    s
}

fn wake_all(s: &mut BrickStore) {
    for cy in 0..2 {
        for cz in 0..2 {
            for cx in 0..2 {
                for b in 0..BRICKS_PER_CHUNK as u8 {
                    s.wake(IVec3::new(cx, cy, cz), b);
                }
            }
        }
    }
}

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

/// The Phase A exit number: every brick of a 262,144-cell region awake, one tick (two Margolus phases).
/// 60 ticks per second needs this under 16.7 ms.
fn ca(c: &mut Criterion) {
    let t = SimRules::new(table(), ReactionTable::default());
    c.bench_function("ca_step_262k_cells_all_awake", |b| {
        let mut s = mixed_region(false);
        let mut tick = 0;
        b.iter(|| {
            wake_all(&mut s);
            tick += 1;
            step(&mut s, &t, 1, tick)
        })
    });
    // The same region with a temperature spread in every cell: every block also conducts heat.
    c.bench_function("ca_step_262k_cells_all_awake_with_heat", |b| {
        let mut s = mixed_region(true);
        let mut tick = 0;
        b.iter(|| {
            wake_all(&mut s);
            tick += 1;
            step(&mut s, &t, 1, tick)
        })
    });
}

/// The fine pass ceiling: 2000 fine water blocks awake inside a stone ring, one tick. Nothing
/// moves, so this is the cost of visiting every sub-brick and evaluating its 35 blocks; the spec's
/// budget for the pass is 3 ms on H1.
fn fine(c: &mut Criterion) {
    let t = SimRules::new(table(), ReactionTable::default());
    c.bench_function("fine_step_2000_sub_bricks_idle", |b| {
        let mut m = MatterStore::new(1024, 4096);
        for z in -1..41 {
            for x in -1..51 {
                m.replace_block(IVec3::new(x, 0, z), Cell::filled(1)).expect("pool holds the floor");
                if x == -1 || x == 50 || z == -1 || z == 40 {
                    m.replace_block(IVec3::new(x, 1, z), Cell::filled(1)).expect("pool holds the ring");
                }
            }
        }
        let blocks: Vec<IVec3> = (0..2000).map(|i| IVec3::new(i % 50, 1, i / 50)).collect();
        for &p in &blocks {
            m.replace_block(p, Cell { volume: 32, ..Cell::filled(3) }).expect("pool holds the layer");
            m.promote(p, 0).expect("fine pool holds the layer");
        }
        let region = FineRegion { center: IVec3::new(25, 1, 20), radius: 64 };
        let mut tick = 0;
        b.iter(|| {
            for &p in &blocks {
                m.wake_fine(p);
            }
            tick += 1;
            m.step(&t, 1, tick, region, Budget::UNBOUNDED)
        })
    });
}

/// The lockstep checksum's worst tick: every brick of a 196,608-cell region (64 x 48 x 64, 384
/// bricks) changed, so `observe` finds all of them dirty and `refresh` rehashes all of them.
fn digest(c: &mut Criterion) {
    c.bench_function("digest_tick_196k_cells_all_awake", |b| {
        let mut s = mixed_region(false);
        for cy in 0..2 {
            for cz in 0..2 {
                for cx in 0..2 {
                    let chunk = IVec3::new(cx, cy, cz);
                    s.fill_chunk(chunk, |l| {
                        let p = chunk * 32 + l;
                        let h = hash_u32((p.x as u32) ^ hash_u32((p.y as u32) ^ hash_u32(p.z as u32)));
                        if p.y >= 48 || h.is_multiple_of(3) { Cell::EMPTY } else { Cell::filled(1 + (h % 3) as u8) }
                    })
                    .expect("pool holds 8 chunks");
                }
            }
        }
        let mut d = RegionDigest::of(&s);
        b.iter(|| {
            wake_all(&mut s);
            d.observe(&s);
            std::hint::black_box(d.refresh(&s))
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10).warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2));
    targets = bricks, ca, fine, digest
}
criterion_main!(benches);
