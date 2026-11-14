//! A joiner's undo and redo, sent as ordered commands and run by the host.
use super::survival_tests::{floor, run_as};
use super::*;
use crate::world::edit_history::{Direction, EditHistory, REMOTE_CELLS};

fn command(world: &mut World, body: &Player, creative: bool, history: &mut EditHistory, direction: Direction) -> Result<()> {
    let (mut tools, mut blocks, mut chunks) = (actions::Tools::default(), HashSet::new(), HashSet::new());
    actions::apply(actions::Command::EditHistory(direction), world, body, &DayNightCycle::default(), 1, 0, creative,
        &mut tools, history, &mut blocks, &mut chunks)
}

fn placed(world: &World, before: &[(IVec3, BlockId)]) -> Vec<IVec3> {
    before.iter().filter(|(p, b)| world.get(*p) != *b).map(|(p, _)| *p).collect()
}

#[test]
fn a_creative_joiner_undoes_and_redoes_its_own_placement() {
    let (mut world, mut body) = floor();
    let mut history = EditHistory::new(REMOTE_CELLS);
    let area: Vec<_> = (195..205).flat_map(|y| (0..16).flat_map(move |x| (0..16).map(move |z| IVec3::new(x, y, z)))).collect();
    let before: Vec<_> = area.iter().map(|&p| (p, world.get(p))).collect();
    world.inventory.hotbar[8] = None;
    world.sync_from_inventory();
    let controls = Controls { seq: 1, slot: 8, pitch: -0.9, place: true, block: BlockId::Planks.as_u32(), ..Default::default() };
    run_as(&mut world, &mut body, &controls, true, &mut history, 1);
    let changed = placed(&world, &before);
    assert!(!changed.is_empty() && changed.iter().all(|&p| world.get(p) == BlockId::Planks), "{changed:?}");
    assert_eq!(history.depth(), (1, 0));

    let idle = Controls { seq: 2, slot: 8, pitch: -0.9, ..Default::default() };
    command(&mut world, &body, true, &mut history, Direction::Undo).expect("undo starts");
    run_as(&mut world, &mut body, &idle, true, &mut history, 1);
    assert!(placed(&world, &before).is_empty());
    assert_eq!(history.depth(), (0, 1));

    command(&mut world, &body, true, &mut history, Direction::Redo).expect("redo starts");
    run_as(&mut world, &mut body, &idle, true, &mut history, 1);
    assert_eq!(placed(&world, &before), changed);
}

#[test]
fn survival_joiners_record_nothing_and_cannot_undo() {
    let (mut world, mut body) = floor();
    let mut history = EditHistory::new(REMOTE_CELLS);
    world.inventory.hotbar[0] = Some(world::inventory::ItemStack::new(world::inventory::Item::Block(BlockId::Dirt), 8));
    world.sync_from_inventory();
    let controls = Controls { seq: 1, pitch: -0.9, place: true, ..Default::default() };
    run_as(&mut world, &mut body, &controls, false, &mut history, 2);
    assert_eq!(history.depth(), (0, 0));
    let refused = command(&mut world, &body, false, &mut history, Direction::Undo).expect_err("survival undo refused");
    assert_eq!(refused.to_string(), "Undo: Creative Only");
}
