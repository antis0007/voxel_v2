use super::*;
use crate::fluid::VoxelScalePreset;
use crate::world::{WorldType, chunk_coord_from_world_block, effects};
use std::collections::HashSet;

fn world() -> World {
    World::new(WorldType::Superflat, VoxelScalePreset::OneMeter)
}

fn admit(w: &mut World, p: IVec3) -> bool {
    w.resident_edit(chunk_coord_from_world_block(p)).is_ok()
}

fn edit(w: &mut World, h: Option<&mut EditHistory>, p: IVec3, new: BlockId) {
    assert!(admit(w, p));
    let old = w.get(p);
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    effects::replace(w, p, old, new).expect("edit commits").publish(w, &mut blocks, &mut chunks);
    if let Some(h) = h {
        h.record(w, p, old, new);
    }
}

fn run(w: &mut World, h: &mut EditHistory, direction: Direction) -> String {
    h.start(w, direction).expect("history has a step");
    let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
    let mut prepare = admit;
    let mut sink = Sink { prepare: &mut prepare, blocks: &mut blocks, chunks: &mut chunks };
    for _ in 0..1000 {
        if let Some(text) = h.step(w, &mut sink, Budget::UNBOUNDED, 64) {
            return text;
        }
    }
    panic!("undo never finished");
}

/// Cells straddling chunk borders at negative and far coordinates.
fn stroke_cells() -> Vec<IVec3> {
    let mut cells = Vec::new();
    for base in [IVec3::new(-33, 12, -1), IVec3::new(1_000_031, 20, -1_000_001)] {
        for d in 0..4 {
            cells.push(base + IVec3::new(d, 0, d));
        }
    }
    cells
}

#[test]
fn undo_then_redo_restores_each_stroke_across_chunk_borders() {
    let mut w = world();
    let mut h = EditHistory::new(LOCAL_CELLS);
    let cells = stroke_cells();
    for &p in &cells {
        assert!(admit(&mut w, p));
    }
    let before: Vec<_> = cells.iter().map(|&p| w.get(p)).collect();
    for &p in &cells {
        edit(&mut w, Some(&mut h), p, BlockId::Planks);
    }
    assert!(h.seal());
    for &p in &cells {
        edit(&mut w, Some(&mut h), p, BlockId::StoneBricks);
    }
    assert!(h.seal());
    assert_eq!(h.depth(), (2, 0));

    assert_eq!(run(&mut w, &mut h, Direction::Undo), format!("Undone: {} Blocks", cells.len()));
    assert!(cells.iter().all(|&p| w.get(p) == BlockId::Planks));
    assert_eq!(run(&mut w, &mut h, Direction::Undo), format!("Undone: {} Blocks", cells.len()));
    assert_eq!(cells.iter().map(|&p| w.get(p)).collect::<Vec<_>>(), before);
    assert_eq!(h.depth(), (0, 2));
    assert_eq!(h.start(&w, Direction::Undo), Err("Undo History Empty"));

    run(&mut w, &mut h, Direction::Redo);
    run(&mut w, &mut h, Direction::Redo);
    assert!(cells.iter().all(|&p| w.get(p) == BlockId::StoneBricks));
    assert_eq!(h.depth(), (2, 0));
}

#[test]
fn undo_keeps_a_cell_someone_else_changed_since() {
    let mut w = world();
    let mut mine = EditHistory::new(REMOTE_CELLS);
    let (a, b) = (IVec3::new(-1, 10, -1), IVec3::new(0, 10, 0));
    assert!(admit(&mut w, a));
    let original = w.get(a);
    edit(&mut w, Some(&mut mine), a, BlockId::Planks);
    edit(&mut w, Some(&mut mine), b, BlockId::Planks);
    mine.seal();
    edit(&mut w, None, b, BlockId::Cobblestone);

    assert_eq!(run(&mut w, &mut mine, Direction::Undo), "Undone: 1 Blocks · Skipped: 1");
    assert_eq!(w.get(a), original);
    assert_eq!(w.get(b), BlockId::Cobblestone);
    run(&mut w, &mut mine, Direction::Redo);
    assert_eq!(w.get(a), BlockId::Planks);
}

#[test]
fn a_new_edit_clears_redo() {
    let mut w = world();
    let mut h = EditHistory::new(LOCAL_CELLS);
    let p = IVec3::new(5, 10, -40);
    edit(&mut w, Some(&mut h), p, BlockId::Planks);
    h.seal();
    run(&mut w, &mut h, Direction::Undo);
    assert_eq!(h.depth(), (0, 1));
    edit(&mut w, Some(&mut h), p + IVec3::X, BlockId::Planks);
    h.seal();
    assert_eq!(h.depth(), (1, 0));
    assert_eq!(h.start(&w, Direction::Redo), Err("Redo History Empty"));
}

#[test]
fn caps_drop_the_oldest_steps_and_refuse_oversized_strokes() {
    let mut w = world();
    let mut h = EditHistory::new(8);
    for i in 0..6 {
        edit(&mut w, Some(&mut h), IVec3::new(i, 10, 0), BlockId::Planks);
        edit(&mut w, Some(&mut h), IVec3::new(i, 11, 0), BlockId::Planks);
        assert!(h.seal());
    }
    assert_eq!(h.depth(), (4, 0));
    for i in 0..9 {
        edit(&mut w, Some(&mut h), IVec3::new(i, 12, 0), BlockId::Planks);
    }
    assert!(!h.seal());
    assert_eq!(h.depth(), (4, 0));

    let mut steps = EditHistory::new(LOCAL_CELLS);
    for i in 0..MAX_ENTRIES as i32 + 5 {
        steps.push(&w, vec![Change { p: IVec3::new(i, 0, 0), old: BlockId::Air, new: BlockId::Planks }]);
    }
    assert_eq!(steps.depth(), (MAX_ENTRIES, 0));
}

#[test]
fn another_worlds_steps_never_apply() {
    let mut first = world();
    let mut h = EditHistory::new(LOCAL_CELLS);
    edit(&mut first, Some(&mut h), IVec3::new(2, 10, 2), BlockId::Planks);
    h.seal();
    let second = World::new(WorldType::Superflat, VoxelScalePreset::HalfMeter);
    assert_ne!(first.sample_config(), second.sample_config());
    assert_eq!(h.start(&second, Direction::Undo), Err("Undo History Empty"));
    assert_eq!(h.depth(), (0, 0));
}
