use super::{B, TREE_DIRECTIONS, World, canopy_loot, effects, mark_chunk_and_neighbors_dirty};
use crate::simulation::sandbox::{MAX_BODIES, MAX_TREE_BODY_VOXELS, RigidBody};
use crate::world::inventory::{DroppedStack, Item, ItemStack};
use glam::{IVec3, Quat, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};

const MAX_DROPS: usize = 1024;
const MAX_CANOPY_SEARCH: usize = 32_768;
const LOG: Item = Item::Block(B::Log);

pub(crate) fn harvest_log(
    world: &mut World,
    cell: IVec3,
    fall: Option<Vec3>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    let at = center(world, cell);
    let nearby = nearby_stack(world, at, LOG, 1);
    if nearby.is_none() && world.inventory.dropped.len() >= MAX_DROPS {
        world.expedition.say_for("Too many dropped items", 2.0);
        return false;
    }
    let Ok(commit) = effects::replace(world, cell, B::Log, B::Air) else {
        return false;
    };
    commit.publish(world, blocks, chunks);
    if let Some(i) = nearby {
        world.inventory.dropped[i].stack.count += 1;
    } else {
        drop_at(world, at, LOG, 1);
    }
    if let Some(direction) = fall
        && fell_tree(world, cell, direction)
    {
        world.expedition.say_for("Timber!", 1.5);
    }
    true
}

/// Claim the unsupported wood and its crown without editing any of their cells.
/// The rigid solver owns the next pose and publishes its world changes.
pub fn fell_tree(world: &mut World, cut: IVec3, direction: Vec3) -> bool {
    if world.sandbox.bodies.len() >= MAX_BODIES || world.ca_view.is_some() {
        return false;
    }
    let Some((mut occupied, logs)) = standing_tree(world, cut) else {
        return false;
    };
    let Some(leaves) = crown(world, &occupied, &logs) else {
        return false;
    };
    if leaves.len() < 4 || occupied.len() + leaves.len() > MAX_TREE_BODY_VOXELS {
        return false;
    }
    occupied.extend(leaves);
    let cfg = world.sample_config();
    if occupied.iter().any(|(p, _)| {
        p.y < cfg.min_world_y()
            || p.y >= cfg.max_world_y()
            || crate::world::orbital::cargo::security::protected(world, *p)
    }) {
        return false;
    }
    let mut fall = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero();
    if fall == Vec3::ZERO {
        fall = Vec3::new(
            (cut.x as f32 * 0.73).sin(),
            0.0,
            (cut.z as f32 * 0.91).cos(),
        )
        .normalize_or(Vec3::X);
    }
    // Use the leading edge of the lowest wood as the pivot.
    let bottom = logs.iter().map(|p| p.y).min().unwrap();
    let origin = occupied
        .iter()
        .map(|(p, _)| *p)
        .filter(|p| p.y == bottom && logs.contains(p))
        .max_by(|a, b| {
            (*a - cut)
                .as_vec3()
                .dot(fall)
                .total_cmp(&(*b - cut).as_vec3().dot(fall))
        })
        .unwrap();
    world.sandbox.bodies.push(RigidBody {
        felled_tree: true,
        origin,
        position: Vec3::ZERO,
        velocity: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        angular: Vec3::Y.cross(fall) * 0.8,
        debt: 0.0,
        settle_time: 0.0,
        was_submerged: false,
        voxels: occupied.iter().map(|&(p, b)| (p - origin, b)).collect(),
        occupied,
    });
    world.activity.wake(origin);
    true
}

fn standing_tree(world: &World, cut: IVec3) -> Option<(Vec<(IVec3, B)>, HashSet<IVec3>)> {
    let top = cut + IVec3::Y;
    if world.get(top) != B::Log {
        return None;
    }
    let mut logs = HashSet::from([top]);
    let mut occupied = vec![(top, B::Log)];
    let mut i = 0;
    while i < occupied.len() {
        let p = occupied[i].0;
        for d in TREE_DIRECTIONS {
            let q = p + d;
            if q.y >= cut.y && !logs.contains(&q) && world.get(q) == B::Log {
                if occupied.len() == MAX_TREE_BODY_VOXELS {
                    return None;
                }
                logs.insert(q);
                occupied.push((q, B::Log));
            }
        }
        i += 1;
    }
    let supported = occupied.iter().any(|(p, _)| {
        let below = *p - IVec3::Y;
        !logs.contains(&below) && {
            let block = world.get(below);
            block.is_solid() && !matches!(block, B::Leaves | B::PineLeaves)
        }
    });
    let overlaps_body = world.sandbox.bodies.iter().any(|body| {
        body.occupied
            .iter()
            .any(|(p, _)| logs.contains(p) || *p == cut)
    });
    (occupied.len() >= 2 && !supported && !overlaps_body).then_some((occupied, logs))
}

fn crown(world: &World, occupied: &[(IVec3, B)], logs: &HashSet<IVec3>) -> Option<Vec<(IVec3, B)>> {
    let reach = (5.0 / world.voxel_scale.meters()).ceil() as u32;
    let mut seen = HashSet::new();
    let mut distances = HashMap::new();
    let mut leaves = Vec::new();
    let mut queue = VecDeque::new();
    let mut rivals = VecDeque::new();
    for &(p, _) in occupied {
        queue.extend(TREE_DIRECTIONS.map(|d| (p + d, 1)));
    }
    while let Some((p, distance)) = queue.pop_front() {
        if !seen.insert(p) {
            continue;
        }
        if seen.len() > MAX_CANOPY_SEARCH {
            return None;
        }
        let material = world.get(p);
        if !matches!(material, B::Leaves | B::PineLeaves) {
            continue;
        }
        leaves.push((p, material));
        distances.insert(p, distance);
        if TREE_DIRECTIONS.iter().any(|d| {
            let q = p + *d;
            !logs.contains(&q) && world.get(q) == B::Log
        }) {
            rivals.push_back((p, 1));
        }
        if distance < reach {
            queue.extend(TREE_DIRECTIONS.map(|d| (p + d, distance + 1)));
        }
    }
    // Shared foliage stays with a standing trunk when that trunk is at least
    // as close along the leaves. Ties belong to the tree that is still rooted.
    while let Some((p, distance)) = rivals.pop_front() {
        if distances.get(&p).is_some_and(|&ours| distance <= ours) {
            distances.remove(&p);
            rivals.extend(TREE_DIRECTIONS.map(|d| (p + d, distance + 1)));
        }
    }
    leaves.retain(|(p, _)| distances.contains_key(p));
    if world
        .sandbox
        .bodies
        .iter()
        .any(|body| body.occupied.iter().any(|(p, _)| distances.contains_key(p)))
    {
        return None;
    }
    Some(leaves)
}

/// Convert the landed material into pickups. Preserve the body on a full
/// drop list or a deferred edit and retry without issuing duplicate items.
pub(crate) fn finish_tree(
    world: &mut World,
    body: &mut RigidBody,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> bool {
    let mut logs: Vec<_> = body
        .occupied
        .iter()
        .filter_map(|&(p, b)| (b == B::Log).then_some(p))
        .collect();
    if let Some((&first, rest)) = logs.split_first() {
        let (lo, hi) = rest
            .iter()
            .fold((first, first), |(lo, hi), &p| (lo.min(p), hi.max(p)));
        let extent = hi - lo;
        let axis = if extent.x >= extent.z && extent.x >= extent.y {
            0
        } else if extent.z >= extent.y {
            2
        } else {
            1
        };
        logs.sort_unstable_by_key(|p| (p[axis], p.y, p.x, p.z));
    }
    let group_size = logs
        .len()
        .div_ceil(48)
        .max(1)
        .min(world.inventory.stack_limit(LOG) as usize);
    if world.inventory.dropped.len() + logs.len().div_ceil(group_size) > MAX_DROPS {
        return false;
    }
    let edits: Vec<_> = body
        .occupied
        .iter()
        .filter_map(|&(p, b)| (world.get(p) == b).then_some((p, B::Air)))
        .collect();
    let mut removed = HashSet::new();
    let mut leaves = Vec::new();
    world.set_many_committed(&edits, |p, old, _| {
        removed.insert(p);
        if matches!(old, B::Leaves | B::PineLeaves) {
            leaves.push((p, old));
        }
    });
    for &p in &removed {
        blocks.insert(p);
        mark_chunk_and_neighbors_dirty(chunks, p);
        world.activity.wake(p);
    }
    let mut impact = None;
    for group in logs.chunks(group_size) {
        let mut cells = group.iter().copied().filter(|p| removed.contains(p));
        if let Some(p) = cells.next() {
            let at = center(world, p);
            drop_at(world, at, LOG, 1 + cells.count() as u32);
            world
                .adventure
                .spawn_voxel_burst(at, B::Log.color(p), 2, 1.2, 0.045);
            impact.get_or_insert(at);
        }
    }
    if let Some(at) = impact
        && world.adventure.sounds.len() < 64
    {
        world.adventure.sounds.push(crate::audio::Event {
            cue: crate::audio::Cue::Impact,
            pos: Some(at),
            gain: 0.65,
            pitch: 0.7,
        });
    }
    let scale = world.voxel_scale.meters();
    let stride = leaves.len().div_ceil(64).max(1);
    for (i, &(p, material)) in leaves.iter().enumerate() {
        let at = center(world, p);
        if i % stride == 0 {
            world.adventure.spawn_voxel_particle(
                at,
                Vec3::Y * 0.3,
                material.color(p),
                scale.min(0.16),
                0.0,
                0.85,
                0.35,
                1.0,
            );
        }
        if let Some(item) =
            crate::world::farming::sapling::leaf_drop(world.seed, scale, p, material)
        {
            drop_at(world, at, item, 1);
        }
    }
    let leaf_cells: Vec<_> = leaves.iter().map(|&(p, _)| p).collect();
    for (p, item, count) in canopy_loot(&leaf_cells) {
        let at = center(world, p);
        drop_at(world, at, item, count);
    }

    let mut occupied = body.occupied.iter();
    body.voxels.retain(|_| {
        let &(p, b) = occupied.next().unwrap();
        !removed.contains(&p) && world.get(p) == b
    });
    body.occupied
        .retain(|&(p, b)| !removed.contains(&p) && world.get(p) == b);
    body.occupied.is_empty()
}

fn center(world: &World, p: IVec3) -> Vec3 {
    (p.as_vec3() + Vec3::splat(0.5)) * world.voxel_scale.meters()
}

fn drop_at(world: &mut World, pos: Vec3, item: Item, count: u32) {
    if let Some(i) = nearby_stack(world, pos, item, count) {
        world.inventory.dropped[i].stack.count += count;
    } else if world.inventory.dropped.len() < MAX_DROPS {
        world.inventory.dropped.push(DroppedStack {
            pos,
            stack: ItemStack::new(item, count),
        });
    }
}

fn nearby_stack(world: &World, at: Vec3, item: Item, count: u32) -> Option<usize> {
    let room = world.inventory.stack_limit(item).checked_sub(count)?;
    world.inventory.dropped.iter().position(|drop| {
        drop.stack.item == item && drop.stack.count <= room && drop.pos.distance_squared(at) <= 1.0
    })
}
