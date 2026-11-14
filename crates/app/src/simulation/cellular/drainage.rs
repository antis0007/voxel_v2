//! Exact finite water with gravity-first flow and local surface levelling.
//! Soil seepage can be far smaller than one legacy eighth-cell.
use super::*;
#[cfg(test)]
#[path = "inertia_tests.rs"]
mod inertia_tests;
#[cfg(test)]
#[path = "waterfall_tests.rs"]
mod waterfall_tests;

fn move_water(
    sources: &mut Residency,
    world: &mut World,
    from: IVec3,
    to: IVec3,
    limit: u64,
    visited: &mut HashSet<IVec3>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> u64 {
    let Some((B::Water, source)) = sources.water_cell(world, from) else {
        return 0;
    };
    let Some((B::Air | B::Water, target)) = sources.water_cell(world, to) else {
        return 0;
    };
    let room = world.environment.water_capacity_ul(to).saturating_sub(target);
    if limit.min(source).min(room) == 0
        || (target > 0 && world.is_steady_surface_water(to))
        || world.is_steady_surface_water(from)
    {
        return 0;
    }
    commit_transfer(sources, world, from, to, source, target, room, limit, visited, blocks, chunks)
}

// A sweep already knows the exact volumes of its distinct adjacent cells.
// Reuse those reads until a commit changes the source or that destination.
fn commit_transfer(
    sources: &mut Residency,
    world: &mut World,
    from: IVec3,
    to: IVec3,
    source: u64,
    target: u64,
    room: u64,
    limit: u64,
    visited: &mut HashSet<IVec3>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> u64 {
    let mut moved = source.min(limit).min(room);
    if moved == 0 {
        return 0;
    }
    // Suspended grains travel with their water and consume destination space.
    // Reserve their proportional volume before admitting the pair.
    let suspended = world
        .environment
        .sediment
        .get(&crate::world::environment::key(from))
        .map_or(0, |s| s.suspended);
    if suspended > 0 {
        let mixture = source + engine_core::environment::sediment_volume(suspended);
        moved = moved.min((u128::from(room) * u128::from(source) / u128::from(mixture)) as u64);
    }
    if moved == 0 {
        return 0;
    }
    if !admission(world, from, to)
        || !sources.prepare(world, &[from, to])
        || !world.apply_water_transfer(from, to, moved)
    {
        world.activity.wake(from);
        return 0;
    }
    let axis = Vec2::new((to.x - from.x) as f32, (to.z - from.z) as f32);
    crate::river_review::transfer(from, to, moved);
    // Keep current for sound and diagnostics. Mass flux is driven by head.
    let quantum = world.environment.quantum();
    let packet = moved as f32 / quantum as f32;
    if axis.length_squared() > 0.0 && packet >= 0.5 {
        if source > moved {
            world.set_water_velocity(from, world.water_velocity(from).lerp(axis * packet, 0.5));
        } else {
            world.remove_water_velocity(from);
        }
        world.set_water_velocity(to, world.water_velocity(to).lerp(axis * packet, 0.5));
    }
    // Newly wet cells wait for the next sweep. An already wet recipient
    // can level once in this sweep if its turn is still ahead.
    if target == 0 || from.y != to.y {
        visited.insert(to);
    }
    let step = |v| crate::world::environment::surface_step(v, quantum);
    if *crate::AB_OLD || step(source) != step(source - moved) { changed(from, blocks, chunks); }
    if *crate::AB_OLD || step(target) != step(target + moved) { changed(to, blocks, chunks); }
    moved
}

pub(super) fn step(
    sources: &mut Residency,
    world: &mut World,
    p: IVec3,
    visited: &mut HashSet<IVec3>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> usize {
    let quantum = world.environment.quantum();
    let volume = quantum * 8;
    let down = p - IVec3::Y;
    let gravity = sources.water_cell(world, down).and_then(|(block, fill)| {
        if !matches!(block, B::Air | B::Water) { return None; }
        let room = world.environment.water_capacity_ul(down).saturating_sub(fill);
        (room > 0 && (block == B::Air || !world.is_steady_surface_water(down)))
            .then_some((fill, room))
    });
    if let Some((target, room)) = gravity {
        let source = sources.water_amount(world, p).unwrap_or(0);
        let fallen = if source > 0 && !world.is_steady_surface_water(p) {
            commit_transfer(sources, world, p, down, source, target, room, volume, visited, blocks, chunks)
        } else {
            0
        };
        if fallen == 0 {
            world.remove_water_velocity(p);
            return 0;
        }
        // Forward only part of the packet so a fall cannot skip alternate rows.
        if fallen > 1 && move_water(
            sources, world, down, down - IVec3::Y, fallen / 2, visited, blocks, chunks,
        ) > 0 {
            return 4;
        }
        return 2;
    }
    let mut source = sources.water_amount(world, p).unwrap_or(0);
    let mut updates = 0;
    // An adjacent drop takes priority over levelling on the same floor.
    let mut room = [0u64; 4];
    let mut neighbors = [None; 4];
    for (i, direction) in HORIZONTAL.iter().enumerate() {
        let to = p + *direction;
        let Some((B::Air | B::Water, fill)) = sources.water_cell(world, to) else {
            continue;
        };
        if fill >= source || !allowed(world, to)
            || (fill > 0 && world.is_steady_surface_water(to))
        {
            continue;
        }
        neighbors[i] = Some(fill);
        let below = to - IVec3::Y;
        let lower = sources.water_cell(world, below);
        if lower.is_none() && allowed(world, below) {
            sources.request_for(
                world,
                chunk_coord_from_world_block(below),
                chunk_coord_from_world_block(p),
            );
            world.activity.wake(p);
            return 0;
        }
        if let Some((B::Air | B::Water, below_fill)) = lower
            && allowed(world, below)
            && (below_fill == 0 || !world.is_steady_surface_water(below))
        {
            room[i] = world.environment.water_capacity_ul(below).saturating_sub(below_fill);
        }
    }
    let has_drop = room.iter().any(|&capacity| capacity > 0);
    let phase = (p.x as u32).wrapping_mul(73856093)
        ^ (p.z as u32).wrapping_mul(19349663)
        ^ world.activity.tick as u32;
    // A half-millimetre head difference can rest on level ground.
    let resting_head = (volume as f64 * 0.0005 / world.voxel_scale.meters() as f64) as u64;
    // Find the shared level of the source and all lower neighbours. Raising
    // each toward that level avoids giving the first edge an entire pair's
    // share and leaving a checkerboard that has to reverse on the next sweep.
    let mut level = source;
    if !has_drop && neighbors.iter().flatten().any(|&fill| source - fill > resting_head) {
        let mut fills = neighbors.map(|fill| fill.unwrap_or(source));
        fills.sort_unstable();
        let mut sum = source;
        for (i, fill) in fills.into_iter().enumerate() {
            if level.saturating_sub(fill) <= resting_head {
                break;
            }
            sum += fill;
            level = sum / (i as u64 + 2);
        }
    }
    let mut steady_source = None;
    for offset in 0..4 {
        let i = (offset + phase as usize) % 4;
        if has_drop && room[i] == 0 {
            continue;
        }
        let to = p + HORIZONTAL[i];
        let Some(target) = neighbors[i] else {
            continue;
        };
        let head = source.saturating_sub(target);
        let limit = if has_drop {
            // Discharge over a lip instead of pooling beside it, but only what
            // can fall. Sending the whole head into a nearly full cell below
            // overshot, and a settled pool's surface swapped mass every tick.
            head.min(room[i] + head / 2)
        } else if head > resting_head {
            level.saturating_sub(target)
        } else {
            0
        };
        if limit == 0 {
            continue;
        }
        let capacity = world.environment.water_capacity_ul(to).saturating_sub(target);
        if capacity == 0 || *steady_source.get_or_insert_with(|| world.is_steady_surface_water(p)) {
            continue;
        }
        let moved = commit_transfer(sources, world, p, to, source, target, capacity, limit, visited, blocks, chunks);
        if moved == 0 {
            continue;
        }
        updates += 2;
        source -= moved;
        // Start the drop immediately while leaving water on the lip. This
        // avoids waiting a sweep before the receiving column starts to fall.
        if has_drop && moved > 1 && move_water(
            sources, world, to, to - IVec3::Y, moved / 2, visited, blocks, chunks,
        ) > 0 {
            visited.insert(to);
            updates += 2;
        }
        if source == 0 {
            break;
        }
    }
    // Retain the existing bounded hydrostatic connection for submerged outlets.
    if updates == 0
        && source == volume
        && (world.activity.tick == 0
            || (world.activity.tick as usize)
                .wrapping_add(p.x as usize)
                .wrapping_add(p.z as usize)
                .is_multiple_of(8))
        && let Some((surface, outlet, head)) = pressure_outlet(sources, world, p, resting_head as i64)
    {
        // Pressure displaces water through the full cells. Withdraw from the
        // free surface so the bottom of the column cannot open a void.
        if move_water(sources, world, surface, outlet, (head / 2).min(volume / 2),
                visited, blocks, chunks) > 0
        {
            updates += 2;
        }
    }
    if updates == 0 {
        world.remove_water_velocity(p);
    }
    updates
}
