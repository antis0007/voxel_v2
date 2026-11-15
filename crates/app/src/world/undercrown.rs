//! Player-facing undercrown interaction. Generation (plan, blocks, reserved
//! volumes) lives in `engine_worldgen::undercrown`; this module re-exports it.
mod interact;
mod rewards;
mod freight;

pub use engine_worldgen::undercrown::{FIRST_VERSION, State, plan};
#[cfg(test)]
pub use engine_worldgen::undercrown::{ObjectId, flags, reserved};
pub use interact::Outcome;

use crate::player::Player;
use glam::Vec3;

/// Shared prompt focus. Reliquaries take priority: the caller only asks for a
/// mechanism prompt when no relic is focused.
pub(crate) fn focus(
    world: &super::World,
    eye: Vec3,
    forward: Vec3,
) -> Option<super::interaction::Prompt> {
    if let Some(prompt)=freight::focus(world,eye,forward) {return Some(prompt);}
    if world.generator_version < FIRST_VERSION {
        return None;
    }
    interact::focus_with_plan(
        &plan(&world.sample_config()),
        &world.undercrown,
        world,
        eye,
        forward,
    )
    .map(|(_, prompt)| prompt)
}

thread_local! {
    /// Raw id of the room the player last stood in; 0 outside the Undercrown.
    static LAST_ROOM: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// On entering an Undercrown room, name it on the message line and count the
/// finds still unclaimed there, so exploring has a readable goal. Never
/// overrides a message already showing.
pub(crate) fn announce_room(world: &mut super::World, feet_m: Vec3) {
    if world.generator_version < FIRST_VERSION {
        return;
    }
    let plan = plan(&world.sample_config());
    let Some(room) = plan.room_at(feet_m) else {
        LAST_ROOM.with(|last| last.set(0));
        return;
    };
    let key = room.id.raw();
    if LAST_ROOM.with(|last| last.replace(key)) == key || world.expedition.message_seconds > 0.0 {
        return;
    }
    let unclaimed = plan
        .props
        .iter()
        .filter(|p| p.id.district() == room.id.district() && p.id.room() == room.id.room())
        .filter(|p| {
            rewards::has_reward(p.id)
                && world.undercrown.get(p.id) & engine_worldgen::undercrown::flags::CLAIMED == 0
        })
        .count();
    let text = match unclaimed {
        0 => room.name.to_string(),
        1 => format!("{} · 1 Find", room.name),
        n => format!("{} · {n} Finds", room.name),
    };
    world.expedition.say_for(text, 3.5);
}

/// Rewards to draw above unclaimed plinths.
pub(crate) fn plinth_displays(world: &super::World) -> Vec<(Vec3, super::inventory::Item)> {
    if world.generator_version < FIRST_VERSION {
        return Vec::new();
    }
    rewards::plinth_displays(&plan(&world.sample_config()), &world.undercrown)
}

/// Runs the focused mechanism and applies any voxel edits. State is committed
/// only after the edit set has been applied, so a save cannot claim a repair
/// whose geometry was never written.
pub(crate) fn interact(world: &mut super::World, player: &Player) -> Outcome {
    if let Some(outcome)=freight::interact(world,player) {return outcome;}
    if world.generator_version < FIRST_VERSION {
        return Outcome::default();
    }
    let cfg = world.sample_config();
    let plan = plan(&cfg);
    if plan.is_empty() {
        return Outcome::default();
    }
    let mut state = std::mem::take(&mut world.undercrown);
    let outcome = interact::interact_with_plan(&plan, &mut state, world, player);
    world.undercrown = state;
    outcome
}
