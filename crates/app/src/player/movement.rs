//! Fast traversal layered on the walk controller, after Mycopunk's movement
//! kit: slide, slide-jump, dash / air dash, air jumps and carried momentum.
//!
//! Tunables are metres and seconds; `update_player_local` converts them with
//! `speed_blocks` / `acceleration_blocks` so every voxel scale feels the same.
//! Below walk speed nothing here changes the old controller: `carry_momentum`
//! is the previous lerp exactly, so ordinary walking is bit-identical.
use glam::Vec2;

/// Horizontal speed needed to start a slide (sprint is 8.5, walk 5.5).
pub const SLIDE_MIN_SPEED: f32 = 6.0;
/// Speed added when a slide starts.
pub const SLIDE_BOOST: f32 = 3.0;
/// A slide start never pushes speed past this (it keeps a faster entry).
pub const SLIDE_MAX_SPEED: f32 = 14.0;
/// Exponential slide friction, 1/s. Ice multiplies it by its grip.
pub const SLIDE_FRICTION: f32 = 1.6;
/// The slide ends below this speed.
pub const SLIDE_END_SPEED: f32 = 3.5;
pub const SLIDE_MAX_TIME: f32 = 1.1;
pub const SLIDE_COOLDOWN: f32 = 0.35;
/// How fast the slide direction turns toward the held direction, 1/s.
pub const SLIDE_STEER: f32 = 2.5;
/// Forward speed a jump out of a slide adds.
pub const SLIDE_JUMP_FORWARD: f32 = 2.5;

pub const DASH_SPEED: f32 = 16.0;
pub const DASH_TIME: f32 = 0.16;
pub const DASH_COOLDOWN: f32 = 0.75;
/// Dashes available per landing; one ground dash or one air dash.
pub const DASH_CHARGES: u8 = 1;

/// Extra jumps per airtime.
pub const AIR_JUMPS: u8 = 1;
/// Air jump launch speed as a fraction of the ground jump.
pub const AIR_JUMP_FACTOR: f32 = 0.9;

/// Horizontal speed cap by gear. Bare legs top out at a slide-jump; greaves
/// allow a full dash; boots allow strafe and wall-run chains (Quake, Titanfall).
pub const BASE_SPEED_CAP: f32 = 14.0;
pub const THRUSTER_SPEED_CAP: f32 = 17.0;
pub const MOMENTUM_SPEED_CAP: f32 = 24.0;

pub fn speed_cap(gear: crate::world::inventory::Mobility) -> f32 {
    if gear.wallrun {
        MOMENTUM_SPEED_CAP
    } else if gear.thrusters {
        THRUSTER_SPEED_CAP
    } else {
        BASE_SPEED_CAP
    }
}

// Wall run (Titanfall): kinetic boots, airborne, holding forward, fast, with a
// wall alongside. Gravity is nearly cancelled for a while, then returns.
pub const WALL_PROBE: f32 = 0.25;
pub const WALLRUN_MIN_SPEED: f32 = 6.0;
pub const WALLRUN_END_SPEED: f32 = 4.0;
pub const WALLRUN_SPEED: f32 = 10.5;
pub const WALLRUN_ACCEL: f32 = 6.0;
pub const WALLRUN_MAX_TIME: f32 = 1.75;
/// Low-gravity phase before gravity ramps back to full at `WALLRUN_MAX_TIME`.
pub const WALLRUN_HOLD: f32 = 0.9;
pub const WALLRUN_GRAVITY: f32 = 0.12;
/// Upward speed given when a run starts, so catching a wall feels like a lift.
pub const WALLRUN_LIFT: f32 = 1.5;
/// A body falling faster than this slips past the wall instead of catching it.
pub const WALLRUN_MAX_FALL: f32 = 9.0;
pub const WALLRUN_COOLDOWN: f32 = 0.3;
/// Speed away from the wall a wall jump adds.
pub const WALLJUMP_PUSH: f32 = 6.5;

// Air strafing (Quake / Source `PM_AirAccelerate`): above move speed in the
// air, acceleration is only added along the wish direction up to this small
// wish speed, so turning with the mouse while strafing gains speed.
pub const AIR_STRAFE_WISH: f32 = 0.9;
pub const AIR_STRAFE_ACCEL: f32 = 12.0;
/// Camera lean away from the wall while running it, radians (Titanfall).
pub const WALLRUN_ROLL: f32 = 0.16;
/// Above walk speed, how fast carried speed bleeds off, m/s².
pub const AIR_MOMENTUM_DECAY: f32 = 1.5;
pub const GROUND_MOMENTUM_DECAY: f32 = 18.0;

/// Extra field of view, degrees, while dashing / sliding.
pub const DASH_FOV_KICK: f32 = 8.0;
pub const SLIDE_FOV_KICK: f32 = 4.0;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Movement {
    pub sliding: bool,
    pub slide_time: f32,
    pub slide_cooldown: f32,
    /// Remaining dash time; velocity is held while it is positive.
    pub dash_time: f32,
    pub dash_cooldown: f32,
    pub dash_charges: u8,
    /// The running dash started in the air, so gravity is suspended.
    pub air_dash: bool,
    pub air_jumps_used: u8,
    pub sneak_was_down: bool,
    fov_kick: f32,
    /// Camera roll in radians: leans away from a wall being run.
    roll: f32,
    /// One-shot cues raised this step (`EVENT_*` bits), drained by
    /// `take_events` into the sound queue after the player update.
    events: u8,
    /// Fastest downward speed (m/s) during the current airtime, for landings.
    pub fall_speed: f32,
    pub wallrunning: bool,
    pub wallrun_time: f32,
    pub wallrun_cooldown: f32,
    /// Horizontal normal of the wall being run, pointing away from it.
    pub wall_normal: Vec2,
}

pub const EVENT_DASH: u8 = 1;
pub const EVENT_SLIDE: u8 = 2;
pub const EVENT_AIR_JUMP: u8 = 4;
pub const EVENT_LAND: u8 = 8;
pub const EVENT_WALLRUN: u8 = 16;
pub const EVENT_WALLJUMP: u8 = 32;
/// Slime rebound off a floor or wall.
pub const EVENT_BOUNCE: u8 = 64;
/// Slime sling release; the snap variant is a release in the full-stretch window.
pub const EVENT_SLING: u8 = 128;
pub const EVENT_SLING_SNAP: u8 = EVENT_SLING | EVENT_AIR_JUMP;
/// Downward speed (m/s) above which a landing is heavy enough to thump.
pub const HARD_LANDING_SPEED: f32 = 11.0;

impl Default for Movement {
    fn default() -> Self {
        Self::new()
    }
}

impl Movement {
    pub(super) fn checkpoint_valid(&self) -> bool {
        [self.slide_time, self.slide_cooldown, self.dash_time, self.dash_cooldown,
            self.fov_kick, self.roll, self.fall_speed, self.wallrun_time,
            self.wallrun_cooldown].into_iter().all(f32::is_finite)
            && self.wall_normal.is_finite()
    }
    pub fn new() -> Self {
        Self {
            sliding: false,
            slide_time: 0.0,
            slide_cooldown: 0.0,
            dash_time: 0.0,
            dash_cooldown: 0.0,
            dash_charges: DASH_CHARGES,
            air_dash: false,
            air_jumps_used: 0,
            sneak_was_down: false,
            fov_kick: 0.0,
            roll: 0.0,
            events: 0,
            fall_speed: 0.0,
            wallrunning: false,
            wallrun_time: 0.0,
            wallrun_cooldown: 0.0,
            wall_normal: Vec2::ZERO,
        }
    }

    pub fn start_wallrun(&mut self, normal: Vec2) {
        self.wallrunning = true;
        self.wallrun_time = 0.0;
        self.wall_normal = normal;
        self.sliding = false;
        self.raise(EVENT_WALLRUN);
    }

    pub fn end_wallrun(&mut self) {
        if self.wallrunning {
            self.wallrunning = false;
            self.wallrun_cooldown = WALLRUN_COOLDOWN;
        }
    }

    /// Gravity multiplier for the current wall run (1 when not running).
    pub fn wallrun_gravity(&self) -> f32 {
        if !self.wallrunning {
            return 1.0;
        }
        let late = ((self.wallrun_time - WALLRUN_HOLD) / (WALLRUN_MAX_TIME - WALLRUN_HOLD))
            .clamp(0.0, 1.0);
        WALLRUN_GRAVITY + (1.0 - WALLRUN_GRAVITY) * late * late
    }

    pub fn raise(&mut self, event: u8) {
        self.events |= event;
    }

    /// Events since the last call; clears them.
    pub fn take_events(&mut self) -> u8 {
        std::mem::take(&mut self.events)
    }

    /// 0..1 screen "boost" strength: full while dashing, part while sliding.
    pub fn boost(&self) -> f32 {
        (self.fov_kick / DASH_FOV_KICK).clamp(0.0, 1.0)
    }

    /// Cooldowns, slide clock and landing refills; once per walking step.
    pub fn tick(&mut self, on_ground: bool, dt: f32) {
        self.slide_cooldown = (self.slide_cooldown - dt).max(0.0);
        self.dash_cooldown = (self.dash_cooldown - dt).max(0.0);
        self.dash_time = (self.dash_time - dt).max(0.0);
        self.wallrun_cooldown = (self.wallrun_cooldown - dt).max(0.0);
        if self.dash_time <= 0.0 {
            self.air_dash = false;
        }
        if self.sliding {
            self.slide_time += dt;
        }
        if self.wallrunning {
            self.wallrun_time += dt;
        }
        if on_ground {
            self.end_wallrun();
            self.wallrun_cooldown = 0.0;
            self.air_jumps_used = 0;
            if self.dash_cooldown <= 0.0 {
                self.dash_charges = DASH_CHARGES;
            }
        }
    }

    /// Every step, in every mode. Outside walking (flight, swimming) slides and
    /// dashes stop; the view kick eases toward the current action.
    pub fn settle(&mut self, walking_mode: bool, dt: f32) {
        if !walking_mode {
            self.sliding = false;
            self.dash_time = 0.0;
            self.air_dash = false;
            self.wallrunning = false;
        }
        let target = if self.dashing() {
            DASH_FOV_KICK
        } else if self.sliding || self.wallrunning {
            SLIDE_FOV_KICK
        } else {
            0.0
        };
        let rate = if target > self.fov_kick { 30.0 } else { 8.0 };
        self.fov_kick += (target - self.fov_kick) * (1.0 - (-rate * dt).exp());
    }

    /// Ease the camera roll toward `target` radians (set by the wall side).
    pub fn ease_roll(&mut self, target: f32, dt: f32) {
        self.roll += (target - self.roll) * (1.0 - (-10.0 * dt).exp());
    }

    pub fn camera_roll(&self) -> f32 {
        self.roll
    }

    pub fn dashing(&self) -> bool {
        self.dash_time > 0.0
    }

    pub fn can_dash(&self) -> bool {
        self.dash_charges > 0 && self.dash_cooldown <= 0.0 && !self.dashing()
    }

    pub fn start_dash(&mut self, airborne: bool) {
        self.dash_charges = self.dash_charges.saturating_sub(1);
        self.dash_time = DASH_TIME;
        self.dash_cooldown = DASH_COOLDOWN;
        self.air_dash = airborne;
        self.sliding = false;
        self.raise(EVENT_DASH);
    }

    pub fn start_slide(&mut self) {
        self.sliding = true;
        self.slide_time = 0.0;
        self.raise(EVENT_SLIDE);
    }

    pub fn end_slide(&mut self) {
        if self.sliding {
            self.sliding = false;
            self.slide_cooldown = SLIDE_COOLDOWN;
        }
    }

    /// Spends an air jump if one is left.
    pub fn take_air_jump(&mut self) -> bool {
        if self.air_jumps_used < AIR_JUMPS {
            self.air_jumps_used += 1;
            self.raise(EVENT_AIR_JUMP);
            true
        } else {
            false
        }
    }

    pub fn air_jump_left(&self) -> bool {
        self.air_jumps_used < AIR_JUMPS
    }

    pub fn fov_kick_degrees(&self) -> f32 {
        self.fov_kick
    }
}

/// Quake / Source air acceleration. Only the part of the wish direction that
/// the velocity does not already cover is accelerated, up to `wish_speed`.
/// Strafing perpendicular to the velocity therefore adds speed while turning,
/// and holding straight ahead adds nothing once above `wish_speed`.
pub fn air_accelerate(vel: Vec2, wish_dir: Vec2, wish_speed: f32, accel: f32, dt: f32) -> Vec2 {
    let current = vel.dot(wish_dir);
    let add = wish_speed - current;
    if add <= 0.0 {
        return vel;
    }
    vel + wish_dir * (accel * wish_speed * dt).min(add)
}

/// Horizontal velocity after steering toward `desired` with blend `t`.
///
/// At or below `target_speed` this is the old `horiz.lerp(desired, t)`. Above
/// it (after a dash, slide or slide-jump) steering may still turn the velocity,
/// but speed only bleeds off at `decay` (blocks/s²) instead of snapping back to
/// walk speed, and never below `target_speed`.
pub fn carry_momentum(
    horiz: Vec2,
    desired: Vec2,
    t: f32,
    target_speed: f32,
    decay: f32,
    dt: f32,
) -> Vec2 {
    let steered = horiz.lerp(desired, t);
    let speed = horiz.length();
    if speed <= target_speed.max(desired.length()) {
        return steered;
    }
    let keep = (speed - decay * dt).max(target_speed);
    let s = steered.length();
    if s < keep && s > 1e-6 {
        steered * (keep / s)
    } else {
        steered
    }
}

