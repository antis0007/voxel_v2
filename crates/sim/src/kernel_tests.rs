//! The parallel brick groups must produce the same bytes as one thread.
use crate::test_support::*;
use crate::*;
use engine_core::hash::hash_u32;
use glam::IVec3;

thread_local! {
    /// Set on the calling thread to make `step` run every brick group on that thread.
    pub(crate) static FORCE_SERIAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 64^3 cells over chunks -1..=0 on every axis: a stone floor, then sand, water, lava, smoke and air
/// with a temperature spread, so every block moves, conducts or rolls dice.
fn busy_region() -> BrickStore {
    let mut s = BrickStore::new(1024);
    for cy in -1..=0 {
        for cz in -1..=0 {
            for cx in -1..=0 {
                let chunk = IVec3::new(cx, cy, cz);
                s.fill_chunk(chunk, |l| {
                    let p = chunk * 32 + l;
                    if p.y == -32 {
                        return Cell::filled(STONE);
                    }
                    let h = hash_u32((p.x as u32) ^ hash_u32((p.y as u32) ^ hash_u32(p.z as u32)));
                    let temperature = AMBIENT_DECIKELVIN + (h >> 8) as u16 % 600;
                    let cell = match h % 6 {
                        0 => Cell::filled(SAND),
                        1 => water(VOLUME_FULL),
                        2 => Cell { volume: 40, ..Cell::filled(LAVA) },
                        3 => Cell { volume: 20, ..Cell::filled(SMOKE) },
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

fn run(serial: bool) -> Vec<(u64, StepStats)> {
    FORCE_SERIAL.with(|f| f.set(serial));
    super::set_kernel_workers(4);
    let t = table();
    let mut s = busy_region();
    let trace = (0..24).map(|tick| {
        let stats = step(&mut s, &t, 0x5EED, tick);
        (s.state_hash(), stats)
    });
    let trace = trace.collect();
    FORCE_SERIAL.with(|f| f.set(false));
    trace
}

#[test]
fn block_spots_match_the_per_cell_split_across_brick_and_chunk_faces() {
    let edges = [-65, -64, -33, -32, -31, -9, -8, -1, 0, 1, 7, 8, 15, 31, 32, 63, 1_000_007, -1_000_001];
    for &x in &edges {
        for &y in &edges {
            for &z in &edges {
                let anchor = IVec3::new(x, y, z);
                let expected: [(IVec3, u8, usize); 8] = std::array::from_fn(|i| {
                    let (chunk, local) = crate::cell::split_voxel(super::cell_pos(anchor, i));
                    let (brick, cell) = crate::cell::brick_and_cell(local);
                    (chunk, brick, cell)
                });
                assert_eq!(super::block_spots(anchor), expected, "anchor {anchor}");
            }
        }
    }
}

/// The premise of the empty-block shortcut: the full rule leaves eight `EMPTY` cells alone on every phase.
#[test]
fn the_rule_does_nothing_to_an_empty_block_on_any_phase() {
    let t = table();
    assert!(!t.reactive(Cell::EMPTY.material));
    for phase_counter in 0..64 {
        for anchor in [IVec3::ZERO, IVec3::new(-33, 7, 1_000_001)] {
            let mut cells = [Cell::EMPTY; 8];
            let outcome = crate::rules::block_rule(&t, &mut cells, 0xABCD, phase_counter, anchor);
            assert_eq!((outcome, cells), (crate::Outcome::default(), [Cell::EMPTY; 8]), "phase {phase_counter}");
        }
    }
}

#[test]
fn parallel_brick_groups_match_one_thread_every_tick() {
    let (serial, parallel) = (run(true), run(false));
    assert!(serial[0].1.blocks as usize >= 2 * super::PARALLEL_MIN_BLOCKS, "the region must reach the parallel path");
    for (tick, (a, b)) in serial.iter().zip(&parallel).enumerate() {
        assert_eq!(a, b, "tick {tick}");
    }
    assert!(serial.iter().any(|(_, s)| s.changed_blocks > 0), "the region must move");
}

