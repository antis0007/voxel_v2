//! Executable per-step gameplay order shared by the window and headless drivers.
//! Tools enter between `gameplay` and `finish`; no elapsed-time batching here.
use super::{Budget, STEP_SECONDS, WorldSimulation};
use crate::{
    player::{Input, Player, update_player},
    world::{self, DayNightCycle, World},
};
use glam::IVec3;
use std::collections::HashSet;

pub enum Hook {
    BeforePlayer,
    BeforeAdventure,
}

#[derive(Default)]
pub struct Actions {
    pub cast: bool,
    pub cycle: bool,
    pub melee: bool,
}

/// The callback only supplies driver-specific input/review hooks. All gameplay
/// system calls and their fixed duration live here, rather than in stage names.
pub fn gameplay(
    world: &mut World,
    player: &mut Player,
    input: &mut Input,
    move_player: bool,
    mut hook: impl FnMut(Hook, &mut World, &mut Player, &mut Input) -> Actions,
) -> bool {
    // The CPU development region owns an isolated headless session. Unmigrated
    // factory/spell writers are suspended there, not run against its mirror.
    let legacy = world.ca_view.is_none();
    if legacy {
        world.expedition.tick(STEP_SECONDS);
        let size = world::factory::step(world);
        world.factory.tick(STEP_SECONDS, size);
    }
    hook(Hook::BeforePlayer, world, player, input);
    let riding = legacy && world::travel::step(world, player, input, move_player, STEP_SECONDS);
    if move_player && !riding {
        if !world.rules.mode.flight() {
            // A cleared double-tap window can never toggle flight on.
            player.flying = false;
            player.jump_tap_timer = 0.0;
        }
        if !world.rules.mode.interacts() {
            player.flying = true;
            player.fly_mode = crate::player::FlyMode::Noclip;
        }
        update_player(player, input, world, STEP_SECONDS);
        let scale = world.voxel_scale.meters();
        if let Some(feet_m) = world.rules.border_clamp(player.pos * scale) {
            player.pos = feet_m / scale;
            player.precise_pos = player.pos.as_dvec3();
            player.vel.x = 0.0;
            player.vel.z = 0.0;
        }
    }
    let actions = hook(Hook::BeforeAdventure, world, player, input);
    let consumed = legacy
        && world::adventure::tick(
            world,
            player,
            STEP_SECONDS,
            actions.cast,
            actions.cycle,
            actions.melee,
        );
    crate::player::viewmodel::tick(player, world, input, STEP_SECONDS);
    input.cast_pressed = false;
    input.cycle_spell = false;
    input.place_pressed = false;
    input.break_pressed = false;
    consumed
}

pub fn finish(
    sim: &mut WorldSimulation,
    world: &mut World,
    player: &Player,
    day: &mut DayNightCycle,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
    budget: Budget,
    settle: bool,
    advance_day: bool,
) {
    world.sandbox.climate.sync(world.sample_config(), day.time_of_day(), day.cloud_controls().coverage);
    super::storms::tick(world, player, day, budget);
    if world.ca_view.is_none() {
        let eye = player.eye(world.voxel_scale) * world.voxel_scale.meters();
        world::block_data::tnt::tick(world, eye, blocks, chunks, budget);
        world::stations::brewing::tick(world);
    }
    sim.update(world, player, STEP_SECONDS, blocks, chunks, budget);
    if settle {
        sim.settle(world);
    }
    if advance_day && world.rules.daylight_cycle && sim.authority_error.is_none() {
        day.advance(STEP_SECONDS);
    }
}
