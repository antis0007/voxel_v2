//! Focus and interaction for authored Undercrown mechanisms.
//!
//! This milestone wires the contract but ships no props, so the public path is
//! inert. Tests exercise the same functions with a fixture plan.
use super::plan::{Action, Plan};
use super::state::{State, flags};
use crate::player::Player;
use crate::world::interaction::Prompt;
use crate::world::{World, trace};
use glam::{IVec3, Vec3};

pub const INTERACT_REACH: f32 = 3.4;
pub const INTERACT_AIM_RADIUS: f32 = 0.55;
pub const FOCUS_HEIGHT: f32 = 0.9;
pub const ACTION_COOLDOWN: f32 = 0.35;

#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub used: bool,
    pub changed: Vec<IVec3>,
    pub message: Option<&'static str>,
}

fn action_text(action: Action, state: u32) -> &'static str {
    match action {
        Action::Inspect => {
            if state & flags::DISCOVERED != 0 {
                "F  Recall this detail"
            } else {
                "F  Examine"
            }
        }
        Action::Operate => {
            if state & flags::OPERATED != 0 {
                "Already set"
            } else {
                "F  Work the mechanism"
            }
        }
        Action::Repair => {
            if state & flags::REPAIRED != 0 {
                "Repair complete"
            } else {
                "F  Repair"
            }
        }
        Action::Open => "F  Open",
        Action::Close => "F  Close",
        Action::Claim => {
            if state & flags::CLAIMED != 0 {
                "Already claimed"
            } else {
                "F  Claim"
            }
        }
        Action::Restore => {
            if state & flags::RESTORED != 0 {
                "Restored"
            } else {
                "F  Restore"
            }
        }
        Action::Ring => "F  Ring",
    }
}

/// Nearest aimed, visible prop. Returns its index and the current prompt.
pub(super) fn focus_with_plan(
    plan: &Plan,
    state: &State,
    world: &World,
    eye: Vec3,
    forward: Vec3,
) -> Option<(usize, Prompt)> {
    if !eye.is_finite() || !forward.is_finite() {
        return None;
    }
    let length_squared = forward.length_squared();
    // Rejects NaN as well as zero; keep the negated form on purpose.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    if !(length_squared > 0.0) {
        return None;
    }
    let forward = forward / length_squared.sqrt();
    let mut best = None;
    let mut best_distance_squared = INTERACT_REACH * INTERACT_REACH;
    for (index, prop) in plan.props.iter().enumerate() {
        let target = prop.pos + Vec3::Y * FOCUS_HEIGHT;
        let offset = target - eye;
        let distance_squared = offset.length_squared();
        let along = offset.dot(forward);
        if !(distance_squared < best_distance_squared && along > 0.0) {
            continue;
        }
        if (offset - forward * along).length_squared() >= INTERACT_AIM_RADIUS * INTERACT_AIM_RADIUS
        {
            continue;
        }
        if trace::solid(world, eye, target).is_some() {
            continue;
        }
        let current = state.get(prop.id);
        best = Some((
            index,
            Prompt {
                title: prop.label,
                action: action_text(prop.action(current), current),
            },
        ));
        best_distance_squared = distance_squared;
    }
    best
}

pub(super) fn interact_with_plan(
    plan: &Plan,
    state: &mut State,
    world: &mut World,
    player: &Player,
) -> Outcome {
    if state.cooldown > 0.0 {
        return Outcome::default();
    }
    let scale = world.voxel_scale.meters();
    let eye = player.eye(world.voxel_scale) * scale;
    let forward = player.forward_dir();
    let Some((index, _)) = focus_with_plan(plan, state, world, eye, forward) else {
        return Outcome::default();
    };
    let prop = plan.props[index];
    let current = state.get(prop.id);
    let action = prop.action(current);
    let message = match action {
        Action::Inspect => {
            if current & flags::DISCOVERED != 0 {
                "You already know this detail."
            } else {
                state.set(prop.id, flags::DISCOVERED);
                "You commit the detail to memory."
            }
        }
        Action::Operate => {
            if current & flags::OPERATED != 0 {
                "The mechanism is already set."
            } else {
                state.set(prop.id, flags::OPERATED);
                "The mechanism moves."
            }
        }
        Action::Repair => {
            if current & flags::REPAIRED != 0 {
                "This repair already holds."
            } else {
                state.set(prop.id, flags::REPAIRED);
                "The repair holds."
            }
        }
        Action::Open => {
            state.set(prop.id, flags::OPEN);
            "The gate opens."
        }
        Action::Close => {
            state.clear(prop.id, flags::OPEN);
            "The gate closes."
        }
        Action::Claim => {
            if current & flags::CLAIMED != 0 {
                "Nothing remains here."
            } else {
                state.set(prop.id, flags::CLAIMED);
                "Claimed."
            }
        }
        Action::Restore => {
            if current & flags::RESTORED != 0 {
                "This system already answers."
            } else {
                state.set(prop.id, flags::RESTORED);
                "The system answers."
            }
        }
        Action::Ring => {
            state.set(prop.id, flags::RESTORED | flags::OPERATED);
            "The sound travels."
        }
    };
    state.cooldown = ACTION_COOLDOWN;
    Outcome {
        used: true,
        changed: Vec::new(),
        message: Some(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::{BlockId, WorldType};

    fn world() -> World {
        World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 17)
    }

    #[test]
    fn focus_requires_reach_aim_and_line_of_sight() {
        let plan = Plan::fixture();
        let world = world();
        let state = State::default();
        let prop = plan.props[0];
        let target = prop.pos + Vec3::Y * FOCUS_HEIGHT;
        let eye = prop.pos + Vec3::new(0.0, 1.62, 2.0);
        let forward = (target - eye).normalize();

        assert!(focus_with_plan(&plan, &state, &world, eye, forward).is_some());

        let too_far = eye + Vec3::new(0.0, 0.0, 2.5);
        assert!(focus_with_plan(&plan, &state, &world, too_far, forward).is_none());

        let off_aim = Vec3::new(0.7, forward.y, forward.z).normalize();
        assert!(focus_with_plan(&plan, &state, &world, eye, off_aim).is_none());

        let mut blocked = self::world();
        let wall = (prop.pos + Vec3::new(0.0, 0.9, 1.0)).floor().as_ivec3();
        for dx in -1..=1 {
            for dy in -1..=1 {
                blocked.set(IVec3::new(wall.x + dx, wall.y + dy, wall.z), BlockId::Stone);
            }
        }
        assert!(focus_with_plan(&plan, &state, &blocked, eye, forward).is_none());
    }

    #[test]
    fn one_shot_is_idempotent_and_failed_focus_does_not_mutate() {
        let plan = Plan::fixture();
        let mut world = world();
        let mut state = State::default();
        let prop = plan.props[0];
        let mut player = Player::new();
        player.pos = prop.pos + Vec3::new(0.0, 0.0, 2.0);
        let eye = player.eye(world.voxel_scale) * world.voxel_scale.meters();
        let target = prop.pos + Vec3::Y * FOCUS_HEIGHT;
        let forward = (target - eye).normalize();
        player.yaw = forward.x.atan2(-forward.z);
        player.pitch = forward.y.asin();

        let first = interact_with_plan(&plan, &mut state, &mut world, &player);
        assert!(first.used);
        assert!(first.changed.is_empty());
        assert_eq!(state.get(prop.id), flags::DISCOVERED);

        state.cooldown = 0.0;
        let second = interact_with_plan(&plan, &mut state, &mut world, &player);
        assert!(second.used);
        assert_eq!(state.get(prop.id), flags::DISCOVERED);
        assert!(state.cooldown > 0.0);

        state.cooldown = 0.0;
        player.pos += Vec3::new(0.0, 0.0, 20.0);
        let failed = interact_with_plan(&plan, &mut state, &mut world, &player);
        assert!(!failed.used);
        assert_eq!(state.get(prop.id), flags::DISCOVERED);
    }

    #[test]
    fn prompt_text_reflects_state() {
        let plan = Plan::fixture();
        let world = world();
        let prop = plan.props[0];
        let target = prop.pos + Vec3::Y * FOCUS_HEIGHT;
        let eye = prop.pos + Vec3::new(0.0, 1.62, 2.0);
        let forward = (target - eye).normalize();
        let mut state = State::default();
        let before = focus_with_plan(&plan, &state, &world, eye, forward)
            .map(|(_, prompt)| prompt)
            .unwrap();
        assert_eq!(before.title, "Test marker");
        assert_eq!(before.action, "F  Examine");
        state.set(prop.id, flags::DISCOVERED);
        let after = focus_with_plan(&plan, &state, &world, eye, forward)
            .map(|(_, prompt)| prompt)
            .unwrap();
        assert_eq!(after.action, "F  Recall this detail");
    }
}
