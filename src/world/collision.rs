//! Player collision and voxel DDA picking in block coordinates.
use super::{World, player_height_blocks, player_step_height_blocks, player_width_blocks};
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

pub fn collides_world(world: &World, min: Vec3, max: Vec3) -> bool { collides_world_local(world,min,max,IVec3::ZERO) }
pub fn collides_world_local(world: &World, min: Vec3, max: Vec3, origin: IVec3) -> bool {
    let a = min.floor().as_ivec3();
    let b = (max - Vec3::splat(0.001)).floor().as_ivec3();
    for z in a.z..=b.z {
        for y in a.y..=b.y {
            for x in a.x..=b.x {
                if world.solid(origin + IVec3::new(x, y, z)) {
                    if let Some(bounds)=super::factory::conduit_bounds(world,origin+IVec3::new(x,y,z)) {
                        if !bounds.iter().any(|&(a,b)|min.cmplt(b-origin.as_vec3()).all()&&max.cmpgt(a-origin.as_vec3()).all()){continue;}
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

pub fn try_step_up_local(
    origin: IVec3,
    world: &World,
    player_scale: f32,
    crouch_t: f32,
    pos: &mut Vec3,
    horiz_delta: Vec3,
) -> bool {
    if horiz_delta.y.abs() > 1e-6 || horiz_delta.length_squared() < 1e-8 {
        return false;
    }

    let step_height = player_step_height_blocks(world.voxel_scale, player_scale);
    let stepped = *pos + Vec3::new(0.0, step_height, 0.0);
    let (up_min, up_max) = player_aabb(stepped, world.voxel_scale, player_scale, crouch_t);
    if collides_world_local(world, up_min, up_max, origin) {
        return false;
    }

    let moved = stepped + horiz_delta;
    let (m_min, m_max) = player_aabb(moved, world.voxel_scale, player_scale, crouch_t);
    if collides_world_local(world, m_min, m_max, origin) {
        return false;
    }

    let mut dropped = moved;
    for _ in 0..8 {
        let next = dropped + Vec3::new(0.0, -step_height / 8.0, 0.0);
        let (d_min, d_max) = player_aabb(next, world.voxel_scale, player_scale, crouch_t);
        if collides_world_local(world, d_min, d_max, origin) {
            break;
        }
        dropped = next;
    }

    *pos = dropped;
    true
}

pub fn raycast_block(world:&World, origin:Vec3, dir:Vec3, max_dist:f32)->Option<(IVec3,IVec3)> {
    raycast_block_precise(world,origin.as_dvec3(),dir,max_dist)
}
pub fn raycast_block_precise(world:&World, precise_origin:glam::DVec3, dir:Vec3, max_dist:f32)->Option<(IVec3,IVec3)> {
    if !precise_origin.is_finite() { return None; }
    let offset=precise_origin.floor().as_ivec3();
    let origin=(precise_origin-offset.as_dvec3()).as_vec3();
    if !origin.is_finite()
        || !dir.is_finite()
        || !max_dist.is_finite()
        || max_dist <= 0.0
        || dir.length_squared() < 1e-12
    {
        return None;
    }
    let dir = dir.normalize();
    let mut cell = origin.floor().as_ivec3();

    let step = IVec3::new(
        if dir.x >= 0.0 { 1 } else { -1 },
        if dir.y >= 0.0 { 1 } else { -1 },
        if dir.z >= 0.0 { 1 } else { -1 },
    );

    let inv_dir = Vec3::new(
        if dir.x.abs() > 1e-6 {
            1.0 / dir.x
        } else {
            f32::INFINITY
        },
        if dir.y.abs() > 1e-6 {
            1.0 / dir.y
        } else {
            f32::INFINITY
        },
        if dir.z.abs() > 1e-6 {
            1.0 / dir.z
        } else {
            f32::INFINITY
        },
    );

    let next_boundary = Vec3::new(
        if dir.x >= 0.0 {
            cell.x as f32 + 1.0
        } else {
            cell.x as f32
        },
        if dir.y >= 0.0 {
            cell.y as f32 + 1.0
        } else {
            cell.y as f32
        },
        if dir.z >= 0.0 {
            cell.z as f32 + 1.0
        } else {
            cell.z as f32
        },
    );

    let mut tmax = Vec3::splat(f32::INFINITY);
    for axis in 0..3 {
        if inv_dir[axis].is_finite() {
            tmax[axis] = (next_boundary[axis] - origin[axis]) * inv_dir[axis];
        }
    }

    let tdelta = Vec3::new(inv_dir.x.abs(), inv_dir.y.abs(), inv_dir.z.abs());
    let mut prev = cell;

    let max_steps = ((max_dist * dir.abs().element_sum()).ceil() as usize)
        .saturating_add(3)
        .min(65_536);
    for _ in 0..max_steps {
        if world.get(cell+offset).is_ray_pickable() {
            if super::factory::conduit_bounds(world,cell+offset).is_none_or(|bounds|bounds.into_iter().any(|(a,b)|super::factory::ray_box(origin,dir,a-offset.as_vec3(),b-offset.as_vec3(),max_dist))) {
                return Some((cell+offset, prev+offset));
            }
        }

        prev = cell;

        let t = if tmax.x < tmax.y && tmax.x < tmax.z {
            let t = tmax.x;
            tmax.x += tdelta.x;
            cell.x += step.x;
            t
        } else if tmax.y < tmax.z {
            let t = tmax.y;
            tmax.y += tdelta.y;
            cell.y += step.y;
            t
        } else {
            let t = tmax.z;
            tmax.z += tdelta.z;
            cell.z += step.z;
            t
        };

        if t > max_dist {
            break;
        }
    }

    None
}

pub fn try_move(world:&World, scale:f32, crouch:f32, pos:&mut Vec3, delta:Vec3, ground:&mut bool, sneak:bool){try_move_local(IVec3::ZERO,world,scale,crouch,pos,delta,ground,sneak)}
pub fn try_step_up(world:&World,scale:f32,crouch:f32,pos:&mut Vec3,delta:Vec3)->bool{try_step_up_local(IVec3::ZERO,world,scale,crouch,pos,delta)}
