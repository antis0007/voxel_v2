//! Creatures from the shared bestiary tradition, placed by `assets/spawns.ron`.
//! Combat identity comes from their bestiary rows (brawlers use the generic
//! melee code, `melee` sets the hit); this module owns the kind numbers and
//! the bodies, authored in metres with forward along -z and the ground at y = 0.
use super::{Enemy, enemy_presentation::{strike, travel, walk}};
use crate::mesh::models::Model;
use glam::{Quat, Vec3};

pub(super) const OWLBEAR: u32 = 53;
pub(super) const GNOLL: u32 = 54;
pub(super) const STIRGE: u32 = 55;
pub(super) const GELATINOUS_CUBE: u32 = 56;
pub(super) const HELL_HOUND: u32 = 57;
pub(super) const YETI: u32 = 58;

pub(super) fn is_kind(kind: u32) -> bool {
    matches!(kind, OWLBEAR..=YETI)
}

const SKIN: f32 = 70.0;
const BONE: f32 = 71.0;
const DARK: [f32; 3] = [0.10, 0.09, 0.08];

fn p(e: &Enemy, x: f32, y: f32, z: f32) -> Vec3 {
    e.pos + Vec3::new(x, y, z)
}
fn block(m: &mut Model, at: Vec3, size: Vec3, color: [f32; 3], material: f32) {
    m.material_box(at, size, Quat::IDENTITY, color, material, 0.0);
}
fn glow(m: &mut Model, at: Vec3, size: Vec3, color: [f32; 3], energy: f32) {
    m.material_box(at, size, Quat::IDENTITY, color, 60.0, energy);
}
fn beam(m: &mut Model, a: Vec3, b: Vec3, width: f32, color: [f32; 3]) {
    m.material_beam(a, b, width, color, SKIN);
}

/// Four two-link legs under a trunk; `hip_y` is the shoulder height.
fn quadruped(e: &Enemy, m: &mut Model, hip_y: f32, half_width: f32, half_length: f32, thickness: f32, color: [f32; 3]) {
    for (i, (side, front)) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].into_iter().enumerate() {
        let offset = if i == 0 || i == 3 { 0.0 } else { std::f32::consts::PI };
        let (distance, lift) = walk(e, 0.3, 0.14, offset, 1.0);
        let stride = travel(e, distance);
        let hip = p(e, side * half_width, hip_y, -front * half_length);
        let knee = hip + Vec3::new(0.0, -hip_y * 0.45 + lift * 0.4, front * 0.1) + stride * 0.5;
        let foot = p(e, side * half_width, 0.06 + lift, -front * half_length) + stride;
        beam(m, hip, knee, thickness, color);
        beam(m, knee, foot, thickness * 0.75, color);
        block(m, foot, Vec3::new(thickness * 1.3, 0.12, thickness * 1.6), DARK, BONE);
    }
}

/// Two legs with knees, for the walkers.
fn biped(e: &Enemy, m: &mut Model, hip_y: f32, half_width: f32, thickness: f32, color: [f32; 3], brace: f32) {
    for side in [-1.0f32, 1.0] {
        let offset = if side < 0.0 { 0.0 } else { std::f32::consts::PI };
        let (distance, lift) = walk(e, 0.26, 0.14, offset, 1.0 - brace);
        let stride = travel(e, distance);
        let hip = p(e, side * half_width, hip_y - brace * 0.1, 0.0);
        let knee = p(e, side * half_width, hip_y * 0.5 + lift * 0.4, -0.08) + stride * 0.5;
        let foot = p(e, side * half_width, 0.07 + lift, 0.0) + stride;
        beam(m, hip, knee, thickness, color);
        beam(m, knee, foot, thickness * 0.8, color);
        block(m, foot, Vec3::new(thickness * 1.4, 0.14, thickness * 2.2), DARK, BONE);
    }
}

fn eyes(m: &mut Model, head: Vec3, spacing: f32, front: f32, color: [f32; 3], energy: f32) {
    for side in [-1.0f32, 1.0] {
        glow(m, head + Vec3::new(side * spacing, 0.0, front), Vec3::new(0.07, 0.05, 0.04), color, energy);
    }
}

fn owlbear(e: &Enemy, m: &mut Model, wind: f32) {
    let fur = [0.33, 0.24, 0.16];
    let feather = [0.52, 0.42, 0.28];
    let release = strike(e);
    // Rears up through the wind-up and drops both forelimbs on the release.
    let rear = wind * 0.5;
    biped(e, m, 1.0, 0.34, 0.2, fur, wind);
    let chest = p(e, 0.0, 1.45 + rear * 0.3, -0.05 - release * 0.15);
    m.material_box(chest, Vec3::new(1.15, 1.1, 0.95), Quat::from_rotation_x(-rear * 0.6), fur, SKIN, 0.0);
    block(m, chest + Vec3::new(0.0, 0.35, -0.25), Vec3::new(0.95, 0.45, 0.5), feather, SKIN);
    for side in [-1.0f32, 1.0] {
        let shoulder = chest + Vec3::new(side * 0.6, 0.35, -0.1);
        let paw = shoulder + Vec3::new(side * 0.15, -0.85 + rear * 1.1 - release * 1.2, -0.45 - wind * 0.3);
        beam(m, shoulder, paw, 0.2, fur);
        block(m, paw, Vec3::new(0.3, 0.16, 0.34), DARK, BONE);
    }
    let head = chest + Vec3::new(0.0, 0.85 - rear * 0.1, -0.3);
    block(m, head, Vec3::new(0.62, 0.56, 0.56), feather, SKIN);
    block(m, head + Vec3::new(0.0, -0.06, -0.33), Vec3::new(0.14, 0.2, 0.18), [0.85, 0.72, 0.3], BONE);
    for side in [-1.0f32, 1.0] {
        block(m, head + Vec3::new(side * 0.26, 0.32, 0.0), Vec3::new(0.1, 0.18, 0.12), feather, SKIN);
    }
    eyes(m, head, 0.17, -0.28, [0.95, 0.75, 0.2], 0.35 + wind * 0.4);
}

fn gnoll(e: &Enemy, m: &mut Model, wind: f32) {
    let fur = [0.52, 0.42, 0.26];
    let mane = [0.24, 0.18, 0.12];
    let release = strike(e);
    biped(e, m, 0.95, 0.17, 0.11, fur, wind);
    let chest = p(e, 0.0, 1.3 - wind * 0.1, -wind * 0.18 - release * 0.1);
    m.material_box(chest, Vec3::new(0.56, 0.6, 0.34), Quat::from_rotation_x(-0.25 - wind * 0.3), fur, SKIN, 0.0);
    block(m, chest + Vec3::new(0.0, 0.3, 0.1), Vec3::new(0.2, 0.5, 0.2), mane, SKIN);
    let head = chest + Vec3::new(0.0, 0.5, -0.22 - wind * 0.1);
    block(m, head, Vec3::new(0.3, 0.3, 0.46), fur, SKIN);
    block(m, head + Vec3::new(0.0, -0.08, -0.3), Vec3::new(0.18, 0.16, 0.2), DARK, SKIN);
    for side in [-1.0f32, 1.0] {
        block(m, head + Vec3::new(side * 0.14, 0.22, 0.05), Vec3::new(0.08, 0.16, 0.06), fur, SKIN);
        // Spear arm leads the charge; the other arm trails.
        let shoulder = chest + Vec3::new(side * 0.32, 0.22, 0.0);
        let hand = shoulder + Vec3::new(side * 0.1, -0.35 + wind * 0.3 * side.max(0.0), -0.3 - release * 0.5 * side.max(0.0));
        beam(m, shoulder, hand, 0.09, fur);
        if side > 0.0 {
            beam(m, hand + Vec3::new(0.0, 0.0, 0.5), hand + Vec3::new(0.0, 0.0, -0.9), 0.04, [0.45, 0.35, 0.22]);
            block(m, hand + Vec3::new(0.0, 0.0, -0.98), Vec3::new(0.05, 0.05, 0.2), [0.6, 0.62, 0.64], BONE);
        }
    }
    eyes(m, head, 0.1, -0.22, [0.9, 0.5, 0.15], 0.3);
}

fn stirge(e: &Enemy, m: &mut Model, time: f32, wind: f32) {
    let hide = [0.42, 0.28, 0.3];
    let flap = (time * 14.0 + e.slot as f32).sin();
    let body = p(e, 0.0, 0.3, 0.0);
    block(m, body, Vec3::new(0.28, 0.24, 0.42), hide, SKIN);
    // Proboscis extends toward the release.
    let release = strike(e);
    beam(m, body + Vec3::new(0.0, -0.02, -0.2), body + Vec3::new(0.0, -0.12 - release * 0.1, -0.55 - wind * 0.2 - release * 0.3), 0.035, [0.75, 0.7, 0.62]);
    for side in [-1.0f32, 1.0] {
        let root = body + Vec3::new(side * 0.12, 0.08, 0.0);
        let tip = root + Vec3::new(side * 0.6, flap * 0.3, -0.1);
        m.material_box((root + tip) * 0.5, Vec3::new(0.6, 0.03, 0.4), Quat::from_rotation_z(side * flap * 0.45), [0.3, 0.2, 0.22], SKIN, 0.0);
        beam(m, body + Vec3::new(side * 0.08, -0.12, 0.1), body + Vec3::new(side * 0.1, -0.3, 0.14), 0.025, DARK);
    }
    eyes(m, body, 0.08, -0.2, [0.9, 0.2, 0.2], 0.4);
}

fn gelatinous_cube(e: &Enemy, m: &mut Model, time: f32, wind: f32) {
    let jelly = [0.62, 0.78, 0.72];
    let wobble = (time * 2.0 + e.slot as f32).sin() * 0.03;
    let size = 1.7 + wind * 0.1;
    m.material_box(p(e, 0.0, size * 0.5, 0.0), Vec3::new(size + wobble, size - wobble, size + wobble), Quat::IDENTITY, jelly, 62.0, 0.08 + wind * 0.2);
    // Things it has eaten, suspended inside.
    for (i, (x, y, z)) in [(-0.4, 0.5, 0.2), (0.35, 1.0, -0.3), (0.1, 1.3, 0.4)].into_iter().enumerate() {
        let drift = (time * 0.7 + i as f32).sin() * 0.04;
        block(m, p(e, x, y + drift, z), Vec3::splat(0.14 + i as f32 * 0.04), if i == 1 { [0.8, 0.78, 0.7] } else { [0.5, 0.42, 0.3] }, BONE);
    }
}

fn hell_hound(e: &Enemy, m: &mut Model, time: f32, wind: f32) {
    let coat = [0.16, 0.1, 0.09];
    let ember = [1.0, 0.45, 0.12];
    let release = strike(e);
    quadruped(e, m, 0.78, 0.2, 0.42, 0.1, coat);
    let trunk = p(e, 0.0, 0.92, 0.0);
    m.material_box(trunk, Vec3::new(0.46, 0.44, 1.0), Quat::from_rotation_x(-wind * 0.2), coat, SKIN, 0.0);
    let head = trunk + Vec3::new(0.0, 0.3 - wind * 0.15, -0.62 - release * 0.2);
    block(m, head, Vec3::new(0.3, 0.3, 0.5), coat, SKIN);
    block(m, head + Vec3::new(0.0, -0.1, -0.32), Vec3::new(0.2, 0.12 + release * 0.12, 0.24), DARK, SKIN);
    // Flames lick along the spine; brighter as the bite comes.
    let pulse = 0.6 + (time * 9.0).sin() * 0.2 + wind * 0.8;
    for i in 0..4 {
        let z = 0.4 - i as f32 * 0.26;
        glow(m, trunk + Vec3::new(0.0, 0.26 + (time * 7.0 + i as f32).sin() * 0.05, z), Vec3::new(0.12, 0.2, 0.14), ember, pulse);
    }
    glow(m, head + Vec3::new(0.0, -0.02, -0.38), Vec3::new(0.14, 0.05, 0.08), ember, pulse * 1.4);
    eyes(m, head, 0.1, -0.24, ember, 1.0);
    for side in [-1.0f32, 1.0] {
        block(m, head + Vec3::new(side * 0.12, 0.2, 0.1), Vec3::new(0.07, 0.14, 0.06), coat, SKIN);
    }
}

fn yeti(e: &Enemy, m: &mut Model, wind: f32) {
    let fur = [0.88, 0.9, 0.92];
    let skin = [0.35, 0.4, 0.48];
    let release = strike(e);
    biped(e, m, 1.2, 0.3, 0.22, fur, wind);
    let chest = p(e, 0.0, 1.9 - wind * 0.1, -wind * 0.12 - release * 0.1);
    m.material_box(chest, Vec3::new(1.3, 1.2, 0.8), Quat::from_rotation_x(-wind * 0.25), fur, SKIN, 0.0);
    block(m, chest + Vec3::new(0.0, -0.1, -0.35), Vec3::new(0.7, 0.7, 0.16), skin, SKIN);
    for side in [-1.0f32, 1.0] {
        let shoulder = chest + Vec3::new(side * 0.72, 0.4, 0.0);
        // Both arms rise together and slam on the release.
        let hand = shoulder + Vec3::new(side * 0.1, -1.0 + wind * 1.3 - release * 1.5, -0.3 - release * 0.4);
        beam(m, shoulder, hand, 0.24, fur);
        block(m, hand, Vec3::new(0.34, 0.2, 0.34), skin, SKIN);
    }
    let head = chest + Vec3::new(0.0, 0.8, -0.1);
    block(m, head, Vec3::new(0.6, 0.6, 0.6), fur, SKIN);
    block(m, head + Vec3::new(0.0, -0.08, -0.3), Vec3::new(0.4, 0.34, 0.1), skin, SKIN);
    eyes(m, head, 0.12, -0.36, [0.7, 0.9, 1.0], 0.3 + wind * 0.5);
}

pub(super) fn body(e: &Enemy, m: &mut Model, time: f32, wind: f32) {
    match e.kind {
        OWLBEAR => owlbear(e, m, wind),
        GNOLL => gnoll(e, m, wind),
        STIRGE => stirge(e, m, time, wind),
        GELATINOUS_CUBE => gelatinous_cube(e, m, time, wind),
        HELL_HOUND => hell_hound(e, m, time, wind),
        YETI => yeti(e, m, wind),
        _ => {}
    }
}
