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
        voxels: voxels.iter().map(|(q, b)| (*q - p, *b)).collect(),
        occupied: voxels,
    });
}
pub(super) fn tick(
    s: &mut Residency,
    w: &mut World,
    dt: f32,
    player: (Vec3, Vec3),
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_micros(800);
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
            let steps = ((body.velocity.length() * dt / 0.25).ceil() as usize).clamp(1, 64);
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
        }
        w.sandbox.bodies.insert(index, body);
        w.sandbox.body_cursor = (index + 1) % w.sandbox.bodies.len();
    }
}
