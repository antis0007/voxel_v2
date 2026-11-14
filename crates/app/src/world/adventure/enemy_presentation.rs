//! Common hostiles: weight, committed weapon poses and attached material pixels.
//! These poses never change collision, attack reach, damage or AI decisions.
use super::Enemy;
use crate::mesh::models::Model;
use glam::{Quat, Vec3};

const STEEL: [f32; 3] = [0.24, 0.29, 0.32];
const DARK: [f32; 3] = [0.10, 0.13, 0.15];
const EDGE: [f32; 3] = [0.48, 0.51, 0.49];
const CLOTH: [f32; 3] = [0.16, 0.12, 0.15];
pub(super) const RECOVERY_S: f32 = 0.42;

/// Simulation-owned impact direction and settling clock. A flash edge starts
/// one reaction; status damage cannot keep restarting it every frame.
#[derive(Default)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub(super) struct Reaction {
    age: f32,
    strength: f32,
    direction: Vec3,
    last_flash: f32,
}
impl Reaction {
    pub(super) fn advance(&mut self, flash: f32, knock: Vec3, yaw: f32, dt: f32) {
        self.age += dt;
        if flash > self.last_flash + 0.025 {
            self.age = 0.0;
            self.strength = (0.35 + knock.length() * 0.14).clamp(0.35, 1.0);
            self.direction = if knock.length_squared() > 0.01 {
                (knock * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero()
            } else { Quat::from_rotation_y(yaw) * Vec3::Z };
        }
        self.last_flash = (flash - dt).max(0.0);
    }
    fn rotation(&self, yaw: f32) -> Quat {
        let t = (self.age / 0.46).clamp(0.0, 1.0);
        // Immediate impact, a small counter-motion, then a quiet settle.
        let pulse = ((t * 1.6 + 0.15) * std::f32::consts::PI).sin()
            * (1.0 - t).powi(2) * self.strength;
        let local = Quat::from_rotation_y(-yaw) * self.direction;
        Quat::from_rotation_x(local.z * pulse * 0.24)
            * Quat::from_rotation_z(-local.x * pulse * 0.22)
    }
}
fn at(e: &Enemy, x: f32, y: f32, z: f32) -> Vec3 {
    e.pos + Vec3::new(x, y, z)
}
fn plate(m: &mut Model, c: Vec3, size: Vec3, rot: Quat, color: [f32; 3]) {
    let kind=if color==CLOTH {72.0} else if color==STEEL || color==DARK || color==EDGE {59.0} else {70.0};
    m.material_box(c, size, rot, color, kind, 0.0);
}
fn rune(m: &mut Model, c: Vec3, size: Vec3, rot: Quat, energy: f32) {
    m.material_box(c, size, rot, [0.32, 0.20, 0.28], 60.0, energy);
}
fn bone(m: &mut Model, a: Vec3, b: Vec3, width: f32, color: [f32; 3]) {
    let delta = b - a;
    plate(
        m,
        (a + b) * 0.5,
        Vec3::new(width, delta.length(), width),
        Quat::from_rotation_arc(Vec3::Y, delta.normalize_or_zero()),
        color,
    );
}
pub(super) fn strike(e: &Enemy) -> f32 {
    if e.tell > 0.0 {
        // The weapon crosses its arc before contact, rather than starting the
        // swing after damage was already delivered by the simulation.
        ease((0.085 - e.tell) / 0.085)
    } else if e.recovery <= 0.0 {
        0.0
    } else {
        let t = (1.0 - e.recovery / RECOVERY_S).clamp(0.0, 1.0);
        1.0 - ease(t)
    }
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[path = "enemy_presentation/locomotion.rs"]
mod locomotion;
#[cfg(feature = "multiplayer")]
#[path = "enemy_presentation/replica.rs"]
mod replica;
#[cfg(feature = "multiplayer")]
pub(super) use replica::ReplicaPose;

#[cfg(feature = "multiplayer")]
pub(super) fn replica_pose(e: &Enemy) -> ReplicaPose {
    ReplicaPose { pos: e.pos, yaw: e.yaw, phase: e.phase, gait: e.gait }
}

#[cfg(feature = "multiplayer")]
pub(super) fn apply_replica_pose(e: &mut Enemy, pose: ReplicaPose) {
    e.pos = pose.pos;
    e.yaw = pose.yaw;
    e.phase = pose.phase;
    e.gait = pose.gait;
}

#[cfg(feature = "multiplayer")]
pub(super) fn advance_replica_pose(e: &mut Enemy, dt: f32) {
    e.reaction.advance(e.flash, e.knock, e.yaw, dt);
    e.flash = (e.flash - dt).max(0.0);
    e.slow = (e.slow - dt).max(0.0);
    // Boss action state remains authoritative; ordinary tells have a known release.
    if !super::is_boss(e.kind) && !super::vault_fight::lead(e)
        && e.kind != super::mobs::CREEPER && !super::wildlife::vehicle::hostile_campaign(e)
    {
        (e.tell, e.recovery) = replica::advance_attack(e.tell, e.recovery, dt, RECOVERY_S);
    }
}
pub(super) use locomotion::{two_bone, hoof_step};

/// Distance in metres sets cadence, with 60% of a cycle on the ground.
pub(super) fn walk(e: &Enemy, stride: f32, height: f32, offset: f32, weight: f32) -> (f32, f32) {
    walk_scaled(e, stride, height, offset, weight, 1.0)
}

/// Fixed-size rigs keep stance travel in world metres.
pub(super) fn walk_scaled(e: &Enemy, stride: f32, height: f32, offset: f32, weight: f32, scale: f32) -> (f32, f32) {
    locomotion::distance_step(e.phase / scale, stride, height, offset, e.gait * weight)
}

/// Ground travel advances the pose; falling and stepping up do not.
pub(super) fn advance_walk(e: &mut Enemy, movement: Vec3, speed: f32, dt: f32) {
    let up = if e.up.length_squared() > 0.5 { e.up.normalize() } else { Vec3::Y };
    let tangent = locomotion::ground_motion(movement, up);
    e.phase += tangent.length();
    e.gait = locomotion::gait_weight(e.gait, tangent.length(), speed, dt);
}

/// Backpedalling and strafing plant feet opposite the measured movement.
pub(super) fn travel(e: &Enemy, distance: f32) -> Vec3 {
    let moving = e.velocity * Vec3::new(1.0, 0.0, 1.0);
    if moving.length_squared() < 0.001 { Vec3::Z * distance }
    else { -(Quat::from_rotation_y(-e.yaw) * moving.normalize()) * distance }
}

fn pose_frame(e: &Enemy, time: f32, wind: f32) -> (Vec3, Quat) {
    let moving = e.gait.clamp(0.0, 1.0);
    let breath = (time * 1.8 + e.slot as f32 * 2.4).sin() * 0.007 * (1.0 - moving);
    let roll = (e.phase * 4.2).sin() * moving * 0.018 * (1.0 - wind);
    let up = if e.up.length_squared() > 0.5 { e.up.normalize() } else { Vec3::Y };
    let upright = Quat::from_rotation_arc(Vec3::Y, up);
    (upright * Vec3::Y * breath, upright * Quat::from_rotation_y(e.yaw)
        * Quat::from_rotation_z(roll)
        * Quat::from_rotation_x(-wind * 0.07)
        * e.reaction.rotation(e.yaw))
}

/// Attack emitters and rendered parts share the same rigid body transform.
pub(super) fn posed_point(e: &Enemy, point: Vec3, time: f32, wind: f32) -> Vec3 {
    let (breath, rotation) = pose_frame(e, time, wind);
    e.pos + rotation * (point + breath - e.pos)
}

pub(super) fn wind(e: &Enemy) -> f32 {
    if e.tell <= 0.0 { return 0.0; }
    let t = (1.0 - e.tell / e.windup()).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t) * (1.0 - strike(e))
}

pub(super) fn pose(e: &Enemy, m: &mut Model, mark: usize, time: f32, wind: f32) {
    let (breath, rotation) = pose_frame(e, time, wind);
    let breath_blocks = breath.y / m.scale_meters();
    for v in &mut m.vertices[mark..] { v.pos[1] += breath_blocks; }
    m.rotate_since(mark, e.pos, rotation);
    status_tint(e, m, mark);
}

fn status_tint(e: &Enemy, m: &mut Model, mark: usize) {
    let (tint, amount) = if e.flash > 0.0 {
        ([0.92, 0.85, 0.76], (e.flash / 0.18).clamp(0.0, 1.0) * 0.50)
    } else if e.slow > 0.0 {
        ([0.39, 0.65, 0.77], 0.28)
    } else {
        return;
    };
    for v in &mut m.vertices[mark..] {
        for i in 0..3 {
            v.color[i] = v.color[i] * (1.0 - amount) + tint[i] * amount;
        }
    }
}

pub(super) fn warden(e: &Enemy, m: &mut Model, _time: f32, wind: f32, _hit: [f32; 3]) {
    let strike = strike(e);
    let brace = wind.max(0.0).powi(2);
    let sink = brace * 0.09 + strike * 0.04;
    for (side, phase) in [(-1.0_f32, 0.0), (1.0, std::f32::consts::PI)] {
        let (step, lift) = walk(e, 0.32, 0.13, phase, 1.0 - brace);
        let hip = at(e, side * 0.17, 0.66 - sink, 0.0);
        let knee = at(e, side * 0.19, 0.34 - sink * 0.4, -step * 0.45 - 0.025);
        let foot = at(e, side * 0.20, 0.075 + lift, step - 0.03);
        bone(m, hip, knee, 0.15, STEEL);
        bone(m, knee, foot, 0.12, DARK);
        plate(m, foot, Vec3::new(0.23, 0.15, 0.34), Quat::IDENTITY, STEEL);
        plate(
            m,
            knee + Vec3::new(0.0, 0.0, -0.045),
            Vec3::new(0.20, 0.16, 0.12),
            Quat::from_rotation_x(-0.12),
            EDGE,
        );
        plate(
            m,
            at(e, side * 0.16, 0.65 - sink, 0.0),
            Vec3::new(0.25, 0.23, 0.27),
            Quat::from_rotation_z(side * 0.12),
            CLOTH,
        );
    }
    let torso = at(e, 0.0, 1.04 - sink, 0.0);
    let lean = Quat::from_rotation_x(-brace * 0.10 + strike * 0.16);
    plate(m, torso, Vec3::new(0.54, 0.57, 0.35), lean, STEEL);
    plate(
        m,
        torso + Vec3::new(0.0, 0.07, -0.17),
        Vec3::new(0.40, 0.35, 0.10),
        lean,
        EDGE,
    );
    rune(
        m,
        torso + Vec3::new(0.0, 0.05, -0.23),
        Vec3::new(0.13, 0.27, 0.035),
        lean,
        brace,
    );
    for side in [-1.0_f32, 1.0] {
        plate(
            m,
            at(e, side * 0.32, 1.29 - sink, 0.0),
            Vec3::new(0.28, 0.22, 0.38),
            Quat::from_rotation_z(side * 0.20),
            DARK,
        );
        plate(
            m,
            at(e, side * 0.34, 1.35 - sink, -0.015),
            Vec3::new(0.30, 0.10, 0.38),
            Quat::from_rotation_z(side * 0.20),
            STEEL,
        );
    }
    let head = at(e, 0.0, 1.52 - sink, -0.015 - strike * 0.05);
    plate(
        m,
        head,
        Vec3::new(0.35, 0.35, 0.34),
        Quat::from_rotation_x(brace * 0.10),
        DARK,
    );
    plate(
        m,
        head + Vec3::new(0.0, 0.08, -0.15),
        Vec3::new(0.39, 0.10, 0.12),
        Quat::IDENTITY,
        STEEL,
    );
    plate(
        m,
        head + Vec3::new(0.0, -0.08, -0.14),
        Vec3::new(0.27, 0.14, 0.13),
        Quat::IDENTITY,
        STEEL,
    );
    rune(
        m,
        head + Vec3::new(0.0, 0.015, -0.185),
        Vec3::new(0.25, 0.045, 0.035),
        Quat::IDENTITY,
        0.20 + brace * 0.8,
    );
    // A physical guard and broad, tapered blade replace the luminous thin rod.
    let shoulder = at(e, 0.33, 1.25 - sink, 0.0);
    let elbow = at(
        e,
        0.48,
        1.04 + brace * 0.27 - sink,
        0.06 + brace * 0.20 - strike * 0.21,
    );
    let hand = at(
        e,
        0.42,
        0.96 + brace * 0.48 - sink,
        -0.18 + brace * 0.30 - strike * 0.52,
    );
    bone(m, shoulder, elbow, 0.13, DARK);
    bone(m, elbow, hand, 0.11, STEEL);
    plate(m, hand, Vec3::splat(0.14), Quat::IDENTITY, DARK);
    let weapon = Quat::from_rotation_x(-0.25 + brace * 1.60 - strike * 0.75);
    let mark = m.mark();
    plate(
        m,
        hand + Vec3::new(0.0, 0.0, -0.11),
        Vec3::new(0.28, 0.065, 0.065),
        Quat::IDENTITY,
        EDGE,
    );
    plate(
        m,
        hand + Vec3::new(0.0, 0.0, -0.43),
        Vec3::new(0.13, 0.055, 0.59),
        Quat::IDENTITY,
        STEEL,
    );
    plate(
        m,
        hand + Vec3::new(0.0, 0.0, -0.77),
        Vec3::new(0.07, 0.045, 0.11),
        Quat::IDENTITY,
        EDGE,
    );
    rune(
        m,
        hand + Vec3::new(0.0, -0.034, -0.43),
        Vec3::new(0.045, 0.025, 0.49),
        Quat::IDENTITY,
        brace.max(strike),
    );
    m.rotate_since(mark, hand, weapon);
    let shield_hand = at(e, -0.42, 1.02 - sink, -0.13 - brace * 0.10);
    bone(m, at(e, -0.32, 1.24 - sink, 0.0), shield_hand, 0.12, DARK);
    if e.guard {
        // Shield faces the player; the former thin side face hid most of it.
        let c = shield_hand + Vec3::new(-0.06, 0.0, -0.14);
        let rot = Quat::from_rotation_y(-0.12) * Quat::from_rotation_x(-brace * 0.12);
        plate(m, c, Vec3::new(0.49, 0.71, 0.12), rot, DARK);
        plate(
            m,
            c + Vec3::new(0.0, 0.0, -0.07),
            Vec3::new(0.40, 0.61, 0.045),
            rot,
            STEEL,
        );
        plate(
            m,
            c + Vec3::new(0.0, -0.34, 0.0),
            Vec3::new(0.32, 0.12, 0.12),
            rot,
            DARK,
        );
        rune(
            m,
            c + Vec3::new(0.0, 0.0, -0.10),
            Vec3::new(0.075, 0.46, 0.025),
            rot,
            0.1 + brace * 0.65,
        );
    } else {
        plate(
            m,
            shield_hand,
            Vec3::new(0.14, 0.22, 0.13),
            Quat::IDENTITY,
            CLOTH,
        );
    }
}

pub(super) fn fallback(e: &Enemy, m: &mut Model, time: f32, wind: f32, hit: [f32; 3]) {
    let wind = wind.max(0.0);
    if e.kind == 3 {
        let dip = wind * 0.10;
        plate(
            m,
            at(e, 0.0, 0.35 - dip, 0.02),
            Vec3::new(0.62, 0.37, 0.69),
            Quat::IDENTITY,
            [0.18, 0.13, 0.12],
        );
        for side in [-1.0_f32, 1.0] {
            plate(
                m,
                at(e, side * 0.17, 0.56 - dip, 0.02),
                Vec3::new(0.27, 0.12, 0.64),
                Quat::from_rotation_z(side * 0.16),
                DARK,
            );
            for i in 0..3 {
                let z = i as f32 * 0.23 - 0.24;
                let phase = i as f32 * 2.1
                    + if side > 0.0 {
                        std::f32::consts::PI
                    } else {
                        0.0
                    };
                let hip = at(e, side * 0.25, 0.30 - dip, z);
                let knee = at(e, side * 0.43, 0.20, z + 0.07);
                let (travel, lift) = walk(e, 0.11, 0.075, phase, 1.0 - wind);
                let foot = at(
                    e,
                    side * 0.53,
                    0.035 + lift,
                    z + travel,
                );
                bone(m, hip, knee, 0.055, STEEL);
                bone(m, knee, foot, 0.035, DARK);
            }
            let jaw = at(e, side * (0.19 + wind * 0.06), 0.25 - dip, -0.46);
            plate(
                m,
                jaw,
                Vec3::new(0.085, 0.10, 0.30),
                Quat::from_rotation_y(-side * (0.35 + wind * 0.50)),
                EDGE,
            );
            rune(
                m,
                at(e, side * 0.12, 0.40 - dip, -0.355),
                Vec3::new(0.10, 0.045, 0.03),
                Quat::IDENTITY,
                0.12 + wind * 0.88,
            );
        }
    } else if e.kind == 9 {
        let strike = strike(e);
        for (side, z, phase) in [
            (-1.0_f32, -0.28, 0.0),
            (1.0, -0.28, std::f32::consts::PI),
            (-1.0, 0.30, std::f32::consts::PI),
            (1.0, 0.30, 0.0),
        ] {
            let (travel, lift) = walk(e, 0.30, 0.16, phase, 1.0 - wind);
            let foot = at(e, side * 0.40, 0.12 + lift, z + travel);
            let knee = at(e, side * 0.44, 0.48 + lift * 0.5, z - travel * 0.35 - 0.08);
            bone(m, at(e, side * 0.37, 0.92, z), knee, 0.24, DARK);
            bone(m, knee, foot, 0.20, STEEL);
            plate(m, foot, Vec3::new(0.31, 0.24, 0.40), Quat::IDENTITY, STEEL);
        }
        plate(
            m,
            at(e, 0.0, 1.12 - wind * 0.12, 0.02),
            Vec3::new(1.02, 0.73, 0.79),
            Quat::from_rotation_x(strike * 0.12),
            [0.28, 0.25, 0.22],
        );
        for x in [-0.37, 0.0, 0.37] {
            plate(
                m,
                at(e, x, 1.48 - wind * 0.12, 0.02),
                Vec3::new(0.31, 0.18, 0.72),
                Quat::from_rotation_z(-x * 0.18),
                DARK,
            );
        }
        plate(
            m,
            at(e, 0.0, 1.78 - wind * 0.10, -0.23),
            Vec3::new(0.64, 0.42, 0.44),
            Quat::from_rotation_x(wind * 0.15),
            STEEL,
        );
        plate(
            m,
            at(e, 0.0, 1.63 - wind * 0.10, -0.43),
            Vec3::new(0.57, 0.14, 0.22),
            Quat::IDENTITY,
            DARK,
        );
        rune(
            m,
            at(e, 0.0, 1.79 - wind * 0.10, -0.465),
            Vec3::new(0.45, 0.06, 0.035),
            Quat::IDENTITY,
            0.15 + wind * 0.85,
        );
        for side in [-1.0_f32, 1.0] {
            let elbow = at(e, side * 0.69, 1.05 + wind * 0.32, 0.02);
            let fist = at(
                e,
                side * 0.77,
                0.54 + wind * 0.70 - strike * 0.5,
                -0.28 - strike * 0.20,
            );
            bone(m, at(e, side * 0.52, 1.30, 0.0), elbow, 0.21, DARK);
            bone(m, elbow, fist, 0.19, STEEL);
            plate(
                m,
                fist,
                Vec3::new(0.34, 0.32, 0.39),
                Quat::from_rotation_x(-wind * 0.40 + strike),
                DARK,
            );
            rune(
                m,
                fist + Vec3::new(0.0, 0.10, -0.20),
                Vec3::new(0.23, 0.05, 0.035),
                Quat::IDENTITY,
                wind.max(strike),
            );
        }
    } else {
        // A caged, angular wisp with an eye seam rather than a cheerful orb.
        let c = e.center() + Vec3::Y * (time * 1.6 + e.slot as f32).sin() * 0.045;
        for side in [-1.0_f32, 1.0] {
            plate(
                m,
                c + Vec3::new(side * 0.19, 0.02, 0.0),
                Vec3::new(0.11, 0.48, 0.29),
                Quat::from_rotation_z(side * (0.12 + wind * 0.20)),
                DARK,
            );
            plate(
                m,
                c + Vec3::new(side * 0.26, -0.07, 0.02),
                Vec3::new(0.075, 0.39, 0.11),
                Quat::from_rotation_z(-side * 0.55),
                STEEL,
            );
        }
        rune(
            m,
            c,
            Vec3::new(0.28, 0.29, 0.24),
            Quat::from_rotation_y((time * 0.25).sin() * 0.15),
            0.20 + wind * 0.8,
        );
        m.glow_voxel(
            c + Vec3::new(0.0, 0.04, -0.14),
            Vec3::new(0.20, 0.045, 0.03),
            Quat::IDENTITY,
            hit,
            0.9,
            0.95,
        );
        for i in 0..3 {
            let y = -0.28 - i as f32 * 0.13;
            plate(
                m,
                c + Vec3::new((time * 1.3 + i as f32).sin() * 0.025, y, 0.03),
                Vec3::new(0.09 - i as f32 * 0.02, 0.10, 0.07),
                Quat::IDENTITY,
                DARK,
            );
        }
    }
}

pub(super) fn threat(e: &Enemy, m: &mut Model, wind: f32) {
    if e.tell <= 0.0 || wind < 0.4 || !super::brawler(e.kind) {
        return;
    }
    let forward = e.lunge.normalize_or_zero();
    if forward.length_squared() < 0.5 { return; }
    let right = Vec3::new(-forward.z, 0.0, forward.x);
    let base = e.pos + Vec3::Y * 0.055;
    let color = if wind > 0.78 {
        [0.98, 0.53, 0.26]
    } else {
        [0.73, 0.19, 0.25]
    };
    // A local direction chevron, not a fabricated ground damage radius.
    for side in [-1.0_f32, 1.0] {
        let tip = base + forward * (0.65 + wind * 0.15);
        let tail = base + forward * 0.35 + right * side * 0.24;
        m.beam(tail, tip, 0.025, color, 0.65 + wind * 0.45);
    }
}
