//! The first guardian commits to a straight rush. Its tell, movement and
//! damage all use the same lane, with a punish window when the rush fails.
use super::*;

const SPEED: f32 = 8.0;
const LENGTH: f32 = 4.8;
const HALF_WIDTH: f32 = 0.9;
pub(super) const WINDUP: f32 = 0.85;
pub(super) const ATTACK_RANGE: f32 = 5.1;
pub(super) const DAMAGE: f32 = 20.0;

pub(super) fn is_lead(site: IVec3, slot: u32, kind: u32) -> bool {
    site == IVec3::ZERO && slot == 0 && kind == 0
}

pub(super) fn lead(e: &Enemy) -> bool {
    e.habitat.is_none() && is_lead(e.site, e.slot, e.kind)
}

pub(super) fn base_health(site: IVec3, slot: u32, kind: u32) -> f32 {
    if is_lead(site, slot, kind) { 92.0 } else { enemy_health(kind) }
}

pub(super) fn begin(e: &mut Enemy) {
    e.charge = LENGTH / SPEED;
    e.recovery = 0.0;
    e.cooldown = 1.25;
}

pub(super) fn expose(e: &mut Enemy) {
    e.charge = 0.0;
    e.tell = 0.0;
    e.recovery = 1.15;
    e.cooldown = e.cooldown.max(1.15);
    e.encounter.exposed = e.recovery;
    e.knock = Vec3::ZERO;
    e.velocity.x = 0.0;
    e.velocity.z = 0.0;
}

/// Ordinary hits still damage the guardian, but only a committed counter
/// interrupts an attack. Hits during the opening leave its timer intact.
pub(super) fn handles_hit(e: &mut Enemy, interrupt: bool) -> bool {
    if !lead(e) { return false; }
    if interrupt && (e.tell > 0.0 || e.charge > 0.0) {
        expose(e);
    }
    true
}

pub(super) fn advance(e: &mut Enemy, world: &World, feet: Vec3, eye: Vec3, dt: f32, sounds: &mut Vec<Sound>) -> Option<Vec3> {
    let start = e.pos;
    let distance = e.charge.min(dt) * SPEED;
    let count = (distance / 0.12).ceil().max(1.0) as u32;
    let step = e.lunge * (distance / count as f32);
    let mut blocked = false;
    for _ in 0..count {
        let next = e.pos + step;
        if !free(world, next, e.kind) || !navigation::on_floor(world, next, e.up) {
            blocked = true;
            break;
        }
        e.pos = next;
    }
    let movement = e.pos - start;
    e.phase += movement.length();
    e.velocity.x = movement.x / dt;
    e.velocity.z = movement.z / dt;
    e.gait += (1.0 - e.gait) * (1.0 - (-dt * 16.0).exp());
    e.charge = (e.charge - dt).max(0.0);

    let offset = feet - start;
    let forward = offset.dot(e.lunge);
    // Swept body contact, including the side of the body at the end of the rush.
    let closest = start + e.lunge * forward.clamp(0.0, movement.length());
    let contact = (feet - closest).with_y(0.0).length_squared() <= HALF_WIDTH * HALF_WIDTH
        && offset.y.abs() < 0.9 && line(world, e.center(), eye);
    if contact {
        e.charge = 0.0;
        e.recovery = enemy_presentation::RECOVERY_S;
        return Some((e.center() - eye).try_normalize().unwrap_or(-e.lunge));
    }
    if blocked || e.charge <= 0.0 {
        if blocked { sound(sounds, Cue::Impact, Some(e.center()), 0.85); }
        expose(e);
    }
    None
}

pub(super) fn telegraph(e: &Enemy, model: &mut Model) {
    if e.tell > 0.0 || e.charge > 0.0 {
        let side = Vec3::Y.cross(e.lunge) * HALF_WIDTH;
        let start = e.pos + Vec3::Y * 0.10;
        let length = if e.tell > 0.0 { LENGTH } else { e.charge * SPEED };
        let end = start + e.lunge * length;
        let color = [1.0, 0.39, 0.12];
        for edge in [-side, side] {
            model.beam(start + edge, end + edge, 0.044, color, 1.4);
        }
        for (center, outward) in [(start, -e.lunge), (end, e.lunge)] {
            let mut from = center + side;
            for index in 1..=4 {
                let (sin, cos) = (index as f32 * std::f32::consts::FRAC_PI_4).sin_cos();
                let to = center + side * cos + outward * (HALF_WIDTH * sin);
                model.beam(from, to, 0.044, color, 1.4);
                from = to;
            }
        }
    } else if e.encounter.exposed > 0.0 {
        let at = e.center() + Vec3::Y * 0.42;
        model.orb(at, 0.09, [0.40, 1.0, 0.74], 1.65);
    }
}
