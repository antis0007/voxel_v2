//! Resource animals: calm grazers the wildlife pass keeps as residents. Stats
//! and drops are bestiary rows; spawn places are `assets/spawns.ron`. This
//! module owns the kind numbers and the four bodies, authored in metres with
//! forward along -z and the ground at y = 0.
use super::{Enemy, enemy_presentation::step};
use crate::mesh::models::Model;
use glam::{Quat, Vec3};

pub(super) const COW: u32 = 49;
pub(super) const BOAR: u32 = 50;
pub(super) const DEER: u32 = 51;
pub(super) const FOWL: u32 = 52;

pub(super) fn is_kind(kind: u32) -> bool {
    matches!(kind, COW | BOAR | DEER | FOWL)
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

fn legs(m: &mut Model, e: &Enemy, plan: &Plan, shin: [f32; 3]) {
    let hip_y = plan.trunk_y - plan.trunk.y * 0.35;
    for (i, (side, front)) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].into_iter().enumerate() {
        // Diagonal pairs swing together, as a walking quadruped's do.
        let offset = if i == 0 || i == 3 { 0.0 } else { std::f32::consts::PI };
        let (distance, lift) = step(e.phase * 2.6 + offset, plan.stride, 0.12, e.gait);
        let z = -front * plan.leg_z + distance;
        let knee = Vec3::new(side * plan.hip_x, hip_y * 0.55 + lift * 0.5, z + front * 0.04);
        let foot = Vec3::new(side * plan.hip_x, 0.06 + lift, z);
        part(m, e, Vec3::new(side * plan.hip_x, (hip_y + knee.y) * 0.5, (-front * plan.leg_z + knee.z) * 0.5),
            Vec3::new(0.14, hip_y - knee.y, 0.16), Quat::IDENTITY, plan.coat, HIDE);
        part(m, e, (knee + foot) * 0.5, Vec3::new(0.11, knee.y - foot.y, 0.12), Quat::IDENTITY, shin, HIDE);
        part(m, e, foot, Vec3::new(0.14, 0.12, 0.17), Quat::IDENTITY, DARK, HOOF);
    }
}

fn hoofed(m: &mut Model, e: &Enemy, time: f32, plan: &Plan, shin: [f32; 3]) {
    let breath = Vec3::Y * (time * 1.6 + e.slot as f32).sin() * 0.008 * (1.0 - e.gait);
    part(m, e, Vec3::new(0.0, plan.trunk_y, 0.0) + breath, plan.trunk, Quat::IDENTITY, plan.coat, HIDE);
    part(m, e, Vec3::new(0.0, plan.trunk_y - plan.trunk.y * 0.3, 0.0) + breath,
        Vec3::new(plan.trunk.x * 0.9, plan.trunk.y * 0.45, plan.trunk.z * 0.85), Quat::IDENTITY, plan.belly, HIDE);
    legs(m, e, plan, shin);
    let neck_root = Vec3::new(0.0, plan.trunk_y + plan.trunk.y * 0.25, -plan.trunk.z * 0.45);
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
    hoofed(m, e, time, &plan, [0.32, 0.22, 0.16]);
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
    hoofed(m, e, time, &plan, [0.22, 0.17, 0.13]);
    // Bristle ridge and tusks.
    part(m, e, Vec3::new(0.0, 0.94, -0.1), Vec3::new(0.12, 0.14, 0.9), Quat::IDENTITY, [0.18, 0.14, 0.11], HIDE);
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
    hoofed(m, e, time, &plan, [0.42, 0.30, 0.20]);
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
        let (distance, lift) = step(e.phase * 3.4 + if side < 0.0 { 0.0 } else { std::f32::consts::PI }, 0.12, 0.08, e.gait);
        part(m, e, Vec3::new(side * 0.07, 0.12 + lift * 0.5, distance), Vec3::new(0.03, 0.22, 0.03), Quat::IDENTITY, [0.86, 0.62, 0.22], HOOF);
        part(m, e, Vec3::new(side * 0.07, 0.02 + lift, distance - 0.03), Vec3::new(0.08, 0.02, 0.12), Quat::IDENTITY, [0.86, 0.62, 0.22], HOOF);
        part(m, e, Vec3::new(side * 0.16, 0.38 + bob, 0.02), Vec3::new(0.05, 0.16, 0.30), Quat::from_rotation_x(-0.15), [0.38, 0.33, 0.28], HIDE);
    }
}

pub(super) fn draw(e: &Enemy, m: &mut Model, time: f32) {
    let mark = m.mark();
    match e.kind {
        COW => cow(m, e, time),
        BOAR => boar(m, e, time),
        DEER => deer(m, e, time),
        FOWL => fowl(m, e, time),
        _ => {}
    }
    m.rotate_since(mark, e.pos, Quat::from_rotation_y(e.yaw));
}
