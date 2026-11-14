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
/// Movement kit cues (dash, slide, air jump, hard landing) into the shared
/// sound queue. Non-positional: they are the listener's own body.
fn movement_sounds(world: &mut World, player: &mut Player) {
    use crate::{audio::{Cue, Event}, player::movement as mv};
    let events = player.movement.take_events();
    if events == 0 {
        return;
    }
    let land_gain = (player.movement.fall_speed / 30.0).clamp(0.45, 1.0);
    for (bit, cue, gain, pitch) in [
        (mv::EVENT_DASH, Cue::Dash, 0.70, 1.0),
        (mv::EVENT_SLIDE, Cue::Slide, 0.55, 1.0),
        (mv::EVENT_AIR_JUMP, Cue::AirJump, 0.55, 1.0),
        (mv::EVENT_LAND, Cue::Land, land_gain, 0.95),
        // Wall run reuses the gritty slide bed, pitched up into a skid.
        (mv::EVENT_WALLRUN, Cue::Slide, 0.45, 1.35),
        (mv::EVENT_WALLJUMP, Cue::AirJump, 0.65, 0.85),
    ] {
        if events & bit != 0 && world.adventure.sounds.len() < 64 {
            world.adventure.sounds.push(Event { cue, pos: None, gain, pitch });
        }
    }
}

pub fn gameplay(
    world: &mut World,
    player: &mut Player,
    input: &mut Input,
    move_player: bool,
    mut hook: impl FnMut(Hook, &mut World, &mut Player, &mut Input) -> Actions,
) -> bool {
    // The development region owns an isolated application session. Unmigrated
    // factory/spell writers are suspended there, not run against its mirror.
    let mut trace=crate::gameplay_trace::Trace::new();
    let legacy = world.ca_view.is_none();
    if legacy {
        world.mining_fractures.tick(STEP_SECONDS);
        world.expedition.tick(STEP_SECONDS);
        // A world without the machines system never runs a factory tick.
        if world.rules.systems.automation() { world::factory::tick_power(world, STEP_SECONDS); }
    }
    trace.mark("factory-clock");
    hook(Hook::BeforePlayer, world, player, input);
    world.adventure.creative = world.rules.mode.creative_tools();
    let riding = legacy && (world::orbital::express::step(world, player, input, move_player)
        || world::travel::step(world, player, input, move_player, STEP_SECONDS));
    if move_player && !riding && !world.adventure.is_dead() {
        // Worn Kinetic greaves / boots gate the fast traversal. OR keeps any
        // grant a driver made (tests, reviews) on top of the gear.
        player.gear = world.inventory.mobility();
        player.gear.thrusters |= player.gear_override.thrusters;
        player.gear.wallrun |= player.gear_override.wallrun;
        player.glide.available = world.inventory.glide_frame();
        crate::player::traversal::prepare(world, player, input, STEP_SECONDS);
        if !world.rules.mode.flight() {
            // A cleared double-tap window can never toggle flight on.
            player.flying = false;
            player.jump_tap_timer = 0.0;
            if world.sample_config().is_hell() && player.anchor.take().is_some() {
                let scale = world.voxel_scale.meters();
                let mut feet = world.adventure.respawn_point
                    .filter(|p| p.is_finite()).map_or_else(|| world.suggested_spawn(), |p| p / scale);
                for _ in 0..32 {
                    let (lo, hi) = world::player_aabb(feet, world.voxel_scale, player.scale, 0.0);
                    if !world::collides_world(world, lo, hi) { break; }
                    feet.y = feet.y.floor() + 1.0;
                }
                player.pos = feet;
                player.precise_pos = feet.as_dvec3();
                player.vel = glam::Vec3::ZERO;
                player.gravity_frame = glam::Quat::IDENTITY;
                world.expedition.say("Returned to Hell: Flight Unavailable");
            }
        }
        let spectating = !world.rules.mode.interacts();
        if spectating {
            player.flying = true;
            player.fly_mode = crate::player::FlyMode::Noclip;
        }
        let before = world::portals::traverse::before(world, player);
        update_player(player, input, world, STEP_SECONDS);
        if player.manyfold_crossing == 0 && player.anchor.is_none() {
            world::portals::traverse::step(world, player, before);
        }
        crate::manyfold::weave::feedback(world, player);
        if legacy { crate::manyfold::undercroft::tick(world, player, STEP_SECONDS); }
        let scale = world.voxel_scale.meters();
        if player.anchor.is_none() && let Some(feet_m) = world.rules.border_clamp(player.pos * scale) {
            player.pos = feet_m / scale;
            player.precise_pos = player.pos.as_dvec3();
            player.anchor = None;
            player.gravity_frame = glam::Quat::IDENTITY;
            player.vel.x = 0.0;
            player.vel.z = 0.0;
        }
        if spectating {
            let _ = player.take_pending_harm();
        }
        movement_sounds(world, player);
        // Environmental harm is decided by the player step and applied here, where
        // health lives. The host path runs this same call, so authority matches.
        if let Some((amount, cause)) = player.take_pending_harm() {
            let burning = matches!(cause, world::adventure::death::Cause::Contact(_));
            if !(burning && world.progression.effects.fire_immune()) {
                world.adventure.environment_damage(amount, cause);
            }
        }
    }
    trace.mark("player");
    if legacy { world::mill::tick(world, player); }
    if legacy { world::industry::presentation::player_exposure(world, player, STEP_SECONDS); }
    let on_hell_moon = player.anchor.is_some() && world.sample_config().is_hell();
    let actions = if on_hell_moon { Actions::default() } else { hook(Hook::BeforeAdventure, world, player, input) };
    if legacy { world::orbital::cargo::tick(world, player); }
    trace.mark("input-hooks");
    if legacy {
        let mut gates = std::mem::take(&mut world.portals);
        gates.audit(world);
        world.portals = gates;
    }
    world.adventure.shield_raised = input.place_block && !player.traversal.sling;
    let consumed = legacy
        && world::adventure::tick(
            world,
            player,
            STEP_SECONDS,
            actions.cast && world.rules.systems.magic(),
            actions.cycle,
            actions.melee,
        );
    trace.mark("adventure");
    crate::player::viewmodel::tick(player, world, input, STEP_SECONDS);
    world::pickups::tick(world, player, STEP_SECONDS);
    trace.mark("viewmodel-pickup");
    input.cast_pressed = false;
    input.dash_pressed = false;
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
    let mut trace=crate::gameplay_trace::Trace::new();
    world.environment.player_edit=false;
    let local = crate::planet::local_time(
        world.world_type,
        player.precise_eye(world.voxel_scale) * world.voxel_scale.meters() as f64,
        day,
    );
    world.sandbox.climate.sync(world.sample_config(), local, day.cloud_controls().coverage);
    if world.ca_view.is_none() {
        let eye = player.eye(world.voxel_scale) * world.voxel_scale.meters();
        world::block_data::tnt::tick(world, eye, blocks, chunks, budget);
        world::stations::brewing::tick(world);
        world::volcanism::tick(world, eye, day, blocks, chunks, budget);
    }
    world.sandbox.climate.set_day_index(day.day_index());
    trace.mark("pre-weather");
    super::storms::tick(world, player, day, budget);
    trace.mark("storms");
    crate::manyfold::tick(world, player, STEP_SECONDS, blocks, chunks);
    world::lightless_life::tick(world, player, day, STEP_SECONDS);
    world::orbital::habitat::tick(world, player);
    trace.mark("realm-and-habitat");
    if world.ca_view.is_none() { world::industry::tick(world, player, day, budget, blocks, chunks); }
    trace.mark("industrial-air");
    sim.update(world, player, STEP_SECONDS, blocks, chunks, budget);
    world::workshop::tick(world);
    trace.mark("world-simulation");
    if settle {
        sim.settle(world);
    }
    if advance_day && world.rules.daylight_cycle && sim.authority_error.is_none() {
        day.advance(STEP_SECONDS);
    }
    world.orbital.planet_tick = crate::planet::tick_of(day);
    if sim.authority_error.is_none() {world.orbital.dynamics.tick(world.seed,day,STEP_SECONDS);}
    trace.mark("orbital");
    world.publish_changes(blocks, chunks);
}
