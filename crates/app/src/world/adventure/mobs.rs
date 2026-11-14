//! Night stalkers: the creeper that closes in silently and bursts, and the
//! skeleton archer that burns by day. Stats are bestiary rows; this module owns
//! the fuse, the blast and both bodies.
use super::{Enemy, Sound, World, line, sound};
use crate::mesh::models::Model;
use engine_audio::Cue;
use glam::{Quat, Vec3};

pub(super) const CREEPER: u32 = 18;
pub(super) const SKELETON: u32 = 19;
/// Seconds from the first hiss to the blast.
pub(super) const FUSE_S: f32 = 1.5;
/// The fuse starts inside this flat distance and is dropped beyond `ABORT_M`.
pub(super) const FUSE_M: f32 = 3.0;
pub(super) const ABORT_M: f32 = 6.5;
pub(super) const BLAST_M: f32 = 4.0;
pub(super) const BLAST_DAMAGE: f32 = 46.0;
pub(super) const CRATER_M: f32 = 2.2;
/// Cells one blast may remove; bounds the burst inside a frame.
const CRATER_CELLS: usize = 160;
/// Health lost per spawn pass by a body standing in daylight.
pub(super) const DAY_BURN: f32 = 7.0;
/// Fuse marker in `Enemy::attack`, distinct from the shot and melee codes.
pub(super) const ATTACK_FUSE: u32 = 4;
pub(super) const ATTACK_SPENT: u32 = 5;

/// Where a creeper stands to stay out of view: two metres behind the player.
pub(super) fn stalk_goal(feet: Vec3, forward: Vec3) -> Vec3 {
    let back = Vec3::new(-forward.x, 0.0, -forward.z).normalize_or_zero();
    feet + back * 2.0
}

/// Runs the fuse. Returns the blast centre on the tick the body bursts; the
/// body is dead afterwards. The fuse ignores whether the player has seen it.
pub(super) fn fuse(
    e: &mut Enemy,
    world: &World,
    feet: Vec3,
    eye: Vec3,
    dt: f32,
    sounds: &mut Vec<Sound>,
) -> Option<Vec3> {
    let delta = e.pos - feet;
    let distance = Vec3::new(delta.x, 0.0, delta.z).length();
    if e.tell > 0.0 {
        if distance > ABORT_M || !line(world, e.center(), eye) {
            e.tell = 0.0;
            e.attack = 0;
            e.cooldown = 0.6;
            return None;
        }
        e.tell -= dt;
        if e.tell <= 0.0 {
            e.tell = 0.0;
            e.health = 0.0;
            e.attack = ATTACK_SPENT;
            return Some(e.center());
        }
        return None;
    }
    if e.cooldown <= 0.0
        && distance < FUSE_M
        && delta.y.abs() < 2.5
        && line(world, e.center(), eye)
    {
        e.tell = FUSE_S;
        e.attack = ATTACK_FUSE;
        sound(sounds, Cue::Warning, Some(e.center()), 0.4);
    }
    None
}

pub(super) fn fusing(e: &Enemy) -> bool {
    e.kind == CREEPER && e.tell > 0.0 && e.attack == ATTACK_FUSE
}

impl super::Adventure {
    /// Resolves one creeper blast: player and bodies in `BLAST_M` take damage
    /// scaled by distance, the ground loses a crater and the burst is shown.
    pub(super) fn burst(&mut self, world: &mut World, at: Vec3, eye: Vec3) {
        let falloff = |d: f32| (1.0 - d / BLAST_M).clamp(0.2, 1.0);
        let to_eye = eye - at;
        if to_eye.length() < BLAST_M && line(world, at, eye) {
            self.damage(BLAST_DAMAGE * falloff(to_eye.length()), at - eye);
        }
        for e in &mut self.enemies {
            let delta = e.center() - at;
            if e.health > 0.0 && delta.length() < BLAST_M && line(world, at, e.center()) {
                e.hurt(BLAST_DAMAGE * 0.6 * falloff(delta.length()));
                e.knock += delta.normalize_or_zero() * 6.0;
                e.flash = 0.12;
            }
        }
        self.carve_sphere(world, at, CRATER_M, CRATER_CELLS);
        self.blast_visual(at, BLAST_M * 0.6, Vec3::Y, [0.95, 0.72, 0.35]);
        sound(&mut self.sounds, Cue::Burst, Some(at), 1.0);
    }
}

const SKIN: f32 = 70.0;
const MOSS: [f32; 3] = [0.36, 0.62, 0.30];
const MOSS_DARK: [f32; 3] = [0.22, 0.42, 0.20];
const BONE: [f32; 3] = [0.84, 0.84, 0.76];
const HUSK: [f32; 3] = [0.69, 0.60, 0.35];
const SOCKET: [f32; 3] = [0.12, 0.12, 0.12];

fn at(e: &Enemy, x: f32, y: f32, z: f32) -> Vec3 {
    e.pos + Vec3::new(x, y, z)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// A split seed pod carried by six jointed roots. The seam opens during the
/// fuse; thorn shutters and an accelerating ember pulse show the remaining time.
pub(super) fn creeper_body(e: &Enemy, m: &mut Model, time: f32, _wind: f32, _hit: [f32; 3]) {
    let armed = if fusing(e) { (1.0 - e.tell / FUSE_S).clamp(0.0, 1.0) } else { 0.0 };
    let crouch = armed * 0.12;
    let pod = at(e, 0.0, 0.74 - crouch, 0.0);
    let wood = [0.28, 0.23, 0.14];
    let pulse = ((time * (5.0 + armed * 12.0)).sin() * 0.5 + 0.5) * armed;
    for side in [-1.0_f32, 1.0] {
        for i in 0..3 {
            let offset = i as f32 * std::f32::consts::TAU / 3.0 + if side < 0.0 { 0.0 } else { std::f32::consts::PI };
            let (travel, lift) = super::enemy_presentation::walk(e, 0.15, 0.10, offset, 1.0 - armed);
            let z = (i as f32 - 1.0) * 0.25;
            let knee = at(e, side * 0.36, 0.35 - crouch * 0.5, z - travel * 0.3);
            let foot = at(e, side * 0.42, 0.045 + lift, z + travel);
            m.material_beam(pod + Vec3::new(side * 0.17, -0.18, z * 0.6), knee, 0.045, wood, 69.0);
            m.material_beam(knee, foot, 0.035, MOSS_DARK, SKIN);
            m.material_box(foot, Vec3::new(0.12, 0.08, 0.19), Quat::IDENTITY, wood, 69.0, 0.0);
        }
        let mark = m.mark();
        m.material_box(pod + Vec3::new(side * 0.16, 0.0, 0.0), Vec3::new(0.29, 0.65, 0.43), Quat::IDENTITY, MOSS, SKIN, 0.0);
        for band in 0..3 {
            m.material_box(pod + Vec3::new(side * 0.17, -0.20 + band as f32 * 0.20, -0.23), Vec3::new(0.27, 0.055, 0.05), Quat::IDENTITY, MOSS_DARK, SKIN, 0.0);
        }
        m.rotate_since(mark, pod + Vec3::Y * 0.27, Quat::from_rotation_z(side * armed * 0.42));
        let root = pod + Vec3::new(side * 0.15, 0.29, 0.02);
        let tip = root + Vec3::new(side * (0.13 + armed * 0.09), 0.30, -0.07);
        m.material_beam(root, tip, 0.038, wood, 69.0);
        m.material_beam(tip, tip + Vec3::new(-side * 0.08, 0.08, -0.12), 0.020, HUSK, SKIN);
    }
    m.material_box(pod + Vec3::new(0.0, -0.02, -0.025), Vec3::new(0.08 + armed * 0.18, 0.43, 0.28), Quat::IDENTITY,
        mix([0.65, 0.33, 0.10], [0.97, 0.73, 0.29], pulse), 60.0, armed * (0.3 + pulse * 0.6));
}
/// Thin bone limbs, a ribbed torso and a bow drawn while the shot winds up.
pub(super) fn skeleton_body(e: &Enemy, m: &mut Model, _time: f32, wind: f32, hit: [f32; 3]) {
    let bob = (e.phase * 2.0).sin().abs() * 0.03 * e.gait;
    for side in [-1.0, 1.0] {
        let offset = if side < 0.0 { 0.0 } else { std::f32::consts::PI };
        let (dz, lift) = super::enemy_presentation::walk(e, 0.22, 0.12, offset, 1.0 - wind);
        let hip = at(e, side * 0.11, 0.86 + bob, 0.0);
        let travel = super::enemy_presentation::travel(e, dz);
        let knee = at(e, side * 0.11, 0.45 + lift * 0.5, -lift * 0.3) + travel * 0.5;
        let foot = at(e, side * 0.11, lift, 0.0) + travel;
        m.material_beam(hip, knee, 0.05, BONE, SKIN);
        m.material_beam(knee, foot, 0.045, BONE, SKIN);
    }
    m.material_box(at(e, 0.0, 0.9 + bob, 0.0), Vec3::new(0.3, 0.1, 0.16), Quat::IDENTITY, BONE, SKIN, 0.0);
    m.material_beam(at(e, 0.0, 0.9 + bob, 0.0), at(e, 0.0, 1.5 + bob, 0.0), 0.05, BONE, SKIN);
    for i in 0..4 {
        let y = 1.05 + i as f32 * 0.12 + bob;
        m.material_box(at(e, 0.0, y, 0.0), Vec3::new(0.38 - i as f32 * 0.02, 0.05, 0.2), Quat::IDENTITY, BONE, SKIN, 0.0);
    }
    m.material_box(at(e, 0.0, 1.5 + bob, 0.0), Vec3::new(0.44, 0.08, 0.18), Quat::IDENTITY, BONE, SKIN, 0.0);
    let draw = wind.clamp(0.0, 1.0);
    let grip = at(e, 0.0, 1.32 + bob, -0.5 - draw * 0.1);
    let nock = grip + Vec3::new(0.0, 0.0, 0.1 + 0.3 * draw);
    for side in [-1.0, 1.0] {
        let shoulder = at(e, side * 0.22, 1.48 + bob, 0.0);
        let hand = if side < 0.0 { grip } else { nock };
        let elbow = shoulder + (hand - shoulder) * 0.5 + Vec3::new(side * 0.1, -0.08, 0.0);
        m.material_beam(shoulder, elbow, 0.045, BONE, SKIN);
        m.material_beam(elbow, hand, 0.04, BONE, SKIN);
    }
    let bow = [0.36, 0.26, 0.14];
    let string = [0.9, 0.9, 0.85];
    let tip_top = grip + Vec3::new(0.0, 0.5, 0.12);
    let tip_bottom = grip + Vec3::new(0.0, -0.5, 0.12);
    let belly = grip + Vec3::new(0.0, 0.0, -0.06);
    m.material_beam(tip_top, belly, 0.035, bow, SKIN);
    m.material_beam(belly, tip_bottom, 0.035, bow, SKIN);
    m.material_beam(tip_top, nock, 0.012, string, SKIN);
    m.material_beam(nock, tip_bottom, 0.012, string, SKIN);
    if draw > 0.0 {
        m.material_beam(nock, nock + Vec3::new(0.0, 0.0, -0.6), 0.02, [0.55, 0.42, 0.28], SKIN);
    }
    let head = at(e, 0.0, 1.76 + bob, 0.0);
    m.material_box(head, Vec3::splat(0.42), Quat::IDENTITY, mix(BONE, hit, 0.5), SKIN, 0.0);
    for sx in [-0.1, 0.1] {
        m.material_box(head + Vec3::new(sx, 0.04, -0.22), Vec3::new(0.1, 0.1, 0.03), Quat::IDENTITY, SOCKET, SKIN, 0.0);
    }
    m.material_box(head + Vec3::new(0.0, -0.12, -0.22), Vec3::new(0.2, 0.05, 0.03), Quat::IDENTITY, SOCKET, SKIN, 0.0);
}

#[cfg(test)]
#[path = "mobs_tests.rs"]
mod tests;
