//! Stationed residents intercept nearby threats and return to their posts.
use super::*;

const DEFENSE_RADIUS: f32 = 10.0;
const REACH: f32 = 2.0;
pub(super) const STRIKE_INTERVAL: f32 = 0.85;
type GuardGoals = [Option<(usize, Vec3)>; crate::world::travel::MAX_GUARDS];

pub(super) fn defend(
    enemies: &mut [Enemy],
    world: &World,
    player: Vec3,
    dt: f32,
    sounds: &mut Vec<Sound>,
) -> GuardGoals {
    let mut goals = [None; crate::world::travel::MAX_GUARDS];
    if world.travel.guard_posts.is_empty() {
        return goals;
    }
    #[cfg(feature = "multiplayer")]
    if world.multiplayer.is_some() {
        return goals;
    }
    let mut next_goal = goals.iter_mut();
    for index in 0..enemies.len() {
        let guard = &enemies[index];
        let Some(post) =
            wildlife::vehicle::villager_id(guard).and_then(|id| world.travel.guard_posts.get(&id))
        else {
            continue;
        };
        let cell = (guard.pos / world.voxel_scale.meters()).floor().as_ivec3();
        if guard.health <= 0.0
            || !world
                .chunks
                .contains_key(&crate::world::chunk_coord_from_world_block(cell))
        {
            continue;
        }
        let centre = guard.center();
        let mut target = None;
        let mut nearest = f32::INFINITY;
        for (foe_index, foe) in enemies.iter().enumerate() {
            if foe.health <= 0.0
                || foe.pos.distance_squared(*post) > DEFENSE_RADIUS * DEFENSE_RADIUS
                || !(foe.hostile() || wildlife::vehicle::hostile_campaign(foe))
                || is_boss(foe.kind)
            {
                continue;
            }
            let target_centre = foe.center();
            let distance_sq = centre.distance_squared(target_centre);
            if distance_sq < nearest && line(world, centre, target_centre) {
                nearest = distance_sq;
                target = Some((foe_index, foe.pos, target_centre));
            }
        }
        let guard = &mut enemies[index];
        guard.cooldown = (guard.cooldown - dt).max(0.0);
        let Some((target, feet, target_centre)) = target else {
            continue;
        };
        let Some(goal) = next_goal.next() else { break };
        if nearest > REACH * REACH {
            *goal = Some((index, feet));
            continue;
        }
        *goal = Some((index, guard.pos));
        let toward = target_centre - centre;
        guard.yaw = (-toward.x).atan2(-toward.z);
        let swing = guard.cooldown <= 0.0 && guard.stagger <= 0.0;
        if swing {
            guard.cooldown = STRIKE_INTERVAL;
            let foe = &mut enemies[target];
            foe.hurt(17.0);
            sound(sounds, Cue::Swing, Some(centre), 0.55);
        }
        let foe = &mut enemies[target];
        if foe.health > 0.0
            && foe.cooldown <= 0.0
            && foe.stagger <= 0.0
            && foe.tell <= 0.0
            && foe.charge <= 0.0
            && foe.recovery <= 0.0
            && (!foe.visible || foe.pos.distance_squared(player) > nearest)
        {
            foe.cooldown = 1.25;
            let damage = 8.0 + foe.menace as f32 * 2.0;
            let guard = &mut enemies[index];
            guard.hurt(damage);
            sound(sounds, Cue::Swing, Some(target_centre), 0.45);
        }
    }
    goals
}
