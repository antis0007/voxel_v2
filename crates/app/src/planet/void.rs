//! Open space between the planets, flown in the star-system frame.
//!
//! Home's atlas turns with home's spin, which is fine near the ground but makes
//! interplanetary distances meaningless. Past home's Hill sphere, and outside
//! every other body's sphere of influence, the player's state lives in
//! system-frame metres instead (`Void`). Flight speed grows with the distance to
//! the nearest surface, so any body is reachable in seconds and an approach
//! slows by itself. Re-entering home's reach or a body's approach band hands the
//! player back to the atlas, where the anchor and its band take over.
use engine_core::cube_sphere::atlas;
use engine_space::{BodyId, Snapshot, System};
use glam::{DQuat, DVec3, Quat};

/// Player state in the star-system frame; never saved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Void {
    /// System-frame metres.
    pub pos: DVec3,
    /// System-frame metres per second.
    pub vel: DVec3,
    /// Player upright frame in system axes; open space has no down to turn toward.
    pub upright: DQuat,
}

const TICK_SECONDS: f64 = 1.0 / 120.0;
/// Fraction of home's Hill radius under which home's atlas takes the player back.
const RETURN_FRACTION: f64 = 0.92;
/// Flight speed per metre of distance to the nearest surface, per second.
const CRUISE_RATE: f64 = 0.8;
const MAX_MPS: f64 = 3.0e7;

/// Where home stands and how it is turned at a tick.
#[derive(Clone, Copy)]
struct Home {
    pos: DVec3,
    rot: DQuat,
}

impl Home {
    fn at(system: &System, home: BodyId, tick: u64) -> Self {
        let mut snapshot = Snapshot::default();
        system.evaluate(tick, &mut snapshot);
        Self { pos: snapshot.position(home), rot: system.orientation(home, tick) }
    }

    fn system_of(self, atlas_m: DVec3) -> DVec3 {
        self.pos + self.rot * atlas::to_body(atlas_m)
    }

    fn atlas_of(self, p: DVec3) -> DVec3 {
        atlas::from_body(self.rot.inverse() * (p - self.pos))
    }

    /// Rotation from system axes to the face-local axes at `atlas_m`.
    fn axes(self, atlas_m: DVec3) -> DQuat {
        (DQuat::from_mat3(&atlas::frame(atlas_m).transpose()) * self.rot.inverse()).normalize()
    }
}

/// Home's reach: its Hill radius, which holds the moon and its approach band.
pub fn home_reach(system: &System) -> Option<f64> {
    let home = system.id("home")?;
    let star = system.parent(home)?;
    let mut snapshot = Snapshot::default();
    system.evaluate(0, &mut snapshot);
    let a = (snapshot.position(home) - snapshot.position(star)).length();
    Some(engine_space::hill_radius(a, 0.0, system.mass(home), system.mass(star)))
}

/// Distance from `p` to the nearest body surface, metres.
fn clearance(system: &System, snapshot: &Snapshot, p: DVec3) -> f64 {
    (0..system.len() as u16)
        .map(BodyId)
        .map(|b| (p - snapshot.position(b)).length() - system.radius(b))
        .fold(f64::MAX, f64::min)
}

/// Atlas metres per second of a point at `pos` moving at `vel` through the system frame.
///
/// Like the approach band, it spans the previous tick to this one: the point
/// arrived here during the last step.
fn atlas_velocity(system: &System, home: BodyId, tick: u64, pos: DVec3, vel: DVec3) -> DVec3 {
    let now = Home::at(system, home, tick);
    let before = Home::at(system, home, tick.saturating_sub(1));
    (now.atlas_of(pos) - before.atlas_of(pos - vel * TICK_SECONDS)) / TICK_SECONDS
}

/// Moves a player in open space one step; true when it did, so the atlas step is skipped.
pub fn step(player: &mut crate::player::Player, input: &crate::player::Input, world: &crate::world::World, dt: f32) -> bool {
    let Some(system) = super::influence::system() else { return false };
    let Some(home) = system.id("home") else { return false };
    if world.world_type != crate::world::WorldType::Planet || player.anchor.is_some() {
        player.void = None;
        return false;
    }
    let Some(reach) = home_reach(system) else { return false };
    let scale = f64::from(world.voxel_scale.meters());
    let tick = world.orbital.planet_tick;
    let frame = Home::at(system, home, tick);
    let mut void = match player.void {
        Some(v) => v,
        None => {
            let atlas_m = player.precise_pos * scale;
            if atlas::to_body(atlas_m).length() <= reach {
                return false;
            }
            if super::influence::approached_body(atlas_m, tick).is_some() {
                return false;
            }
            // Exact entry over the step that brought the player here, as the approach band converts.
            let before = Home::at(system, home, tick.saturating_sub(1)).system_of(atlas_m - player.vel.as_dvec3() * scale * TICK_SECONDS);
            let pos = frame.system_of(atlas_m);
            Void { pos, vel: (pos - before) / TICK_SECONDS, upright: frame.axes(atlas_m).inverse() * player.gravity_frame.as_dquat() }
        }
    };
    let mut snapshot = Snapshot::default();
    system.evaluate(tick, &mut snapshot);
    let dominant = system.dominant_body(&snapshot, void.pos);
    let home_distance = (void.pos - snapshot.position(home)).length();
    // A body's approach band begins outside its sphere; the anchor takes over at its edge.
    let approached = super::influence::approached_body(frame.atlas_of(void.pos), tick).is_some();
    if home_distance < reach * RETURN_FRACTION || approached {
        release(player, system, home, tick, void, scale);
        return false;
    }

    let dt64 = f64::from(dt);
    let (sy, cy) = f64::from(player.yaw).sin_cos();
    let (sp, cp) = f64::from(player.pitch).sin_cos();
    let forward = void.upright * DVec3::new(sy * cp, sp, -cy * cp);
    let right = void.upright * DVec3::new(cy, 0.0, sy);
    let up = void.upright * DVec3::Y;
    let axis = |plus: bool, minus: bool| f64::from(plus as i8 - minus as i8);
    if player.flying {
        let wish = forward * axis(input.forward, input.back) + right * axis(input.right, input.left) + up * axis(input.jump, input.sneak);
        let boost = if input.sprint { f64::from(player.flight.boost.clamp(1.0, 4.0)) } else { 1.0 };
        let base = f64::from(player.flight.speed.clamp(4.0, 64.0));
        let speed = (base.max(CRUISE_RATE * clearance(system, &snapshot, void.pos)) * boost).min(MAX_MPS);
        let target = wish.normalize_or_zero() * speed;
        void.vel = void.vel.lerp(target, (f64::from(crate::world::FLY_ACCEL) * dt64).min(1.0));
    } else {
        void.vel += system.pull(&snapshot, dominant, void.pos) * dt64;
    }
    void.pos += void.vel * dt64;
    let star = BodyId(0);
    let from_star = void.pos - snapshot.position(star);
    if from_star.length() < system.radius(star) {
        void.pos = snapshot.position(star) + from_star.normalize_or(DVec3::Y) * system.radius(star);
        void.vel = DVec3::ZERO;
    }
    player.void = Some(void);
    place(player, frame, void, scale);
    true
}

/// Writes the atlas pose a system-frame state shows to the renderer and the rest of the game.
fn place(player: &mut crate::player::Player, frame: Home, void: Void, scale: f64) {
    let atlas_m = frame.atlas_of(void.pos);
    let axes = frame.axes(atlas_m);
    player.precise_pos = atlas_m / scale;
    player.pos = player.precise_pos.as_vec3();
    player.vel = (axes * void.vel / scale).as_vec3();
    player.gravity_frame = (axes * void.upright).as_quat().normalize();
    player.on_ground = false;
}

/// Hands the player back to home's atlas with the exact atlas velocity, keeping the view.
fn release(player: &mut crate::player::Player, system: &System, home: BodyId, tick: u64, void: Void, scale: f64) {
    let frame = Home::at(system, home, tick);
    place(player, frame, void, scale);
    let look = player.forward_dir();
    player.yaw = look.x.atan2(-look.z);
    player.pitch = look.y.clamp(-1.0, 1.0).asin();
    player.vel = (atlas_velocity(system, home, tick, void.pos, void.vel) / scale).as_vec3();
    player.gravity_frame = Quat::IDENTITY;
    player.void = None;
}

#[cfg(test)]
#[path = "void_tests.rs"]
mod tests;
