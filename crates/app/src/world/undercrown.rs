//! Player-facing undercrown interaction. Generation (plan, blocks, reserved
//! volumes) lives in `engine_worldgen::undercrown`; this module re-exports it.
mod interact;

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

/// Runs the focused mechanism and applies any voxel edits. State is committed
/// only after the edit set has been applied, so a save cannot claim a repair
/// whose geometry was never written.
pub(crate) fn interact(world: &mut super::World, player: &Player) -> Outcome {
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
