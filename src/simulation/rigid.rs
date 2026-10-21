//! Bounded voxel assemblies with swept translation and rotational collision.
use super::{
    cellular::{admission, changed},
    residency::Residency,
    sandbox::{MAX_BODIES, MAX_BODY_VOXELS, RigidBody},
};
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

fn structural(b: B) -> bool {
    matches!(b, B::RigidCore | B::Planks | B::Log)
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
            continue;
        }
        if !super::cellular::allowed(w, q)
            || w.sandbox
                .bodies
                .iter()
                .any(|body| body.occupied.iter().any(|(r, _)| *r == q))
        {
            return;
        }
        voxels.push((q, b));
        if voxels.len() > MAX_BODY_VOXELS {
            return;
        }
        queue.extend(DIRS.map(|d| q + d));
    }
    if voxels.is_empty() {
        return;
    }
    w.sandbox.bodies.push(RigidBody {
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
    deadline: std::time::Instant,
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
    for _ in 0..count {
        if w.sandbox.bodies.is_empty() || std::time::Instant::now() >= deadline {
            break;
        }
        let index = w.sandbox.body_cursor % w.sandbox.bodies.len();
        let mut body = w.sandbox.bodies.remove(index);
        let mut ready = true;
        for &(p, _) in &body.occupied {
            ready &= s.request(w, chunk(p));
        }
        let valid = !ready
            || body
                .occupied
                .iter()
                .all(|&(p, b)| s.read(w, p).is_some_and(|(id, _)| id == b));
        if !valid {
            continue;
        } // External mining dissolves the assembly without restoring removed mass.
        if ready {
            let dt = body.debt.min(1.0 / 20.0);
            body.debt -= dt;
            let scale = w.voxel_scale.meters();
            body.velocity.y = (body.velocity.y - 9.81 / scale * dt).max(-30.0 / scale);
            // Assemblies obey the same coarse buoyancy rule as the player:
            // wooden frames float, dense cores sink, and everything in water
            // loses speed to drag. This is sampled from the body's occupied
            // cells, so partial submersion settles instead of snapping.
            // Sample a bounded subset of the assembly instead of querying the
            // world for every voxel every tick. Eight probes are enough to
            // distinguish a floating edge from a fully submerged frame.
            let stride = (body.occupied.len() / 8).max(1);
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
                        w.adventure
                            .splash(surface, ((-body.velocity.y * scale) / 9.0).clamp(0.22, 1.0));
                    }
                }
                let fraction = submerged as f32 / sampled as f32;
                let density = body
                    .voxels
                    .iter()
                    .map(|(_, b)| b.material().physics.density)
                    .sum::<f32>()
                    / body.voxels.len() as f32;
                let density_ratio = density / 1000.0;
                body.velocity.y +=
                    fraction * (1.0 - density_ratio).clamp(-1.0, 1.0) * 9.81 / scale * dt * 4.0;
                let drag = (fraction * 2.4 * dt).min(0.55);
                body.velocity *= 1.0 - drag;
                body.angular *= 1.0 - drag * 0.7;
            }
            body.was_submerged = submerged > 0 && sampled > 0;
            let steps = ((body.velocity.length() * dt / 0.25).ceil() as usize).clamp(1, 64);
            let mut broke_apart = false;
            for substep in 0..steps {
                if std::time::Instant::now() >= deadline {
                    body.debt =
                        (body.debt + dt * (steps - substep) as f32 / steps as f32).min(0.25);
                    break;
                }
                let h = dt / steps as f32;
                let rest_center = body.voxels.iter().map(|(p, _)| p.as_vec3()).sum::<Vec3>()
                    / body.voxels.len() as f32;
                let rotation =
                    (Quat::from_scaled_axis(body.angular * h) * body.rotation).normalize();
                let position = body.position + body.velocity * h + body.rotation * rest_center
                    - rotation * rest_center;
                let proposed: Vec<_> = body
                    .voxels
                    .iter()
                    .map(|&(p, b)| {
                        (
                            body.origin + (position + rotation * p.as_vec3()).round().as_ivec3(),
                            b,
                        )
                    })
                    .collect();
                let old: HashSet<_> = body.occupied.iter().map(|(p, _)| *p).collect();
                let mut unique = HashSet::new();
                let mut blocked = false;
                let mut loaded = true;
                let mut contact = None;
                for (voxel_index, &(p, _)) in proposed.iter().enumerate() {
                    if !unique.insert(p) || !admission(w, body.origin, p) {
                        blocked = true;
                        break;
                    }
                    loaded &= s.request(w, chunk(p));
                    let min = p.as_vec3();
                    if min.cmplt(player.1).all() && (min + Vec3::ONE).cmpgt(player.0).all() {
                        blocked = true;
                        break;
                    }
                    if !old.contains(&p) && s.read(w, p).is_some_and(|(b, _)| b != B::Air) {
                        blocked = true;
                        contact = Some((p, (body.occupied[voxel_index].0 - p).as_vec3()));
                        break;
                    }
                }
                if !loaded {
                    break;
                }
                if blocked {
                    // A felled trunk that lands hard splits into wood on impact
                    // rather than skidding on as one body. Below the threshold it
                    // keeps tipping and settles as before.
                    let impact_m = body.velocity.length() * scale;
                    if body.voxels.iter().all(|(_, b)| *b == B::Log) && impact_m > FELL_BREAK_SPEED_M {
                        let cells: Vec<_> = body.occupied.iter().map(|(p, _)| *p).collect();
                        crate::world::expedition::drop_log_cells(w, &cells, blocks, chunks);
                        for p in cells {
                            w.activity.wake(p);
                        }
                        broke_apart = true;
                        break;
                    }
                    let center = body
                        .voxels
                        .iter()
                        .map(|(p, _)| body.rotation * p.as_vec3())
                        .sum::<Vec3>()
                        / body.voxels.len() as f32;
                    let supports: Vec<_> = body
                        .occupied
                        .iter()
                        .filter(|(p, _)| {
                            !old.contains(&(*p - IVec3::Y))
                                && s.read(w, *p - IVec3::Y).is_some_and(|(b, _)| b.is_solid())
                        })
                        .map(|(p, _)| (*p - body.origin).as_vec3())
                        .collect();
                    if !supports.is_empty() {
                        let support =
                            supports.iter().copied().sum::<Vec3>() / supports.len() as f32;
                        body.angular += (center + body.position - support).cross(Vec3::NEG_Y) * h;
                    }
                    if let Some((p, direction)) = contact {
                        let axis = if direction.y.abs() >= direction.x.abs().max(direction.z.abs())
                        {
                            1
                        } else if direction.x.abs() > direction.z.abs() {
                            0
                        } else {
                            2
                        };
                        let mut normal = Vec3::ZERO;
                        normal[axis] = direction[axis].signum();
                        if let Some(other) = w
                            .sandbox
                            .bodies
                            .iter_mut()
                            .find(|other| other.occupied.iter().any(|(q, _)| *q == p))
                        {
                            let relative = (body.velocity - other.velocity).dot(normal);
                            if relative < 0.0 {
                                let m = body.voxels.len() as f32;
                                let n = other.voxels.len() as f32;
                                let impulse = -(1.05) * relative / (1.0 / m + 1.0 / n);
                                body.velocity += normal * (impulse / m);
                                other.velocity -= normal * (impulse / n);
                            }
                        } else {
                            let speed = body.velocity.dot(normal);
                            if speed < 0.0 {
                                body.velocity -= normal * speed;
                            }
                        }
                    } else {
                        body.velocity = Vec3::ZERO;
                    }
                    body.velocity *= 0.85;
                    body.angular = (body.angular * 0.8).clamp_length_max(1.5);
                    break;
                }
                let positions: Vec<_> = body
                    .occupied
                    .iter()
                    .chain(&proposed)
                    .map(|(p, _)| *p)
                    .collect();
                if !s.prepare(w, &positions) {
                    break;
                }
                if proposed != body.occupied {
                    let heat: Vec<_> = body
                        .occupied
                        .iter()
                        .map(|(p, _)| w.sandbox.cell(*p))
                        .collect();
                    if !w
                        .sandbox
                        .can_admit(&proposed.iter().map(|(p, _)| *p).collect::<Vec<_>>())
                    {
                        break;
                    }
                    let mut edits: Vec<_> =
                        body.occupied.iter().map(|(p, _)| (*p, B::Air)).collect();
                    edits.extend(proposed.iter().copied());
                    w.set_many(&edits);
                    for &(p, _) in &body.occupied {
                        w.sandbox.set(p, Default::default());
                    }
                    for (&(p, _), state) in proposed.iter().zip(heat) {
                        w.sandbox.set(p, state);
                    }
                    for p in positions {
                        changed(p, blocks, chunks);
                    }
                    body.occupied = proposed;
                }
                body.position = position;
                body.rotation = rotation;
                let shift = body.position.floor().as_ivec3();
                body.origin += shift;
                body.position -= shift.as_vec3();
            }
            // The trunk was already converted to wood: do not reinsert the body.
            if broke_apart {
                continue;
            }
            // A felled tree is represented only by logs. Once the assembly has
            // stopped moving, give every remaining log back as loot and remove
            // the temporary body instead of leaving a permanent log sculpture.
            let felled_tree = body.voxels.iter().all(|(_, b)| *b == B::Log);
            if felled_tree {
                let calm =
                    body.velocity.length_squared() < 0.12 && body.angular.length_squared() < 0.06;
                body.settle_time = if calm { body.settle_time + dt } else { 0.0 };
                if body.settle_time >= 0.9 {
                    let cells: Vec<_> = body.occupied.iter().map(|(p, _)| *p).collect();
                    crate::world::expedition::drop_log_cells(w, &cells, blocks, chunks);
                    for p in cells {
                        w.activity.wake(p);
                    }
                    continue;
                }
            } else {
                body.settle_time = 0.0;
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
    use std::time::{Duration, Instant};

    /// A felled trunk dropped from height breaks into wood; a slow one does not.
    fn fall(speed_voxels: f32) -> (usize, usize) {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 3);
        let base = IVec3::new(0, 60, 0);
        let voxels: Vec<_> = (0..4).map(|y| (IVec3::new(0, y, 0), B::Log)).collect();
        let occupied: Vec<_> = voxels.iter().map(|(p, b)| (base + *p, *b)).collect();
        world.set_many(&occupied.iter().map(|(p, _)| (*p, B::Air)).collect::<Vec<_>>());
        world.set_many(&occupied);
        world.sandbox.bodies.push(RigidBody {
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
                Instant::now() + Duration::from_secs(1),
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
