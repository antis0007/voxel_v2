//! Planet frame transitions for the ordinary player and renderer.
use engine_core::cube_sphere::atlas;
use glam::{DVec3, Vec3};
pub mod ground_mesh;
pub(crate) mod hell_moons;
pub mod influence;
pub mod orbit;
pub mod review;
pub(crate) mod rails;
pub mod void;

/// Smallest drawn angular radius in radians for the sun.
const MIN_DISC_RADIUS: f64 = 0.0035;
/// Moon radius floor, 2.2 degrees; the flat-world sky uses the same size.
const MIN_MOON_RADIUS: f64 = 0.0384;
/// Giant radius floor, 2.6 degrees. Its true size is under 0.1 degree, which
/// leaves bands and ring invisible, so the sky exaggerates it.
const MIN_GIANT_RADIUS: f64 = 0.045;

pub fn cross_face(player: &mut crate::player::Player, scale: f64) {
    let point = player.precise_pos * scale;
    let (_, local) = atlas::split(point);
    if local.x.abs() <= atlas::HALF_M && local.z.abs() <= atlas::HALF_M { return; }
    let body = atlas::to_body(point);
    let next = atlas::from_body(body);
    let old_frame = atlas::frame(point);
    let new_frame = atlas::frame(next);
    let rotation = new_frame.transpose() * old_frame;
    let look = rotation * player.forward_dir().as_dvec3();
    player.yaw = (look.x.atan2(-look.z)) as f32;
    player.pitch = look.y.clamp(-1.0, 1.0).asin() as f32;
    player.vel = (rotation * player.vel.as_dvec3()).as_vec3();
    player.precise_pos = next / scale;
}

/// Chart velocity for a cruise velocity given in face axes at `point_m`.
// Chart x and z stretch with height and across a face. Over resident terrain
// the chart is the voxel world's own metric; once terrain unloads the globe is
// the reference, so cruise speed turns physical in every direction.
pub fn cruise_velocity(point_m: DVec3, velocity: Vec3) -> Vec3 {
    use crate::altitude::{LOAD_STOP_M, TRANSITION_M};
    let t = ((point_m.y - TRANSITION_M) / (LOAD_STOP_M - TRANSITION_M)).clamp(0.0, 1.0);
    let k = t * t * (3.0 - 2.0 * t);
    if k <= 0.0 { return velocity; }
    let stretch = |axis: DVec3| (atlas::to_body(point_m + axis) - atlas::to_body(point_m - axis)).length() * 0.5;
    let x = 1.0 + (stretch(DVec3::X) - 1.0) * k;
    let z = 1.0 + (stretch(DVec3::Z) - 1.0) * k;
    Vec3::new((f64::from(velocity.x) / x) as f32, velocity.y, (f64::from(velocity.z) / z) as f32)
}

pub fn lighting(state: &mut crate::world::LightingState, eye: DVec3, tick: u64) {
    let sky = camera(eye, true, tick);
    state.sun_dir = Vec3::from_array([sky[5][0],sky[5][1],sky[5][2]]);
    state.moon_dir = Vec3::from_array([sky[6][0],sky[6][1],sky[6][2]]);
    state.daylight = ((state.sun_dir.y + 0.10) / 0.20).clamp(0.0,1.0);
    state.sun_intensity = state.daylight * 1.4;
    state.moon_intensity = 0.05;
}

/// Orbital tick for the flat calendar; the planet turns once per 480 s day.
pub fn tick_of(day: &crate::world::DayNightCycle) -> u64 {
    ((day.day_index() as f64 + day.time_of_day() as f64) * 480.0 * 120.0) as u64
}

/// Time of day where the player stands: solar on planets, the flat clock elsewhere.
// Beds, night mobs, climate and farming must follow the sky the player sees.
pub fn local_time(world_type: crate::world::WorldType, eye_m: DVec3, day: &crate::world::DayNightCycle) -> f32 {
    if world_type == crate::world::WorldType::Planet {
        solar_time(eye_m, tick_of(day))
    } else {
        day.time_of_day()
    }
}

/// Set the clock so the sky at `eye_m` reads `target` (a `/time` command is a local request on planets).
/// Each pass moves the clock by the remaining local error; the orbit's slow drift settles in three.
pub fn set_local_time(world_type: crate::world::WorldType, eye_m: DVec3, day: &mut crate::world::DayNightCycle, target: f32) {
    if world_type != crate::world::WorldType::Planet {
        day.set_time_of_day(target);
        return;
    }
    let mut clock = day.time_of_day();
    for _ in 0..3 {
        day.set_time_of_day(clock);
        let error = (f64::from(target) - f64::from(local_time(world_type, eye_m, day)) + 0.5).rem_euclid(1.0) - 0.5;
        clock = (f64::from(clock) + error).rem_euclid(1.0) as f32;
    }
    day.set_time_of_day(clock);
}

/// Local solar time at `eye`: 0.5 at noon, 0 at midnight.
///
/// Sky gradient, fog, ambient and sun colour are authored against the flat
/// clock. The clock comes from the hour angle, the sun's turn about the spin
/// axis from the local meridian, so it holds at every latitude and tilt; the
/// sun's height alone saturates wherever it never reaches the flat zenith.
pub fn solar_time(eye: DVec3, tick: u64) -> f32 {
    let Some((system, home, star)) = system().and_then(|s| Some((s, s.id("home")?, s.id("star")?))) else {
        return 0.5;
    };
    let sun = system.orientation(home, tick).conjugate()
        * rails::with_snapshot(system, tick, |s| s.position(star) - s.position(home));
    hour_time(atlas::to_body(eye), sun)
}

/// Solar time from body-frame directions to the eye and the sun about +Y.
fn hour_time(eye: DVec3, sun: DVec3) -> f32 {
    let flat = |v: DVec3| DVec3::new(v.x, 0.0, v.z);
    let (e, s) = (flat(eye), flat(sun));
    // The body turns positively about +Y, so the sun runs negatively and the
    // hour angle grows as time passes.
    let hour = (-DVec3::Y.dot(e.cross(s))).atan2(e.dot(s));
    (0.5 + hour / std::f64::consts::TAU).rem_euclid(1.0) as f32
}

/// Rows taking an eye-frame direction to the star system's fixed axes.
// Stars are infinitely far, so only the home spin and the eye's face frame turn them.
pub fn sky_axes(eye: DVec3, enabled: bool, tick: u64) -> [[f32; 4]; 3] {
    let spin = enabled.then(system).flatten().and_then(|s| Some(s.orientation(s.id("home")?, tick)));
    let Some(spin) = spin else { return [[0.0; 4]; 3]; };
    let rows = (glam::DMat3::from_quat(spin) * atlas::frame(eye)).transpose();
    [rows.x_axis, rows.y_axis, rows.z_axis].map(|r| r.as_vec3().extend(0.0).to_array())
}

/// Star rows for flat worlds: undo the sun's turn about +Z.
// LightingState::from_time_and_day moves the sun on a circle about +Z. Midnight
// (angle -pi/2) is identity, the pose the galaxy band was tuned at.
pub fn flat_sky_axes(sun: Vec3) -> [[f32; 4]; 3] {
    let (s, c) = (sun.y.atan2(sun.x) + std::f32::consts::FRAC_PI_2).sin_cos();
    [[c, s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]]
}

fn system() -> Option<&'static engine_space::System> {
    static SYSTEM: std::sync::OnceLock<Option<engine_space::System>> = std::sync::OnceLock::new();
    SYSTEM.get_or_init(|| {
        let file = engine_space::parse(include_str!("../../../assets/star_systems.ron")).ok()?;
        engine_space::System::compile(file.systems.first()?, 1.0 / 120.0).ok()
    }).as_ref()
}

/// Camera-relative planet coordinates share the world camera's local axes.
pub fn camera(eye: DVec3, enabled: bool, tick: u64) -> [[f32; 4]; 8] {
    if !enabled { return [[0.0; 4]; 8]; }
    let frame = atlas::frame(eye);
    let mut result = [
        [atlas::RADIUS_M as f32, atlas::HALF_M as f32, atlas::PITCH_M as f32, 1.0],
        atlas::to_body(eye).as_vec3().extend(eye.y as f32).to_array(),
        frame.x_axis.as_vec3().extend(0.0).to_array(),
        frame.y_axis.as_vec3().extend(0.0).to_array(),
        frame.z_axis.as_vec3().extend(0.0).to_array(),
        [0.0; 4], [0.0; 4], [0.0; 4],
    ];
    if let Some(system) = system()
        && let Some(home) = system.id("home") {
            let mut snapshot = engine_space::Snapshot::default();
            system.evaluate(tick, &mut snapshot);
            let rotation = system.orientation(home, tick).conjugate();
            // The giant's spin axis in the eye frame rides in w of the frame rows; the sky
            // draws its bands and ring about it. Ecliptic north stands in without a giant.
            let pole = system.id("giant").map_or(DVec3::Y, |giant| system.orientation(giant, tick) * DVec3::Y);
            let pole = frame.transpose() * (rotation * pole);
            for (row, value) in [pole.x, pole.y, pole.z].into_iter().enumerate() {
                result[2 + row][3] = value as f32;
            }
            for (slot, name, floor) in [(5, "star", MIN_DISC_RADIUS), (6, "moon", MIN_MOON_RADIUS), (7, "giant", MIN_GIANT_RADIUS)] {
                if let Some(body) = system.id(name) {
                    let delta = rotation * (snapshot.position(body) - snapshot.position(home)) - atlas::to_body(eye);
                    let direction = frame.transpose() * delta.normalize();
                    let cos_radius = (system.radius(body) / delta.length()).clamp(0.0, 0.95).asin().max(floor).cos();
                    result[slot] = direction.as_vec3().extend(cos_radius as f32).to_array();
                }
            }
        }
    result
}

pub fn spawn(cfg: &crate::world::WorldSampleConfig) -> Vec3 {
    if engine_worldgen::planetary::active(cfg) {
        return (engine_worldgen::planetary::spawn_point(cfg) / f64::from(cfg.voxel_meters())).as_vec3();
    }
    let mut best = (f32::MAX, 0, 0);
    for z in -8..=8 {
        for x in -8..=8 {
            let (x, z) = (x * 64, z * 64);
            let h = engine_worldgen::terrain_height(cfg, x, z);
            let meters = cfg.world_y(h);
            if meters > 16.0 && meters < best.0 { best = (meters, x, z); }
        }
    }
    // Ocean-heavy custom recipes may put the nearest land beyond the local grid.
    // Pick nearby dry solve cells, then verify against the walked terrain.
    if (cfg.planet_settings().is_some() || engine_worldgen::planetary::active(cfg)) && best.0 == f32::MAX {
        let planet = engine_worldgen::geo::planet_for(cfg);
        let mut candidates: Vec<_> = planet.height.iter().enumerate()
            .filter(|(i, h)| **h > 18.0 && planet.lake[*i] <= **h)
            .map(|(i, _)| {
                let [x, z] = planet.grid.at(i);
                (x * x + z * z, cfg.blocks_for_height(x as f32), cfg.blocks_for_height(z as f32))
            }).collect();
        candidates.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        for (_, x, z) in candidates.into_iter().take(32) {
            let h = engine_worldgen::terrain_height(cfg, x, z);
            let meters = cfg.world_y(h);
            if meters > 16.0 { best = (meters, x, z); break; }
        }
    }
    let (x, z) = (best.1, best.2);
    // No dry sample in the search grid leaves `best` at the origin, which may be under water.
    let floor = engine_worldgen::terrain_height(cfg, x, z).max(cfg.water_level());
    Vec3::new(x as f32 + 0.5, floor as f32 + 2.0, z as f32 + 0.5)
}

/// Heading with the widest view from a planet spawn, in the player's yaw convention (0 looks along -z).
///
/// The spawn cell is fixed by terrain, so a new player may stand with a hillside or cliff at the
/// nose. Sixteen headings are scored by the steepest terrain rise to the horizon; the lowest wins,
/// ties go to the heading nearest the default. Pure terrain sampling: no wall clock, no gate.
pub fn spawn_yaw(cfg: &crate::world::WorldSampleConfig, feet: Vec3) -> f32 {
    if !engine_worldgen::planetary::active(cfg) { return 0.0; }
    let eye_m = f64::from(cfg.world_y(feet.y.floor() as i32)) + 1.6;
    const REACH_M: [f64; 7] = [10.0, 20.0, 40.0, 80.0, 160.0, 320.0, 640.0];
    let scale = f64::from(cfg.voxel_meters());
    let mut best = (f64::MAX, 0.0_f32);
    for k in 0..16 {
        let yaw = k as f32 * std::f32::consts::TAU / 16.0;
        let (s, c) = (f64::from(yaw.sin()), f64::from(-yaw.cos()));
        let mut rise = f64::NEG_INFINITY;
        for d in REACH_M {
            let x = (f64::from(feet.x) + s * d / scale).floor() as i32;
            let z = (f64::from(feet.z) + c * d / scale).floor() as i32;
            let ground = f64::from(cfg.world_y(engine_worldgen::terrain_height(cfg, x, z)));
            rise = rise.max((ground - eye_m) / d);
        }
        // Wrap so 0 and just under tau count as equally close to the default.
        let turn = f64::from(yaw.min(std::f32::consts::TAU - yaw));
        let score = rise + turn * 1e-4;
        if score < best.0 { best = (score, yaw); }
    }
    best.1
}

#[cfg(test)]
mod tests {
    use engine_core::cube_sphere::atlas;

    #[test]
    fn solar_time_follows_longitude_and_runs_forward_at_every_latitude() {
        let r = atlas::RADIUS_M;
        for lat in [0.0_f64, 0.6, 1.2, 1.5] {
            let eye = |lon: f64| atlas::from_body(r * glam::DVec3::new(lat.cos() * lon.cos(), lat.sin(), -lat.cos() * lon.sin()));
            let t0 = super::solar_time(eye(0.0), 1_000);
            // Half a turn of longitude is half a day away.
            let opposite = (super::solar_time(eye(std::f64::consts::PI), 1_000) - t0).rem_euclid(1.0);
            assert!((opposite - 0.5).abs() < 0.01, "lat {lat}: opposite side {opposite}");
            // A tenth of a solar day later the clock moved a tenth forward.
            let later = (super::solar_time(eye(0.0), 1_000 + 5_760) - t0).rem_euclid(1.0);
            assert!((later - 0.1).abs() < 0.01, "lat {lat}: clock advanced {later}");
        }
    }

    #[test]
    fn hour_time_is_noon_under_the_sun() {
        let eye = glam::DVec3::new(1.0, 0.3, 0.0);
        assert!((super::hour_time(eye, glam::DVec3::new(5.0, -1.0, 0.0)) - 0.5).abs() < 1e-6);
        assert!(super::hour_time(eye, glam::DVec3::new(-5.0, 1.0, 0.0)).abs() < 1e-6);
    }

    #[test]
    fn skyhook_chart_longitudes_fall_outside_the_atlas() {
        // Ports live on the compressed Earth chart. The vertex shader must not
        // fold those positions through the six-face atlas: the face index
        // clamps and tan() of the leftover longitude is a different direction.
        let port = 0.5 * crate::world::orbital::megastructures::PLANET_RADIUS_M;
        let face = ((port + atlas::PITCH_M * 0.5) / atlas::PITCH_M).floor();
        assert!(face > 5.0, "chart longitude {port} m is atlas face {face}");
        let local = port - 5.0 * atlas::PITCH_M;
        let turns = local / atlas::HALF_M * 0.25;
        assert!(turns.abs() > 1.0, "leftover longitude stays inside one face quadrant");
    }

    #[test]
    fn the_giant_pole_rides_in_the_frame_rows() {
        let sky = super::camera(glam::DVec3::new(0.0, 60_000.0, 0.0), true, 9_000);
        let pole = glam::Vec3::new(sky[2][3], sky[3][3], sky[4][3]);
        assert!((pole.length() - 1.0).abs() < 1e-4, "pole {pole}");
    }

    /// Prints yaw and pitch toward the sun, moon and giant for review poses:
    /// `cargo test -p minecraft_style_voxel_prototype sky_body_poses -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn sky_body_poses() {
        for clearance in [6.0, 60_000.0] {
            for day in [0.0, 0.3, 0.6, 1.0, 1.5, 2.0] {
                for time in [0.0, 0.25, 0.3, 0.5, 0.75] {
                    let tick = ((day + time) * 480.0 * 120.0) as u64;
                    let sky = super::camera(glam::DVec3::new(0.0, clearance, 0.0), true, tick);
                    let pose = |row: [f32; 4]| {
                        let v = glam::Vec3::new(row[0], row[1], row[2]);
                        (v.x.atan2(-v.z), v.y.clamp(-1.0, 1.0).asin())
                    };
                    let moon = glam::Vec3::new(sky[6][0], sky[6][1], sky[6][2]);
                    let sun = glam::Vec3::new(sky[5][0], sky[5][1], sky[5][2]);
                    println!("clear={clearance} day={day} time={time} sun={:?} moon={:?} giant={:?} moon_lit={:.2}",
                        pose(sky[5]), pose(sky[6]), pose(sky[7]), 0.5 - 0.5 * moon.dot(sun));
                }
            }
        }
    }
}
