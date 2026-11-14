//! Focus and interaction for authored Undercrown mechanisms. From generator
//! 32 a mechanism writes the voxels it changes (a stair, a bridge, a drained
//! hall, an opened door) and a claimed plinth grants its reward.
use engine_worldgen::undercrown::{Action, Plan, Prop, PropKind, State, flags, v2};
use crate::player::Player;
use crate::world::interaction::Prompt;
use crate::world::{World, trace};
use glam::{IVec3, Vec3};

pub const INTERACT_REACH: f32 = 3.4;
pub const INTERACT_AIM_RADIUS: f32 = 0.55;
pub const FOCUS_HEIGHT: f32 = 0.9;
pub const ACTION_COOLDOWN: f32 = 0.35;

fn lens_flooded(plan: &Plan, world: &World, prop: &Prop, current: u32) -> bool {
    prop.kind == PropKind::ObservatoryLens && current & flags::RESTORED == 0
        && plan.v2.as_ref().and_then(|layout| layout.flooded_observatory())
            .is_some_and(|gallery| gallery.lens_contacts(world.voxel_scale.meters()).into_iter().any(|m| {
                world.water_fill((m / world.voxel_scale.meters()).floor().as_ivec3()) > 0.125
            }))
}

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
                "[F] Recall"
            } else {
                "[F] Examine"
            }
        }
        Action::Operate => {
            if state & flags::OPERATED != 0 {
                "Already set"
            } else {
                "[F] Operate"
            }
        }
        Action::Repair => {
            if state & flags::REPAIRED != 0 {
                "Repair complete"
            } else {
                "[F] Repair"
            }
        }
        Action::Open => "[F] Open",
        Action::Close => "[F] Close",
        Action::Claim => {
            if state & flags::CLAIMED != 0 {
                "Already claimed"
            } else {
                "[F] Claim"
            }
        }
        Action::Restore => {
            if state & flags::RESTORED != 0 {
                "Restored"
            } else {
                "[F] Restore"
            }
        }
        Action::Ring => "[F] Ring",
    }
}

/// Point the focus ray aims at: the centre of the prop's stamped model, or
/// the prop point itself on the box plan, which stamps nothing.
fn focus_target(plan: &Plan, prop: &Prop) -> Vec3 {
    plan.model_box(prop).map_or(prop.pos + Vec3::Y * FOCUS_HEIGHT, |(lo, hi)| (lo + hi) * 0.5)
}

fn cell_box(world: &World, (lo, hi): (Vec3, Vec3)) -> (IVec3, IVec3) {
    let scale = world.voxel_scale.meters();
    ((lo / scale).floor().as_ivec3(), ((hi / scale).ceil().as_ivec3() - IVec3::ONE))
}

fn in_box(cell: IVec3, (lo, hi): (IVec3, IVec3)) -> bool {
    cell.cmpge(lo).all() && cell.cmple(hi).all()
}

/// True when the prop's model has at least one solid voxel left. A prop
/// whose model was carved, mined or never stamped offers no prompt.
fn model_visible(world: &World, cells: (IVec3, IVec3)) -> bool {
    let (lo, hi) = cells;
    (lo.y..=hi.y).any(|y| (lo.z..=hi.z).any(|z| (lo.x..=hi.x).any(|x| world.get(IVec3::new(x, y, z)).is_solid())))
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
        let target = focus_target(plan, prop);
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
        if let Some(model) = plan.model_box(prop) {
            let cells = cell_box(world, model);
            if !model_visible(world, cells) {
                continue;
            }
            // The ray ends inside the model, so only a hit outside it blocks.
            if trace::solid(world, eye, target).is_some_and(|(_, cell)| !in_box(cell, cells)) {
                continue;
            }
        } else if trace::solid(world, eye, target).is_some() {
            continue;
        }
        let current = state.get(prop.id);
        best = Some((
            index,
            Prompt {
                title: prop.label,
                action: if lens_flooded(plan, world, prop, current) { "Lens mount flooded" }
                    else { action_text(prop.action(current), current) },
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
    if lens_flooded(plan, world, &prop, current) {
        state.cooldown = ACTION_COOLDOWN;
        return Outcome { used: true, message: Some("Lens mount flooded"), ..Outcome::default() };
    }
    let before = v2::current(state);
    let action = prop.action(current);
    if action == Action::Claim && current & flags::CLAIMED == 0 && plan.v2.is_some()
        && !super::rewards::can_grant(world, prop.id)
    {
        state.cooldown = ACTION_COOLDOWN;
        return Outcome { used: true, message: Some("Pickup Limit · Claim Pending"), ..Outcome::default() };
    }
    let message = match action {
        Action::Inspect => {
            let teaches_transfer = world.generator_version >= v2::observatory::ENCOUNTER_VERSION
                && prop.kind == PropKind::Marker && prop.id.district() == 1 && prop.id.room() == 9;
            if teaches_transfer && world.adventure.spell_log.add(95) {
                state.set(prop.id, flags::DISCOVERED);
                "Water transfer discovered"
            } else if current & flags::DISCOVERED != 0 {
                "Detail Known"
            } else {
                state.set(prop.id, flags::DISCOVERED);
                "Detail Recorded"
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
                let granted = if plan.v2.is_some() {
                    super::rewards::grant(world, prop.id, prop.pos)
                } else {
                    super::rewards::Granted::default()
                };
                match (granted.spell, granted.wand) {
                    (true, true) => "Spell discovered · Wand found",
                    (true, false) => "Spell discovered",
                    (false, true) => "Wand found",
                    (false, false) if prop.kind == PropKind::Lectern => "Spell already known",
                    (false, false) => "Claimed.",
                }
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
        m if m == v2::Mech::BELL => "Bell Rung · Lower Stair Restored",
        m if m == v2::Mech::STAIR => "Stair Restored",
        m if m == v2::Mech::BRIDGE => "Tower Bridge Extended",
        m if m == v2::Mech::WINCH => "Lower Portcullis Open",
        m if m == v2::Mech::SLUICE => "Courthouse Draining",
        m if m == v2::Mech::COOLING => "Lava Channel Cooled",
        m if m == v2::Mech::LENS => "East Seal Open",
        m if m == v2::Mech::FREIGHT => "Furnace Crossing Ready",
        _ if after.0 & !before.0 == 0 => "Water Level Restored",
        _ => "Mechanism Set",
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

    /// A player standing at `from` (metres) and looking at the prop's model.
    fn face(plan: &Plan, world: &World, prop: Prop, from: Vec3) -> Player {
        let mut player = Player::new();
        player.pos = from / world.voxel_scale.meters();
        let eye = player.eye(world.voxel_scale) * world.voxel_scale.meters();
        let forward = (focus_target(plan, &prop) - eye).normalize();
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
        let player = face(&plan, &world, bell, bell.pos + Vec3::new(1.6, -0.2, 0.0));
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
        let player = face(&plan, &world, lens, lens.pos + Vec3::new(1.8, -0.2, 0.0));
        let restored = interact_with_plan(&plan, &mut state, &mut world, &player);
        assert!(restored.used && restored.changed.contains(&cell));
        assert_eq!(world.get(cell), open);

        // Claiming the gloves grants them once.
        state.cooldown = 0.0;
        let gloves = find("Bellkeeper's Gloves");
        let player = face(&plan, &world, gloves, gloves.pos + Vec3::new(-1.8, -0.2, 0.0));
        let before = world.inventory.pack.iter().chain(world.inventory.hotbar.iter()).flatten().count();
        assert!(interact_with_plan(&plan, &mut state, &mut world, &player).used);
        let after = world.inventory.pack.iter().chain(world.inventory.hotbar.iter()).flatten().count();
        assert!(after > before, "claim granted nothing");
        state.cooldown = 0.0;
        interact_with_plan(&plan, &mut state, &mut world, &player);
        let again = world.inventory.pack.iter().chain(world.inventory.hotbar.iter()).flatten().count();
        assert_eq!(again, after, "a second claim granted more");
    }

    /// The Bellkeeper's plinth on a generator-32 plan, the player facing it.
    fn plinth_scene(scale: VoxelScalePreset) -> (std::rc::Rc<Plan>, World, Prop, Vec3, Vec3) {
        let world = World::with_seed(WorldType::Hills, scale, 424242);
        let plan = super::super::plan(&world.sample_config());
        let prop = *plan.props.iter().find(|p| p.label == "Bellkeeper's Gloves").unwrap();
        let player = face(&plan, &world, prop, prop.pos + Vec3::new(-1.8, -0.2, 0.0));
        let eye = player.eye(world.voxel_scale) * world.voxel_scale.meters();
        (plan, world, prop, eye, player.forward_dir())
    }

    fn clear_model(plan: &Plan, world: &mut World, prop: Prop) {
        let (lo, hi) = cell_box(world, plan.model_box(&prop).unwrap());
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    world.set(IVec3::new(x, y, z), BlockId::Air);
                }
            }
        }
    }

    fn assert_prompt_follows_model(scale: VoxelScalePreset) {
        let (plan, mut world, prop, eye, forward) = plinth_scene(scale);
        let state = State::default();
        let focused = focus_with_plan(&plan, &state, &world, eye, forward).map(|(i, _)| plan.props[i].id);
        assert_eq!(focused, Some(prop.id), "the stamped plinth was not focused");
        clear_model(&plan, &mut world, prop);
        assert!(focus_with_plan(&plan, &state, &world, eye, forward).is_none(), "a prompt floats over air");
    }

    #[test]
    fn prompt_needs_a_visible_model() {
        assert_prompt_follows_model(VoxelScalePreset::OneMeter);
    }

    #[test]
    fn prompt_needs_a_visible_model_at_half_metre() {
        assert_prompt_follows_model(VoxelScalePreset::HalfMeter);
    }

    #[test]
    fn invisible_prop_cannot_be_used() {
        let (plan, mut world, prop, _, _) = plinth_scene(VoxelScalePreset::OneMeter);
        let player = face(&plan, &world, prop, prop.pos + Vec3::new(-1.8, -0.2, 0.0));
        clear_model(&plan, &mut world, prop);
        let mut state = State::default();
        assert!(!interact_with_plan(&plan, &mut state, &mut world, &player).used);
        assert_eq!(state.get(prop.id), 0);
    }
}
