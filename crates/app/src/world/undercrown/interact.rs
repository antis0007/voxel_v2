//! Focus and interaction for authored Undercrown mechanisms. From generator
//! 32 a mechanism writes the voxels it changes (a stair, a bridge, a drained
//! hall, an opened door) and a claimed plinth grants its reward.
use engine_worldgen::undercrown::{Action, Plan, State, flags, v2};
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
    let before = v2::current(state);
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
                if plan.v2.is_some() {
                    super::rewards::grant(world, prop.id, prop.pos);
                }
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
    let after = v2::current(state);
    let mut changed = Vec::new();
    let mut message = message;
    if before != after
        && let Some(layout) = &plan.v2
    {
        let edits = layout.edits(&world.sample_config(), before, after);
        changed = edits.iter().filter(|(p, b)| world.get(*p) != *b).map(|(p, _)| *p).collect();
        world.set_many_mechanism(&edits);
        message = mechanism_message(before, after);
    }
    Outcome {
        used: true,
        changed,
        message: Some(message),
    }
}

/// What the player hears when a mechanism moves.
fn mechanism_message(before: v2::Mech, after: v2::Mech) -> &'static str {
    let gained = v2::Mech(after.0 & !before.0);
    match gained {
        m if m == v2::Mech::BELL => "The bell's note runs down the stone. Far off, a stair settles back into place.",
        m if m == v2::Mech::STAIR => "The platform takes the weight. The stair is whole again.",
        m if m == v2::Mech::BRIDGE => "The lift swings out and locks: a bridge to the tower.",
        m if m == v2::Mech::WINCH => "The chain takes up. Below, a portcullis grinds open.",
        m if m == v2::Mech::SLUICE => "Water roars into the bypass. The courthouse is draining.",
        m if m == v2::Mech::COOLING => "Cold water floods the channel. The lava crusts over.",
        m if m == v2::Mech::LENS => "Light gathers in the lens and falls somewhere east. A seal lets go.",
        m if m == v2::Mech::FREIGHT => "The platform locks into its rails. A crossing waits at the furnace.",
        _ if after.0 & !before.0 == 0 => "The water finds its old level.",
        _ => "The mechanism moves.",
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

    fn face(world: &World, prop: engine_worldgen::undercrown::Prop, from: Vec3) -> Player {
        let mut player = Player::new();
        player.pos = from;
        let eye = player.eye(world.voxel_scale) * world.voxel_scale.meters();
        let forward = (prop.pos + Vec3::Y * FOCUS_HEIGHT - eye).normalize();
        player.yaw = forward.x.atan2(-forward.z);
        player.pitch = forward.y.asin();
        player
    }

    #[test]
    fn generator_32_mechanisms_write_their_voxels_and_the_seal_opens_only_for_the_lens() {
        let mut world = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 424242);
        let plan = super::super::plan(&world.sample_config());
        let layout = plan.v2.as_ref().expect("generator 32 plan");
        let mut state = State::default();
        let find = |label: &str| *plan.props.iter().find(|p| p.label == label).unwrap();

        // The archive door is tombstone; a player edit cannot open it.
        let door = layout.edits(&world.sample_config(), v2::Mech::default(), v2::Mech::LENS);
        let (cell, open) = door[0];
        assert_eq!(world.get(cell), BlockId::TombStone);
        world.set_many(&[(cell, BlockId::Air)]);
        assert_eq!(world.get(cell), BlockId::TombStone);

        // Ringing the bell restores the listening stair.
        let bell = find("Bell clapper");
        let player = face(&world, bell, bell.pos + Vec3::new(1.6, -0.2, 0.0));
        let rung = interact_with_plan(&plan, &mut state, &mut world, &player);
        assert!(rung.used, "the clapper was not focused");
        assert!(!rung.changed.is_empty(), "ringing changed no voxels");
        for p in &rung.changed {
            assert!(world.get(*p).is_solid(), "a restored stair cell is not solid");
        }
        assert!(v2::current(&state).has(v2::Mech::BELL));

        // Restoring the lens opens the seal.
        state.cooldown = 0.0;
        let lens = find("Observatory lens");
        let player = face(&world, lens, lens.pos + Vec3::new(1.8, -0.2, 0.0));
        let restored = interact_with_plan(&plan, &mut state, &mut world, &player);
        assert!(restored.used && restored.changed.contains(&cell));
        assert_eq!(world.get(cell), open);

        // Claiming the gloves grants them once.
        state.cooldown = 0.0;
        let gloves = find("Bellkeeper's Gloves");
        let player = face(&world, gloves, gloves.pos + Vec3::new(-1.8, -0.2, 0.0));
        let before = world.inventory.pack.iter().chain(world.inventory.hotbar.iter()).flatten().count();
        assert!(interact_with_plan(&plan, &mut state, &mut world, &player).used);
        let after = world.inventory.pack.iter().chain(world.inventory.hotbar.iter()).flatten().count();
        assert!(after > before, "claim granted nothing");
        state.cooldown = 0.0;
        interact_with_plan(&plan, &mut state, &mut world, &player);
        let again = world.inventory.pack.iter().chain(world.inventory.hotbar.iter()).flatten().count();
        assert_eq!(again, after, "a second claim granted more");
    }
}
