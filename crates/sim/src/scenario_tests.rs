//! Headless scenarios for the cellular kernel, on a synthetic material table so
//! this crate never names a block. Every scenario asserts conservation.
use crate::test_support::*;
use crate::*;
use engine_core::material::MovementClass as C;
use glam::IVec3;
use proptest::prelude::*;

const BIG: IVec3 = IVec3::new(64, 64, 64);

#[test]
fn sand_falls_piles_and_the_world_goes_idle() {
    let (t, mut s) = (table(), BrickStore::new(2000));
    let lo = IVec3::new(-12, 0, -12);
    fill(&mut s, lo, IVec3::new(12, 1, 12), Cell::filled(STONE));
    fill(&mut s, IVec3::new(-2, 9, -2), IVec3::new(2, 13, 2), Cell::filled(SAND));
    let before = total(&s, -BIG, BIG, SAND);
    let ticks = settle(&mut s, &t, 7, 400);
    assert_eq!(total(&s, -BIG, BIG, SAND), before, "sand is conserved");
    assert_eq!(before, 4 * 4 * 4 * VOLUME_FULL as u32);
    for y in 1..20 {
        for z in -12..12 {
            for x in -12..12 {
                let p = IVec3::new(x, y, z);
                if at(&s, p).material == SAND {
                    assert_ne!(at(&s, p - IVec3::Y).volume, 0, "sand at {p} hangs over air after {ticks} ticks");
                }
            }
        }
    }
    let idle = s.state_hash();
    let stats = step(&mut s, &t, 7, 99);
    assert_eq!((s.state_hash(), s.active_bricks()), (idle, 0), "a settled world stays settled");
    assert_eq!((stats.active_bricks, stats.blocks), (0, 0), "and a tick over it processes no block at all");
}

#[test]
fn water_levels_out_in_a_tank_and_nothing_is_lost() {
    let (t, mut s) = (table(), BrickStore::new(2000));
    tank(&mut s, IVec3::new(0, 0, 0), IVec3::new(12, 10, 6));
    fill(&mut s, IVec3::new(1, 1, 1), IVec3::new(4, 7, 5), water(VOLUME_FULL));
    let before = total(&s, -BIG, BIG, WATER);
    settle(&mut s, &t, 3, 800);
    assert_eq!(total(&s, -BIG, BIG, WATER), before, "water is conserved");
    // Water only moves when neighbours differ by two levels, so a settled surface can keep a slope of
    // one level (1/64 of a cell) per column and no more. Measured here: 8 levels, an eighth of a cell,
    // across ten columns; at the old eight levels per cell it was about a cell.
    let column = |x, z| total(&s, IVec3::new(x, 1, z), IVec3::new(x + 1, 10, z + 1), WATER) as i32;
    for z in 1..5 {
        for x in 1..10 {
            assert!((column(x, z) - column(x + 1, z)).abs() <= 1, "step in the surface at x={x} z={z}");
        }
        let all: Vec<i32> = (1..11).map(|x| column(x, z)).collect();
        let spread = all.iter().max().unwrap() - all.iter().min().unwrap();
        assert!(spread <= 10, "surface tilts {spread} levels across the tank at z={z}: {all:?}");
    }
}

#[test]
fn sand_sinks_through_water_and_smoke_rises_through_air() {
    let (t, mut s) = (table(), BrickStore::new(2000));
    tank(&mut s, IVec3::new(0, 0, 0), IVec3::new(5, 14, 5));
    fill(&mut s, IVec3::new(1, 1, 1), IVec3::new(4, 5, 4), water(VOLUME_FULL));
    fill(&mut s, IVec3::new(1, 5, 1), IVec3::new(4, 7, 4), Cell::filled(SAND));
    settle(&mut s, &t, 1, 800);
    for z in 1..4 {
        for x in 1..4 {
            let col: Vec<u8> = (1..14).map(|y| at(&s, IVec3::new(x, y, z)).material).collect();
            let top_sand = col.iter().rposition(|m| *m == SAND);
            let low_water = col.iter().position(|m| *m == WATER);
            if let (Some(sand), Some(w)) = (top_sand, low_water) {
                assert!(sand < w, "sand above water in column {x},{z}: {col:?}");
            }
        }
    }
    let (mut s, mut smoke) = (BrickStore::new(2000), 0);
    // Closed: the store is unbounded, so smoke under an open top rises for ever.
    tank(&mut s, IVec3::new(0, 0, 0), IVec3::new(4, 20, 4));
    fill(&mut s, IVec3::new(0, 19, 0), IVec3::new(4, 20, 4), Cell::filled(STONE));
    fill(&mut s, IVec3::new(1, 1, 1), IVec3::new(3, 3, 3), Cell::filled(SMOKE));
    smoke += total(&s, -BIG, BIG, SMOKE);
    settle(&mut s, &t, 1, 800);
    assert_eq!(total(&s, -BIG, BIG, SMOKE), smoke, "smoke is conserved");
    let mean_y = |s: &BrickStore| {
        let (mut sum, mut n) = (0, 0);
        for y in 1..20 {
            let v = total(s, IVec3::new(1, y, 1), IVec3::new(3, y + 1, 3), SMOKE);
            sum += v * y as u32;
            n += v;
        }
        sum as f32 / n as f32
    };
    assert!(mean_y(&s) > 10.0, "smoke started at y 1..3 and ended at {}", mean_y(&s));
}

#[test]
fn cohesive_solids_never_move() {
    let (t, mut s) = (table(), BrickStore::new(100));
    fill(&mut s, IVec3::new(0, 5, 0), IVec3::new(3, 8, 3), Cell::filled(DIRT));
    let hash = s.state_hash();
    for tick in 0..20 {
        step(&mut s, &t, 1, tick);
    }
    assert_eq!((s.state_hash(), s.active_bricks()), (hash, 0), "floating dirt stays and its bricks go idle");
}

#[test]
fn matter_crosses_chunk_faces_at_negative_coordinates() {
    let (t, mut s) = (table(), BrickStore::new(2000));
    // The floor is the top of chunk y = -2; x and z straddle the chunk edge at 0.
    fill(&mut s, IVec3::new(-10, -34, -10), IVec3::new(10, -33, 10), Cell::filled(STONE));
    fill(&mut s, IVec3::new(-1, -20, -1), IVec3::new(1, -16, 1), Cell::filled(SAND));
    let before = total(&s, -BIG, BIG, SAND);
    settle(&mut s, &t, 5, 600);
    assert_eq!(total(&s, -BIG, BIG, SAND), before);
    let lowest = (-40..-10).find(|&y| (-12..12).any(|x| (-12..12).any(|z| at(&s, IVec3::new(x, y, z)).material == SAND)));
    assert_eq!(lowest, Some(-33), "sand came to rest on the floor, through the chunk face at y = -32");
}

/// A scattering of materials over [0, 24) x [0, 16) x [0, 24): crosses brick faces and a chunk face.
fn scatter(seed: u64) -> BrickStore {
    let mut s = BrickStore::new(2000);
    let mut state = seed;
    let mut next = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    fill(&mut s, IVec3::new(0, 0, 0), IVec3::new(24, 1, 24), Cell::filled(STONE));
    for _ in 0..500 {
        let r = next();
        let p = IVec3::new((r % 24) as i32, 1 + ((r >> 8) % 15) as i32, ((r >> 16) % 24) as i32);
        let cell = match (r >> 32) % 6 {
            0 | 1 => Cell::filled(SAND),
            2 | 3 => water(1 + ((r >> 40) % VOLUME_FULL as u64) as u8),
            4 => Cell::filled(SMOKE),
            _ => Cell::filled(STONE),
        };
        put(&mut s, p, cell);
    }
    s
}

/// Wake every stored brick of every chunk the scene can reach in 40 ticks (smoke rises out of the
/// start chunk, liquids spill across its faces): the 3x3x3 chunks around it.
fn wake_everything(s: &mut BrickStore) {
    for cz in -1..=1 {
        for cy in -1..=1 {
            for cx in -1..=1 {
                for brick in 0..BRICKS_PER_CHUNK as u8 {
                    s.wake(IVec3::new(cx, cy, cz), brick);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    /// Stepping only the dirty bricks gives exactly the world you get by waking every brick every tick.
    /// A wake that is missed across a brick face makes the two diverge.
    #[test]
    fn waking_only_what_changed_misses_nothing(seed in any::<u64>(), game_seed in any::<u32>()) {
        let t = table();
        let (mut driven, mut full) = (scatter(seed), scatter(seed));
        prop_assert_eq!(driven.state_hash(), full.state_hash());
        for tick in 0..40 {
            step(&mut driven, &t, game_seed, tick);
            wake_everything(&mut full);
            step(&mut full, &t, game_seed, tick);
        }
        prop_assert_eq!(driven.state_hash(), full.state_hash());
    }

    /// One block never creates or destroys anything and never leaves an invalid cell.
    #[test]
    fn a_block_conserves_each_material(raw in prop::collection::vec((0u8..7, 1u8..=VOLUME_FULL), 8), r in any::<u32>(), a in -50i32..50) {
        let t = table();
        let mut cells: [Cell; 8] = std::array::from_fn(|i| {
            let (m, v) = raw[i];
            match t[m as usize].class {
                C::Void => Cell::EMPTY,
                C::Liquid | C::Gas => Cell { volume: v, ..Cell::filled(m) },
                _ => Cell::filled(m),
            }
        });
        let sum = |c: &[Cell; 8], m: u8| c.iter().filter(|x| x.material == m).map(|x| x.volume as u32).sum::<u32>();
        let before: Vec<u32> = (0..7).map(|m| sum(&cells, m)).collect();
        block_rule(&t, &mut cells, r, r.rotate_left(7), IVec3::new(a, a, a));
        for m in 0..7 {
            prop_assert_eq!(sum(&cells, m), before[m as usize], "material {} changed", m);
        }
        for c in cells {
            prop_assert!(c.volume <= VOLUME_FULL);
            prop_assert!(c.volume > 0 || c == Cell::EMPTY, "an emptied cell must be EMPTY, got {:?}", c);
        }
    }
}

/// The world a fixed seed produces, hashed. If this changes, the hash function, the phase order or a
/// rule changed: every world evolves differently (docs/adr/0008-ca-determinism.md). Record the new
/// value only on purpose.
#[test]
fn the_canonical_scenario_hash_is_pinned() {
    const PINNED: u64 = 12_902_218_551_230_034_786;
    let t = table();
    let mut s = scatter(0xC0FFEE);
    for tick in 0..60 {
        step(&mut s, &t, 0xBEEF, tick);
    }
    assert_eq!(s.state_hash(), PINNED, "canonical scenario drifted; if intended, set PINNED to this");
}

/// The pin means something: the same scene under another game seed evolves differently, and two runs of
/// one seed agree.
#[test]
fn the_pinned_scenario_depends_on_the_seed_and_repeats() {
    let t = table();
    let run = |seed| {
        let mut s = scatter(0xC0FFEE);
        for tick in 0..60 {
            step(&mut s, &t, seed, tick);
        }
        s.state_hash()
    };
    assert_eq!(run(0xBEEF), run(0xBEEF));
    assert_ne!(run(0xBEEF), run(0xBEF0));
}

/// A full pool refuses a move that needs a new brick, and a refused move allocates nothing: with one
/// free slot and a block that needs two, neither is taken.
#[test]
fn a_move_that_needs_more_bricks_than_the_pool_has_allocates_none() {
    let t = table();
    // Two water cells at the top of bricks (0,1,0) and (1,1,0), walled in by stone inside those same
    // bricks so they cannot spill, sit over empty space in the bricks below. They fall in one block of
    // the second phase: it needs two new bricks.
    let setup = |capacity| {
        let mut s = BrickStore::new(capacity);
        for p in [(7, 3), (8, 3)] {
            put(&mut s, IVec3::new(p.0, 8, p.1), water(VOLUME_FULL));
            put(&mut s, IVec3::new(p.0, 8, p.1 - 1), Cell::filled(STONE));
            put(&mut s, IVec3::new(p.0, 8, p.1 + 1), Cell::filled(STONE));
        }
        put(&mut s, IVec3::new(6, 8, 3), Cell::filled(STONE));
        put(&mut s, IVec3::new(9, 8, 3), Cell::filled(STONE));
        s
    };
    let mut tight = setup(3);
    assert_eq!((tight.bricks_in_use(), tight.free_bricks()), (2, 1));
    let before = total(&tight, -BIG, BIG, WATER);
    let stats = step(&mut tight, &t, 1, 0);
    assert!(stats.stalled_blocks > 0, "the block that needed two bricks stalled: {stats:?}");
    assert_eq!(tight.bricks_in_use(), 2, "and took none of the one free slot");
    assert_eq!(total(&tight, -BIG, BIG, WATER), before, "nothing was lost");
    assert_eq!(at(&tight, IVec3::new(7, 8, 3)), water(VOLUME_FULL), "the water is where it was");
    // With room the same world falls.
    let mut roomy = setup(8);
    let stats = step(&mut roomy, &t, 1, 0);
    assert_eq!(stats.stalled_blocks, 0);
    assert!(roomy.bricks_in_use() > 2, "the cells fell into new bricks");
}
