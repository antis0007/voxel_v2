//! Ledge mantle (Titanfall, Dishonored, Mycopunk): Jump and Forward held while
//! airborne against a wall pull the body up and over a ledge within reach.
//! No gear needed; carried forward speed survives the vault.
use super::{Input, Player, movement};
use crate::world::{World, collides_world_local, player_aabb, speed_blocks};
use glam::{IVec3, Vec3};

/// Highest ledge above the feet a mantle reaches, metres. An ordinary jump
/// clears about 1.25 m, so this adds roughly an arm's pull on top.
const REACH_M: f32 = 1.9;
/// How far ahead of the body a ledge is looked for, metres.
const PROBE_M: f32 = 0.3;
/// Search step up the wall, metres.
const STEP_M: f32 = 0.125;
/// Clearance above the ledge the pull carries the feet, metres.
const CLEAR_M: f32 = 0.2;
/// Least forward speed while vaulting, so the body tops out onto the ledge, m/s.
const VAULT_MPS: f32 = 3.5;

/// Called after a blocked horizontal sweep in the air. `gravity` is the walking
/// gravity in voxels per second squared. Returns true when a mantle began.
pub(super) fn try_start(player: &mut Player, input: &Input, world: &World, origin: IVec3, gravity: f32) -> bool {
    if !input.jump || !input.forward || input.sneak || player.on_ground || player.flying
        || player.swimming || player.glide.deployed || world.adventure.is_dead() {
        return false;
    }
    let m = world.voxel_scale.meters();
    let forward = player.flat_forward();
    let (scale, crouch) = (player.scale, player.crouch_t);
    let free = |p: Vec3| {
        let (lo, hi) = player_aabb(p, world.voxel_scale, scale, crouch);
        !collides_world_local(world, lo, hi, origin)
    };
    let ahead = forward * (PROBE_M / m);
    if free(player.pos + ahead) {
        return false;
    }
    let step = STEP_M / m;
    let mut h = step;
    while h <= REACH_M / m {
        let lift = Vec3::Y * h;
        // A ceiling over the head ends the search: no pulling through a roof.
        if !free(player.pos + lift) {
            return false;
        }
        if free(player.pos + lift + ahead) {
            let rise = h + CLEAR_M / m;
            let need = (2.0 * gravity * rise).sqrt();
            // Already rising fast enough: the jump clears it unaided, and a
            // mantle in progress is not restarted every step.
            if player.vel.y >= need * 0.8 {
                return false;
            }
            player.vel.y = need;
            let along = player.vel.x * forward.x + player.vel.z * forward.z;
            let push = along.max(speed_blocks(world.voxel_scale, VAULT_MPS));
            player.vel.x = forward.x * push;
            player.vel.z = forward.z * push;
            player.movement.end_wallrun();
            player.movement.fall_speed = 0.0;
            // A gritty scramble: the slide bed is the closest existing cue.
            player.movement.raise(movement::EVENT_SLIDE);
            return true;
        }
        h += step;
    }
    false
}
