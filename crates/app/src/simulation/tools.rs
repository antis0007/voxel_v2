use crate::{player::{Input, Player}, simulation::{WorldSimulation, fixed},
    world::{self, World, inventory::Item}};
use glam::{IVec3, Vec3};

pub fn select(world: &mut World, player: &mut Player, slot: usize) {
    let slot = slot.min(world::inventory::HOTBAR_SLOTS - 1);
    world.adventure.hand_slot = slot;
    player.selected_block = world.inventory.hotbar[slot]
        .and_then(|stack| stack.item.placeable_block()).unwrap_or(world::BlockId::Air);
}

/// Run player movement and admitted terrain tools on the published view.
pub fn step(sim: &mut WorldSimulation, world: &mut World, player: &mut Player,
    input: &mut Input, slot: usize) -> Option<IVec3> {
    let place = input.place_block || input.place_pressed;
    let mine = input.break_block || input.break_pressed;
    let pick = input.pick_block;
    input.pick_block = false;
    let slot = slot.min(world::inventory::HOTBAR_SLOTS - 1);
    select(world, player, slot);
    fixed::gameplay(world, player, input, true, |_, _, _, _| fixed::Actions::default());
    world.expedition.message_seconds = (world.expedition.message_seconds - crate::simulation::STEP_SECONDS).max(0.0);
    let hit = world::raycast_block_precise(world, player.precise_eye(world.voxel_scale),
        player.forward_dir(), 5.0 / world.voxel_scale.meters());
    let region = sim.cpu_region.as_mut()?;
    region.tool_slot = slot;
    if pick && let Some((at, _)) = hit {
        let block = world.get(at);
        if let Some(index) = world.inventory.hotbar.iter().position(|s|
            s.is_some_and(|s| s.item == Item::Block(block) && s.count > 0)) {
            region.tool_slot = index;
        }
    }
    let slot = region.tool_slot;
    select(world, player, slot);
    let (at, previous) = hit?;
    if !(mine || place && !player.traversal.sling) || sim.player_tick.0 < region.tool_after { return Some(at); }
    region.tool_after = sim.player_tick.0.saturating_add(12);
    let result = if mine {
        region.mine_owned(world, sim.player_tick.0, at)
    } else {
        let (min, max) = world::player_aabb(player.pos, world.voxel_scale, player.scale, player.crouch_t);
        let cell_min = previous.as_vec3();
        if min.cmplt(cell_min + Vec3::ONE).all() && max.cmpgt(cell_min).all() {
            Err("placement intersects player".into())
        } else {
            region.place_owned(world, sim.player_tick.0, previous, slot)
        }
    };
    match result {
        Ok(command) => {
            eprintln!("[ca-tool] accepted sequence={} at={}", command.sequence, command.at);
            world.expedition.say(if mine { "Mining pending" } else { "Placement pending" });
        }
        Err(error) => {
            eprintln!("[ca-tool] rejected: {error}");
            world.expedition.say(if mine { "Mining blocked" } else { "Placement blocked" });
        }
    }
    Some(at)
}
