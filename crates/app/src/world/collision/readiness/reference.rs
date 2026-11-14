// Frozen V16 original. Imports only changed; no new readiness/movement helpers.
#![allow(dead_code, unused_imports)]
//! Player collision and voxel DDA picking in block coordinates.
use crate::world::{World, BlockId, player_height_blocks, player_step_height_blocks, player_width_blocks};
use crate::fluid::VoxelScalePreset;
use glam::{IVec3, Vec3};
pub fn player_aabb(
    pos: Vec3,
    voxel_scale: VoxelScalePreset,
    player_scale: f32,
    crouch_t: f32,
) -> (Vec3, Vec3) {
    let half_w = player_width_blocks(voxel_scale, player_scale) * 0.5;
    let height = player_height_blocks(voxel_scale, player_scale);

    // Sneaking reduces player height by ~0.3m (1.8m -> 1.5m in Minecraft)
    let height_drop = 0.3 * player_scale / voxel_scale.meters().max(1e-6);
    let current_height = height - height_drop * crouch_t;
    (
        Vec3::new(pos.x - half_w, pos.y, pos.z - half_w),
        Vec3::new(pos.x + half_w, pos.y + current_height, pos.z + half_w),
    )
}

pub fn collides_world(world: &World, min: Vec3, max: Vec3) -> bool {
    collides_world_local(world, min, max, IVec3::ZERO)
}
pub fn collides_world_local(world: &World, min: Vec3, max: Vec3, origin: IVec3) -> bool {
    collides_with(world.sample_config(), min, max, origin, |p| world.get(p), Some(world))
}

pub(super) fn collides_with(
    cfg: crate::world::WorldSampleConfig,
    min: Vec3,
    max: Vec3,
    origin: IVec3,
    mut get: impl FnMut(IVec3) -> BlockId,
    world: Option<&World>,
) -> bool {
    let a = min.floor().as_ivec3();
    let b = (max - Vec3::splat(0.001)).floor().as_ivec3();
    let floor = cfg.min_world_y();
    for z in a.z..=b.z {
        for y in a.y..=b.y {
            for x in a.x..=b.x {
                let cell = origin + IVec3::new(x, y, z);
                if world.is_some_and(|world| world.ca_reservations.contains(&cell)) {
                    return true;
                }
                let block = get(cell);
                if cell.y <= floor || block.is_solid() {
                    if engine_content::fittings::state(block).is_some()
                        && !engine_content::fittings::boxes(block,cell,cfg.voxel_meters()).iter().any(|&(a,b)|
                            min.cmplt(b-origin.as_vec3()).all() && max.cmpgt(a-origin.as_vec3()).all()) { continue; }
                    if engine_content::shaped::is_shaped(block) {
                        let sample = std::cell::RefCell::new(&mut get);
                        let links = engine_content::shaped::links(block, cell, |p| sample.borrow_mut()(p));
                        let hit = engine_content::shaped::boxes(block, cell, links).iter().any(|&(a, b)| {
                            min.cmplt(b - origin.as_vec3()).all() && max.cmpgt(a - origin.as_vec3()).all()
                        });
                        if !hit { continue; }
                    }
                    if let Some(bounds) =
                        world.and_then(|world| crate::world::factory::conduit_bounds(world, cell))
                        && !bounds.iter().any(|&(a, b)| {
                            min.cmplt(b - origin.as_vec3()).all()
                                && max.cmpgt(a - origin.as_vec3()).all()
                        }) {
                            continue;
                        }
                    return true;
                }
            }
        }
    }
    false
}

pub fn try_move_local(
    origin: IVec3,
    world: &World,
    player_scale: f32,
    crouch_t: f32,
    pos: &mut Vec3,
    delta: Vec3,
    on_ground: &mut bool,
    sneak_edge_check: bool,
) {
    if !delta.is_finite() || !pos.is_finite() {
        return;
    }
    for axis in 0..3 {
        if delta[axis].abs() < 1e-6 {
            continue;
        }
        // Check intermediate positions so neither fast flight nor a long fall
        // can cross a one-voxel wall between endpoints.
        let steps = (delta[axis].abs() / 0.45).ceil().max(1.0) as u32;
        let mut step = Vec3::ZERO;
        step[axis] = delta[axis] / steps as f32;
        for _ in 0..steps {
            let candidate = *pos + step;
            let (min, max) = player_aabb(candidate, world.voxel_scale, player_scale, crouch_t);
            if collides_world_local(world, min, max, origin) {
                // Advance to contact instead of stopping a whole movement step away.
                let mut low = 0.0;
                let mut high = 1.0;
                for _ in 0..10 {
                    let mid = (low + high) * 0.5;
                    let (min, max) =
                        player_aabb(*pos + step * mid, world.voxel_scale, player_scale, crouch_t);
                    if collides_world_local(world, min, max, origin) {
                        high = mid;
                    } else {
                        low = mid;
                    }
                }
                let contact = *pos + step * low;
                if sneak_edge_check && (axis == 0 || axis == 2) {
                    let drop = Vec3::Y * (-0.05 / world.voxel_scale.meters().max(1e-6));
                    let (min, max) =
                        player_aabb(contact + drop, world.voxel_scale, player_scale, crouch_t);
                    if !collides_world_local(world, min, max, origin) {
                        break;
                    }
                }
                *pos = contact;
                if axis == 1 && delta.y < 0.0 {
                    *on_ground = true;
                }
                break;
            }
            if sneak_edge_check && (axis == 0 || axis == 2) {
                let drop_amount = 0.05 / world.voxel_scale.meters().max(1e-6);
                let drop = Vec3::new(0.0, -drop_amount, 0.0);
                let (d_min, d_max) =
                    player_aabb(candidate + drop, world.voxel_scale, player_scale, crouch_t);
                if !collides_world_local(world, d_min, d_max, origin) {
                    // Movement is rejected, block would cause the player to fall.
                    break;
                }
            }
            *pos = candidate;
        }
    }
}

