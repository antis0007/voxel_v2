//! A joiner's survival actions, run through the host's authority step.
use super::*;

pub(super) fn floor() -> (World, Player) {
    let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
    let mut edits = Vec::new();
    for x in 0..16 {
        for z in 0..16 {
            edits.push((IVec3::new(x, 199, z), BlockId::Dirt));
        }
    }
    world.set_many(&edits);
    world.adventure.begin_journey();
    world.ensure_starter_wand();
    world.sync_from_inventory();
    let mut body = Player::new();
    body.pos = Vec3::new(8.5, 200.0, 5.5);
    body.precise_pos = body.pos.as_dvec3();
    (world, body)
}

fn run(world: &mut World, body: &mut Player, controls: &Controls, steps: usize) {
    let mut history = world::edit_history::EditHistory::new(world::edit_history::REMOTE_CELLS);
    run_as(world, body, controls, false, &mut history, steps);
}

pub(super) fn run_as(world: &mut World, body: &mut Player, controls: &Controls, creative: bool,
    history: &mut world::edit_history::EditHistory, steps: usize) {
    let tools = actions::Tools::default();
    let (mut cooldown, mut changes, mut blocks, mut chunks) = (0.0, BTreeMap::new(), HashSet::new(), HashSet::new());
    let mut mining = world::mining::Progress::default();
    let mut sim = crate::simulation::WorldSimulation::new();
    let mut pending = authority::Pending::default();
    for _ in 0..steps {
        authority::step(world, body, controls, &tools, creative, true, &mut cooldown, &mut mining, history, STEP, &mut changes, &mut blocks, &mut chunks, &mut sim, &mut pending);
    }
}

#[test]
fn a_joiner_mines_the_cell_it_looks_at_after_the_hold_time() {
    let (mut world, mut body) = floor();
    let under = IVec3::new(8, 199, 5);
    assert!(!world.get(under).is_empty());
    let controls = Controls { seq: 1, pitch: -1.5, mine: true, ..Default::default() };
    run(&mut world, &mut body, &controls, 1);
    assert!(!world.get(under).is_empty(), "a survival break takes its hold time");
    run(&mut world, &mut body, &controls, 240);
    assert!(world.get(under).is_empty(), "mined cell is {:?}", world.get(under));
}

#[test]
fn a_joiner_places_from_its_hand_slot() {
    let (mut world, mut body) = floor();
    let slot = 8;
    world.inventory.hotbar[slot] = Some(world::inventory::ItemStack::new(world::inventory::Item::Block(BlockId::Dirt), 8));
    world.sync_from_inventory();
    let controls = Controls { seq: 1, slot: slot as u8, pitch: -0.9, place: true, ..Default::default() };
    run(&mut world, &mut body, &controls, 2);
    let after = world.inventory.hotbar[slot].map_or(0, |s| s.count);
    let hotbar: Vec<_> = world.inventory.hotbar.iter().map(|s| s.map(|s| (s.item.name(), s.count))).collect();
    assert_eq!(after, 7, "one dirt placed from slot {slot}: {hotbar:?}");
}

