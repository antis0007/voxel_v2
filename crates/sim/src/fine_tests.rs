//! Promotion and demotion conserve volume; a pool slot is never aliased.
use super::*;
use glam::IVec3;

fn water(volume: u8) -> Cell {
    Cell { material: 7, volume, ..Cell::EMPTY }
}

#[test]
fn promotion_splits_a_uniform_block_and_demotes_back() {
    let mut store = FineStore::new(4);
    let block = IVec3::new(3, -2, 9);
    let block_cell = water(15);
    store.promote(block, block_cell, 0).expect("slot");
    let cells = store.cells(block).expect("fine");
    assert!(cells.iter().all(|c| *c == block_cell));
    // One block unit is 64 fine units, so the same `volume` in every cell.
    let total: u32 = cells.iter().map(|c| u32::from(c.volume)).sum();
    assert_eq!(total, u32::from(block_cell.volume) * FINE_CELLS as u32);
    assert_eq!(store.demote_uniform(block, 0, 0), Some(block_cell));
    assert!(!store.is_fine(block));
    assert_eq!(store.in_use(), 0);
}

#[test]
fn a_full_block_round_trips_without_a_remainder() {
    let mut store = FineStore::new(1);
    let block = IVec3::ZERO;
    let stone = Cell::filled(3);
    store.promote(block, stone, 0).expect("slot");
    let cells = *store.cells(block).expect("fine");
    assert_eq!(collapsed(&cells).remainder, 0);
    assert_eq!(collapsed(&cells).cell, stone);
    assert_eq!(store.demote_uniform(block, 0, 0), Some(stone));
}

#[test]
fn a_heterogeneous_sub_brick_refuses_uniform_demotion() {
    let mut store = FineStore::new(1);
    let block = IVec3::X;
    store.promote(block, Cell::filled(3), 0).expect("slot");
    assert!(store.set(block, IVec3::new(1, 2, 3), Cell::filled(4), 1));
    assert_eq!(store.demote_uniform(block, 10, 0), None);
    assert!(store.is_fine(block));
}

#[test]
fn hysteresis_keeps_a_changed_block_fine() {
    let mut store = FineStore::new(1);
    let block = IVec3::Y;
    store.promote(block, Cell::filled(3), 0).expect("slot");
    assert!(store.set(block, IVec3::new(0, 0, 0), Cell::filled(4), 5));
    assert_eq!(store.demote_uniform(block, 6, 60), None);
    assert_eq!(store.demote_uniform(block, 65, 60), None, "cells differ");
}

#[test]
fn a_written_back_equal_cell_demotes_after_hysteresis() {
    let mut store = FineStore::new(1);
    let block = IVec3::Z;
    store.promote(block, Cell::filled(3), 0).expect("slot");
    assert!(store.set(block, IVec3::new(1, 1, 1), Cell::filled(3), 5));
    assert_eq!(store.demote_uniform(block, 64, 60), None);
    assert_eq!(store.demote_uniform(block, 65, 60), Some(Cell::filled(3)));
}

#[test]
fn averaged_collapse_carries_every_unit() {
    let mut cells = [Cell::EMPTY; FINE_CELLS];
    for (i, cell) in cells.iter_mut().enumerate() {
        *cell = if i < 40 {
            Cell { material: 2, volume: 20, ..Cell::EMPTY }
        } else {
            Cell { material: 3, volume: 10, ..Cell::EMPTY }
        };
    }
    let collapse = collapsed(&cells);
    let total = 40 * 20 + 24 * 10;
    assert_eq!(
        u32::from(collapse.cell.volume) * FINE_CELLS as u32 + collapse.remainder,
        total
    );
    assert_eq!(collapse.cell.material, 2, "the material holding most volume");
    assert_eq!(collapse.cell.volume, 16);
    assert_eq!(collapse.remainder, 16);
}

#[test]
fn an_empty_sub_brick_collapses_to_empty_space() {
    let collapse = collapsed(&[Cell::EMPTY; FINE_CELLS]);
    assert_eq!(collapse.cell, Cell::EMPTY);
    assert_eq!(collapse.remainder, 0);
}

#[test]
fn the_pool_refuses_overflow_and_reuses_freed_slots() {
    let mut store = FineStore::new(2);
    store.promote(IVec3::X, Cell::filled(1), 0).expect("first");
    store.promote(IVec3::Y, Cell::filled(1), 0).expect("second");
    assert_eq!(store.promote(IVec3::Z, Cell::filled(1), 0), Err(PoolFull));
    assert_eq!(store.free_slots(), 0);
    store.demote_uniform(IVec3::X, 0, 0).expect("uniform");
    assert_eq!(store.free_slots(), 1);
    store.promote(IVec3::Z, Cell::filled(1), 0).expect("reused");
    assert_eq!(store.in_use(), 2);
    assert!(store.is_fine(IVec3::Z) && !store.is_fine(IVec3::X));
}

#[test]
fn a_write_outside_the_four_cubed_sub_brick_changes_nothing() {
    let mut store = FineStore::new(1);
    let block = IVec3::ZERO;
    store.promote(block, Cell::filled(3), 0).expect("slot");
    for outside in [IVec3::new(-1, 0, 0), IVec3::new(0, 4, 0), IVec3::new(0, 0, 4)] {
        assert!(!store.set(block, outside, Cell::filled(9), 1));
    }
    assert_eq!(store.cells(block), Some(&[Cell::filled(3); FINE_CELLS]));
}

#[test]
fn promoting_an_authored_partial_block_keeps_its_cells() {
    let mut store = FineStore::new(2);
    let block = IVec3::ONE;
    store.promote(block, Cell::filled(3), 0).expect("slot");
    store.set(block, IVec3::new(0, 0, 0), Cell::EMPTY, 1);
    store.promote(block, Cell::filled(5), 2).expect("already fine");
    assert_eq!(store.cells(block).expect("fine")[0], Cell::EMPTY);
}
