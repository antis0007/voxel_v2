//! Anchor dock machines still walking their rounds: a tall sentry frame with a
//! searchlight head and a shoulder bolt-thrower, and a low four-legged hound.
//! Plates use the enemy-metal material (59); eyes and vents glow.
use super::Enemy;
use crate::mesh::models::Model;
use glam::{Quat, Vec3};

pub const SENTRY: u32 = 16;
pub const HOUND: u32 = 17;

const PLATE: [f32; 3] = [0.30, 0.33, 0.34];
const DARK: [f32; 3] = [0.09, 0.11, 0.12];
const RUST: [f32; 3] = [0.40, 0.21, 0.11];
const PAINT: [f32; 3] = [0.72, 0.52, 0.12];
const EYE: [f32; 3] = [1.0, 0.18, 0.10];

fn at(e: &Enemy, x: f32, y: f32, z: f32) -> Vec3 {
    e.pos + Vec3::new(x, y, z)
}

fn plate(m: &mut Model, c: Vec3, size: Vec3, rot: Quat, color: [f32; 3]) {
    m.material_box(c, size, rot, color, 59.0, 0.0);
}

fn strut(m: &mut Model, a: Vec3, b: Vec3, width: f32, color: [f32; 3]) {
    let d = b - a;
    plate(m, (a + b) * 0.5, Vec3::new(width, d.length(), width), Quat::from_rotation_arc(Vec3::Y, d.normalize_or_zero()), color);
}

/// Reverse-knee leg: hip, knee, ankle, then a flat foot.
fn leg(m: &mut Model, hip: Vec3, knee: Vec3, foot: Vec3, width: f32) {
    strut(m, hip, knee, width, PLATE);
    strut(m, knee, foot, width * 0.8, DARK);
    plate(m, knee, Vec3::splat(width * 1.7), Quat::IDENTITY, RUST);
    plate(m, foot + Vec3::new(0.0, width * 0.4, 0.04), Vec3::new(width * 1.6, width * 0.8, width * 2.4), Quat::IDENTITY, DARK);
}

pub(super) fn sentry(e: &Enemy, m: &mut Model, time: f32, wind: f32, hit: [f32; 3]) {
    let crouch = wind * 0.12;
    let bob = (e.phase * 2.0).sin() * 0.03 * e.gait;
    let hip_y = 1.10 - crouch + bob;
    for (x, phase) in [(-0.26, 0.0), (0.26, std::f32::consts::PI)] {
        let (swing, lift) = super::enemy_presentation::walk(e, 0.42, 0.18, phase, 1.0 - wind);
        let hip = at(e, x, hip_y, 0.0);
        let knee = at(e, x * 1.15, hip_y * 0.55, -0.26 + swing * 0.5);
        let foot = at(e, x * 1.2, lift, swing);
        leg(m, hip, knee, foot, 0.10);
    }
    // Pelvis, torso and a chest plate with a worn stripe.
    plate(m, at(e, 0.0, hip_y, 0.0), Vec3::new(0.56, 0.22, 0.30), Quat::IDENTITY, DARK);
    let lean = Quat::from_rotation_x(0.08 + wind * 0.22);
    plate(m, at(e, 0.0, hip_y + 0.46, 0.02), Vec3::new(0.62, 0.62, 0.40), lean, PLATE);
    plate(m, at(e, 0.0, hip_y + 0.40, -0.22), Vec3::new(0.46, 0.30, 0.04), lean, PAINT);
    plate(m, at(e, 0.0, hip_y + 0.40, -0.23), Vec3::new(0.46, 0.06, 0.04), lean, DARK);
    // Searchlight head: a lamp housing that sweeps, then locks on the target.
    let sweep = if e.tell > 0.0 || e.visible { 0.0 } else { (time * 0.9).sin() * 0.6 };
    let head = at(e, 0.0, hip_y + 0.92, -0.04);
    let look = Quat::from_rotation_y(sweep) * Quat::from_rotation_x(wind * 0.25);
    plate(m, head, Vec3::new(0.38, 0.26, 0.34), look, DARK);
    plate(m, head + look * Vec3::new(0.0, 0.0, -0.18), Vec3::new(0.30, 0.18, 0.04), look, RUST);
    let glow = 1.6 + wind * 2.4;
    m.orb(head + look * Vec3::new(0.0, 0.0, -0.21), 0.07, if e.flash > 0.0 { hit } else { EYE }, glow);
    m.glow_orb(head + look * Vec3::new(0.0, 0.0, -0.26), 0.12 + wind * 0.08, EYE, 0.9 + wind, 0.35);
    strut(m, head + Vec3::new(0.14, 0.13, 0.08), head + Vec3::new(0.18, 0.55, 0.10), 0.02, DARK);
    // Shoulder bolt-thrower on the right; the barrel drops level during windup.
    let shoulder = at(e, 0.44, hip_y + 0.74, 0.02);
    let aim = Quat::from_rotation_x(-0.35 + wind * 0.35);
    plate(m, shoulder, Vec3::new(0.18, 0.22, 0.26), Quat::IDENTITY, PLATE);
    plate(m, shoulder + aim * Vec3::new(0.0, 0.0, -0.30), Vec3::new(0.10, 0.10, 0.50), aim, DARK);
    m.orb(shoulder + aim * Vec3::new(0.0, 0.0, -0.56), 0.035, EYE, glow * wind);
    // Left arm: a grapple hook at rest.
    let hand = at(e, -0.46, hip_y + 0.18 + wind * 0.2, -0.10 - wind * 0.2);
    strut(m, at(e, -0.40, hip_y + 0.74, 0.02), hand, 0.07, PLATE);
    plate(m, hand, Vec3::new(0.14, 0.18, 0.14), Quat::from_rotation_z(0.4), RUST);
    // Vent glow on the back, brighter as the frame works.
    for i in 0..3 {
        let pulse = 0.4 + 0.6 * ((time * 2.4 + i as f32 * 0.9).sin() * 0.5 + 0.5);
        m.cuboid(at(e, -0.16 + i as f32 * 0.16, hip_y + 0.30, 0.23), Vec3::new(0.06, 0.16, 0.02), Quat::IDENTITY, [1.0, 0.45, 0.12], pulse * (1.0 + e.gait));
    }
}

pub(super) fn hound(e: &Enemy, m: &mut Model, time: f32, wind: f32, hit: [f32; 3]) {
    let crouch = wind * 0.14;
    let body_y = 0.52 - crouch;
    for (i, (x, z)) in [(-0.20, 0.30), (0.20, 0.30), (-0.20, -0.28), (0.20, -0.28)].into_iter().enumerate() {
        // Diagonal pairs move together, like a trotting dog.
        let phase = if i == 0 || i == 3 { 0.0 } else { std::f32::consts::PI };
        let (swing, lift) = super::enemy_presentation::walk(e, 0.30, 0.16, phase, 1.0 - wind);
        let hip = at(e, x, body_y - 0.05, z);
        let knee = at(e, x * 1.3, body_y * 0.5, z + if z > 0.0 { 0.12 } else { -0.12 } + swing * 0.4);
        let foot = at(e, x * 1.3, lift, z + swing);
        leg(m, hip, knee, foot, 0.06);
    }
    let pitch = Quat::from_rotation_x(wind * -0.3);
    plate(m, at(e, 0.0, body_y, 0.0), Vec3::new(0.46, 0.30, 0.92), pitch, PLATE);
    plate(m, at(e, 0.0, body_y + 0.17, 0.05), Vec3::new(0.32, 0.06, 0.60), pitch, RUST);
    plate(m, at(e, 0.0, body_y - 0.12, 0.0), Vec3::new(0.34, 0.10, 0.70), pitch, DARK);
    // Head: a wedge with twin eyes and a jaw that opens on the lunge.
    let head = at(e, 0.0, body_y + 0.08, -0.60);
    plate(m, head, Vec3::new(0.30, 0.22, 0.36), pitch, DARK);
    plate(m, head + Vec3::new(0.0, -0.14, -0.06), Vec3::new(0.26, 0.06, 0.30), Quat::from_rotation_x(wind * 0.7), RUST);
    for side in [-1.0, 1.0] {
        m.orb(head + Vec3::new(side * 0.09, 0.04, -0.19), 0.035, if e.flash > 0.0 { hit } else { EYE }, 1.8 + wind * 2.0);
    }
    // Whip antenna tail.
    let wag = (time * 6.0).sin() * 0.12 * e.gait;
    strut(m, at(e, 0.0, body_y + 0.12, 0.46), at(e, wag, body_y + 0.48, 0.70), 0.025, DARK);
    m.orb(at(e, wag, body_y + 0.50, 0.71), 0.03, EYE, 0.8);
}

