//! Bounded voxel assemblies with swept translation and rotational collision.
use super::{
    cellular::allowed,
    residency::Residency,
    sandbox::{MAX_BODIES, MAX_BODY_VOXELS, RigidBody},
};
use crate::fast_hash::{FastMap, FastSet};
use crate::world::{BlockId as B, World, chunk_coord_from_world_block as chunk};
use glam::{IVec3, Quat, Vec3};
use std::collections::HashSet;
const DIRS: [IVec3; 6] = [
    IVec3::X,
    IVec3::NEG_X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::Z,
    IVec3::NEG_Z,
];
/// Impact speed (m/s) at which a felled trunk breaks into wood.
const FELL_BREAK_SPEED_M: f32 = 3.0;
mod contact;
mod fracture;

/// Check the terrain-edit wake footprint once per pose, after collision.
fn admit_motion(world: &World, positions: &[IVec3], wake_chunks: &mut FastSet<IVec3>) -> bool {
    wake_chunks.clear();
    let side = crate::world::CHUNK_SIZE;
    for &p in positions {
        if !allowed(world, p) || crate::world::orbital::cargo::security::protected(world, p) {
            return false;
        }
        wake_chunks.insert(chunk(p));
        let local = p.rem_euclid(IVec3::splat(side));
        if local.min_element() == 0 || local.max_element() == side - 1 {
            wake_chunks.extend(crate::world::activity::WAKE_NEIGHBORS.map(|d| chunk(p + d)));
        }
    }
    let coords: Vec<_> = wake_chunks.iter().copied().collect();
    world.activity.can_wake(&coords)
}

/// External impulses use the same mass-conserving fracture path as collisions.
pub(super) fn fracture_at(bodies: &mut Vec<RigidBody>, index: usize, cell: IVec3, energy: f32, speed: f32) {
    if index >= bodies.len() || bodies.len() >= MAX_BODIES { return; }
    if bodies[index].voxels.len() < 4 || speed < 3.0 || energy < 6.0 { return; }
    let mut body = bodies.remove(index);
    fracture::split(&mut body, bodies, contact::Impact { cell, terrain: None, energy, speed });
    bodies.insert(index, body);
}

/// Only run connectivity after an external edit, never on the hot motion path.
/// Missing cells stay missing; disconnected pieces inherit point velocity.
fn repair(body: &mut RigidBody, s: &Residency, w: &mut World) -> bool {
    if body.occupied.iter().all(|&(p, b)| s.read(w, p).is_some_and(|(id, _)| id == b)) {
        return true;
    }
    let old_center = body.occupied_center();
    let mut remaining: crate::fast_hash::FastMap<_, _> = body.occupied.iter().copied()
        .filter(|&(p, b)| s.read(w, p).is_some_and(|(id, _)| id == b)).collect();
    let mut groups = Vec::new();
    // Original cell order makes fragment admission deterministic at the cap.
    for &(seed, _) in &body.occupied {
        let Some(b) = remaining.remove(&seed) else { continue };
        let mut cells = vec![(seed, b)];
        let mut cursor = 0;
        while cursor < cells.len() {
            let p = cells[cursor].0;
            for d in DIRS {
                let q = p + d;
                if let Some(b) = remaining.remove(&q) { cells.push((q, b)); }
            }
            cursor += 1;
        }
        groups.push(cells);
    }
    if groups.is_empty() { return false; }
    let mut fragments = Vec::new();
    for cells in groups {
        let origin = cells[0].0;
        let mut fragment = RigidBody {
            felled_tree: body.felled_tree,
            origin, position: Vec3::ZERO, velocity: body.velocity,
            rotation: Quat::IDENTITY, angular: body.angular, debt: body.debt,
            settle_time: 0.0, was_submerged: body.was_submerged,
            voxels: cells.iter().map(|&(p, b)| (p - origin, b)).collect(), occupied: cells,
        };
        let (_, center, _) = fragment.mass_properties();
        fragment.velocity += body.angular.cross(origin.as_vec3() + center - old_center);
        fragments.push(fragment);
    }
    *body = fragments.remove(0);
    // Reserve the removed parent's slot. Overflow stays as ordinary terrain.
    let room = MAX_BODIES.saturating_sub(w.sandbox.bodies.len() + 1);
    w.sandbox.bodies.extend(fragments.into_iter().take(room));
    true
}

fn structural(b: B) -> bool {
    matches!(b, B::RigidCore | B::Log) || fragment_material(b)
}

/// Authored construction can detach after edits. Natural stone and uncut
/// trees stay in the static world; loaded chunks do not initiate collapse.
pub(super) fn fragment_material(b: B) -> bool {
    matches!(b, B::Planks | B::StoneBricks | B::WeatheredBricks | B::Cobblestone
        | B::Concrete | B::SteelPlate | B::RustedSteel | B::HazardPaint | B::WindowGlass
        | B::CopperBlock | B::TinBlock | B::IronBlock | B::BronzeBlock | B::SteelBlock)
}

fn advance(body: &mut RigidBody, position: Vec3, rotation: Quat) {
    let shift = position.floor().as_ivec3();
    body.origin += shift;
    body.position = position - shift.as_vec3();
    body.rotation = rotation;
}

fn supported(s: &Residency, w: &World, body: &RigidBody, owned: &crate::fast_hash::FastSet<IVec3>, center: Vec3) -> bool {
    let mut footprint = Vec::new();
    for &(p, _) in &body.occupied {
        let below = p - IVec3::Y;
        if !owned.contains(&below) && s.read(w, below).is_some_and(|(b, _)| b.is_solid()) {
            for (x, z) in [(-1, -1), (-1, 1), (1, -1), (1, 1)] {
                footprint.push((p.x as i64 * 2 + x, p.z as i64 * 2 + z));
            }
        }
    }
    if footprint.is_empty() { return false; }
    footprint.sort_unstable();
    footprint.dedup();
    let turn = |a: (i64, i64), b: (i64, i64), c: (i64, i64)|
        (b.0-a.0)*(c.1-a.1) - (b.1-a.1)*(c.0-a.0);
    let mut hull = Vec::new();
    for &p in &footprint {
        while hull.len() >= 2 && turn(hull[hull.len()-2], hull[hull.len()-1], p) <= 0 { hull.pop(); }
        hull.push(p);
    }
    let lower = hull.len();
    for &p in footprint.iter().rev().skip(1) {
        while hull.len() > lower && turn(hull[hull.len()-2], hull[hull.len()-1], p) <= 0 { hull.pop(); }
        hull.push(p);
    }
    // Use the convex support polygon, not its AABB: diagonal feet must not
    // suspend a centre of mass outside the area they actually support.
    hull.windows(2).all(|edge| {
        let (a, b) = (edge[0], edge[1]);
        (b.0-a.0) as f64 * (center.z as f64 * 2.0-a.1 as f64)
            - (b.1-a.1) as f64 * (center.x as f64 * 2.0-a.0 as f64) >= -1e-5
    })
}

fn support_manifold(s: &Residency, w: &World, body: &RigidBody,
    owned: &crate::fast_hash::FastSet<IVec3>, owners: &crate::fast_hash::FastMap<IVec3, usize>) -> contact::Manifold {
    let mut manifold = contact::Manifold::default();
    for &(p, material) in &body.occupied {
        let below = p - IVec3::Y;
        if !owned.contains(&below) && let Some((obstacle, _)) = s.read(w, below).filter(|(b, _)| b.is_solid()) {
            manifold.push(contact::Contact {
                cell: p, obstacle_cell: below, other: owners.get(&below).copied(),
                point: p.as_vec3() + Vec3::new(0.5, 0.0, 0.5),
                normal: Vec3::Y, material, obstacle,
            });
        }
    }
    manifold
}

pub(super) fn capture(s: &mut Residency, w: &mut World, p: IVec3) {
    if w.sandbox.bodies.len() >= MAX_BODIES
        || w.sandbox
            .bodies
            .iter()
            .any(|b| b.occupied.iter().any(|(q, _)| *q == p))
    {
        return;
    }
    let mut queue = vec![p];
    let mut seen = HashSet::new();
    let mut voxels = Vec::new();
    let automatic = s.read(w, p).is_some_and(|(b, _)| b != B::RigidCore);
    let mut anchored = false;
    let mut has_core = false;
    while let Some(q) = queue.pop() {
        if !seen.insert(q) {
            continue;
        }
        if !s.request_for(w, chunk(q), chunk(p)) {
            w.activity.wake(p);
            return;
        }
        let Some((b, _)) = s.read(w, q) else {
            w.activity.wake(p);
            return;
        };
        if !structural(b) {
            if b.is_solid() && !matches!(b, B::Leaves | B::PineLeaves) { anchored = true; }
            continue;
        }
        if !super::cellular::allowed(w, q)
            || crate::world::orbital::cargo::security::protected(w, q)
            || w.sandbox
                .bodies
                .iter()
                .any(|body| body.occupied.iter().any(|(r, _)| *r == q))
        {
            return;
        }
        voxels.push((q, b));
        has_core |= b == B::RigidCore;
        if voxels.len() > MAX_BODY_VOXELS {
            return;
        }
        queue.extend(DIRS.map(|d| q + d));
    }
    if voxels.is_empty() || (automatic && anchored && !has_core) {
        return;
    }
    w.sandbox.bodies.push(RigidBody {
        felled_tree: false,
        origin: p,
        position: Vec3::ZERO,
        velocity: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        angular: Vec3::ZERO,
        debt: 0.0,
        settle_time: 0.0,
        was_submerged: false,
        voxels: voxels.iter().map(|(q, b)| (*q - p, *b)).collect(),
        occupied: voxels,
    });
}
pub(super) fn tick(
    s: &mut Residency,
    w: &mut World,
    dt: f32,
    player: (Vec3, Vec3),
    deadline: super::Budget,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) {
    let count = w.sandbox.bodies.len();
    if count == 0 || !dt.is_finite() || dt <= 0.0 {
        return;
    }
    for body in &mut w.sandbox.bodies {
        body.debt = (body.debt + dt).min(0.25);
    }
    let scale = w.voxel_scale.meters();
    #[cfg(feature = "multiplayer")]
    let remote_bounds: Vec<_> = w.multiplayer.iter().flat_map(|context| &context.players)
        .map(|target| (target.min / scale, target.max / scale))
        .collect();
    let mut ready_chunks = FastMap::default();
    let mut wake_chunks = FastSet::default();
    let mut terrain_chips = 0;
    for _ in 0..count {
        if w.sandbox.bodies.is_empty() || !deadline.has_time() {
            break;
        }
        let index = w.sandbox.body_cursor % w.sandbox.bodies.len();
        let mut body = w.sandbox.bodies.remove(index);
        ready_chunks.clear();
        let mut ready = true;
        for &(p, _) in &body.occupied {
            let coord = chunk(p);
            ready &= *ready_chunks.entry(coord).or_insert_with(|| s.request(w, coord));
        }
        if ready && !repair(&mut body, s, w) {
            continue;
        }
        if ready {
            if body.felled_tree && body.settle_time >= 0.9 {
                let cells: Vec<_> = body.occupied.iter().map(|(p, _)| *p).collect();
                if s.prepare(w, &cells) && crate::world::expedition::finish_tree(w, &mut body, blocks, chunks) {
                    continue;
                }
                body.debt = 0.0;
                w.sandbox.bodies.insert(index, body);
                w.sandbox.body_cursor = (index + 1) % w.sandbox.bodies.len();
                continue;
            }
            let (mass, rest_center, _) = body.mass_properties();
            let world_center = body.origin.as_vec3() + body.position + body.rotation * rest_center;
            let mut old: crate::fast_hash::FastSet<_> = body.occupied.iter().map(|(p, _)| *p).collect();
            let radius = body.voxels.iter().map(|(p, _)| (p.as_vec3() - rest_center).length())
                .fold(0.0_f32, f32::max);
            let can_rest = body.velocity.y <= 0.0 && body.velocity.y * scale > -0.5
                && body.angular.length() * radius * scale < 0.15;
            let grounded = can_rest && supported(s, w, &body, &old, world_center);
            // Sleeping contraptions retain their identity. Recheck support so
            // digging out the ground wakes them; impulses reset settle_time.
            if body.settle_time >= 0.9 && body.velocity == Vec3::ZERO && body.angular == Vec3::ZERO {
                let flooded = body.occupied.iter().step_by(body.occupied.len().div_ceil(8).max(1))
                    .any(|(p, _)| DIRS.iter().any(|d| s.read(w, *p + *d).is_some_and(|(b, _)| b == B::Water)));
                if grounded && !body.was_submerged && !flooded {
                    body.debt = 0.0;
                    w.sandbox.bodies.insert(index, body);
                    w.sandbox.body_cursor = (index + 1) % w.sandbox.bodies.len();
                    continue;
                }
                body.settle_time = 0.0;
            }
            // Include tip speed in the sweep. Limit elapsed time instead of
            // allowing fast fragments to tunnel once the substep cap is hit.
            let sweep_speed = body.velocity.length() + body.angular.length() * radius + 5.0 * 9.81 / scale * 0.05;
            let dt = body.debt.min(1.0 / 20.0).min(16.0 / sweep_speed.max(1.0));
            body.debt -= dt;
            let mut acceleration = -9.81 / scale;
            let resting = grounded;
            let mut water_drag = 0.0;
            // Assemblies obey the same coarse buoyancy rule as the player:
            // wooden frames float, dense cores sink, and everything in water
            // loses speed to drag. This is sampled from the body's occupied
            // cells, so partial submersion settles instead of snapping.
            // Sample a bounded subset of the assembly instead of querying the
            // world for every voxel every tick. Eight probes are enough to
            // distinguish a floating edge from a fully submerged frame.
            let stride = body.occupied.len().div_ceil(8).max(1);
            let mut sampled = 0usize;
            let mut submerged = 0usize;
            for (p, _) in body.occupied.iter().step_by(stride) {
                sampled += 1;
                // The body's own cells contain structural blocks, not water, so
                // probe the six neighbours instead of the cell itself.
                if DIRS.iter().any(|d| w.get(*p + *d) == B::Water) {
                    submerged += 1;
                }
            }
            if submerged > 0 && sampled > 0 {
                if !body.was_submerged && body.velocity.y * scale < -1.2 {
                    let centre_cells = body
                        .occupied
                        .iter()
                        .map(|(p, _)| p.as_vec3() + Vec3::splat(0.5))
                        .sum::<Vec3>()
                        / body.occupied.len() as f32;
                    let centre = centre_cells * scale;
                    if let Some(surface) = w.water_surface_at(centre) {
                        let strength = ((-body.velocity.y * scale) / 9.0).clamp(0.22, 1.0);
                        w.adventure.splash(surface, strength);
                        w.disturb_water_surface(surface, strength);
                    }
                }
                let fraction = submerged as f32 / sampled as f32;
                let density_ratio = mass / body.voxels.len() as f32;
                acceleration += fraction * (1.0 - density_ratio).clamp(-1.0, 1.0) * 9.81 / scale * 4.0;
                water_drag = fraction * 2.4;
            }
            body.was_submerged = submerged > 0 && sampled > 0;
            let steps = ((sweep_speed * dt / 0.25).ceil() as usize).clamp(1, 64);
            let tree = body.felled_tree;
            let mut proposed = Vec::with_capacity(body.voxels.len());
            let mut positions = Vec::with_capacity(body.voxels.len() * 2);
            let mut heat = Vec::with_capacity(body.voxels.len());
            let mut edits = Vec::with_capacity(body.voxels.len() * 2);
            let mut proposed_cells = FastSet::default();
            let owners: crate::fast_hash::FastMap<_, _> = w.sandbox.bodies.iter().enumerate()
                .flat_map(|(i, b)| b.occupied.iter().map(move |(p, _)| (*p, i))).collect();
            let mut support_contacts = contact::Manifold::default();
            if resting {
                support_contacts = support_manifold(s, w, &body, &old, &owners);
            }
            let mut fractured = false;
            let mut simulated = 0.0;
            for substep in 0..steps {
                if !deadline.has_time() {
                    body.debt =
                        (body.debt + dt * (steps - substep) as f32 / steps as f32).min(0.25);
                    break;
                }
                let h = dt / steps as f32;
                let previous_velocity = body.velocity;
                let previous_angular = body.angular;
                let previous_settle = body.settle_time;
                let support_checkpoint = support_contacts.checkpoint(&w.sandbox.bodies);
                // A fresh cut tips about the bottom of the trunk. Gravity
                // accelerates the tip until the tree comes off that pivot.
                let hinged = tree && body.voxels.len() > 1 && body.angular != Vec3::ZERO
                    && body.velocity.length() * scale < 1.5
                    && (body.rotation * Vec3::Y).y > 0.15 && !body.was_submerged;
                if hinged {
                    let arm = body.rotation * rest_center;
                    body.angular += arm.cross(Vec3::Y * acceleration) * h / arm.length_squared().max(1.0);
                }
                body.velocity.y = (body.velocity.y + acceleration * h).max(-30.0 / scale);
                body.velocity *= (-water_drag * h).exp();
                body.angular *= (-(0.18 + water_drag * 0.7) * h).exp();
                if resting && !support_contacts.is_empty() {
                    contact::solve(&mut body, &mut w.sandbox.bodies, &support_contacts, scale);
                }
                let rotation =
                    (Quat::from_scaled_axis(body.angular * h) * body.rotation).normalize();
                let voxel_rotation = super::sandbox::VoxelRotation::new(rotation);
                let position = if hinged {
                    body.velocity = if (rotation * Vec3::Y).y <= 0.15 {
                        body.angular.cross(rotation * rest_center)
                    } else {
                        Vec3::ZERO
                    };
                    body.position
                } else {
                    body.position + body.velocity * h + body.rotation * rest_center - rotation * rest_center
                };
                let base = body.origin + position.round().as_ivec3();
                proposed.clear();
                proposed.extend(body
                    .voxels
                    .iter()
                    .map(|&(p, b)| (base + voxel_rotation.apply(p), b)));
                if proposed == body.occupied {
                    advance(&mut body, position, rotation);
                    simulated += h;
                    continue;
                }
                let mut blocked = false;
                let mut loaded = true;
                let mut manifold = contact::Manifold::default();
                let origin_allowed = allowed(w, body.origin);
                for (voxel_index, &(p, _)) in proposed.iter().enumerate() {
                    let from = body.occupied[voxel_index].0;
                    if p == from { continue; }
                    if !origin_allowed || !allowed(w, p) {
                        blocked = true;
                        break;
                    }
                    let min = p.as_vec3();
                    let max = min + Vec3::ONE;
                    let occupied = min.cmplt(player.1).all() && max.cmpgt(player.0).all();
                    #[cfg(feature = "multiplayer")]
                    let occupied = occupied || remote_bounds.iter().any(|&(lo, hi)| min.cmplt(hi).all() && max.cmpgt(lo).all());
                    if occupied {
                        blocked = true;
                        break;
                    }
                    if old.contains(&p) { continue; }
                    let coord = chunk(p);
                    if !*ready_chunks.entry(coord).or_insert_with(|| s.request(w, coord)) {
                        loaded = false;
                        continue;
                    }
                    let Some((obstacle, _)) = s.read(w, p) else {
                        loaded = false;
                        continue;
                    };
                    if obstacle != B::Air {
                        blocked = true;
                        let direction = (from - p).as_vec3();
                        let axis = if direction.y.abs() >= direction.x.abs().max(direction.z.abs()) { 1 }
                            else if direction.x.abs() > direction.z.abs() { 0 } else { 2 };
                        let mut normal = Vec3::ZERO;
                        normal[axis] = direction[axis].signum();
                        if normal != Vec3::ZERO {
                            manifold.push(contact::Contact {
                                cell: from,
                                obstacle_cell: p,
                                other: owners.get(&p).copied(),
                                point: p.as_vec3() + Vec3::splat(0.5) + normal * 0.5,
                                normal, material: body.voxels[voxel_index].1, obstacle,
                            });
                        }
                    }
                }
                if !loaded {
                    support_checkpoint.restore(&mut w.sandbox.bodies);
                    body.velocity = previous_velocity;
                    body.angular = previous_angular;
                    body.settle_time = previous_settle;
                    body.debt = (body.debt + h * (steps - substep) as f32).min(0.25);
                    break;
                }
                if blocked {
                    // A felled trunk that lands hard splits into wood on impact
                    // rather than skidding on as one body. Below the threshold it
                    // keeps tipping and settles as before.
                    if hinged {
                        body.velocity = body.angular.cross(body.rotation * rest_center);
                    }
                    let impact = contact::solve(&mut body, &mut w.sandbox.bodies, &manifold, scale);
                    let chipped = terrain_chips < 2 && impact.is_some_and(|hit| {
                        let Some((p, normal)) = hit.terrain else { return false };
                        if hit.speed < 5.0 || hit.energy < 10.0 { return false; }
                        let hardness = w.get(p).material().physics.hardness.max(0.0);
                        hit.speed > 5.0 + 2.0 * hardness && hit.energy > 10.0 + 10.0 * hardness
                            && crate::world::expedition::eject_terrain(
                                w, p, normal, (hit.speed * 0.3).min(4.0))
                    });
                    terrain_chips += usize::from(chipped);
                    if tree && impact.is_some_and(|i| {
                        i.speed > FELL_BREAK_SPEED_M && i.terrain.is_none_or(|(p, _)| {
                            let obstacle = w.get(p);
                            obstacle.is_solid() && !matches!(obstacle, B::Leaves | B::PineLeaves)
                        })
                    }) {
                        body.settle_time = 0.9;
                        break;
                    }
                    if let Some(impact) = impact {
                        if fracture::split(&mut body, &mut w.sandbox.bodies, impact) {
                            fractured = true;
                            body.debt = (body.debt + h * (steps - substep - 1) as f32).min(0.25);
                            break;
                        }
                    }
                    if chipped {
                        // The next sweep must rebuild its body ownership index.
                        body.debt = (body.debt + h * (steps - substep - 1) as f32).min(0.25);
                        simulated += h;
                        break;
                    }
                    if manifold.is_empty() {
                        body.velocity = Vec3::ZERO;
                        if tree {
                            body.angular = Vec3::ZERO;
                        }
                        simulated += h;
                        break;
                    }
                    // Keep the last collision-free pose, but spend the next
                    // sweep on the resolved velocity so tangential motion slides.
                    simulated += h;
                    continue;
                }
                {
                    edits.clear();
                    proposed_cells.clear();
                    proposed_cells.extend(proposed.iter().map(|&(p, _)| p));
                    // Only the changed boundary touches the expensive world
                    // setter. Interior cells retain their material and light.
                    edits.extend(body.occupied.iter()
                        .filter(|(p, _)| !proposed_cells.contains(p))
                        .map(|(p, _)| (*p, B::Air)));
                    edits.extend(proposed.iter().zip(&body.occupied).filter_map(|(&(p, b), &(from, _))| {
                        (p != from && (!old.contains(&p) || s.read(w, p).is_some_and(|(id, _)| id != b)))
                            .then_some((p, b))
                    }));
                    positions.clear();
                    positions.extend(edits.iter().map(|(p, _)| *p));
                    let has_heat = !w.sandbox.cells.is_empty();
                    if !admit_motion(w, &positions, &mut wake_chunks)
                        || !s.prepare(w, &positions)
                    {
                        support_checkpoint.restore(&mut w.sandbox.bodies);
                        body.velocity = previous_velocity;
                        body.angular = previous_angular;
                        body.settle_time = previous_settle;
                        body.debt = (body.debt + h * (steps - substep) as f32).min(0.25);
                        break;
                    }
                    if has_heat {
                        // Only recorded heat follows the material. Ambient
                        // temperature is sampled at the new position.
                        heat.clear();
                        heat.extend(body.occupied.iter().map(|(p, _)| w.sandbox.cells.get(&super::sandbox::key(*p)).copied()));
                    }
                    w.set_many(&edits);
                    if has_heat {
                        for &(p, _) in &body.occupied {
                            w.sandbox.cells.remove(&super::sandbox::key(p));
                        }
                        for (&(p, _), &state) in proposed.iter().zip(&heat) {
                            if let Some(state) = state {
                                w.sandbox.set(p, state);
                            } else {
                                w.sandbox.cells.remove(&super::sandbox::key(p));
                            }
                        }
                    }
                    for &(p, _) in &edits {
                        blocks.insert(p);
                        crate::world::mark_chunk_and_neighbors_dirty(chunks, p);
                    }
                    std::mem::swap(&mut body.occupied, &mut proposed);
                    old.clear();
                    old.extend(body.occupied.iter().map(|(p, _)| *p));
                    if resting {
                        support_contacts = support_manifold(s, w, &body, &old, &owners);
                    }
                }
                advance(&mut body, position, rotation);
                simulated += h;
            }
            if fractured {
                w.sandbox.bodies.insert(index, body);
                w.sandbox.body_cursor = (index + 1) % w.sandbox.bodies.len();
                continue;
            }
            let felled_tree = body.felled_tree;
            // Debris from a cracked crater keeps its blocks and simply stops being a body.
            let debris = body.voxels.iter().any(|(_, b)| !structural(*b));
            if felled_tree || debris {
                let calm =
                    body.velocity.length_squared() * scale * scale < 0.12
                    && body.angular.length_squared() * radius.max(0.5).powi(2) * scale * scale < 0.06;
                if body.settle_time < 0.9 {
                    body.settle_time = if calm { body.settle_time + simulated } else { 0.0 };
                }
                if body.settle_time >= 0.9 {
                    if felled_tree {
                        body.velocity = Vec3::ZERO;
                        body.angular = Vec3::ZERO;
                    } else {
                        for (p, _) in body.occupied {
                            w.activity.wake(p);
                        }
                        continue;
                    }
                }
            } else {
                let calm = body.velocity.length_squared() * scale * scale < 0.02
                    && body.angular.length_squared() * radius.max(0.5).powi(2) * scale * scale < 0.02;
                body.settle_time = if calm { body.settle_time + simulated } else { 0.0 };
                if body.settle_time >= 0.9 && !body.was_submerged {
                    body.velocity = Vec3::ZERO;
                    body.angular = Vec3::ZERO;
                }
            }
        }
        w.sandbox.bodies.insert(index, body);
        w.sandbox.body_cursor = (index + 1) % w.sandbox.bodies.len();
    }
}

#[cfg(test)]
mod felling_tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::simulation::sandbox::RigidBody;
    use crate::world::{World, WorldType};


    /// A felled trunk dropped from height breaks into wood; a slow one does not.
    fn fall(speed_voxels: f32) -> (usize, usize) {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 3);
        let base = IVec3::new(0, 60, 0);
        let voxels: Vec<_> = (0..4).map(|y| (IVec3::new(0, y, 0), B::Log)).collect();
        let occupied: Vec<_> = voxels.iter().map(|(p, b)| (base + *p, *b)).collect();
        world.set_many(&occupied.iter().map(|(p, _)| (*p, B::Air)).collect::<Vec<_>>());
        world.set_many(&occupied);
        world.sandbox.bodies.push(RigidBody {
            felled_tree: true,
            origin: base,
            position: Vec3::ZERO,
            velocity: Vec3::new(0.0, -speed_voxels, 0.0),
            rotation: glam::Quat::IDENTITY,
            angular: Vec3::ZERO,
            debt: 0.0,
            settle_time: 0.0,
            was_submerged: false,
            voxels: voxels.clone(),
            occupied,
        });
        // Simulation only runs on resident chunks. Make the column from the
        // floor to the trunk resident, the same way player edits do.
        world.set_many(&[(IVec3::new(0, 2, 0), B::Stone), (IVec3::new(0, 40, 0), B::Stone)]);
        let before = world.inventory.dropped.len();
        let mut sources = Residency::default();
        let (mut blocks, mut chunks) = (HashSet::new(), HashSet::new());
        for _ in 0..200 {
            tick(
                &mut sources,
                &mut world,
                1.0 / 60.0,
                (Vec3::splat(1000.0), Vec3::splat(1001.0)),
                crate::simulation::Budget::UNBOUNDED,
                &mut blocks,
                &mut chunks,
            );
        }
        (world.sandbox.bodies.len(), world.inventory.dropped.len() - before)
    }

    #[test]
    fn hard_landing_breaks_felled_trunk_into_wood() {
        // 3 m/s is the threshold; 20 voxels/s at 1 m voxels is 20 m/s.
        let (bodies, _) = fall(20.0);
        assert_eq!(bodies, 0, "hard landing should dissolve the body");
    }
}
