//! Resource animals: calm grazers the wildlife pass keeps as residents. Stats
//! and drops are bestiary rows; spawn places are `assets/spawns.ron`. This
//! module owns the kind numbers and the four bodies, authored in metres with
//! forward along -z and the ground at y = 0.
use super::{Enemy, enemy_presentation::{walk, two_bone, hoof_step, strike, wind}};
use crate::mesh::models::Model;
use glam::{Quat, Vec3};

pub(super) const COW: u32 = 49;
pub(super) const BOAR: u32 = 50;
pub(super) const DEER: u32 = 51;
pub(super) const FOWL: u32 = 52;

pub(super) fn is_kind(kind: u32) -> bool {
    body_kind(kind).is_some()
}

fn body_kind(kind: u32) -> Option<u32> {
    match kind {
        COW | BOAR | DEER | FOWL => Some(kind),
        super::bestiary::FIRST_GENERATED.. => super::bestiary::species(kind)
            .filter(|species| species.role == super::bestiary::Role::Grazer)
            .and_then(|species| species.animal_body)
            .filter(|body| matches!(*body, COW | BOAR | DEER | FOWL)),
        _ => None,
    }
}

const HIDE: f32 = 70.0;
const HOOF: f32 = 71.0;
const DARK: [f32; 3] = [0.14, 0.12, 0.11];

/// Body plan shared by the hoofed animals: a trunk, a neck, a head and four
/// two-link legs. Everything is relative to the feet.
struct Plan {
    coat: [f32; 3],
    belly: [f32; 3],
    trunk: Vec3,
    trunk_y: f32,
    hip_x: f32,
    leg_z: f32,
    stride: f32,
    head_at: Vec3,
    head: Vec3,
}

fn part(m: &mut Model, e: &Enemy, offset: Vec3, size: Vec3, rotation: Quat, color: [f32; 3], material: f32) {
    m.material_box(e.pos + offset, size, rotation, color, material, 0.0);
}

fn legs(m: &mut Model, e: &Enemy, plan: &Plan, shin: [f32; 3], body: Vec3) {
    let hip_y = plan.trunk_y - plan.trunk.y * 0.35;
    for (i, (side, front)) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].into_iter().enumerate() {
        // Walking is four separate footfalls, not a permanently paired trot.
        let (distance, lift) = hoof_step(e.phase, plan.stride, 0.12, i, e.gait);
        let hip = Vec3::new(side * plan.hip_x, hip_y, -front * plan.leg_z);
        let foot = Vec3::new(hip.x, 0.06 + lift, hip.z + distance);
        let link = ((hip_y - 0.06).powi(2) + plan.stride.powi(2)).sqrt() * 0.5 + 0.012;
        let hip = hip + body;
        let (knee, foot) = two_bone(hip, foot, link, front);
        m.material_beam(e.pos + hip, e.pos + knee, 0.07, plan.coat, HIDE);
        m.material_beam(e.pos + knee, e.pos + foot, 0.055, shin, HIDE);
        part(m, e, foot, Vec3::new(0.14, 0.12, 0.17), Quat::IDENTITY, DARK, HOOF);
    }
}

/// Live head pose: a nod in step while walking and, when standing, an
/// occasional slow dip to graze. Keyed to the slot so a herd never moves as one.
fn posed(mut plan: Plan, e: &Enemy, time: f32) -> (Plan, Vec3) {
    let gait = e.gait.clamp(0.0, 1.0);
    let (_, lift) = hoof_step(e.phase, plan.stride, 1.0, 0, e.gait);
    let nod = (lift - 0.5) * 0.05 * gait;
    let cycle = (time * 0.23 + e.slot as f32 * 1.7).sin();
    let t = ((cycle - 0.35) / 0.4).clamp(0.0, 1.0);
    let graze = t * t * (3.0 - 2.0 * t) * (1.0 - gait);
    let low = Vec3::new(0.0, plan.head.y * 0.6, plan.head_at.z * 1.08);
    let body = body_motion(e, &plan, time);
    plan.head_at = plan.head_at.lerp(low, graze) - Vec3::Y * nod + body;
    (plan, body)
}

/// One shared body displacement keeps hips, neck, hide and accessories joined.
/// Feet remain in their ground frame rather than bobbing with the trunk.
fn body_motion(e: &Enemy, plan: &Plan, time: f32) -> Vec3 {
    let lift: f32 = (0..4).map(|leg| hoof_step(e.phase, plan.stride, 1.0, leg, e.gait).1).sum();
    let breath = (time * 1.6 + e.slot as f32).sin() * 0.008 * (1.0 - e.gait.clamp(0.0, 1.0));
    Vec3::Y * (breath + 0.006 * e.gait.clamp(0.0, 1.0) - lift * 0.012)
}

fn hoofed(m: &mut Model, e: &Enemy, plan: &Plan, shin: [f32; 3], breath: Vec3) {
    part(m, e, Vec3::new(0.0, plan.trunk_y, 0.0) + breath, plan.trunk, Quat::IDENTITY, plan.coat, HIDE);
    part(m, e, Vec3::new(0.0, plan.trunk_y - plan.trunk.y * 0.3, 0.0) + breath,
        Vec3::new(plan.trunk.x * 0.9, plan.trunk.y * 0.45, plan.trunk.z * 0.85), Quat::IDENTITY, plan.belly, HIDE);
    legs(m, e, plan, shin, breath);
    let neck_root = Vec3::new(0.0, plan.trunk_y + plan.trunk.y * 0.25, -plan.trunk.z * 0.45) + breath;
    let neck = (neck_root + plan.head_at) * 0.5;
    let d = plan.head_at - neck_root;
    part(m, e, neck, Vec3::new(plan.head.x * 0.7, d.length(), plan.head.z * 0.6),
        Quat::from_rotation_arc(Vec3::Y, d.normalize_or_zero()), plan.coat, HIDE);
    part(m, e, plan.head_at, plan.head, Quat::IDENTITY, plan.coat, HIDE);
    for side in [-1.0f32, 1.0] {
        part(m, e, plan.head_at + Vec3::new(side * plan.head.x * 0.42, plan.head.y * 0.1, -plan.head.z * 0.5),
            Vec3::splat(0.06), Quat::IDENTITY, DARK, HIDE);
    }
}

fn cow(m: &mut Model, e: &Enemy, time: f32) {
    let plan = Plan {
        coat: [0.36, 0.25, 0.18], belly: [0.86, 0.82, 0.74],
        trunk: Vec3::new(0.78, 0.74, 1.40), trunk_y: 1.02, hip_x: 0.28, leg_z: 0.52, stride: 0.26,
        head_at: Vec3::new(0.0, 1.22, -0.98), head: Vec3::new(0.34, 0.36, 0.46),
    };
    let (plan, body) = posed(plan, e, time);
    hoofed(m, e, &plan, [0.32, 0.22, 0.16], body);
    // Horns and a muzzle.
    for side in [-1.0f32, 1.0] {
        part(m, e, plan.head_at + Vec3::new(side * 0.24, 0.2, 0.08), Vec3::new(0.18, 0.07, 0.07),
            Quat::from_rotation_z(side * 0.5), [0.80, 0.74, 0.62], HOOF);
    }
    part(m, e, plan.head_at + Vec3::new(0.0, -0.08, -0.28), Vec3::new(0.26, 0.2, 0.14), Quat::IDENTITY, [0.78, 0.62, 0.58], HIDE);
}

fn boar(m: &mut Model, e: &Enemy, time: f32) {
    let plan = Plan {
        coat: [0.26, 0.2, 0.15], belly: [0.33, 0.27, 0.21],
        trunk: Vec3::new(0.56, 0.52, 1.02), trunk_y: 0.62, hip_x: 0.2, leg_z: 0.36, stride: 0.2,
        head_at: Vec3::new(0.0, 0.62, -0.72), head: Vec3::new(0.30, 0.30, 0.44),
    };
    let (plan, body) = posed(plan, e, time);
    hoofed(m, e, &plan, [0.22, 0.17, 0.13], body);
    // Bristle ridge and tusks.
    part(m, e, Vec3::new(0.0, 0.94, -0.1) + body, Vec3::new(0.12, 0.14, 0.9), Quat::IDENTITY, [0.18, 0.14, 0.11], HIDE);
    for side in [-1.0f32, 1.0] {
        part(m, e, plan.head_at + Vec3::new(side * 0.12, -0.06, -0.26), Vec3::new(0.04, 0.12, 0.04),
            Quat::from_rotation_x(0.5), [0.88, 0.84, 0.72], HOOF);
    }
}

fn deer(m: &mut Model, e: &Enemy, time: f32) {
    let plan = Plan {
        coat: [0.55, 0.40, 0.26], belly: [0.80, 0.72, 0.60],
        trunk: Vec3::new(0.46, 0.52, 1.10), trunk_y: 1.00, hip_x: 0.18, leg_z: 0.42, stride: 0.34,
        head_at: Vec3::new(0.0, 1.56, -0.82), head: Vec3::new(0.22, 0.26, 0.40),
    };
    let (plan, body) = posed(plan, e, time);
    hoofed(m, e, &plan, [0.42, 0.30, 0.20], body);
    // Antlers: a beam and two tines each side, keyed to the slot so herds differ.
    if e.slot % 2 == 0 {
        for side in [-1.0f32, 1.0] {
            let root = plan.head_at + Vec3::new(side * 0.08, 0.16, 0.06);
            let beam = root + Vec3::new(side * 0.18, 0.34, 0.1);
            m.material_beam(e.pos + root, e.pos + beam, 0.035, [0.62, 0.55, 0.42], HOOF);
            m.material_beam(e.pos + beam, e.pos + beam + Vec3::new(side * 0.1, 0.16, -0.08), 0.03, [0.62, 0.55, 0.42], HOOF);
            m.material_beam(e.pos + root.lerp(beam, 0.5), e.pos + root.lerp(beam, 0.5) + Vec3::new(0.0, 0.14, -0.14), 0.028, [0.62, 0.55, 0.42], HOOF);
        }
    }
}

fn fowl(m: &mut Model, e: &Enemy, time: f32) {
    let coat = [0.46, 0.40, 0.34];
    let bob = (time * 2.2 + e.slot as f32).sin() * 0.01;
    part(m, e, Vec3::new(0.0, 0.34 + bob, 0.0), Vec3::new(0.30, 0.26, 0.42), Quat::from_rotation_x(-0.2), coat, HIDE);
    part(m, e, Vec3::new(0.0, 0.46 + bob, 0.22), Vec3::new(0.12, 0.14, 0.18), Quat::from_rotation_x(0.6), [0.30, 0.26, 0.22], HIDE);
    part(m, e, Vec3::new(0.0, 0.62 + bob, -0.24), Vec3::new(0.12, 0.13, 0.16), Quat::IDENTITY, coat, HIDE);
    part(m, e, Vec3::new(0.0, 0.60 + bob, -0.36), Vec3::new(0.05, 0.04, 0.12), Quat::IDENTITY, [0.86, 0.62, 0.22], HOOF);
    for side in [-1.0f32, 1.0] {
        let (distance, lift) = walk(e, 0.12, 0.08,
            if side < 0.0 { 0.0 } else { std::f32::consts::PI }, 1.0);
        m.material_beam(e.pos + Vec3::new(side * 0.07, 0.23 + bob, 0.0),
            e.pos + Vec3::new(side * 0.07, 0.02 + lift, distance), 0.015, [0.86, 0.62, 0.22], HOOF);
        part(m, e, Vec3::new(side * 0.07, 0.02 + lift, distance - 0.03), Vec3::new(0.08, 0.02, 0.12), Quat::IDENTITY, [0.86, 0.62, 0.22], HOOF);
        part(m, e, Vec3::new(side * 0.16, 0.38 + bob, 0.02), Vec3::new(0.05, 0.16, 0.30), Quat::from_rotation_x(-0.15), [0.38, 0.33, 0.28], HIDE);
    }
}

pub(super) fn draw(e: &Enemy, m: &mut Model, time: f32) {
    let mark = m.mark();
    let body = body_kind(e.kind);
    match body {
        Some(COW) => cow(m, e, time),
        Some(BOAR) => boar(m, e, time),
        Some(DEER) => deer(m, e, time),
        Some(FOWL) => fowl(m, e, time),
        _ => {}
    }
    if body == Some(BOAR) {
        // A provoked boar drops its head through the wind-up and hooks the
        // tusks up on the release, pivoting on its front hooves.
        let pitch = -wind(e) * 0.24 + strike(e) * 0.14;
        if pitch != 0.0 { m.rotate_since(mark, e.pos + Vec3::new(0.0, 0.0, -0.36), Quat::from_rotation_x(pitch)); }
    }
    let up = if e.up.length_squared() > 0.5 { e.up.normalize() } else { Vec3::Y };
    m.rotate_since(mark, e.pos, Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(e.yaw));
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;

    #[test]
    fn boar_turns_on_whoever_hurts_it_and_other_grazers_never_do() {
        let mut boar = Enemy::new(IVec3::ZERO, 0, BOAR, Vec3::ZERO, 0);
        assert!(!boar.hostile(), "an unhurt boar grazes");
        boar.hurt(1.0);
        assert!(boar.hostile(), "a hurt boar fights back");
        for kind in [COW, DEER, FOWL, super::super::wildlife::SHEEP] {
            let mut e = Enemy::new(IVec3::ZERO, 0, kind, Vec3::ZERO, 0);
            e.hurt(1.0);
            assert!(!e.hostile(), "kind {kind} flees when hurt");
        }
    }
}
