//! Readable enemy perception: an awareness meter fed by sight and noise, a
//! search after losing the player, and the marker that shows all of it.
//!
//! Fun contract (docs/superpowers/specs/2026-11-14-enemy-perception-stealth-design.md):
//! outside point-blank range a hostile is always visibly *suspicious* before it
//! *spots* the player; darkness, crouching, stillness and staying behind it all
//! slow detection; noise lures but never spots on its own.
use super::*;

/// Awareness gained per second by a fully lit, moving player straight ahead at
/// point-blank distance. Every factor below scales this down.
const SIGHT_RATE: f32 = 1.7;
/// Inside this many metres a facing hostile notices at once.
const POINT_BLANK_M: f32 = 2.2;
/// Ambushers wait for the player to come closer before they give up the disguise.
const AMBUSH_POINT_BLANK_M: f32 = 1.6;
/// Per-second loss without stimulus; slow enough that suspicion can be read.
const DECAY: f32 = 0.22;
/// Above this the body stops wandering and goes to look.
pub(super) const SUSPICIOUS: f32 = 0.3;
/// Hearing alone never crosses into a full sighting.
const HEARING_CAP: f32 = 0.95;
/// Noise through solid blocks carries this fraction of its radius.
const MUFFLED: f32 = 0.55;
/// Search after losing the player: seconds, number of probe points, probe radius.
const SEARCH_S: f32 = 7.0;
const SEARCH_STEPS: u8 = 3;
const SEARCH_RADIUS_M: f32 = 3.0;
/// Seconds the red marker stays after a fresh sighting.
const SPOTTED_FLASH_S: f32 = 1.0;
/// Cadence of the player's body noise (steps, casting).
pub(super) const BODY_NOISE_S: f32 = 0.35;
/// Noise radii in metres.
pub(super) const WALK_M: f32 = 3.0;
pub(super) const SPRINT_M: f32 = 7.0;
pub(super) const CAST_M: f32 = 10.0;
pub(super) const MELEE_M: f32 = 8.0;
pub(super) const IMPACT_M: f32 = 9.0;
/// `make_noise(level)` (block breaking) maps a 0..1 level to this many metres.
pub(super) const LEVEL_M: f32 = 13.0;

/// One audible event this tick, in world metres.
#[derive(Clone, Copy)]
pub(super) struct Noise {
    pub pos: Vec3,
    pub radius: f32,
}

/// What the player looks and sounds like this tick, shared by every hostile.
pub(super) struct Senses {
    /// 0 (pitch dark) to 1 (daylight or beside a torch) at the player's eye.
    pub light: f32,
    pub crouched: bool,
    pub still: bool,
    pub sprinting: bool,
    pub casting: bool,
    /// False when the game mode hides the player from mobs (creative).
    pub noticeable: bool,
}

/// Bodies with their own senses (burrowers' ruckus, field foes) skip this meter.
fn own_senses(kind: u32) -> bool {
    burrowers::is_kind(kind) || field_foes::is_kind(kind)
}

impl Senses {
    fn exposure(&self) -> f32 {
        let posture = if self.crouched { 0.45 } else { 1.0 }
            * if self.still { 0.6 } else { 1.0 }
            * if self.sprinting { 1.6 } else { 1.0 }
            * if self.casting { 1.5 } else { 1.0 };
        (0.25 + 0.75 * self.light) * posture
    }
}

/// Light at the eye from placed lights and the sky. Roofs and caves block the sky.
pub(super) fn light_at(world: &World, night: &mut super::night::State, eye: Vec3) -> f32 {
    let s = world.voxel_scale.meters();
    let cell = (eye / s).floor().as_ivec3();
    let block = crate::world::block_light::level_at(world, cell) as f32 / 15.0;
    let sky = if super::night::sheltered(night, world, cell) {
        0.0
    } else if crate::world::bed::is_night(world.sandbox.climate.time_of_day()) {
        0.35
    } else {
        1.0
    };
    block.max(sky)
}

/// Facing factor: full in front, reduced to the side, small behind.
fn cone(e: &Enemy, to_player: Vec3) -> f32 {
    let flat_forward = Vec3::new(-e.yaw.sin(), 0.0, -e.yaw.cos());
    let up = if e.up.length_squared() > 0.5 { e.up } else { Vec3::Y };
    let forward = Quat::from_rotation_arc(Vec3::Y, up) * flat_forward;
    let along = forward.dot(to_player.normalize_or_zero());
    if along > 0.5 { 1.0 } else if along > -0.2 { 0.55 } else { 0.2 }
}

fn distance_factor(distance: f32, range: f32) -> f32 {
    let span = (range - POINT_BLANK_M).max(0.1);
    ((range - distance) / span).clamp(0.0, 1.0).powf(1.3) + 0.15
}

/// Mark the enemy as having found the player right now.
fn spot(e: &mut Enemy, feet: Vec3) -> bool {
    let fresh = !e.visible && e.aware < 1.0;
    e.visible = true;
    e.aware = 1.0;
    e.last_seen = feet;
    e.alert = 4.0;
    e.investigate = None;
    e.search = 0.0;
    if fresh {
        e.spotted_flash = SPOTTED_FLASH_S;
    }
    fresh
}

/// One sight check for an ordinary hostile. `los` already includes the range
/// test and the mobs-can-see rule. Returns true on the transition to spotted.
pub(super) fn see(e: &mut Enemy, senses: &Senses, eye: Vec3, feet: Vec3,
    distance: f32, range: f32, los: bool, interval: f32) -> bool {
    // Decay pauses while the body is looking at the player.
    e.glimpsed = los;
    if !los {
        e.visible = false;
        return false;
    }
    // Bosses own their arenas, a hunter that still sees its quarry keeps it, and
    // a body that was just hit (aware already full) finds its attacker at once.
    if is_boss(e.kind) || e.visible || e.aware >= 1.0 || e.provoked && e.alert > 0.0 {
        return spot(e, feet);
    }
    let facing = cone(e, eye - e.center());
    if distance < POINT_BLANK_M && facing > 0.3 {
        return spot(e, feet);
    }
    e.aware += SIGHT_RATE * facing * distance_factor(distance, range) * senses.exposure() * interval;
    if e.aware >= 1.0 {
        return spot(e, feet);
    }
    if e.aware >= SUSPICIOUS {
        // Glimpsed: go and look where the shape was.
        e.investigate = Some(feet);
    }
    e.visible = false;
    false
}

/// Awareness growth for a disguised ambusher, which watches in every direction
/// but waits for the player to come close. True when it should drop the disguise.
pub(super) fn watch(e: &mut Enemy, senses: &Senses, distance: f32, los: bool, dt: f32) -> bool {
    if los && senses.noticeable {
        if distance < AMBUSH_POINT_BLANK_M {
            return true;
        }
        e.aware += SIGHT_RATE * distance_factor(distance, 6.0) * senses.exposure() * dt;
    }
    // Hearing caps below 1, so a loud noise close by also counts as waking.
    e.aware >= 0.9
}

/// Feed this tick's noises to one hostile that cannot currently see the player.
pub(super) fn hear(e: &mut Enemy, world: &World, noises: &[Noise]) {
    if e.visible || is_boss(e.kind) || own_senses(e.kind) {
        return;
    }
    let ear = e.center();
    for noise in noises {
        let mut radius = noise.radius;
        let distance = ear.distance(noise.pos);
        if distance > radius {
            continue;
        }
        if !line(world, ear, noise.pos) {
            radius *= MUFFLED;
            if distance > radius {
                continue;
            }
        }
        let gain = 0.25 + 0.6 * (1.0 - distance / radius);
        if e.aware < HEARING_CAP {
            e.aware = (e.aware + gain).min(HEARING_CAP);
        }
        e.investigate = Some(noise.pos);
    }
}

/// Awareness fades while nothing new arrives; the red flash runs out.
pub(super) fn decay(e: &mut Enemy, dt: f32) {
    e.spotted_flash = (e.spotted_flash - dt).max(0.0);
    if !e.visible && !e.glimpsed {
        e.aware = (e.aware - DECAY * dt).max(0.0);
    }
}

/// A hit always reveals the attacker's position.
pub(super) fn struck(e: &mut Enemy, feet: Vec3) {
    if e.aware < 1.0 && !e.visible {
        e.spotted_flash = SPOTTED_FLASH_S;
    }
    e.aware = 1.0;
    e.last_seen = feet;
    e.alert = e.alert.max(4.0);
    e.investigate = None;
}

/// Unaware targets take sneak-attack damage. Only bodies that run this meter
/// qualify: burrowers and field foes sense the world their own way.
pub(super) fn unaware(e: &Enemy) -> bool {
    !e.visible && e.aware < SUSPICIOUS && !is_boss(e.kind) && e.hostile() && !own_senses(e.kind)
}

/// Positional hash for search probes: no RNG, no wall clock.
fn probe_offset(e: &Enemy) -> Vec3 {
    let mut h = (e.site.x as u32).wrapping_mul(0x9E37_79B1)
        ^ (e.site.y as u32).wrapping_mul(0x85EB_CA77)
        ^ (e.site.z as u32).wrapping_mul(0xC2B2_AE3D)
        ^ e.slot.wrapping_mul(0x27D4_EB2F)
        ^ (e.search_step as u32).wrapping_mul(0x1656_67B1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 13;
    // Successive probes swing roughly a third of a turn so the sweep covers ground.
    let angle = (h & 0xFFFF) as f32 / 65536.0 * std::f32::consts::TAU
        + e.search_step as f32 * 2.1;
    let radius = SEARCH_RADIUS_M * (0.6 + ((h >> 16) & 0xFF) as f32 / 255.0 * 0.4);
    Vec3::new(angle.cos(), 0.0, angle.sin()) * radius
}

fn reached(a: Vec3, b: Vec3, within: f32) -> bool {
    let d = a - b;
    d.x * d.x + d.z * d.z < within * within
}

/// Where a hostile that cannot see the player goes, and at what fraction of its
/// speed. `None` returns it to its home wander.
pub(super) fn pursue(e: &mut Enemy, dt: f32) -> Option<(Vec3, f32)> {
    if e.visible {
        return None;
    }
    if e.alert > 0.0 {
        // Fresh trail: run to where the player was last seen, then sweep.
        e.search = SEARCH_S;
        e.search_step = 0;
        if reached(e.pos, e.last_seen, 1.0) {
            e.alert = 0.0;
        } else {
            return Some((e.last_seen, 1.0));
        }
    }
    if e.search > 0.0 {
        e.search -= dt;
        if e.search_step < SEARCH_STEPS {
            let probe = e.last_seen + probe_offset(e);
            let per_step = SEARCH_S / SEARCH_STEPS as f32;
            let step_deadline = SEARCH_S - per_step * (e.search_step as f32 + 1.0);
            if reached(e.pos, probe, 0.8) || e.search < step_deadline {
                e.search_step += 1;
            }
            return Some((probe, 0.65));
        }
        e.search = 0.0;
    }
    if e.aware >= SUSPICIOUS
        && let Some(spot) = e.investigate
    {
        if reached(e.pos, spot, 1.0) {
            // Stand and look around; awareness decays from here.
            e.investigate = None;
            return None;
        }
        return Some((spot, 0.5));
    }
    None
}

/// Marker over the head: amber while suspicious, a dim pulse while searching,
/// red for a moment after a sighting. Presentation only.
pub(super) fn marker(e: &Enemy, m: &mut Model, time: f32) {
    if e.health <= 0.0 || e.encounter.concealed || is_boss(e.kind)
        || wildlife::passive(e.kind) || own_senses(e.kind) || !e.hostile()
    {
        return;
    }
    let (color, size, glow) = if e.spotted_flash > 0.0 {
        let t = e.spotted_flash / SPOTTED_FLASH_S;
        ([1.0, 0.22, 0.14], 0.07 + 0.05 * t, 1.0)
    } else if e.visible {
        return;
    } else if e.search > 0.0 || e.alert > 0.0 {
        let pulse = 0.5 + 0.5 * (time * 4.0 + e.slot as f32).sin();
        ([0.95, 0.62, 0.22], 0.05, 0.35 + 0.3 * pulse)
    } else if e.aware >= 0.08 {
        let a = (e.aware / 1.0).clamp(0.0, 1.0);
        ([1.0, 0.74, 0.22], 0.035 + 0.06 * a, 0.4 + 0.6 * a)
    } else {
        return;
    };
    let up = if e.up.length_squared() > 0.5 { e.up } else { Vec3::Y };
    let head = e.center() + (e.center() - e.pos).dot(up).max(0.3) * up;
    let bob = (time * 2.2 + e.slot as f32).sin() * 0.04;
    let at = head + up * (0.38 + bob);
    let spin = Quat::from_axis_angle(up, time * 1.6) * Quat::from_rotation_arc(Vec3::Y, up)
        * Quat::from_axis_angle(Vec3::new(1.0, 0.0, 1.0).normalize(), std::f32::consts::FRAC_PI_4 * 1.2);
    m.glow_voxel(at, Vec3::splat(size), spin, color, glow, 0.9);
}
