//! Secondary body motion shared by every hostile rig: attitude (lean, bank,
//! aircraft tilt), turn follow-through and landing squash.
//!
//! The channel is measured from the body's own travel rather than read from AI
//! state, so any behaviour — scripted dash, navigation walk, knockback, boss
//! leap or a multiplayer replica — animates the same way without per-kind code.
//! It is presentation only: AI, collision, reach and damage never read it.
//!
//! Attitude comes from one physical rule. A body that holds itself up must push
//! against gravity plus whatever acceleration and drag it is fighting, so its
//! up axis leans toward `g·up + lean·(a + drag·v)`. Aircraft use the full rule
//! (a rotor disc tips into the direction it flies and banks through turns);
//! walkers use a small fraction (they lean into starts and turns). When the
//! needed thrust vanishes the body is falling and the attitude relaxes.
use super::super::{Enemy, bestiary, flying, is_boss, tall};
use crate::mesh::models::Model;
use glam::{Quat, Vec3};
use std::f32::consts::{PI, TAU};

const GRAVITY: f32 = 9.8;
/// Displacement per tick above this is a teleport, respawn or replica snap.
const SNAP_MPS: f32 = 45.0;

/// How a body carries itself. Derived from the species row, so new kinds get
/// sensible motion without code; heavier bodies respond slower and lean less.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rig {
    /// Share of the thrust-tilt rule applied: 1 is full aircraft attitude.
    pub lean: f32,
    /// Speed-proportional drag (1/s) that holds a cruise attitude.
    pub drag: f32,
    /// Largest attitude angle in radians.
    pub max_tilt: f32,
    /// Attitude spring frequency (rad/s), critically damped.
    pub stiffness: f32,
    /// Turn follow-through spring frequency (rad/s).
    pub turn: f32,
    /// Largest visual yaw lag behind the simulated facing, in radians.
    pub max_turn_lag: f32,
    /// Peak squash per m/s of landing speed.
    pub squash: f32,
    /// Pivot at the body centre (flyers) rather than the feet.
    pub centred: bool,
}

impl Rig {
    pub(crate) const AIRCRAFT: Rig = Rig {
        lean: 1.0,
        drag: 0.6,
        max_tilt: 0.62,
        stiffness: 7.0,
        turn: 9.0,
        max_turn_lag: 0.7,
        squash: 0.0,
        centred: true,
    };
    pub(crate) const GROUND: Rig = Rig {
        lean: 0.14,
        drag: 0.0,
        max_tilt: 0.12,
        stiffness: 10.0,
        turn: 15.0,
        max_turn_lag: 0.55,
        squash: 0.018,
        centred: false,
    };

    pub(crate) fn of(e: &Enemy) -> Rig {
        let (base, height) = match bestiary::species(e.kind) {
            Some(s) => (if s.flying { Self::AIRCRAFT } else { Self::GROUND }, s.body_height),
            None if flying(e.kind) => (Self::AIRCRAFT, 1.0),
            None => (Self::GROUND, if is_boss(e.kind) { 3.0 } else if tall(e.kind) { 1.9 } else { 1.0 }),
        };
        base.weighted(height)
    }

    /// Scales response by body size: a 1.8 m body is the reference.
    fn weighted(self, height: f32) -> Rig {
        let mass = (height / 1.8).clamp(0.5, 3.0);
        let k = mass.sqrt().recip();
        Rig {
            lean: self.lean * k.min(1.0),
            stiffness: self.stiffness * k,
            turn: self.turn * k,
            squash: self.squash * k,
            ..self
        }
    }
}

/// Per-body secondary motion state, advanced once per simulation tick.
#[derive(Clone, Debug, Default)]
pub(crate) struct BodyMotion {
    last: Option<Vec3>,
    last_yaw: f32,
    velocity: Vec3,
    accel: Vec3,
    /// Attitude as a world-space rotation vector (axis × angle).
    tilt: Vec3,
    tilt_rate: Vec3,
    /// Shown yaw minus simulated yaw.
    lag: f32,
    lag_rate: f32,
    squash: f32,
    squash_rate: f32,
}

impl BodyMotion {
    pub(crate) fn advance(&mut self, pos: Vec3, yaw: f32, up: Vec3, rig: &Rig, dt: f32) {
        if !(dt.is_finite() && dt > 0.0) || !pos.is_finite() {
            return;
        }
        let Some(last) = self.last.filter(|l| l.distance(pos) <= SNAP_MPS * dt + 0.5) else {
            *self = Self { last: Some(pos), last_yaw: yaw, ..Self::default() };
            return;
        };
        self.last = Some(pos);
        let up = if up.length_squared() > 0.5 { up.normalize() } else { Vec3::Y };

        // Measured travel, low-passed so blocked sub-steps don't spike.
        let measured = (pos - last) / dt;
        let previous = self.velocity;
        self.velocity = previous.lerp(measured, blend(dt, 0.06));
        let accel = (self.velocity - previous) / dt;
        self.accel = self.accel.lerp(accel, blend(dt, 0.10));

        // Attitude target from the thrust rule.
        let vertical = self.accel.dot(up);
        let lateral = |v: Vec3| v - up * v.dot(up);
        let push = lateral(self.accel + self.velocity * rig.drag) * rig.lean;
        let target = if GRAVITY + vertical < GRAVITY * 0.3 {
            Vec3::ZERO // free fall: nothing to push against
        } else {
            let thrust = (up * GRAVITY + push).normalize_or(up);
            let axis = up.cross(thrust);
            let sin = axis.length();
            if sin < 1e-5 { Vec3::ZERO } else { axis / sin * sin.asin().min(rig.max_tilt) }
        };
        spring(&mut self.tilt, &mut self.tilt_rate, target, rig.stiffness, dt);

        // Turn follow-through: the sim snaps facing, the body catches up.
        let turned = wrap(yaw - self.last_yaw);
        self.last_yaw = yaw;
        self.lag = (self.lag - turned).clamp(-rig.max_turn_lag, rig.max_turn_lag);
        let (mut lag, mut rate) = (Vec3::X * self.lag, Vec3::X * self.lag_rate);
        spring(&mut lag, &mut rate, Vec3::ZERO, rig.turn, dt);
        (self.lag, self.lag_rate) = (lag.x, rate.x);

        // Landing: vertical speed stops abruptly → squash, then recover.
        let fall = -previous.dot(up);
        let now = -self.velocity.dot(up);
        if rig.squash > 0.0 && fall > 2.5 && now < fall * 0.35 {
            let peak = ((fall - 2.5) * rig.squash).min(0.22);
            let omega = 16.0;
            self.squash_rate += peak * omega * std::f32::consts::E;
        }
        let (mut s, mut r) = (Vec3::X * self.squash, Vec3::X * self.squash_rate);
        spring(&mut s, &mut r, Vec3::ZERO, 16.0, dt);
        (self.squash, self.squash_rate) = (s.x.clamp(-0.1, 0.3), r.x);
    }

    fn idle(&self) -> bool {
        self.tilt.length_squared() < 1e-8 && self.lag.abs() < 1e-4 && self.squash.abs() < 1e-4
    }

    fn rotation(&self, up: Vec3) -> Quat {
        Quat::from_scaled_axis(self.tilt) * Quat::from_axis_angle(up, self.lag)
    }
}

fn local_up(e: &Enemy) -> Vec3 {
    if e.up.length_squared() > 0.5 { e.up.normalize() } else { Vec3::Y }
}

fn pivot(e: &Enemy, rig: &Rig) -> Vec3 {
    if rig.centred { e.center() } else { e.pos }
}

/// Advances the channel from where the body ended up last tick.
pub(crate) fn advance(e: &mut Enemy, dt: f32) {
    // Review A/B: `VOXEL_REVIEW_NO_MOTION` leaves every channel idle (unposed).
    static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *OFF.get_or_init(|| std::env::var_os("VOXEL_REVIEW_NO_MOTION").is_some()) {
        return;
    }
    let rig = Rig::of(e);
    let (pos, yaw, up) = (e.pos, e.yaw, local_up(e));
    e.motion.advance(pos, yaw, up, &rig, dt);
}

/// Poses body vertices drawn since `mark`. Rigs call this once, after their
/// body and before any world-anchored overlay (floor marks, beams to targets,
/// rooted stumps), so overlays keep their true positions.
pub(crate) fn pose_body(e: &Enemy, m: &mut Model, mark: usize) {
    if e.motion.idle() {
        return;
    }
    let rig = Rig::of(e);
    let up = local_up(e);
    if e.motion.squash.abs() > 1e-4 {
        let s = e.motion.squash;
        let to_y = Quat::from_rotation_arc(up, Vec3::Y);
        m.rotate_since(mark, e.pos, to_y);
        m.scale_since(mark, e.pos, Vec3::new(1.0 + s * 0.5, 1.0 - s, 1.0 + s * 0.5));
        m.rotate_since(mark, e.pos, to_y.inverse());
    }
    m.rotate_since(mark, pivot(e, &rig), e.motion.rotation(up));
}

/// Same transform for a single point, so emitters and muzzles match the body.
pub(crate) fn pose_point(e: &Enemy, point: Vec3) -> Vec3 {
    if e.motion.idle() {
        return point;
    }
    let rig = Rig::of(e);
    let up = local_up(e);
    let mut p = point;
    if e.motion.squash.abs() > 1e-4 {
        let s = e.motion.squash;
        let to_y = Quat::from_rotation_arc(up, Vec3::Y);
        let local = to_y * (p - e.pos) * Vec3::new(1.0 + s * 0.5, 1.0 - s, 1.0 + s * 0.5);
        p = e.pos + to_y.inverse() * local;
    }
    let c = pivot(e, &rig);
    c + e.motion.rotation(up) * (p - c)
}

/// Frame-rate independent exponential blend factor for time constant `tau`.
fn blend(dt: f32, tau: f32) -> f32 {
    1.0 - (-dt / tau).exp()
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

/// Exact critically damped spring step; stable for any `dt`.
fn spring(x: &mut Vec3, v: &mut Vec3, target: Vec3, omega: f32, dt: f32) {
    let decay = (-omega * dt).exp();
    let offset = *x - target;
    let j = *v + offset * omega;
    *x = target + (offset + j * dt) * decay;
    *v = (*v - j * (omega * dt)) * decay;
}

