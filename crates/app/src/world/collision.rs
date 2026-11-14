//! Player collision and voxel DDA picking in block coordinates.
use super::{World, BlockId, player_height_blocks, player_step_height_blocks, player_width_blocks};
use crate::fluid::VoxelScalePreset;
use glam::{IVec3, Vec3};
mod readiness;
pub(crate) use readiness::{collides_ready_local, try_move_ready_local};
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
    cfg: super::WorldSampleConfig,
    min: Vec3,
    max: Vec3,
    origin: IVec3,
    mut get: impl FnMut(IVec3) -> BlockId,
    world: Option<&World>,
) -> bool {
    let result = collides_try_with(cfg, min, max, origin,
        |p| Ok::<BlockId, std::convert::Infallible>(get(p)), world);
    match result { Ok(hit) => hit, Err(never) => match never {} }
}

// One collision traversal for synchronous and readiness-aware readers. Readiness
// callers validate their bounds first; the infallible adapter preserves the
// historical public API and numerical/read ordering.
fn collides_try_with<E>(
    cfg: super::WorldSampleConfig,
    min: Vec3,
    max: Vec3,
    origin: IVec3,
    mut get: impl FnMut(IVec3) -> Result<BlockId, E>,
    world: Option<&World>,
) -> Result<bool, E> {
    let a = min.floor().as_ivec3();
    let b = (max - Vec3::splat(0.001)).floor().as_ivec3();
    let floor = cfg.min_world_y();
    for z in a.z..=b.z {
        for y in a.y..=b.y {
            for x in a.x..=b.x {
                let cell = origin + IVec3::new(x, y, z);
                if world.is_some_and(|world| world.ca_reservations.contains(&cell)) {
                    return Ok(true);
                }
                let block = get(cell)?;
                if cell.y <= floor || block.is_solid() {
                    if engine_content::fittings::state(block).is_some()
                        && !engine_content::fittings::boxes(block,cell,cfg.voxel_meters()).iter().any(|&(a,b)|
                            min.cmplt(b-origin.as_vec3()).all() && max.cmpgt(a-origin.as_vec3()).all()) { continue; }
                    if engine_content::shaped::is_shaped(block) {
                        let mut links = 0;
                        if engine_content::shaped::is_fence(block) {
                            for (i, d) in engine_content::shaped::FACINGS.iter().enumerate() {
                                let neighbor = get(cell + *d)?;
                                if engine_content::shaped::is_fence(neighbor) || neighbor.is_opaque() {
                                    links |= 1 << i;
                                }
                            }
                        }
                        let hit = engine_content::shaped::boxes(block, cell, links).iter().any(|&(a, b)| {
                            min.cmplt(b - origin.as_vec3()).all() && max.cmpgt(a - origin.as_vec3()).all()
                        });
                        if !hit { continue; }
                    }
                    if let Some(bounds) =
                        world.and_then(|world| super::factory::conduit_bounds(world, cell))
                        && !bounds.iter().any(|&(a, b)| {
                            min.cmplt(b - origin.as_vec3()).all()
                                && max.cmpgt(a - origin.as_vec3()).all()
                        }) {
                            continue;
                        }
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Recover a body that is already inside solid voxels.
///
/// Streaming edits, growth, and saves made before foliage became solid can
/// leave the player embedded. Resolving the smallest cardinal escape keeps a
/// bad frame from becoming a permanently stuck camera (and a permanently
/// see-through leaf block).
pub fn resolve_penetration_local(
    origin: IVec3,
    world: &World,
    player_scale: f32,
    crouch_t: f32,
    pos: &mut Vec3,
) -> bool {
    if !pos.is_finite() {
        return false;
    }
    let scale = world.voxel_scale.meters().max(1e-6);
    let (min, max) = player_aabb(*pos, world.voxel_scale, player_scale, crouch_t);
    if !collides_world_local(world, min, max, origin) {
        return false;
    }

    // Prefer lifting out of a canopy or shallow floor. Then try the sides,
    // nearest first. Each candidate is a complete body test, never a per-axis
    // teleport into another wall.
    let mut candidates = Vec::with_capacity(64);
    for step in 1..=16 {
        candidates.push(Vec3::Y * (step as f32 * 0.25 / scale));
    }
    for step in 1..=8 {
        let d = step as f32 * 0.5 / scale;
        candidates.extend([Vec3::X * d, Vec3::NEG_X * d, Vec3::Z * d, Vec3::NEG_Z * d]);
    }
    for step in 1..=8 {
        candidates.push(Vec3::NEG_Y * (step as f32 * 0.5 / scale));
    }

    for offset in candidates {
        let candidate = *pos + offset;
        let (min, max) = player_aabb(candidate, world.voxel_scale, player_scale, crouch_t);
        if !collides_world_local(world, min, max, origin) {
            *pos = candidate;
            return true;
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
    let result = try_move_with(world.voxel_scale, player_scale, crouch_t, pos, delta,
        on_ground, sneak_edge_check, |min, max| {
            Ok::<bool, std::convert::Infallible>(collides_world_local(world, min, max, origin))
        });
    match result { Ok(()) => {}, Err(never) => match never {} }
}

// The original numerical sequence, shared by infallible and transactional callers.
fn try_move_with<E>(
    voxel_scale: VoxelScalePreset,
    player_scale: f32,
    crouch_t: f32,
    pos: &mut Vec3,
    delta: Vec3,
    on_ground: &mut bool,
    sneak_edge_check: bool,
    mut collides: impl FnMut(Vec3, Vec3) -> Result<bool, E>,
) -> Result<(), E> {
    if !delta.is_finite() || !pos.is_finite() {
        return Ok(());
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
            let (min, max) = player_aabb(candidate, voxel_scale, player_scale, crouch_t);
            if collides(min, max)? {
                // Advance to contact instead of stopping a whole movement step away.
                let mut low = 0.0;
                let mut high = 1.0;
                for _ in 0..10 {
                    let mid = (low + high) * 0.5;
                    let (min, max) =
                        player_aabb(*pos + step * mid, voxel_scale, player_scale, crouch_t);
                    if collides(min, max)? {
                        high = mid;
                    } else {
                        low = mid;
                    }
                }
                let contact = *pos + step * low;
                if sneak_edge_check && (axis == 0 || axis == 2) {
                    let drop = Vec3::Y * (-0.05 / voxel_scale.meters().max(1e-6));
                    let (min, max) =
                        player_aabb(contact + drop, voxel_scale, player_scale, crouch_t);
                    if !collides(min, max)? {
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
                let drop_amount = 0.05 / voxel_scale.meters().max(1e-6);
                let drop = Vec3::new(0.0, -drop_amount, 0.0);
                let (d_min, d_max) =
                    player_aabb(candidate + drop, voxel_scale, player_scale, crouch_t);
                if !collides(d_min, d_max)? {
                    // Movement is rejected, block would cause the player to fall.
                    break;
                }
            }
            *pos = candidate;
        }
    }
    Ok(())
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

pub fn raycast_block(
    world: &World,
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
) -> Option<(IVec3, IVec3)> {
    raycast_block_precise(world, origin.as_dvec3(), dir, max_dist)
}
pub fn raycast_block_precise(
    world: &World,
    precise_origin: glam::DVec3,
    dir: Vec3,
    max_dist: f32,
) -> Option<(IVec3, IVec3)> {
    raycast_precise(world, precise_origin, dir, max_dist, false, |p| Some(world.get(p)))
}

pub fn raycast_liquids_precise(world: &World, origin: glam::DVec3, dir: Vec3, reach: f32) -> Option<(IVec3, IVec3)> {
    raycast_precise(world, origin, dir, reach, true, |p| Some(world.get(p)))
}

/// For visual hints only. Stop at an unknown cell instead of looking through it.
pub(crate) fn raycast_loaded_block_precise(
    world: &World,
    origin: glam::DVec3,
    dir: Vec3,
    reach: f32,
    read: impl FnMut(IVec3) -> Option<BlockId>,
) -> Option<(IVec3, IVec3)> {
    raycast_precise(world, origin, dir, reach, false, read)
}

fn raycast_precise(
    world: &World,
    precise_origin: glam::DVec3,
    dir: Vec3,
    max_dist: f32,
    water: bool,
    mut read: impl FnMut(IVec3) -> Option<BlockId>,
) -> Option<(IVec3, IVec3)> {
    if !precise_origin.is_finite() {
        return None;
    }
    let offset = precise_origin.floor().as_ivec3();
    let origin = (precise_origin - offset.as_dvec3()).as_vec3();
    if !origin.is_finite()
        || !dir.is_finite()
        || !max_dist.is_finite()
        || max_dist <= 0.0
        || dir.length_squared() < 1e-12
    {
        return None;
    }
    let dir = dir.normalize();
    let scale = world.voxel_scale.meters();
    let start_m = precise_origin.as_vec3() * scale;
    let end_m = start_m + dir * (max_dist * scale);
    let shell = super::trace::shell_hit(world, start_m, end_m);
    let mut traveled = 0.0f32;
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
        if let Some((t, owner, face)) = shell {
            if traveled >= t * max_dist - 1e-4 {
                return Some((owner, face));
            }
        }
        let world_cell = cell + offset;
        let poke = super::trace::shell_poke(world, start_m, end_m, world_cell, shell.is_some());
        let block = if water || !poke { Some(read(world_cell)?) } else { None };
        if water {
            let a = cell.as_vec3();
            let liquid = block.unwrap_or(BlockId::Air);
            let height = match liquid {
                BlockId::Water => world.water_microlitres(world_cell) as f32
                    / engine_core::environment::cell_volume(world.voxel_scale) as f32,
                BlockId::Lava if world.get(world_cell + IVec3::Y) != liquid => 1.0 - crate::mesh::LAVA_SURFACE_INSET,
                BlockId::SulfuricAcid if world.get(world_cell + IVec3::Y) != liquid => 1.0 - crate::mesh::ACID_SURFACE_INSET,
                BlockId::Lava | BlockId::SulfuricAcid => 1.0,
                _ => 0.0,
            };
            if height > 0.0 && super::factory::ray_box(origin, dir, a,
                a + Vec3::new(1.0, height, 1.0), max_dist) {
                return Some((cell + offset, prev + offset));
            }
        }
        if !poke && let Some(block) = block.filter(|block| block.is_ray_pickable())
            && (engine_content::fittings::state(block).is_none()
                || engine_content::fittings::boxes(block,world_cell,scale).iter().any(|&(a,b)|
                    super::factory::ray_box(origin,dir,a-offset.as_vec3(),b-offset.as_vec3(),max_dist)))
            && super::factory::conduit_bounds(world, cell + offset).is_none_or(|bounds| {
                bounds.into_iter().any(|(a, b)| {
                    super::factory::ray_box(
                        origin,
                        dir,
                        a - offset.as_vec3(),
                        b - offset.as_vec3(),
                        max_dist,
                    )
                })
            }) {
                return Some((cell + offset, prev + offset));
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

        if let Some((st, owner, face)) = shell {
            let shell_dist = st * max_dist;
            if traveled < shell_dist && t >= shell_dist && shell_dist <= max_dist {
                return Some((owner, face));
            }
        }
        if t > max_dist {
            break;
        }
        traveled = t;
    }

    None
}

