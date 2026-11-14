//! Orbital flight above home's atmosphere, in a home-centred inertial frame.
//!
//! Below the atmosphere ceiling the atlas flies an arcade cruise: velocity
//! follows the keys and stops when they are released. Above it a flying player
//! keeps every metre per second they gained. Thrust is an acceleration, home
//! pulls with its inverse-square gravity, and the atlas turns underneath, so
//! circular speed holds an altitude and less than that falls back.
//!
//! The state is kept in non-rotating axes at home's centre (`Orbit`) and the
//! atlas pose is derived from it every step, as `Void` and `Anchor` do. A
//! kick-drift-kick step keeps a coasting orbit closed over many revolutions.
use engine_core::cube_sphere::atlas;
use glam::{DQuat, DVec3, Quat};

/// Player state around home; never saved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbit {
    /// Metres from home's centre, system axes.
    pub pos: DVec3,
    /// Metres per second relative to home's centre, system axes.
    pub vel: DVec3,
    /// Player upright frame in system axes; its up follows the radial up.
    pub upright: DQuat,
    /// Atlas position this state last wrote; a teleport or another frame's
    /// step changes it, and the state is then rebuilt from the atlas.
    shown: DVec3,
    /// Seconds since the atmosphere was left.
    pub age: f32,
    /// Seconds Sprint has been held; thrust doubles with each.
    boost_held: f32,
}

const TICK_SECONDS: f64 = 1.0 / 120.0;
/// Orbital flight starts this far above the atmosphere ceiling, so hovering
/// at the ceiling cannot flip between the two flight models.
const ENTER_MARGIN_M: f64 = crate::altitude::HYSTERESIS_M;
/// Home's gravitational parameter: the game's 9.81 m/s² at the atlas radius.
pub const MU: f64 = 9.81 * atlas::RADIUS_M * atlas::RADIUS_M;
/// Creative thrust per metre per second of the flight speed setting, per second.
const CREATIVE_THRUST: f64 = 4.0;
/// Suit thrust outside Creative, m/s².
const SUIT_THRUST: f64 = 12.0;
/// Sprint doubles Creative thrust once per this many seconds held.
const BOOST_DOUBLING_S: f32 = 1.0;
/// Numeric ceiling on thrust, m/s²; no speed limit applies.
const MAX_THRUST: f64 = 5.0e4;
/// Braking removes ground-relative speed with this time constant in Creative.
const BRAKE_SECONDS: f64 = 0.6;
/// Longest single integration step; a slow frame is split.
const MAX_STEP_S: f64 = 1.0 / 60.0;

/// Altitude where orbital flight takes over from the atlas cruise, metres.
pub fn ceiling_m() -> f64 {
    crate::altitude::LOAD_STOP_M
}

/// Speed of a circular orbit at `radius_m` from home's centre.
pub fn circular_speed(radius_m: f64) -> f64 {
    (MU / radius_m.max(1.0)).sqrt()
}

/// Shape of the coasting path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elements {
    /// Lowest point above the atlas radius, metres; negative underground.
    pub periapsis_m: f64,
    /// Highest point above the atlas radius, metres; `None` on an escape path.
    pub apoapsis_m: Option<f64>,
    pub speed_mps: f64,
    pub circular_mps: f64,
}

/// What a coasting path does next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    /// The path dips into the atmosphere.
    Suborbital,
    /// A closed path that clears the atmosphere.
    Orbit,
    /// Fast enough to leave home.
    Escape,
}

impl Elements {
    pub fn of(pos: DVec3, vel: DVec3) -> Self {
        let r = pos.length().max(1.0);
        let energy = vel.length_squared() * 0.5 - MU / r;
        let h2 = pos.cross(vel).length_squared();
        let e = (1.0 + 2.0 * energy * h2 / (MU * MU)).max(0.0).sqrt();
        // Semi-latus rectum over 1 + e holds for every conic, including a parabola.
        let periapsis = h2 / MU / (1.0 + e);
        let apoapsis = (energy < 0.0 && e < 1.0).then(|| h2 / MU / (1.0 - e).max(1e-12));
        Self {
            periapsis_m: periapsis - atlas::RADIUS_M,
            apoapsis_m: apoapsis.map(|a| a - atlas::RADIUS_M),
            speed_mps: vel.length(),
            circular_mps: circular_speed(r),
        }
    }

    pub fn path(&self) -> Path {
        match self.apoapsis_m {
            None => Path::Escape,
            Some(_) if self.periapsis_m >= ceiling_m() => Path::Orbit,
            Some(_) => Path::Suborbital,
        }
    }
}

impl Orbit {
    pub fn elements(&self) -> Elements {
        Elements::of(self.pos, self.vel)
    }
}

/// One coasting step with a constant thrust acceleration: kick, drift, kick.
pub fn integrate(pos: &mut DVec3, vel: &mut DVec3, thrust: DVec3, dt: f64) {
    let pull = |p: DVec3| {
        let r2 = p.length_squared().max(1.0);
        -p * (MU / (r2 * r2.sqrt()))
    };
    let steps = (dt / MAX_STEP_S).ceil().max(1.0);
    let h = dt / steps;
    for _ in 0..steps as u32 {
        *vel += (pull(*pos) + thrust) * (h * 0.5);
        *pos += *vel * h;
        *vel += (pull(*pos) + thrust) * (h * 0.5);
    }
}

/// Home's turn at a tick, system axes from body axes.
fn turn(tick: u64) -> Option<DQuat> {
    let system = super::influence::system()?;
    Some(system.orientation(system.id("home")?, tick))
}

fn atlas_of(rot: DQuat, pos: DVec3) -> DVec3 {
    atlas::from_body(rot.inverse() * pos)
}

/// Rotation from system axes to the face-local axes at `atlas_m`.
fn axes(rot: DQuat, atlas_m: DVec3) -> DQuat {
    (DQuat::from_mat3(&atlas::frame(atlas_m).transpose()) * rot.inverse()).normalize()
}

fn near_structure(world: &crate::world::World, from: DVec3, to: DVec3) -> bool {
    crate::player::creative_flight::structure_on_path(world, from, to)
}

/// Thrust acceleration in m/s² for the held keys, before direction.
fn thrust(player: &crate::player::Player, input: &crate::player::Input, creative: bool, boost_held: f32) -> f64 {
    let fine = if input.alt_down { 0.1 } else { 1.0 };
    if !creative {
        return SUIT_THRUST * if input.sprint { 2.0 } else { 1.0 } * fine;
    }
    let base = f64::from(crate::player::creative_flight::base_speed(&player.flight, input)) * CREATIVE_THRUST;
    let ramp = 2f64.powf(f64::from(boost_held / BOOST_DOUBLING_S).min(40.0));
    (base * ramp * fine).min(MAX_THRUST)
}

/// Moves a player in orbital flight one step; true when it did, so the atlas step is skipped.
pub fn step(player: &mut crate::player::Player, input: &crate::player::Input, world: &crate::world::World, dt: f32) -> bool {
    let cfg = world.sample_config();
    if world.world_type != crate::world::WorldType::Planet || cfg.is_hell()
        || player.anchor.is_some() || player.void.is_some() || !player.flying
    {
        player.orbit = None;
        return false;
    }
    let tick = world.orbital.planet_tick;
    let (Some(rot), Some(system)) = (turn(tick), super::influence::system()) else {
        player.orbit = None;
        return false;
    };
    let before = turn(tick.saturating_sub(1)).unwrap_or(rot);
    let scale = f64::from(world.voxel_scale.meters());
    let atlas_m = player.precise_pos * scale;
    // The Hill radius is fixed for the system; evaluating it solves every body's orbit.
    static REACH: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    let reach = *REACH.get_or_init(|| super::void::home_reach(system).unwrap_or(f64::MAX));

    let mut orbit = match player.orbit.filter(|o| o.shown == player.precise_pos) {
        Some(orbit) => orbit,
        None => {
            player.orbit = None;
            if atlas_m.y < ceiling_m() + ENTER_MARGIN_M
                || near_structure(world, player.precise_pos, player.precise_pos)
                || super::influence::approached_body(atlas_m, tick).is_some()
            {
                return false;
            }
            // Exact entry over the step that brought the player here: the
            // atlas turned during it, which is where the ground's speed comes from.
            let pos = rot * atlas::to_body(atlas_m);
            let was = before * atlas::to_body(atlas_m - player.vel.as_dvec3() * scale * TICK_SECONDS);
            Orbit {
                pos,
                vel: (pos - was) / TICK_SECONDS,
                upright: axes(rot, atlas_m).inverse() * player.gravity_frame.as_dquat().normalize(),
                shown: player.precise_pos,
                age: 0.0,
                boost_held: 0.0,
            }
        }
    };

    let dt64 = f64::from(dt);
    let creative = world.rules.mode.creative_tools();
    orbit.boost_held = if input.sprint && creative { orbit.boost_held + dt } else { 0.0 };
    orbit.age += dt;

    let (sy, cy) = f64::from(player.yaw).sin_cos();
    let (sp, cp) = f64::from(player.pitch).sin_cos();
    let forward = orbit.upright * DVec3::new(sy * cp, sp, -cy * cp);
    let right = orbit.upright * DVec3::new(cy, 0.0, sy);
    let up = orbit.upright * DVec3::Y;
    let axis = |plus: bool, minus: bool| f64::from(plus as i8 - minus as i8);
    let wish = (forward * axis(input.forward, input.back) + right * axis(input.right, input.left)
        + up * axis(input.jump, input.sneak)).normalize_or_zero();
    let power = thrust(player, input, creative, orbit.boost_held);
    let mut push = wish * power;
    if input.dash {
        // Brake against the ground turning below: the speed the atlas shows.
        let ground = (orbit.pos - before * (rot.inverse() * orbit.pos)) / TICK_SECONDS;
        let drift = orbit.vel - ground;
        let rate = if creative { (drift.length() / BRAKE_SECONDS).max(power) } else { power };
        push += -drift.normalize_or_zero() * rate.min(drift.length() / dt64.max(1e-6)).min(MAX_THRUST);
        if creative {
            // Creative holds station while braking instead of falling out of the sky.
            let r = orbit.pos.length().max(1.0);
            push += orbit.pos * (MU / (r * r * r));
        }
    }

    let from = player.precise_pos;
    integrate(&mut orbit.pos, &mut orbit.vel, push, dt64);

    // The upright follows the radial up by the shortest turn, carrying the view.
    let radial = orbit.pos.normalize_or(DVec3::Y);
    orbit.upright = (DQuat::from_rotation_arc(orbit.upright * DVec3::Y, radial) * orbit.upright).normalize();

    let next = atlas_of(rot, orbit.pos);
    let leaving = orbit.pos.length() - atlas::RADIUS_M < ceiling_m()
        || orbit.pos.length() > reach
        || super::influence::approached_body(next, tick).is_some()
        || near_structure(world, from, next / scale);
    place(player, rot, &mut orbit, scale);
    if leaving {
        release(player, rot, before, orbit, scale);
        return false;
    }
    player.orbit = Some(orbit);
    true
}

/// Writes the atlas pose an orbital state shows to the renderer and the rest of the game.
fn place(player: &mut crate::player::Player, rot: DQuat, orbit: &mut Orbit, scale: f64) {
    let atlas_m = atlas_of(rot, orbit.pos);
    let axes = axes(rot, atlas_m);
    player.precise_pos = atlas_m / scale;
    player.pos = player.precise_pos.as_vec3();
    player.vel = (axes * orbit.vel / scale).as_vec3();
    player.gravity_frame = (axes * orbit.upright).as_quat().normalize();
    player.on_ground = false;
    orbit.shown = player.precise_pos;
}

/// Hands the player back to the atlas cruise with the atlas velocity, keeping the view.
fn release(player: &mut crate::player::Player, rot: DQuat, before: DQuat, orbit: Orbit, scale: f64) {
    let look = player.forward_dir();
    player.yaw = look.x.atan2(-look.z);
    player.pitch = look.y.clamp(-1.0, 1.0).asin();
    let now = atlas_of(rot, orbit.pos);
    let was = atlas_of(before, orbit.pos - orbit.vel * TICK_SECONDS);
    if atlas::split(now).0 == atlas::split(was).0 {
        player.vel = ((now - was) / (TICK_SECONDS * scale)).as_vec3();
    } else {
        // Across a face seam the chart difference is meaningless; use the
        // ground-relative velocity in the new face's axes.
        let ground = (orbit.pos - before * (rot.inverse() * orbit.pos)) / TICK_SECONDS;
        let felt = (axes(rot, now) * (orbit.vel - ground)).as_vec3();
        player.vel = super::cruise_velocity(now, felt) / scale as f32;
    }
    player.gravity_frame = Quat::IDENTITY;
    player.orbit = None;
}
