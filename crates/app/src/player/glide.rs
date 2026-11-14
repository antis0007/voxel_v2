//! Glide frame flight model in metres and seconds (update pack T11).
//!
//! Lift acts across the airflow and drag along it; there is no thrust term, so
//! still-air flight only trades height for speed and loses energy to drag.
//! A chest-mounted hang glider carries the frame.
use glam::Vec3;

/// Neutral-pitch airspeed at which lift carries the full weight, m/s.
pub const TRIM_SPEED: f32 = 12.0;
/// Drag terms as fractions of the trim lift coefficient. Best glide is about 7.
const DRAG_PARASITIC: f32 = 0.087;
const DRAG_INDUCED: f32 = 0.061;
/// Sideways airflow drag: the frame weathervanes into the heading.
const DRAG_SIDE: f32 = 0.9;
/// Lift coefficient range reachable by pitching, as a factor of trim.
const LIFT_RANGE: (f32, f32) = (0.35, 1.5);
/// Speed above which lift and drag stop growing, guarding the explicit step.
const MAX_AIRSPEED: f32 = 45.0;

/// Frame state. Transient: loading mid-air starts stowed, and `available`
/// is refreshed from worn gear every fixed step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Glide {
    pub available: bool,
    pub deployed: bool,
    /// Air at the body this step, m/s (`weather_wind`).
    pub wind: Vec3,
    /// Horizontal distance the air has carried since load, metres: the wind
    /// integrated each fixed step, so precipitation slants and drifts without
    /// jumps when the weather turns.
    pub drift_m: glam::DVec2,
}

/// Air at a voxel position in m/s: `world::wind_at` plus a tornado's updraft.
/// Sampled once per fixed step by every peer, so it is lockstep-safe.
pub fn weather_wind(world: &crate::world::World, day: &crate::world::DayNightCycle, pos: Vec3) -> Vec3 {
    let xz_m = glam::DVec2::new(f64::from(pos.x), f64::from(pos.z)) * f64::from(world.voxel_scale.meters());
    let planet = Some(world.sample_config());
    let wind = crate::world::wind_at(day, planet, xz_m);
    Vec3::new(wind.x, crate::world::updraft_at(day, planet, xz_m), wind.y)
}

/// The air the frame flies in: the full wind, bounded by `ground.glide_max_ms`.
pub fn glide_air(wind: Vec3) -> Vec3 {
    wind.clamp_length_max(engine_content::climate::storm_dials().ground.glide_max_ms as f32)
}

/// Wind drag on a walking or falling body, m/s^2, for velocity `vel` (m/s).
/// Quadratic in the relative wind above `ground.push_min_ms`, so a breeze is
/// never felt and a gale or eyewall shoves; a tornado's updraft lifts.
pub fn wind_push(vel: Vec3, wind: Vec3, on_ground: bool) -> Vec3 {
    let g = &engine_content::climate::storm_dials().ground;
    let mut rel = wind - vel;
    if on_ground {
        rel.y = rel.y.max(0.0);
    }
    let speed = rel.length();
    let excess = speed - g.push_min_ms as f32;
    if excess <= 0.0 || speed < 1e-3 {
        return Vec3::ZERO;
    }
    let k = if on_ground { g.push_ground } else { g.push_air } as f32;
    rel / speed * (k * excess * speed)
}

/// Lift factor from view pitch in radians; looking down dives, looking up flares.
pub fn lift_factor(pitch: f32) -> f32 {
    (1.0 + pitch * 1.1).clamp(LIFT_RANGE.0, LIFT_RANGE.1)
}

/// Air acceleration on the frame for `vel` relative to `wind`, without gravity.
/// Coefficients scale with `gravity` so trim speed and glide ratio hold under
/// any world gravity.
pub fn aero_accel(vel: Vec3, wind: Vec3, yaw: f32, pitch: f32, gravity: f32) -> Vec3 {
    let air = vel - wind;
    let speed = air.length();
    if speed < 1e-3 {
        return Vec3::ZERO;
    }
    let along = air / speed;
    let q = speed.min(MAX_AIRSPEED) * speed.min(MAX_AIRSPEED);
    let heading = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
    let side = Vec3::Y.cross(heading).normalize_or_zero();
    // At right angles to the airflow in the heading plane: up for forward flight,
    // forward for air rising from below. Never re-oriented by sign: near-level
    // crossflow has a y of rounding size, and flipping on it reversed the lift.
    let lift_dir = along.cross(side).normalize_or_zero();
    let k = gravity / (TRIM_SPEED * TRIM_SPEED);
    let c = lift_factor(pitch);
    let lift = lift_dir * k * c * q;
    let drag = -along * k * (DRAG_PARASITIC + DRAG_INDUCED * c * c) * q;
    let slip = air.dot(side);
    let side_drag = -side * k * DRAG_SIDE * slip * slip.abs();
    lift + drag + side_drag
}

/// One explicit step of glide flight. `gravity` is metres per second squared.
pub fn step(vel: Vec3, wind: Vec3, yaw: f32, pitch: f32, gravity: f32, dt: f32) -> Vec3 {
    vel + (aero_accel(vel, wind, yaw, pitch, gravity) - Vec3::Y * gravity) * dt
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Earth gravity and the game's movement gravity (`world::GRAVITY`).
    const GRAVITIES: [f32; 2] = [9.81, crate::world::GRAVITY];
    const G: f32 = crate::world::GRAVITY;
    const DT: f32 = 1.0 / 120.0;

    /// Flies `seconds` and returns (velocity, horizontal distance, height lost).
    fn fly(mut vel: Vec3, yaw: impl Fn(f32) -> f32, pitch: f32, seconds: f32) -> (Vec3, f32, f32) {
        let mut pos = Vec3::ZERO;
        let mut t = 0.0;
        while t < seconds {
            vel = step(vel, Vec3::ZERO, yaw(t), pitch, G, DT);
            pos += vel * DT;
            t += DT;
        }
        (vel, Vec3::new(pos.x, 0.0, pos.z).length(), -pos.y)
    }

    #[test]
    fn still_air_never_adds_energy() {
        for g in GRAVITIES {
        for pitch in [-0.5, -0.2, 0.0, 0.2, 0.5] {
            for start in [Vec3::ZERO, Vec3::new(0.0, -6.0, -3.0), Vec3::new(0.0, 0.0, -25.0)] {
                let mut vel = start;
                let mut height = 0.0;
                let mut energy = 0.5 * vel.length_squared();
                for i in 0..120 * 60 {
                    let yaw = (i as f32 * DT * 0.7).sin();
                    vel = step(vel, Vec3::ZERO, yaw, pitch, g, DT);
                    height += vel.y * DT;
                    let now = 0.5 * vel.length_squared() + g * height;
                    // Explicit integration error stays far below the drag loss.
                    assert!(now <= energy + 1e-3, "pitch {pitch} start {start} step {i}: {energy} -> {now}");
                    energy = now;
                }
            }
        }
        }
    }

    #[test]
    fn neutral_trim_glides_at_a_usable_ratio() {
        let (vel, distance, lost) = fly(Vec3::new(0.0, 0.0, -10.0), |_| 0.0, 0.0, 40.0);
        let ratio = distance / lost;
        let speed = Vec3::new(vel.x, 0.0, vel.z).length();
        assert!((5.0..9.0).contains(&ratio), "glide ratio {ratio}");
        assert!((9.0..15.0).contains(&speed), "trim speed {speed}");
        assert!((1.0..3.0).contains(&-vel.y), "sink {}", -vel.y);
    }

    #[test]
    fn diving_is_faster_and_steeper_than_flaring() {
        let (dive, dive_d, dive_h) = fly(Vec3::new(0.0, 0.0, -10.0), |_| 0.0, -0.45, 20.0);
        let (flare, flare_d, flare_h) = fly(Vec3::new(0.0, 0.0, -10.0), |_| 0.0, 0.3, 20.0);
        assert!(dive.length() > flare.length() + 2.0, "{dive} vs {flare}");
        assert!(dive_d / dive_h < flare_d / flare_h, "dive ratio must be worse");
    }

    #[test]
    fn a_stall_recovers_into_forward_flight() {
        let (vel, _, lost) = fly(Vec3::ZERO, |_| 0.0, 0.0, 6.0);
        assert!(Vec3::new(vel.x, 0.0, vel.z).length() > 6.0, "no recovery: {vel}");
        assert!(-vel.y < 5.0 && lost < 45.0, "fell like a stone: {vel}, lost {lost} m");
    }

    #[test]
    fn the_flight_path_follows_the_heading() {
        let turn = std::f32::consts::FRAC_PI_2;
        let (vel, _, _) = fly(Vec3::new(0.0, 0.0, -10.0), |t| if t > 1.0 { turn } else { 0.0 }, 0.0, 5.0);
        let heading = Vec3::new(turn.sin(), 0.0, -turn.cos());
        let flat = Vec3::new(vel.x, 0.0, vel.z).normalize();
        assert!(flat.dot(heading) > 0.95, "still sliding sideways: {flat}");
    }

    #[test]
    fn rising_air_extends_the_glide() {
        let lift = Vec3::new(0.0, 3.0, 0.0);
        let mut vel = Vec3::new(0.0, 0.0, -10.0);
        let mut height = 0.0;
        for _ in 0..120 * 20 {
            vel = step(vel, lift, 0.0, 0.0, G, DT);
            height += vel.y * DT;
        }
        assert!(height > 0.0, "a 3 m/s updraft should out-climb a glide sink: {height}");
    }
}

#[cfg(test)]
#[path = "glide_tests.rs"]
mod controller_tests;
