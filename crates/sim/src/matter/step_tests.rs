//! The coordinated tick: fine anchors, promotion on change, freezing, caps and demotion.
use super::fine_kernel::{fine_anchors, split_fine};
use crate::test_support::*;
use crate::{BlockView, Cell, FineRegion, MAX_FINE_AWAKE, MatterStore, PHASE_OFFSETS, VOLUME_FULL, split_voxel};
use engine_core::Budget;
use engine_core::reaction::SimRules;
use glam::IVec3;
use std::time::Duration;

fn here() -> FineRegion {
    FineRegion::around(IVec3::ZERO)
}

/// Step until no brick and no fine block is awake. Returns the ticks it took; panics past `limit`.
fn settle_matter(m: &mut MatterStore, t: &SimRules, seed: u32, limit: u32) -> u32 {
    for tick in 0..limit {
        m.step(t, seed, tick, here(), Budget::UNBOUNDED);
        if m.active_bricks() == 0 && m.fine_awake() == 0 {
            return tick + 1;
        }
    }
    panic!("still moving after {limit} ticks");
}

/// A stone box over [lo, hi) with the inside left empty and an open top.
fn stone_tank(m: &mut MatterStore, lo: IVec3, hi: IVec3) {
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            for x in lo.x..hi.x {
                if x == lo.x || x == hi.x - 1 || z == lo.z || z == hi.z - 1 || y == lo.y {
                    m.replace_block(IVec3::new(x, y, z), Cell::filled(STONE)).unwrap();
                }
            }
        }
    }
}

/// A full fine water block resting on a stone floor, air all round.
fn water_on_floor(m: &mut MatterStore, block: IVec3) {
    stone_floor(m, block - IVec3::new(4, 1, 4), block + IVec3::new(4, 0, 4));
    m.replace_block(block, water(VOLUME_FULL)).unwrap();
    m.promote(block, 0).unwrap();
}

fn stone_floor(m: &mut MatterStore, lo: IVec3, hi: IVec3) {
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            for x in lo.x..hi.x {
                m.replace_block(IVec3::new(x, y, z), Cell::filled(STONE)).unwrap();
            }
        }
    }
}

/// Volume of `material` over a box in fine units: a uniform block counts as 64 cells of itself.
fn fine_units(m: &MatterStore, lo: IVec3, hi: IVec3, material: u8) -> u64 {
    let mut sum = 0;
    for y in lo.y..hi.y {
        for z in lo.z..hi.z {
            for x in lo.x..hi.x {
                match m.block(IVec3::new(x, y, z)) {
                    BlockView::Uniform(c) if c.material == material => sum += 64 * u64::from(c.volume),
                    BlockView::Uniform(_) => {}
                    BlockView::Fine(cells) => {
                        sum += cells.iter().filter(|c| c.material == material).map(|c| u64::from(c.volume)).sum::<u64>()
                    }
                }
            }
        }
    }
    sum
}

#[test]
fn an_odd_phase_block_spills_fine_water_into_the_uniform_air_beside_it() {
    let (t, mut m) = (table(), MatterStore::new(64, 64));
    stone_floor(&mut m, IVec3::new(-4, 0, -4), IVec3::new(4, 1, 4));
    let b = IVec3::new(0, 1, 0);
    m.replace_block(b, water(VOLUME_FULL)).unwrap();
    m.promote(b, 0).unwrap();
    let (lo, hi) = (IVec3::new(-4, 0, -4), IVec3::new(4, 4, 4));
    let before = fine_units(&m, lo, hi, WATER);
    let mut woken = Vec::new();
    // Fine x = 3 is the last column of block b, x = 4 the first of the block beside it.
    let anchor = IVec3::new(3, 4, 1);
    let out = m.run_fine_block(&t, 7, 1, 0, anchor, &mut woken);
    assert!(out.changed && !out.stalled && out.promoted == 1, "{out:?}");
    assert!(m.is_fine(b + IVec3::X) && !m.is_fine(b - IVec3::Y));
    assert_eq!(fine_units(&m, lo, hi, WATER), before, "water is conserved");
    assert_eq!(woken, vec![b, b + IVec3::X]);
}

#[test]
fn fine_anchors_cover_a_block_with_8_even_and_27_odd_blocks() {
    let b = IVec3::new(1, -2, 3);
    let even = fine_anchors(&[b], PHASE_OFFSETS[0]);
    assert_eq!(even.len(), 8);
    assert!(even.iter().all(|a| (*a - b * 4).cmpge(IVec3::ZERO).all() && (*a - b * 4).cmple(IVec3::splat(2)).all()));
    let odd = fine_anchors(&[b], PHASE_OFFSETS[1]);
    assert_eq!(odd.len(), 27);
    assert!(odd.iter().all(|a| (*a - b * 4).cmpge(IVec3::splat(-1)).all() && (*a - b * 4).cmple(IVec3::splat(3)).all()));
    let pair = fine_anchors(&[b, b + IVec3::X], PHASE_OFFSETS[1]);
    assert_eq!(pair.len(), 45, "nine shared face anchors listed once");
    assert!(pair.windows(2).all(|w| (w[0].z, w[0].y, w[0].x) < (w[1].z, w[1].y, w[1].x)));
}

#[test]
fn split_fine_floors_negative_coordinates() {
    assert_eq!(split_fine(IVec3::new(-1, 4, 7)), (IVec3::new(-1, 1, 1), IVec3::new(3, 0, 3)));
}

#[test]
fn fine_water_spills_sideways_and_the_stone_floor_stays_uniform() {
    let (t, mut m) = (table(), MatterStore::new(64, 64));
    let b = IVec3::new(0, 1, 0);
    water_on_floor(&mut m, b);
    let (lo, hi) = (IVec3::new(-4, 0, -4), IVec3::new(4, 4, 4));
    let before = fine_units(&m, lo, hi, WATER);
    let stats = m.step(&t, 7, 0, here(), Budget::UNBOUNDED);
    for side in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
        assert!(m.is_fine(b + side), "water spilled into {side}");
    }
    assert!(!m.is_fine(b - IVec3::Y) && !m.is_fine(b + IVec3::Y), "stone below and air above never change");
    assert!(stats.fine.promoted >= 4 && stats.fine.changed_blocks > 0, "{stats:?}");
    assert_eq!(fine_units(&m, lo, hi, WATER), before, "water is conserved");
}

#[test]
fn the_block_pass_leaves_a_fine_block_alone() {
    let (t, mut m) = (table(), MatterStore::new(64, 64));
    stone_floor(&mut m, IVec3::new(-4, 0, -4), IVec3::new(4, 1, 4));
    for y in 1..5 {
        m.replace_block(IVec3::new(0, y, 0), water(VOLUME_FULL)).unwrap();
    }
    let f = IVec3::new(1, 1, 0);
    m.promote(f, 0).unwrap();
    let (lo, hi) = (IVec3::new(-4, 0, -4), IVec3::new(4, 8, 4));
    let before = fine_units(&m, lo, hi, WATER);
    m.step(&t, 7, 0, here(), Budget::UNBOUNDED);
    let (chunk, local) = split_voxel(f);
    assert_eq!(m.blocks.get(chunk, local), Cell::EMPTY, "the coarse backing of a fine block is never moved");
    assert!(fine_units(&m, f, f + IVec3::ONE, WATER) > 0, "water reached the fine block through the fine pass");
    assert!(m.is_fine(IVec3::new(0, 1, 0)), "the uniform water beside it changed and so became fine");
    assert_eq!(fine_units(&m, lo, hi, WATER), before, "water is conserved");
}

#[test]
fn an_enclosed_fine_block_sleeps_then_demotes_after_hysteresis() {
    let (t, mut m) = (table(), MatterStore::new(64, 64));
    stone_floor(&mut m, IVec3::new(-1, 0, -1), IVec3::new(2, 3, 2));
    let b = IVec3::new(0, 1, 0);
    m.replace_block(b, water(VOLUME_FULL)).unwrap();
    m.promote(b, 0).unwrap();
    let stats = m.step(&t, 7, 0, here(), Budget::UNBOUNDED);
    assert_eq!((stats.fine.awake, stats.fine.promoted, stats.fine.changed_blocks), (1, 0, 0), "{stats:?}");
    assert_eq!(m.fine_awake(), 0, "nothing moved, so it sleeps");
    let mut demoted_at = None;
    for tick in 1..=61 {
        assert!(m.is_fine(b) || demoted_at.is_some());
        if m.step(&t, 7, tick, here(), Budget::UNBOUNDED).fine.demoted == 1 {
            demoted_at = Some(tick);
        }
    }
    assert_eq!(demoted_at, Some(60), "sixty quiet ticks after promotion");
    assert_eq!((m.is_fine(b), m.fine_blocks()), (false, 0));
    assert_eq!(uniform(&m, b), water(VOLUME_FULL));
}

#[test]
fn fine_blocks_outside_the_region_freeze_and_stay_awake() {
    let (t, mut m) = (table(), MatterStore::new(64, 64));
    let b = IVec3::new(40, 1, 0);
    water_on_floor(&mut m, b);
    for tick in 0..2 {
        let stats = m.step(&t, 7, tick, here(), Budget::UNBOUNDED);
        assert_eq!((stats.fine.awake, stats.fine.frozen, stats.fine.blocks), (1, 1, 0), "tick {tick}: {stats:?}");
    }
    assert!(!m.is_fine(b + IVec3::X));
    let wide = FineRegion { center: IVec3::ZERO, radius: 48 };
    let stats = m.step(&t, 7, 2, wide, Budget::UNBOUNDED);
    assert!(stats.fine.frozen == 0 && stats.fine.blocks > 0 && m.is_fine(b + IVec3::X), "{stats:?}");
}

#[test]
fn at_most_2000_fine_blocks_step_per_tick() {
    let (t, mut m) = (table(), MatterStore::new(64, 2048));
    for i in 0..(MAX_FINE_AWAKE as i32 + 1) {
        let p = IVec3::new(i % 64, 0, i / 64);
        m.replace_block(p, Cell::filled(STONE)).unwrap();
        m.promote(p, 0).unwrap();
    }
    let wide = FineRegion { center: IVec3::ZERO, radius: 200 };
    let stats = m.step(&t, 7, 0, wide, Budget::UNBOUNDED);
    assert_eq!((stats.fine.awake, stats.fine.deferred), (2001, 1), "{stats:?}");
    let stats = m.step(&t, 7, 1, wide, Budget::UNBOUNDED);
    assert_eq!((stats.fine.awake, stats.fine.deferred), (1, 0), "the deferred block steps next tick: {stats:?}");
}

#[test]
fn a_full_fine_pool_stalls_the_move_and_keeps_the_block_awake() {
    let (t, mut m) = (table(), MatterStore::new(64, 1));
    let b = IVec3::new(0, 1, 0);
    water_on_floor(&mut m, b);
    let (lo, hi) = (IVec3::new(-4, 0, -4), IVec3::new(4, 4, 4));
    let before = fine_units(&m, lo, hi, WATER);
    let stats = m.step(&t, 7, 0, here(), Budget::UNBOUNDED);
    assert!(stats.fine.stalled_blocks > 0 && stats.fine.promoted == 0, "{stats:?}");
    assert_eq!(fine_units(&m, lo, hi, WATER), before, "nothing is lost on a stall");
    assert!(!m.is_fine(b + IVec3::X));
    assert_eq!(m.fine_awake(), 1, "a stalled block is retried");
}

#[test]
fn a_zero_budget_cuts_the_fine_pass_and_keeps_everything_awake() {
    let (t, mut m) = (table(), MatterStore::new(64, 64));
    let b = IVec3::new(0, 1, 0);
    water_on_floor(&mut m, b);
    let stats = m.step(&t, 7, 0, here(), Budget::wall(Duration::ZERO));
    assert!(stats.fine.truncated && stats.fine.blocks == 0, "{stats:?}");
    assert_eq!(m.fine_awake(), 1);
    let stats = m.step(&t, 7, 1, here(), Budget::UNBOUNDED);
    assert!(!stats.fine.truncated && stats.fine.blocks > 0, "{stats:?}");
}

#[test]
fn one_seed_gives_one_history() {
    let run = || {
        let (t, mut m) = (table(), MatterStore::new(64, 256));
        stone_floor(&mut m, IVec3::new(-6, 0, -4), IVec3::new(0, 1, 4));
        let b = IVec3::new(-1, 1, 0);
        m.replace_block(b, Cell::filled(SAND)).unwrap();
        m.promote(b, 0).unwrap();
        let start = m.state_hash();
        for tick in 0..40 {
            m.step(&t, 11, tick, here(), Budget::UNBOUNDED);
        }
        (start, m.state_hash())
    };
    let (start, a) = run();
    let (_, b) = run();
    assert_eq!(a, b, "the same seed replays the same ticks");
    assert_ne!(start, a, "the sand moved");
}

#[test]
fn a_spill_promotes_spreads_settles_and_demotes() {
    let (t, mut m) = (table(), MatterStore::new(64, 256));
    stone_tank(&mut m, IVec3::ZERO, IVec3::new(6, 4, 6));
    let source = IVec3::new(2, 2, 2);
    for y in 0..4 {
        for z in 0..4 {
            for x in 0..4 {
                m.set_fine(source, IVec3::new(x, y, z), water(VOLUME_FULL), 0).unwrap();
            }
        }
    }
    let (lo, hi) = (IVec3::ZERO, IVec3::new(6, 6, 6));
    assert_eq!(fine_units(&m, lo, hi, WATER), 64 * 64);
    let settled = settle_matter(&mut m, &t, 5, 600);
    assert_eq!(fine_units(&m, lo, hi, WATER), 64 * 64, "water is conserved");
    for tick in settled..settled + 70 {
        m.step(&t, 5, tick, here(), Budget::UNBOUNDED);
    }
    assert!(!m.is_fine(source), "the drained source block is uniform air again");
    let mut remaining = 0;
    for y in 1..4 {
        for z in 1..5 {
            for x in 1..5 {
                let p = IVec3::new(x, y, z);
                if let BlockView::Fine(cells) = m.block(p) {
                    remaining += 1;
                    assert_eq!(y, 1, "only the floor layer holds a surface: {p}");
                    assert!(cells.iter().any(|c| *c != cells[0]), "a uniform block would have demoted: {p}");
                }
            }
        }
    }
    assert!(remaining > 0 && remaining <= 16, "{remaining} fine blocks remain");
}

fn uniform(m: &MatterStore, block: IVec3) -> Cell {
    match m.block(block) {
        BlockView::Uniform(cell) => cell,
        BlockView::Fine(_) => panic!("{block} is fine"),
    }
}
