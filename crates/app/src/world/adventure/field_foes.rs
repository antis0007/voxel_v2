//! Field foes: three committed-attack archetypes that own their whole tick.
//!
//! - Blade Rotor (60): a hovering spinner. Spins up, locks a line, dashes along
//!   it. Spinning blades turn hits aside (a regenerating shield); a dash that
//!   meets a wall leaves it stunned on the ground with no guard.
//! - Stone Colossus (61): sleeps as a boulder pile. Wakes when approached or
//!   struck, walks in, raises both fists and slams a committed spot. Its body
//!   is armoured; only the slam's recovery exposes the core on its back.
//! - Ruin Walker (62): a six-legged sentry that keeps its distance and charges
//!   a beam. The sight line tracks until half a second before release, then
//!   locks, so a sidestep defeats it. Enough damage during the charge breaks it.
//!
//! - Rotor Bloom (63): a Blade Rotor rooted as a leafy bud. Sits still until
//!   someone walks close or strikes it, then its leaves fold out into rotor
//!   blades and it lifts off, chasing with contact cuts. After a flight it
//!   tires and drops, blades winding down, unguarded, before rooting again.
//! - Pine Rotor (64) and Palm Rotor (65): the bloom's trick at tree scale. The
//!   crown stands on a real-looking trunk until someone walks under it or
//!   strikes it, then shears off the trunk and flies, branches or fronds swung
//!   level into blades. Tired, it drops to the ground unguarded; left alone, it
//!   flies back and sits on its stump again.
//!
//! At range the Stone Colossus tears a chunk of real ground loose, lifts it
//! overhead and hurls it at where the player stood on release; the chunk
//! lands as blocks again (see `hurled`).
//!
//! Every attack commits to its telegraph; nothing tracks after the lock and no
//! damage passes through walls. Mode lives in `Enemy::attack`, its timer in
//! `Enemy::tell`. Effects go through the burrowers' `Fx` queue.
use super::{Cue, Enemy, World, burrowers::Fx, enemy_presentation::motion, free, line, navigation, status::Affix};
use crate::mesh::models::Model;
use crate::world::trace;
use engine_core::hash::hash_u32;
use glam::{Quat, Vec3};
use std::f32::consts::TAU;

pub(super) const ROTOR: u32 = 60;
pub(super) const COLOSSUS: u32 = 61;
pub(super) const WALKER: u32 = 62;
pub(super) const BLOOM: u32 = 63;
pub(super) const PINE_ROTOR: u32 = 64;
pub(super) const PALM_ROTOR: u32 = 65;

pub(super) fn is_kind(kind: u32) -> bool {
    matches!(kind, ROTOR | COLOSSUS | WALKER | BLOOM | PINE_ROTOR | PALM_ROTOR)
}


// Modes, shared numbering across the three kinds.
const IDLE: u32 = 0; // rotor hover, colossus dormant, walker patrol
const WINDUP: u32 = 1;
const STRIKE: u32 = 2; // rotor dash
const STUN: u32 = 3; // rotor grounded, colossus exposed, walker broken
const WAKE: u32 = 4; // colossus rising
const WALK: u32 = 5; // colossus awake
const HEAVE: u32 = 6; // colossus tearing up and lifting a chunk of ground

const ROTOR_GUARD: f32 = 30.0;
const ROTOR_WINDUP_S: f32 = 0.9;
const ROTOR_LOCK_S: f32 = 0.25;
const ROTOR_DASH_MPS: f32 = 14.0;
const ROTOR_DASH_S: f32 = 1.1;
const ROTOR_STUN_S: f32 = 2.2;
const ROTOR_HIT: f32 = 12.0;

const COLOSSUS_WAKE_M: f32 = 7.0;
const COLOSSUS_RISE_S: f32 = 1.6;
const COLOSSUS_WINDUP_S: f32 = 1.25;
const COLOSSUS_EXPOSED_S: f32 = 2.8;
const COLOSSUS_SLAM_M: f32 = 3.6;
const COLOSSUS_HIT: f32 = 24.0;
/// Seconds from gripping the ground to the throw.
const HEAVE_S: f32 = 1.5;
/// Share of the heave spent with the fists down in the dirt.
const HEAVE_GRIP: f32 = 0.3;
/// The colossus throws only at players between these ranges (metres).
const HEAVE_MIN_M: f32 = 7.5;
const HEAVE_MAX_M: f32 = 26.0;
const HEAVE_HIT: f32 = 20.0;
const HEAVE_COOLDOWN_S: f32 = 4.0;
/// Radius of the torn-up chunk (metres).
const HEAVE_CHUNK_M: f32 = 0.5;

const BLOOM_TRIGGER_M: f32 = 4.5;
const BLOOM_UNFOLD_S: f32 = 0.7;
const BLOOM_FLIGHT_S: f32 = 7.0;
const BLOOM_TIRED_S: f32 = 2.6;
const BLOOM_HOVER_M: f32 = 1.4;
const BLOOM_CUT_S: f32 = 0.6;
const BLOOM_HIT: f32 = 16.0;
const BLOOM_DORMANT_GUARD: f32 = 1000.0;

/// Crown hub height above the trunk's foot while it sits on the stump.
const PINE_CROWN_M: f32 = 3.6;
const PALM_CROWN_M: f32 = 4.2;
/// Flat distance from the trunk that springs a tree rotor.
const TREE_TRIGGER_M: f32 = 5.0;
const TREE_UNFOLD_S: f32 = 0.9;
const TREE_FLIGHT_S: f32 = 8.0;
const TREE_TIRED_S: f32 = 3.0;
const TREE_RETURN_MPS: f32 = 3.0;
const TREE_CUT_S: f32 = 0.7;
const TREE_HIT: f32 = 18.0;

const WALKER_CHARGE_S: f32 = 2.0;
const WALKER_LOCK_S: f32 = 0.5;
const WALKER_RANGE_M: f32 = 36.0;
const WALKER_BREAK: f32 = 15.0;
const WALKER_HIT: f32 = 26.0;

fn unit(k: u32) -> f32 {
    (hash_u32(k) & 0xffff) as f32 / 65535.0
}

fn menace(e: &Enemy) -> f32 {
    1.0 + e.menace as f32 * 0.2
}

/// Moves by `delta` in small sub-steps; false when a step was blocked.
fn step(e: &mut Enemy, world: &World, delta: Vec3) -> bool {
    let steps = (delta.length() / 0.18).ceil().max(1.0) as usize;
    for _ in 0..steps {
        let q = e.pos + delta / steps as f32;
        if !free(world, q, e.kind) {
            return false;
        }
        e.pos = q;
    }
    true
}

/// Ground walk with a one-metre step-up.
fn walk(e: &mut Enemy, world: &World, dir: Vec3, speed: f32, dt: f32) {
    let flat = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
    if flat == Vec3::ZERO {
        return;
    }
    let delta = flat * speed * dt;
    if !step(e, world, delta) {
        let up = e.pos + Vec3::Y * 1.05;
        if free(world, up, e.kind) && free(world, up + delta, e.kind) {
            e.pos = up + delta;
        }
    }
    e.phase += delta.length();
    face(e, flat);
}

fn face(e: &mut Enemy, dir: Vec3) {
    if dir.x != 0.0 || dir.z != 0.0 {
        e.yaw = (-dir.x).atan2(-dir.z);
    }
}

/// Height of the first solid cell below `at`, at most `reach` metres down.
fn ground(world: &World, at: Vec3, reach: f32) -> f32 {
    trace::solid(world, at, at - Vec3::Y * reach).map_or(at.y - reach, |(hit, _)| hit.y)
}

fn element(e: &Enemy) -> [f32; 3] {
    match e.affixes.first() {
        Some(Affix::Fiery) => [1.0, 0.45, 0.12],
        Some(Affix::Frostbound) => [0.55, 0.85, 1.0],
        Some(Affix::Static) => [0.75, 0.7, 1.0],
        Some(Affix::Toxic) => [0.5, 0.95, 0.3],
        _ => [0.95, 0.25, 0.15],
    }
}

/// One step for a field foe; returns damage to the player and its direction.
#[allow(clippy::too_many_arguments)]
pub(super) fn tick(e: &mut Enemy, world: &World, feet: Vec3, eye: Vec3, dt: f32, time: f32, hunt: bool, fx: &mut Vec<Fx>) -> (f32, Vec3) {
    e.tell = (e.tell - dt).max(0.0);
    e.cooldown = (e.cooldown - dt).max(0.0);
    e.recovery = (e.recovery - dt).max(0.0);
    let hunt = hunt && e.hostile();
    e.visible = hunt && line(world, e.center(), eye);
    let damage = match e.kind {
        ROTOR => rotor(e, world, eye, dt, time, hunt, fx),
        COLOSSUS => colossus(e, world, feet, eye, dt, hunt, fx),
        BLOOM => bloom(e, world, feet, eye, dt, time, hunt, fx),
        PINE_ROTOR | PALM_ROTOR => tree_rotor(e, world, feet, eye, dt, time, hunt, fx),
        _ => walker(e, world, eye, dt, hunt, fx),
    } * menace(e);
    if damage > 0.0 {
        (damage, (e.center() - eye).normalize_or_zero() * damage)
    } else {
        (0.0, Vec3::ZERO)
    }
}

fn rotor(e: &mut Enemy, world: &World, eye: Vec3, dt: f32, time: f32, hunt: bool, fx: &mut Vec<Fx>) -> f32 {
    let to = eye - e.center();
    let distance = to.length();
    if e.attack == STUN {
        e.shield = 0.0;
        e.velocity.y = (e.velocity.y - 14.0 * dt).max(-14.0);
        let v = e.velocity;
        if !step(e, world, v * dt) {
            e.velocity = Vec3::ZERO;
        }
        if e.tell <= 0.0 {
            e.attack = IDLE;
            e.cooldown = 1.5;
            fx.push(Fx::Sound { cue: Cue::Warning, at: e.center(), gain: 0.35, pitch: 1.4 });
        }
        return 0.0;
    }
    // Spinning blades turn shots aside until they are stopped.
    e.shield = (e.shield + 10.0 * dt).min(ROTOR_GUARD);
    if !hunt && matches!(e.attack, WINDUP | STRIKE) {
        e.attack = IDLE;
        e.velocity = Vec3::ZERO;
        e.cooldown = 1.5;
    }
    match e.attack {
        WINDUP => {
            e.velocity *= (1.0 - 4.0 * dt).max(0.0);
            let v = e.velocity;
            step(e, world, v * dt);
            if e.tell > ROTOR_LOCK_S && distance > 0.1 {
                e.aim = to / distance;
                face(e, e.aim);
            }
            if e.tell <= 0.0 {
                e.attack = STRIKE;
                e.tell = ROTOR_DASH_S;
                e.lunge = e.aim * ROTOR_DASH_MPS * e.affixes.speed_mul();
                e.charge = 0.0;
                fx.push(Fx::Sound { cue: Cue::Swing, at: e.center(), gain: 0.9, pitch: 0.7 });
            }
            0.0
        }
        STRIKE => {
            let lunge = e.lunge;
            if !step(e, world, lunge * dt) {
                // Blades bite the wall: grounded, unguarded, open to every hit.
                e.attack = STUN;
                e.tell = ROTOR_STUN_S;
                e.stagger = ROTOR_STUN_S;
                e.velocity = -e.aim * 2.0;
                let at = e.center() + e.aim * 0.5;
                fx.push(Fx::Sound { cue: Cue::Clang, at, gain: 1.0, pitch: 0.9 });
                fx.push(Fx::Burst { at, color: [0.85, 0.8, 0.7], count: 18 });
                return 0.0;
            }
            let mut damage = 0.0;
            if e.charge == 0.0 && (e.center() - eye).length() < 1.2 {
                e.charge = 1.0;
                damage = ROTOR_HIT;
                fx.push(Fx::Sound { cue: Cue::Impact, at: eye, gain: 0.8, pitch: 1.2 });
            }
            if e.tell <= 0.0 {
                e.attack = IDLE;
                e.cooldown = 2.4;
                e.velocity = e.lunge * 0.2;
            }
            damage
        }
        _ => {
            let seed = unit(e.slot) * TAU;
            let mut goal = if hunt {
                let flat = Vec3::new(to.x, 0.0, to.z).normalize_or(Vec3::X);
                // Keep a ring around the player, drifting sideways.
                let side = flat.cross(Vec3::Y) * (time * 0.4 + seed).sin() * 3.0;
                eye - flat * 7.0 + side
            } else {
                e.home + Vec3::new((time * 0.3 + seed).cos() * 3.0, 0.0, (time * 0.3 + seed).sin() * 3.0)
            };
            goal.y = goal.y.max(ground(world, e.pos + Vec3::Y, 8.0) + 2.2);
            let want = goal - e.pos;
            let speed = super::bestiary::species(e.kind).map_or(2.4, |s| s.speed) * e.affixes.speed_mul();
            let target_v = want.normalize_or_zero() * speed * (want.length() / 2.0).min(1.0);
            e.velocity = e.velocity.lerp(target_v, (dt * 3.0).min(1.0));
            let v = e.velocity;
            if !step(e, world, v * dt) {
                e.velocity = Vec3::ZERO;
            }
            if hunt {
                face(e, to);
            }
            if hunt && e.cooldown <= 0.0 && distance < 14.0 && e.visible {
                e.attack = WINDUP;
                e.tell = ROTOR_WINDUP_S;
                e.aim = to / distance.max(0.1);
                fx.push(Fx::Sound { cue: Cue::Warning, at: e.center(), gain: 0.6, pitch: 1.3 });
            }
            0.0
        }
    }
}

/// Drops toward the ground under gravity, sliding to a stop.
fn settle(e: &mut Enemy, world: &World, dt: f32) {
    e.velocity.x *= (1.0 - 5.0 * dt).max(0.0);
    e.velocity.z *= (1.0 - 5.0 * dt).max(0.0);
    e.velocity.y = (e.velocity.y - 14.0 * dt).max(-14.0);
    let v = e.velocity;
    if !step(e, world, v * dt) {
        e.velocity.y = 0.0;
        let side = Vec3::new(v.x, 0.0, v.z);
        if !step(e, world, side * dt) {
            e.velocity = Vec3::ZERO;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn bloom(e: &mut Enemy, world: &World, feet: Vec3, eye: Vec3, dt: f32, time: f32, hunt: bool, fx: &mut Vec<Fx>) -> f32 {
    let flat = Vec3::new(feet.x - e.pos.x, 0.0, feet.z - e.pos.z);
    match e.attack {
        IDLE => {
            // Rooted disguise. A huge guard reads strikes without hurting it,
            // the same trick as the dormant colossus.
            let struck = e.shield < BLOOM_DORMANT_GUARD - 0.1 && e.shield > 0.0;
            e.shield = BLOOM_DORMANT_GUARD;
            settle(e, world, dt);
            if struck || (hunt && flat.length() < BLOOM_TRIGGER_M && (feet.y - e.pos.y).abs() < 3.0) {
                e.attack = WAKE;
                e.tell = BLOOM_UNFOLD_S;
                e.shield = ROTOR_GUARD;
                face(e, flat);
                fx.push(Fx::Sound { cue: Cue::Swing, at: e.center(), gain: 0.9, pitch: 1.6 });
                fx.push(Fx::Burst { at: e.pos + Vec3::Y * 0.4, color: [0.3, 0.5, 0.22], count: 22 });
            }
            0.0
        }
        WAKE => {
            // Leaves snap open into blades and it pops off the ground.
            e.shield = ROTOR_GUARD;
            step(e, world, Vec3::Y * 3.0 * dt);
            if e.tell <= 0.0 {
                e.attack = WALK;
                e.tell = BLOOM_FLIGHT_S;
                e.cooldown = 0.0;
                e.velocity = Vec3::ZERO;
                fx.push(Fx::Sound { cue: Cue::Warning, at: e.center(), gain: 0.8, pitch: 1.5 });
            }
            0.0
        }
        STUN => {
            // Tired: grounded, blades winding down, nothing turns hits aside.
            e.shield = 0.0;
            settle(e, world, dt);
            if e.tell <= 0.0 {
                if hunt && flat.length() < BLOOM_TRIGGER_M * 2.0 {
                    e.attack = WAKE;
                    e.tell = BLOOM_UNFOLD_S;
                    fx.push(Fx::Sound { cue: Cue::Swing, at: e.center(), gain: 0.8, pitch: 1.5 });
                } else {
                    e.attack = IDLE;
                    e.home = e.pos;
                }
            }
            0.0
        }
        _ => {
            e.shield = (e.shield + 10.0 * dt).min(ROTOR_GUARD);
            let seed = unit(e.slot) * TAU;
            let mut goal = if hunt {
                // Bores straight in with a slight weave; the blades are the weapon.
                let side = flat.normalize_or(Vec3::X).cross(Vec3::Y) * (time * 2.2 + seed).sin() * 0.8;
                Vec3::new(feet.x, feet.y + 1.0, feet.z) + side
            } else {
                e.home + Vec3::Y * BLOOM_HOVER_M
            };
            goal.y = goal.y.max(ground(world, e.pos + Vec3::Y, 8.0) + BLOOM_HOVER_M * 0.6);
            let want = goal - e.pos;
            let speed = super::bestiary::species(e.kind).map_or(4.5, |s| s.speed) * e.affixes.speed_mul();
            let target_v = want.normalize_or_zero() * speed * (want.length() / 1.5).min(1.0);
            e.velocity = e.velocity.lerp(target_v, (dt * 2.5).min(1.0));
            let v = e.velocity;
            if !step(e, world, v * dt) {
                e.velocity *= -0.4;
            }
            face(e, flat);
            let mut damage = 0.0;
            let blades = e.pos + Vec3::Y * 0.6;
            let near = (blades - eye).length().min((blades - (feet + Vec3::Y * 0.9)).length());
            if e.cooldown <= 0.0 && near < 1.5 && line(world, blades, eye) {
                e.cooldown = BLOOM_CUT_S;
                damage = BLOOM_HIT;
                fx.push(Fx::Sound { cue: Cue::Impact, at: eye, gain: 0.8, pitch: 1.4 });
                fx.push(Fx::Burst { at: blades, color: [0.85, 0.8, 0.7], count: 8 });
            }
            if e.tell <= 0.0 {
                e.attack = STUN;
                e.tell = BLOOM_TIRED_S;
                e.stagger = BLOOM_TIRED_S;
                fx.push(Fx::Sound { cue: Cue::Warning, at: e.center(), gain: 0.4, pitch: 0.7 });
            }
            damage
        }
    }
}

fn crown_m(kind: u32) -> f32 {
    if kind == PALM_ROTOR { PALM_CROWN_M } else { PINE_CROWN_M }
}

fn foliage(kind: u32) -> [f32; 3] {
    if kind == PALM_ROTOR { [0.27, 0.47, 0.19] } else { [0.13, 0.27, 0.16] }
}

/// Tree rotor. `home` is the trunk's foot; while it sits (`IDLE`) the crown
/// hub is pinned on top of the trunk. `STRIKE` is the flight back to the stump.
#[allow(clippy::too_many_arguments)]
fn tree_rotor(e: &mut Enemy, world: &World, feet: Vec3, eye: Vec3, dt: f32, time: f32, hunt: bool, fx: &mut Vec<Fx>) -> f32 {
    if e.attack == IDLE && e.pos.distance_squared(e.home + Vec3::Y * crown_m(e.kind)) > 1e-4 {
        // First tick: spawners lift flyers off the floor; root the trunk on it.
        let start = e.home + Vec3::Y * 0.5;
        if let Some((hit, _)) = trace::solid(world, start, start - Vec3::Y * 6.0) {
            e.home.y = hit.y;
        }
    }
    let spin = match e.attack {
        IDLE => 0.0,
        WAKE => 1.0 + tree_open(e) * 27.0,
        STUN => 28.0 * (e.tell / TREE_TIRED_S).clamp(0.0, 1.0).powi(2),
        STRIKE => 12.0,
        _ => 28.0,
    };
    e.phase = (e.phase + spin * dt).rem_euclid(TAU);
    let top = e.home + Vec3::Y * crown_m(e.kind);
    let near = |at: Vec3| {
        let flat = Vec3::new(feet.x - at.x, 0.0, feet.z - at.z);
        hunt && flat.length() < TREE_TRIGGER_M && (feet.y - at.y).abs() < 4.0
    };
    match e.attack {
        IDLE => {
            // Sitting on the trunk. The huge guard reads a strike without
            // hurting it, the same trick as the bloom and the dormant colossus.
            let struck = e.shield < BLOOM_DORMANT_GUARD - 0.1 && e.shield > 0.0;
            e.shield = BLOOM_DORMANT_GUARD;
            e.pos = top;
            e.velocity = Vec3::ZERO;
            if struck || near(e.home) {
                e.attack = WAKE;
                e.tell = TREE_UNFOLD_S;
                e.shield = ROTOR_GUARD;
                face(e, feet - e.home);
                fx.push(Fx::Sound { cue: Cue::Swing, at: top, gain: 1.0, pitch: 1.1 });
                fx.push(Fx::Burst { at: top, color: foliage(e.kind), count: 26 });
            }
            0.0
        }
        WAKE => {
            // Spins up in place, then shears off the trunk.
            e.shield = ROTOR_GUARD;
            if e.tell <= 0.0 {
                e.attack = WALK;
                e.tell = TREE_FLIGHT_S;
                e.cooldown = 0.0;
                e.velocity = Vec3::Y * 1.5;
                let at = e.pos;
                fx.push(Fx::Sound { cue: Cue::Impact, at, gain: 0.9, pitch: 0.7 });
                fx.push(Fx::Burst { at: at - Vec3::Y * 0.2, color: [0.62, 0.48, 0.3], count: 18 });
                fx.push(Fx::Sound { cue: Cue::Warning, at, gain: 0.8, pitch: 1.2 });
            }
            0.0
        }
        STUN => {
            // Tired: on the ground, blades winding down, nothing turns hits aside.
            e.shield = 0.0;
            settle(e, world, dt);
            if e.tell <= 0.0 {
                if near(e.pos) {
                    e.attack = WAKE;
                    e.tell = TREE_UNFOLD_S * 0.6;
                    fx.push(Fx::Sound { cue: Cue::Swing, at: e.center(), gain: 0.8, pitch: 1.2 });
                } else {
                    e.attack = STRIKE;
                    e.velocity = Vec3::ZERO;
                }
            }
            0.0
        }
        STRIKE => {
            // Flies home and sits on the stump again; someone close turns it back.
            e.shield = (e.shield + 10.0 * dt).min(ROTOR_GUARD);
            let want = top - e.pos;
            if near(e.pos) {
                e.attack = WALK;
                e.tell = TREE_FLIGHT_S * 0.5;
            } else if want.length() < 0.25 {
                e.attack = IDLE;
                e.pos = top;
                e.velocity = Vec3::ZERO;
                e.shield = BLOOM_DORMANT_GUARD;
            } else {
                let v = want.normalize_or_zero() * TREE_RETURN_MPS * (want.length() / 1.0).min(1.0);
                e.velocity = e.velocity.lerp(v, (dt * 3.0).min(1.0));
                let v = e.velocity;
                // Clips through loose foliage near the stump rather than
                // stalling a metre short of it forever.
                if !step(e, world, v * dt) && want.length() < 2.5 {
                    e.pos += v * dt;
                }
                face(e, want);
            }
            0.0
        }
        _ => {
            e.shield = (e.shield + 10.0 * dt).min(ROTOR_GUARD);
            let flat = Vec3::new(feet.x - e.pos.x, 0.0, feet.z - e.pos.z);
            let seed = unit(e.slot ^ 0x7e3) * TAU;
            let mut goal = if hunt {
                // Drops onto the player in wide swoops; the spinning crown is the weapon.
                let side = flat.normalize_or(Vec3::X).cross(Vec3::Y) * (time * 1.6 + seed).sin() * 1.2;
                Vec3::new(feet.x, feet.y + 1.1, feet.z) + side
            } else {
                top
            };
            goal.y = goal.y.max(ground(world, e.pos + Vec3::Y, 8.0) + BLOOM_HOVER_M * 0.6);
            let want = goal - e.pos;
            let speed = super::bestiary::species(e.kind).map_or(4.0, |s| s.speed) * e.affixes.speed_mul();
            let target_v = want.normalize_or_zero() * speed * (want.length() / 1.5).min(1.0);
            e.velocity = e.velocity.lerp(target_v, (dt * 2.0).min(1.0));
            let v = e.velocity;
            if !step(e, world, v * dt) {
                e.velocity *= -0.4;
            }
            face(e, flat);
            let mut damage = 0.0;
            let blades = e.pos + Vec3::Y * 0.2;
            let reach = (blades - eye).length().min((blades - (feet + Vec3::Y * 0.9)).length());
            if e.cooldown <= 0.0 && reach < 1.8 && line(world, blades, eye) {
                e.cooldown = TREE_CUT_S;
                damage = TREE_HIT;
                fx.push(Fx::Sound { cue: Cue::Impact, at: eye, gain: 0.85, pitch: 1.2 });
                fx.push(Fx::Burst { at: blades, color: foliage(e.kind), count: 10 });
            }
            if e.tell <= 0.0 || (!hunt && want.length() < 0.4) {
                e.attack = STUN;
                e.tell = TREE_TIRED_S;
                e.stagger = TREE_TIRED_S;
                fx.push(Fx::Sound { cue: Cue::Warning, at: e.center(), gain: 0.4, pitch: 0.6 });
            }
            damage
        }
    }
}

fn colossus(e: &mut Enemy, world: &World, feet: Vec3, eye: Vec3, dt: f32, hunt: bool, fx: &mut Vec<Fx>) -> f32 {
    let armour = e.max_health() * 2.0;
    let flat = Vec3::new(feet.x - e.pos.x, 0.0, feet.z - e.pos.z);
    let reach = flat.length();
    match e.attack {
        IDLE => {
            // Struck while dormant: the shield dropped below its full value.
            let struck = e.shield < armour - 0.1 && e.shield > 0.0;
            e.shield = armour;
            if struck || (hunt && reach < COLOSSUS_WAKE_M) {
                e.attack = WAKE;
                e.tell = COLOSSUS_RISE_S;
                face(e, flat);
                fx.push(Fx::Sound { cue: Cue::ForgeSlam, at: e.center(), gain: 0.9, pitch: 0.5 });
                fx.push(Fx::Burst { at: e.pos + Vec3::Y * 0.8, color: [0.5, 0.47, 0.43], count: 30 });
            }
            0.0
        }
        WAKE => {
            e.shield = armour;
            if e.tell <= 0.0 {
                e.attack = WALK;
                e.cooldown = 0.8;
            }
            0.0
        }
        WINDUP => {
            e.shield = armour;
            navigation::gravity(e, world, dt);
            if e.tell > 0.0 {
                return 0.0;
            }
            let at = e.aim;
            let damage = if (feet - at).length() < COLOSSUS_SLAM_M && (feet.y - at.y).abs() < 2.0 { COLOSSUS_HIT } else { 0.0 };
            fx.push(Fx::Sound { cue: Cue::ForgeSlam, at, gain: 1.0, pitch: 0.6 });
            fx.push(Fx::Burst { at: at + Vec3::Y * 0.3, color: [0.45, 0.4, 0.34], count: 40 });
            e.attack = STUN;
            e.tell = COLOSSUS_EXPOSED_S;
            e.shield = 0.0;
            fx.push(Fx::Sound { cue: Cue::Warning, at: e.center() + Vec3::Y, gain: 0.5, pitch: 0.8 });
            damage
        }
        HEAVE => colossus_heave(e, world, feet, dt, fx),
        STUN => {
            // Recovery: the core on its back is open.
            e.shield = 0.0;
            navigation::gravity(e, world, dt);
            if e.tell <= 0.0 {
                e.attack = WALK;
                e.cooldown = 1.5;
            }
            0.0
        }
        _ => {
            e.shield = armour;
            navigation::gravity(e, world, dt);
            if !hunt {
                return 0.0;
            }
            let speed = super::bestiary::species(e.kind).map_or(1.1, |s| s.speed) * e.affixes.speed_mul();
            if reach > 3.2 {
                walk(e, world, flat, speed, dt);
            } else {
                face(e, flat);
            }
            if reach < 4.5 && e.cooldown <= 0.0 && line(world, e.center(), eye) {
                e.attack = WINDUP;
                e.tell = COLOSSUS_WINDUP_S;
                // Commits to where the player stands now.
                e.aim = feet;
                fx.push(Fx::Sound { cue: Cue::Warning, at: e.center(), gain: 0.8, pitch: 0.55 });
            } else if (HEAVE_MIN_M..HEAVE_MAX_M).contains(&reach) && e.cooldown <= 0.0 && line(world, e.center(), eye) {
                // Out of fist reach: rip up the ground at its side to throw.
                let beside = e.pos + Quat::from_rotation_y(e.yaw) * Vec3::new(0.0, 0.0, -1.4);
                let top = ground(world, beside + Vec3::Y * 1.5, 4.0);
                e.aim = Vec3::new(beside.x, top - HEAVE_CHUNK_M * 0.6, beside.z);
                e.attack = HEAVE;
                e.tell = HEAVE_S;
                fx.push(Fx::Heave { key: (e.site, e.slot), at: e.aim, radius: HEAVE_CHUNK_M });
                fx.push(Fx::Sound { cue: Cue::ForgeSlam, at: e.aim, gain: 0.85, pitch: 0.42 });
            }
            0.0
        }
    }
}

/// Where a heaving colossus holds its chunk: in the dirt at first, then
/// rising overhead for the throw. `None` when it is not heaving.
pub(super) fn heave_hold(e: &Enemy) -> Option<Vec3> {
    if e.kind != COLOSSUS || e.attack != HEAVE {
        return None;
    }
    let t = (1.0 - e.tell / HEAVE_S).clamp(0.0, 1.0);
    let overhead = e.pos + Quat::from_rotation_y(e.yaw) * Vec3::new(0.0, 3.9, 0.25);
    let up = ((t - HEAVE_GRIP) / (1.0 - HEAVE_GRIP - 0.1)).clamp(0.0, 1.0);
    let ease = up * up * (3.0 - 2.0 * up);
    Some(e.aim + (overhead - e.aim) * ease)
}

/// The heave: grip, lift, then throw at where the player stands on release.
fn colossus_heave(e: &mut Enemy, world: &World, feet: Vec3, dt: f32, fx: &mut Vec<Fx>) -> f32 {
    navigation::gravity(e, world, dt);
    face(e, Vec3::new(feet.x - e.pos.x, 0.0, feet.z - e.pos.z));
    if e.tell > 0.0 {
        return 0.0;
    }
    let from = heave_hold(e).unwrap_or(e.pos + Vec3::Y * 3.9);
    let to = feet + Vec3::Y * 0.9;
    let flight_s = (from.distance(to) / 17.0).clamp(0.6, 1.4);
    fx.push(Fx::Hurl { key: (e.site, e.slot), from, to, flight_s, damage: HEAVE_HIT * menace(e), kind: e.kind });
    fx.push(Fx::Sound { cue: Cue::Impact, at: from, gain: 0.8, pitch: 0.45 });
    e.attack = WALK;
    e.cooldown = HEAVE_COOLDOWN_S;
    0.0
}

fn muzzle(e: &Enemy) -> Vec3 {
    e.pos + Vec3::Y * 2.3 + Quat::from_rotation_y(e.yaw) * Vec3::new(0.0, 0.0, -0.75)
}

fn walker(e: &mut Enemy, world: &World, eye: Vec3, dt: f32, hunt: bool, fx: &mut Vec<Fx>) -> f32 {
    navigation::gravity(e, world, dt);
    let to = eye - muzzle(e);
    let distance = to.length();
    let speed = super::bestiary::species(e.kind).map_or(1.6, |s| s.speed) * e.affixes.speed_mul();
    match e.attack {
        WINDUP => {
            if e.tell > WALKER_LOCK_S && distance > 0.1 {
                e.aim = to / distance;
                face(e, e.aim);
            }
            // Enough damage during the charge cracks the lens.
            if e.health + e.shield < e.charge - WALKER_BREAK {
                e.attack = STUN;
                e.tell = 1.8;
                e.stagger = 1.8;
                e.cooldown = 3.0;
                fx.push(Fx::Sound { cue: Cue::Clang, at: muzzle(e), gain: 1.0, pitch: 1.1 });
                fx.push(Fx::Burst { at: muzzle(e), color: [1.0, 0.5, 0.2], count: 16 });
                return 0.0;
            }
            if e.tell > 0.0 {
                return 0.0;
            }
            let from = muzzle(e);
            let far = from + e.aim * WALKER_RANGE_M;
            let end = trace::solid(world, from, far).map_or(far, |(hit, _)| hit);
            // Hit only if the locked ray passes the player before any wall.
            let along = (eye - from).dot(e.aim);
            let closest = from + e.aim * along.clamp(0.0, (end - from).length());
            let damage = if along > 0.0 && (eye - closest).length() < 0.9 && along <= (end - from).length() + 0.5 { WALKER_HIT } else { 0.0 };
            e.lunge = end;
            e.recovery = 0.25;
            e.attack = IDLE;
            e.cooldown = 4.0;
            fx.push(Fx::Sound { cue: Cue::Impact, at: end, gain: 1.0, pitch: 0.6 });
            fx.push(Fx::Burst { at: end, color: element(e), count: 24 });
            damage
        }
        STUN => {
            if e.tell <= 0.0 {
                e.attack = IDLE;
            }
            0.0
        }
        _ => {
            if !hunt {
                return 0.0;
            }
            // Hold a firing band: close in when far, back off when crowded.
            let flat = Vec3::new(to.x, 0.0, to.z);
            if distance > 16.0 {
                walk(e, world, flat, speed, dt);
            } else if distance < 9.0 {
                walk(e, world, -flat, speed * 0.8, dt);
                face(e, flat);
            } else {
                face(e, flat);
            }
            if e.cooldown <= 0.0 && distance < WALKER_RANGE_M && e.visible {
                e.attack = WINDUP;
                e.tell = WALKER_CHARGE_S;
                e.charge = e.health + e.shield;
                e.aim = to / distance.max(0.1);
                fx.push(Fx::Sound { cue: Cue::Warning, at: muzzle(e), gain: 0.8, pitch: 0.9 });
            }
            0.0
        }
    }
}

/// Projectile contact against the real body shape.
pub(super) fn hit(e: &Enemy, at: Vec3, pad: f32) -> bool {
    match e.kind {
        COLOSSUS => {
            let low = if e.attack == IDLE { 1.2 } else { 3.2 };
            let y = at.y.clamp(e.pos.y, e.pos.y + low);
            (at - Vec3::new(e.pos.x, y, e.pos.z)).length() < 1.3 + pad
        }
        WALKER => (at - (e.pos + Vec3::Y * 2.2)).length() < 1.2 + pad,
        PINE_ROTOR | PALM_ROTOR => {
            // On the stump the whole tree answers a hit, so striking it wakes it.
            let crown = (at - e.center()).length() < 1.1 + pad;
            let trunk = tree_attached(e) && {
                let y = at.y.clamp(e.home.y, e.pos.y);
                (at - Vec3::new(e.home.x, y, e.home.z)).length() < 0.45 + pad
            };
            crown || trunk
        }
        _ => (at - e.center()).length() < 0.9 + pad,
    }
}

/// Each rig draws its body, hands it to the shared motion channel, then adds
/// world-anchored parts (rooted stumps, beams to fixed targets) unposed.
pub(super) fn model(e: &Enemy, m: &mut Model, time: f32) {
    let body = m.mark();
    match e.kind {
        ROTOR => rotor_model(e, m, time),
        BLOOM => bloom_model(e, m, time),
        PINE_ROTOR | PALM_ROTOR => {
            tree_stump(e, m);
            let crown = m.mark();
            tree_rotor_model(e, m, time);
            motion::pose_body(e, m, crown);
            return;
        }
        COLOSSUS => colossus_model(e, m, time),
        _ => {
            walker_model(e, m, time);
            motion::pose_body(e, m, body);
            walker_beam(e, m);
            return;
        }
    }
    motion::pose_body(e, m, body);
}

const METAL: f32 = 59.0;
const GLOW: f32 = 60.0;
const STONE: f32 = 71.0;
const PLANT: f32 = 70.0;

/// Emissive bar from `a` to `b`.
fn glow_beam(m: &mut Model, a: Vec3, b: Vec3, width: f32, color: [f32; 3], energy: f32) {
    let span = b - a;
    let length = span.length();
    if length < 0.01 {
        return;
    }
    let rot = Quat::from_rotation_arc(Vec3::Z, span / length);
    m.cuboid((a + b) * 0.5, Vec3::new(width, width, length), rot, color, energy);
}

fn lit(color: [f32; 3], flash: f32) -> [f32; 3] {
    let k = 1.0 + flash.min(0.2) * 4.0;
    [color[0] * k, color[1] * k, color[2] * k]
}

fn rotor_model(e: &Enemy, m: &mut Model, time: f32) {
    let steel = lit([0.52, 0.55, 0.58], e.flash);
    let dark = [0.14, 0.15, 0.17];
    let edge = element(e);
    let stunned = e.attack == STUN;
    let spin = match e.attack {
        WINDUP => 8.0 + (1.0 - e.tell / ROTOR_WINDUP_S) * 26.0,
        STRIKE => 34.0,
        STUN => 1.5 * (e.tell / ROTOR_STUN_S),
        _ => 8.0,
    };
    let bob = if stunned { 0.0 } else { (time * 2.6 + e.slot as f32).sin() * 0.06 };
    let tilt = if stunned { Quat::from_rotation_x(0.7) } else { Quat::IDENTITY };
    let frame = Quat::from_rotation_y(e.yaw) * tilt;
    let hub = e.pos + Vec3::Y * (0.45 + bob);
    m.material_box(hub, Vec3::new(0.42, 0.3, 0.42), frame, steel, METAL, 0.0);
    m.material_box(hub + frame * Vec3::new(0.0, 0.22, 0.0), Vec3::new(0.16, 0.18, 0.16), frame, dark, METAL, 0.0);
    let angle = time * spin + e.slot as f32;
    for i in 0..4 {
        let q = frame * Quat::from_rotation_y(angle + i as f32 * TAU / 4.0);
        let tip = hub + q * Vec3::new(0.0, 0.0, -0.62);
        m.material_box(tip, Vec3::new(0.08, 0.04, 0.85), q, steel, METAL, 0.0);
        let charged = if e.attack == WINDUP || e.attack == STRIKE { 1.4 } else { 0.4 };
        m.material_box(hub + q * Vec3::new(0.05, 0.0, -0.95), Vec3::new(0.03, 0.03, 0.22), q, edge, GLOW, charged);
    }
    let eye_glow = match e.attack {
        WINDUP => 1.0 + (1.0 - e.tell / ROTOR_WINDUP_S) * 2.0,
        STUN => 0.1,
        _ => 0.7,
    };
    m.material_box(hub + frame * Vec3::new(0.0, 0.0, -0.22), Vec3::new(0.12, 0.08, 0.04), frame, edge, GLOW, eye_glow);
}

fn bloom_model(e: &Enemy, m: &mut Model, time: f32) {
    let leaf = lit([0.26, 0.46, 0.2], e.flash);
    let vein = [0.4, 0.58, 0.28];
    let stalk = [0.32, 0.27, 0.17];
    let steel = lit([0.55, 0.57, 0.6], e.flash);
    let edge = element(e);
    let frame = Quat::from_rotation_y(e.yaw);
    // 0 = rooted bud, 1 = blades level and spinning.
    let open = match e.attack {
        IDLE => 0.0,
        WAKE => 1.0 - e.tell / BLOOM_UNFOLD_S,
        STUN => 0.55 + 0.45 * (e.tell / BLOOM_TIRED_S),
        _ => 1.0,
    };
    let ease = open * open * (3.0 - 2.0 * open);
    let spin = match e.attack {
        IDLE => 0.0,
        WAKE => 4.0 + ease * 28.0,
        STUN => 30.0 * (e.tell / BLOOM_TIRED_S).powi(2),
        _ => 32.0,
    };
    // Idle sway sells the plant; it stops dead when the blades take over.
    let sway = (1.0 - ease) * (time * 1.1 + e.slot as f32).sin() * 0.06;
    let hub = e.pos + Vec3::Y * (0.55 + ease * 0.1);
    // Root ball, roots (hanging once airborne) and stalk.
    m.material_box(e.pos + Vec3::Y * 0.12, Vec3::new(0.34, 0.24, 0.34), frame, stalk, PLANT, 0.0);
    for i in 0..3 {
        let a = i as f32 * TAU / 3.0 + e.slot as f32;
        let tip = e.pos + Vec3::new(a.sin() * 0.25, -0.15 * ease, a.cos() * 0.25);
        m.material_beam(e.pos + Vec3::Y * 0.1, tip, 0.04, stalk, PLANT);
    }
    m.material_beam(e.pos + Vec3::Y * 0.2, hub, 0.07, stalk, PLANT);
    // Closed flower head that squashes open when it wakes.
    let bud = hub + Vec3::Y * (0.32 - ease * 0.15);
    m.material_box(bud, Vec3::new(0.26, 0.3 * (1.0 - ease * 0.5), 0.26), frame * Quat::from_rotation_z(sway), vein, PLANT, 0.0);
    let eye_glow = match e.attack {
        IDLE => 0.0,
        STUN => 0.15,
        _ => 1.2,
    };
    if eye_glow > 0.0 {
        m.material_box(bud + frame * Vec3::new(0.0, 0.0, -0.14), Vec3::new(0.12, 0.05, 0.03), frame, edge, GLOW, eye_glow * ease);
    }
    // Five drooping fronds swing level, steel edges slide out, and the crown
    // spins as a rotor.
    let angle = time * spin + e.slot as f32;
    for i in 0..5 {
        let around = frame * Quat::from_rotation_y(angle + i as f32 * TAU / 5.0);
        let droop = (1.0 - ease) * 0.9 + sway;
        let q = around * Quat::from_rotation_x(-droop);
        let length = 0.9 + ease * 0.35;
        let mid = hub + q * Vec3::new(0.0, 0.0, -length * 0.5);
        m.material_box(mid, Vec3::new(0.26 - ease * 0.12, 0.04, length), q, leaf, PLANT, 0.0);
        if ease > 0.05 {
            let blade = hub + q * Vec3::new(0.09, 0.0, -length * 0.55);
            m.material_box(blade, Vec3::new(0.05, 0.03, length * ease), q, steel, METAL, 0.0);
            let hot = if e.attack == WALK { 1.2 } else { 0.3 };
            m.material_box(hub + q * Vec3::new(0.11, 0.0, -length * 0.95), Vec3::new(0.03, 0.03, 0.2), q, edge, GLOW, hot * ease);
        }
    }
}

fn tree_attached(e: &Enemy) -> bool {
    e.attack == IDLE
        || (e.attack == WAKE && e.pos.distance_squared(e.home + Vec3::Y * crown_m(e.kind)) < 1e-4)
}

pub(super) fn tree_dormant(e: &Enemy) -> bool {
    matches!(e.kind, PINE_ROTOR | PALM_ROTOR) && e.attack == IDLE
}

/// Rooted trunk at `home`: world-anchored, never posed with the flying crown.
fn tree_stump(e: &Enemy, m: &mut Model) {
    let palm = e.kind == PALM_ROTOR;
    let crown_h = crown_m(e.kind);
    let bark = if palm { [0.46, 0.37, 0.25] } else { [0.31, 0.22, 0.15] };
    let ring = if palm { [0.34, 0.27, 0.18] } else { [0.24, 0.17, 0.11] };
    let attached = tree_attached(e);
    // Trunk stays rooted at `home`; once the crown leaves it is a cut stump.
    let stump = if attached { crown_h } else { crown_h - 0.25 };
    let segments = if palm { 9 } else { 6 };
    for i in 0..segments {
        let t0 = i as f32 / segments as f32;
        let t1 = (i + 1) as f32 / segments as f32;
        // Palms bow gently; both ends stay on the trunk line so the crown sits true.
        let bow = |t: f32| if palm { Vec3::X * (t * std::f32::consts::PI).sin() * 0.28 } else { Vec3::ZERO };
        let a = e.home + Vec3::Y * (t0 * stump) + bow(t0);
        let b = e.home + Vec3::Y * (t1 * stump) + bow(t1);
        let taper = 1.0 - t0 * if palm { 0.25 } else { 0.45 };
        let width = if palm { 0.36 } else { 0.42 } * taper;
        m.material_box((a + b) * 0.5, Vec3::new(width, (b - a).length() + 0.02, width), Quat::from_rotation_arc(Vec3::Y, (b - a).normalize_or(Vec3::Y)), bark, PLANT, 0.0);
        if palm {
            m.material_box(b, Vec3::new(width + 0.06, 0.06, width + 0.06), Quat::IDENTITY, ring, PLANT, 0.0);
        }
    }
    if !attached {
        // Pale cut face on the stump.
        m.material_box(e.home + Vec3::Y * stump, Vec3::new(0.3, 0.04, 0.3), Quat::IDENTITY, [0.78, 0.62, 0.4], PLANT, 0.0);
    }
}

fn tree_open(e: &Enemy) -> f32 {
    // 0 = sitting tree, 1 = blades level and spinning.
    let open = match e.attack {
        IDLE => 0.0,
        WAKE => {
            let duration = if tree_attached(e) { TREE_UNFOLD_S } else { TREE_UNFOLD_S * 0.6 };
            1.0 - e.tell / duration
        }
        STUN => e.tell / TREE_TIRED_S,
        STRIKE => 0.6,
        _ => 1.0,
    }.clamp(0.0, 1.0);
    open * open * (3.0 - 2.0 * open)
}

fn tree_rotor_model(e: &Enemy, m: &mut Model, time: f32) {
    let palm = e.kind == PALM_ROTOR;
    let leaf = lit(foliage(e.kind), e.flash);
    let steel = lit([0.55, 0.57, 0.6], e.flash);
    let edge = element(e);
    let frame = Quat::from_rotation_y(unit(e.slot ^ 0x7e3) * TAU);
    let ease = tree_open(e);
    let sway = (1.0 - ease) * (time * 0.9 + e.slot as f32).sin() * 0.05;
    let hub = e.pos;
    let angle = e.phase;
    let hot = if e.attack == WALK { 1.2 } else { 0.3 };
    if palm {
        // Coconuts, then seven arching fronds that swing level into blades.
        let nuts = 1.0 - ease;
        if nuts > 0.05 {
            for i in 0..3 {
                let a = i as f32 * TAU / 3.0 + e.slot as f32;
                m.material_box(hub + Vec3::new(a.sin() * 0.16, -0.18, a.cos() * 0.16), Vec3::splat(0.16 * nuts), Quat::IDENTITY, [0.36, 0.25, 0.13], PLANT, 0.0);
            }
        }
        for i in 0..7 {
            let around = frame * Quat::from_rotation_y(angle + i as f32 * TAU / 7.0);
            let droop = (1.0 - ease) * (0.35 + (i % 2) as f32 * 0.15) + sway;
            let length = 1.9 + ease * 0.2;
            let q = around * Quat::from_rotation_x(-droop);
            let elbow = hub + q * Vec3::new(0.0, 0.0, -length * 0.5);
            // The outer half hangs further while it sits; it stiffens in flight.
            let q2 = around * Quat::from_rotation_x(-droop - (1.0 - ease) * 0.6);
            let tip = elbow + q2 * Vec3::new(0.0, 0.0, -length * 0.5);
            m.material_box((hub + elbow) * 0.5, Vec3::new(0.5 - ease * 0.15, 0.04, length * 0.5), q, leaf, PLANT, 0.0);
            m.material_box((elbow + tip) * 0.5, Vec3::new(0.42 - ease * 0.2, 0.04, length * 0.5), q2, leaf, PLANT, 0.0);
            if ease > 0.05 {
                let strip = length * 0.5 * ease;
                let inner_edge = 0.25 - ease * 0.075;
                let outer_edge = 0.21 - ease * 0.1;
                m.material_box(elbow + q * Vec3::new(inner_edge, -0.01, strip * 0.5), Vec3::new(0.05, 0.03, strip), q, steel, METAL, 0.0);
                m.material_box(elbow + q2 * Vec3::new(outer_edge, -0.01, -strip * 0.5), Vec3::new(0.05, 0.03, strip), q2, steel, METAL, 0.0);
                let marker = strip.min(0.22);
                m.material_box(elbow + q2 * Vec3::new(outer_edge, -0.025, -strip + marker * 0.5), Vec3::new(0.03, 0.03, marker), q2, edge, GLOW, hot * ease);
            }
        }
    } else {
        // Six whorls of branches. Sitting, they hang down the trunk as a cone;
        // awake, they pull up tight under the hub and spin, alternate whorls
        // counter-rotating.
        let tiers = 6;
        for i in 0..tiers {
            let k = i as f32;
            let rest_y = -2.1 + k * 0.5;
            let fly_y = -0.5 + k * 0.16;
            let y = rest_y + (fly_y - rest_y) * ease;
            let length = (1.25 - k * 0.17) + ease * 0.15;
            let droop = (1.0 - ease) * 0.4 + sway;
            let dir = if i % 2 == 0 { 1.0 } else { -1.0 };
            let centre = hub + Vec3::Y * y;
            for b in 0..5 {
                let around = frame * Quat::from_rotation_y(angle * dir + b as f32 * TAU / 5.0 + k * 0.6);
                let q = around * Quat::from_rotation_x(-droop);
                m.material_box(centre + q * Vec3::new(0.0, 0.0, -length * 0.5), Vec3::new(0.42 - ease * 0.18, 0.14 - ease * 0.06, length), q, leaf, PLANT, 0.0);
                if ease > 0.05 && i < 4 {
                    m.material_box(centre + q * Vec3::new(0.12, -0.04, -length * 0.55), Vec3::new(0.05, 0.03, length * ease), q, steel, METAL, 0.0);
                    m.material_box(centre + q * Vec3::new(0.14, -0.04, -length * 0.95), Vec3::new(0.03, 0.03, 0.2), q, edge, GLOW, hot * ease);
                }
            }
        }
        // Dense needle core hides the trunk top while it sits; spire on top.
        let core = 1.0 - ease;
        if core > 0.05 {
            m.material_box(hub + Vec3::Y * -0.8, Vec3::new(0.55, 2.6 * core, 0.55), frame, leaf, PLANT, 0.0);
        }
        m.material_box(hub + Vec3::Y * (0.6 - ease * 0.3), Vec3::new(0.18, 0.5, 0.18), frame, leaf, PLANT, 0.0);
    }
    // Eye under the crown, only once it has woken.
    let eye_glow = match e.attack {
        IDLE => 0.0,
        STUN => 0.15,
        _ => 1.2,
    };
    if eye_glow > 0.0 {
        let facing = Quat::from_rotation_y(e.yaw);
        let head = hub - Vec3::Y * (0.12 + 0.12 * ease);
        m.material_box(head + facing * Vec3::new(0.0, -0.04, -0.18), Vec3::new(0.14, 0.06, 0.04), facing, edge, GLOW, eye_glow * ease);
        m.material_box(head, Vec3::new(0.34, 0.24, 0.34), facing, steel, METAL, 0.0);
    }
}

fn colossus_model(e: &Enemy, m: &mut Model, time: f32) {
    let rock = lit([0.5, 0.47, 0.43], e.flash);
    let moss = [0.34, 0.42, 0.26];
    let core = element(e);
    let frame = Quat::from_rotation_y(e.yaw);
    let at = |x: f32, y: f32, z: f32| e.pos + frame * Vec3::new(x, y, z);
    // Rise from the pile: 0 = dormant boulders, 1 = standing.
    let rise = match e.attack {
        IDLE => 0.0,
        WAKE => 1.0 - e.tell / COLOSSUS_RISE_S,
        _ => 1.0,
    };
    let ease = rise * rise * (3.0 - 2.0 * rise);
    let lerp = |a: Vec3, b: Vec3| a + (b - a) * ease;
    // Fists rise during the windup and fall on the slam.
    let lift = if e.attack == WINDUP { 1.0 - e.tell / COLOSSUS_WINDUP_S } else { 0.0 };
    let grip = heave_hold(e);
    let (stride_l, stride_r) = ((e.phase * 1.6).sin() * 0.25, -(e.phase * 1.6).sin() * 0.25);
    let pile = [(0.0, 0.5, 0.0), (0.8, 0.35, 0.3), (-0.7, 0.4, -0.2), (0.3, 0.3, -0.8), (-0.4, 0.95, 0.4), (0.6, 0.85, -0.3)];
    let stand = [
        (0.0, 1.9, 0.0),                         // torso
        (0.0, 3.0, -0.15),                       // head
        (1.25, 2.1 + lift * 1.4, -0.3 - lift * 0.3), // right fist
        (-1.25, 2.1 + lift * 1.4, -0.3 - lift * 0.3), // left fist
        (0.55, 0.55, stride_l),                  // right leg
        (-0.55, 0.55, stride_r),                 // left leg
    ];
    let sizes = [Vec3::new(1.9, 1.5, 1.3), Vec3::new(0.9, 0.75, 0.85), Vec3::splat(0.85), Vec3::splat(0.85), Vec3::new(0.7, 1.1, 0.75), Vec3::new(0.7, 1.1, 0.75)];
    for i in 0..6 {
        let (px, py, pz) = pile[i];
        let (sx, sy, sz) = stand[i];
        let mut c = lerp(at(px, py, pz), at(sx, sy, sz));
        // Heaving: both fists grip the chunk wherever it is.
        if let (Some(g), 2 | 3) = (grip, i) {
            c = g + frame * Vec3::new(if i == 2 { 0.75 } else { -0.75 }, 0.0, 0.0);
        }
        let size = sizes[i] * (0.8 + ease * 0.2);
        m.material_box(c, size, frame * Quat::from_rotation_z((i as f32 * 1.7).sin() * 0.15 * (1.0 - ease)), rock, STONE, 0.0);
        if i < 2 {
            m.material_box(c + Vec3::Y * size.y * 0.5, Vec3::new(size.x * 0.8, 0.08, size.z * 0.7), frame, moss, STONE, 0.0);
        }
    }
    if ease > 0.0 {
        // Shoulders and arms join the fists to the torso.
        for side in [-1.0f32, 1.0] {
            let shoulder = at(side * 0.95, 2.4, 0.0);
            let fist = match grip {
                Some(g) => g + frame * Vec3::new(side * 0.75, 0.0, 0.0),
                None => at(side * 1.25, 2.1 + lift * 1.4, -0.3 - lift * 0.3),
            };
            m.material_beam(shoulder, lerp(shoulder, fist), 0.3 * ease, rock, STONE);
        }
        let eyes = if e.attack == STUN { 0.2 } else { 0.9 };
        m.material_box(at(0.0, 3.05, -0.6), Vec3::new(0.5, 0.1, 0.05), frame, core, GLOW, eyes * ease);
    }
    // The core on its back; hidden in the pile, bright and pulsing only while exposed.
    if ease <= 0.0 {
        return;
    }
    let open = e.attack == STUN;
    let pulse = if open { 2.4 + (time * 10.0).sin() * 0.6 } else { 0.25 };
    let core_at = lerp(at(0.0, 1.0, 0.5), at(0.0, 2.1, 0.68));
    m.material_box(core_at, Vec3::new(0.5, 0.55, 0.12), frame, core, GLOW, pulse);
}

fn walker_model(e: &Enemy, m: &mut Model, time: f32) {
    let shell = lit([0.46, 0.43, 0.38], e.flash);
    let trim = [0.62, 0.48, 0.24];
    let dark = [0.12, 0.12, 0.13];
    let beam = element(e);
    let frame = Quat::from_rotation_y(e.yaw);
    let at = |x: f32, y: f32, z: f32| e.pos + frame * Vec3::new(x, y, z);
    let sway = (time * 1.3 + e.slot as f32).sin() * 0.04;
    let body = at(0.0, 2.2 + sway, 0.0);
    m.material_box(body, Vec3::new(1.5, 0.7, 1.5), frame, shell, METAL, 0.0);
    m.material_box(body + Vec3::Y * 0.5, Vec3::new(1.05, 0.4, 1.05), frame, shell, METAL, 0.0);
    m.material_box(body + Vec3::Y * 0.78, Vec3::new(0.6, 0.18, 0.6), frame, trim, METAL, 0.0);
    m.material_box(body - Vec3::Y * 0.42, Vec3::new(1.2, 0.16, 1.2), frame, dark, METAL, 0.0);
    // Six legs, alternating tripods.
    for i in 0..6 {
        let a = i as f32 * TAU / 6.0 + TAU / 12.0;
        let dir = Vec3::new(a.sin(), 0.0, a.cos());
        let phase = e.phase * 2.4 + if i % 2 == 0 { 0.0 } else { std::f32::consts::PI };
        let lift = phase.sin().max(0.0) * 0.35;
        let hip = body + frame * (dir * 0.7);
        let foot = e.pos + frame * (dir * 1.8 + Vec3::new(0.0, lift, phase.cos() * 0.25));
        let knee = (hip + foot) * 0.5 + Vec3::Y * 0.9;
        m.material_beam(hip, knee, 0.1, shell, METAL);
        m.material_beam(knee, foot, 0.08, dark, METAL);
    }
    let charging = e.attack == WINDUP;
    let charge = if charging { 1.0 - e.tell / WALKER_CHARGE_S } else { 0.0 };
    let lens = match e.attack {
        STUN => 0.05,
        WINDUP => 0.8 + charge * 3.0,
        _ => 0.6,
    };
    m.material_box(muzzle(e), Vec3::new(0.3, 0.3, 0.1), frame, beam, GLOW, lens);
}

/// Sight line and shot trail: start at the posed lens, end at fixed world points.
fn walker_beam(e: &Enemy, m: &mut Model) {
    let beam = element(e);
    let charging = e.attack == WINDUP;
    let charge = if charging { 1.0 - e.tell / WALKER_CHARGE_S } else { 0.0 };
    let from = motion::pose_point(e, muzzle(e));
    if charging {
        // Thin sight line: tracks, then freezes at the lock.
        let width = if e.tell > WALKER_LOCK_S { 0.015 } else { 0.035 };
        glow_beam(m, from, from + e.aim * 30.0, width, beam, 1.5 + charge * 2.0);
    } else if e.recovery > 0.0 {
        glow_beam(m, from, e.lunge, 0.12 * (e.recovery / 0.25), beam, 4.0);
    }
}
