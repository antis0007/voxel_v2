//! Bounded contact patches and accumulated sequential impulses.
use crate::simulation::sandbox::RigidBody;
use crate::world::BlockId;
use glam::{IVec3, Mat3, Vec2, Vec3};

const MAX_PATCHES: usize = 16;
const ITERATIONS: usize = 8;

#[derive(Clone, Copy)]
pub(super) struct Contact {
    pub cell: IVec3,
    pub obstacle_cell: IVec3,
    pub other: Option<usize>,
    pub point: Vec3,
    pub normal: Vec3,
    pub material: BlockId,
    pub obstacle: BlockId,
}

struct Patch {
    other: Option<usize>,
    normal: Vec3,
    corners: [Contact; 4],
}

#[derive(Default)]
pub(super) struct Manifold {
    patches: Vec<Patch>,
}

pub(super) struct Checkpoint(Vec<(usize, Vec3, Vec3, f32)>);

impl Checkpoint {
    pub fn restore(&self, bodies: &mut [RigidBody]) {
        for &(index, velocity, angular, settle_time) in &self.0 {
            bodies[index].velocity = velocity;
            bodies[index].angular = angular;
            bodies[index].settle_time = settle_time;
        }
    }
}

fn tangents(n: Vec3) -> [Vec3; 2] {
    let t = n.cross(if n.y.abs() < 0.9 { Vec3::Y } else { Vec3::X });
    [t, n.cross(t)]
}

impl Manifold {
    /// Residency can defer the entire sweep after support has been solved.
    /// Its equal-and-opposite impulses must then be rolled back together.
    pub fn checkpoint(&self, bodies: &[RigidBody]) -> Checkpoint {
        let mut saved = Vec::new();
        for patch in &self.patches {
            if let Some(index) = patch.other {
                if saved.iter().any(|(i, _, _, _)| *i == index) {
                    continue;
                }
                let body = &bodies[index];
                saved.push((index, body.velocity, body.angular, body.settle_time));
            }
        }
        Checkpoint(saved)
    }

    pub fn push(&mut self, contact: Contact) {
        let [a, b] = tangents(contact.normal);
        for offset in [a + b, a - b, -a + b, -a - b] {
            let mut corner = contact;
            corner.point += offset * 0.48;
            self.insert(corner);
        }
    }

    fn insert(&mut self, contact: Contact) {
        if let Some(patch) = self
            .patches
            .iter_mut()
            .find(|p| p.other == contact.other && p.normal == contact.normal)
        {
            let [a, b] = tangents(contact.normal);
            // Keep the footprint's extreme points, rather than its first cells.
            // A wide slab therefore receives a distributed support impulse.
            for (corner, axis) in patch.corners.iter_mut().zip([a + b, a - b, -a + b, -a - b]) {
                if contact.point.dot(axis) > corner.point.dot(axis) {
                    *corner = contact;
                }
            }
        } else if self.patches.len() < MAX_PATCHES {
            self.patches.push(Patch {
                other: contact.other,
                normal: contact.normal,
                corners: [contact; 4],
            });
        }
    }

    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }
}

#[derive(Clone, Copy)]
struct Properties {
    center: Vec3,
    inverse_mass: f32,
    inverse_inertia: Mat3,
}

impl Properties {
    fn of(body: &RigidBody) -> Self {
        let (mass, center, inverse) = body.mass_properties();
        let rotation = Mat3::from_quat(body.rotation);
        Self {
            center: body.origin.as_vec3()
                + body.position
                + body.rotation * center
                + Vec3::splat(0.5),
            inverse_mass: mass.recip(),
            inverse_inertia: rotation * inverse * rotation.transpose(),
        }
    }

    fn response(self, arm: Vec3, axis: Vec3) -> f32 {
        self.inverse_mass + axis.dot((self.inverse_inertia * arm.cross(axis)).cross(arm))
    }

    fn apply(self, body: &mut RigidBody, arm: Vec3, impulse: Vec3) {
        body.velocity += impulse * self.inverse_mass;
        body.angular += self.inverse_inertia * arm.cross(impulse);
        if impulse.length_squared() > 1e-12 {
            body.settle_time = 0.0;
        }
    }
}

struct Constraint {
    contact: Contact,
    arm: Vec3,
    other_arm: Vec3,
    other_properties: Option<Properties>,
    normal_mass: f32,
    tangent_mass: Vec2,
    tangents: [Vec3; 2],
    friction: f32,
    bounce: f32,
    normal_impulse: f32,
    tangent_impulse: Vec2,
}

fn relative(body: &RigidBody, other: Option<&RigidBody>, arm: Vec3, other_arm: Vec3) -> Vec3 {
    body.velocity + body.angular.cross(arm)
        - other.map_or(Vec3::ZERO, |b| b.velocity + b.angular.cross(other_arm))
}

#[derive(Clone, Copy)]
pub(super) struct Impact {
    pub cell: IVec3,
    pub terrain: Option<(IVec3, Vec3)>,
    pub energy: f32,
    pub speed: f32,
}

/// Collision energy uses metre velocities and density-normalized voxel mass.
/// Shape tensors are evaluated once per participant, outside the iteration loop.
pub(super) fn solve(
    body: &mut RigidBody,
    others: &mut [RigidBody],
    manifold: &Manifold,
    scale: f32,
) -> Option<Impact> {
    let properties = Properties::of(body);
    let mut other_properties: Vec<(usize, Properties)> = Vec::new();
    let mut constraints = Vec::with_capacity(manifold.patches.len() * 4);
    let mut impact: Option<Impact> = None;
    for patch in &manifold.patches {
        for (index, &contact) in patch.corners.iter().enumerate() {
            if patch.corners[..index]
                .iter()
                .any(|c| c.point == contact.point)
            {
                continue;
            }
            let other = contact.other.map(|index| {
                if let Some((_, p)) = other_properties.iter().find(|(i, _)| *i == index) {
                    return *p;
                }
                let p = Properties::of(&others[index]);
                other_properties.push((index, p));
                p
            });
            let arm = contact.point - properties.center;
            let other_arm = other.map_or(Vec3::ZERO, |p| contact.point - p.center);
            let effective_mass = |axis| {
                (properties.response(arm, axis)
                    + other.map_or(0.0, |p| p.response(other_arm, axis)))
                .max(1e-6)
                .recip()
            };
            let normal_mass = effective_mass(contact.normal);
            let velocity = relative(body, contact.other.map(|i| &others[i]), arm, other_arm);
            let speed = (-velocity.dot(contact.normal) * scale).max(0.0);
            let energy = 0.5 * normal_mass * speed * speed;
            if impact.is_none_or(|i| energy > i.energy) {
                impact = Some(Impact {
                    cell: contact.cell,
                    terrain: contact.other.is_none().then_some((contact.obstacle_cell, contact.normal)),
                    energy,
                    speed,
                });
            }
            let a = contact.material.material().physics;
            let b = contact.obstacle.material().physics;
            let restitution = (a.restitution.max(0.0) * b.restitution.max(0.0))
                .sqrt()
                .clamp(0.0, 0.8);
            let tangents = tangents(contact.normal);
            constraints.push(Constraint {
                contact,
                arm,
                other_arm,
                other_properties: other,
                normal_mass,
                tangent_mass: Vec2::new(effective_mass(tangents[0]), effective_mass(tangents[1])),
                tangents,
                friction: (a.friction.max(0.0) * b.friction.max(0.0)).sqrt().min(2.0),
                bounce: if speed > 1.0 {
                    speed / scale * restitution
                } else {
                    0.0
                },
                normal_impulse: 0.0,
                tangent_impulse: Vec2::ZERO,
            });
        }
    }
    for _ in 0..ITERATIONS {
        for c in &mut constraints {
            let mut other = c.contact.other.map(|i| &mut others[i]);
            let velocity = relative(body, other.as_deref(), c.arm, c.other_arm);
            let old_normal = c.normal_impulse;
            c.normal_impulse =
                (old_normal + (c.bounce - velocity.dot(c.contact.normal)) * c.normal_mass).max(0.0);
            let impulse = c.contact.normal * (c.normal_impulse - old_normal);
            properties.apply(body, c.arm, impulse);
            if let (Some(b), Some(p)) = (other.as_deref_mut(), c.other_properties) {
                p.apply(b, c.other_arm, -impulse);
            }

            let velocity = relative(body, other.as_deref(), c.arm, c.other_arm);
            let old_tangent = c.tangent_impulse;
            let slip = Vec2::new(velocity.dot(c.tangents[0]), velocity.dot(c.tangents[1]));
            c.tangent_impulse = (old_tangent - slip * c.tangent_mass)
                .clamp_length_max(c.friction * c.normal_impulse);
            let delta = c.tangent_impulse - old_tangent;
            let impulse = c.tangents[0] * delta.x + c.tangents[1] * delta.y;
            properties.apply(body, c.arm, impulse);
            if let (Some(b), Some(p)) = (other.as_deref_mut(), c.other_properties) {
                p.apply(b, c.other_arm, -impulse);
            }
        }
    }
    body.velocity = body.velocity.clamp_length_max(120.0 / scale);
    body.angular = body.angular.clamp_length_max(12.0);
    for (i, _) in other_properties {
        others[i].velocity = others[i].velocity.clamp_length_max(120.0 / scale);
        others[i].angular = others[i].angular.clamp_length_max(12.0);
    }
    impact
}
