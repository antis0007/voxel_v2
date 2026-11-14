//! Planet-scale weather shared by the orbital shell, the cloud march and play.
//!
//! Climate (latitude belts) is fixed; synoptic weather (continental noise and
//! cyclones) rides an eastward steering flow and evolves on the synced day
//! clock, so every peer and every renderer reads the same storms. The result
//! adjusts the day's `CloudControls` per position; the clipmap and the shell
//! both evaluate the local field with those adjusted controls.
//!
//! On a planet world the storms are the shell's own catalogue
//! (`weather::planet_sky`: lows, tropical cyclones, tornadoes); the hashed
//! cyclones below are only the fallback for worlds without a planet sky, with
//! their dials in `assets/climate.ron` (`hills`).
use super::CloudControls;
use engine_core::cube_sphere::atlas;
use engine_core::hash::hash01;
use engine_worldgen::value_noise3;
use glam::{DVec2, DVec3, Vec3};

/// Body-frame north pole: the home face is +y with north toward chart -z
/// (`GLOBE_NORTH` in globe_shell.wgsl).
pub const NORTH: DVec3 = DVec3::new(0.0, 0.0, -1.0);
/// Angular wavelength of the largest coverage noise: about 38 km on the 30.6 km sphere.
const LARGE_SCALE: f32 = 5.0;
/// Latitude of the storm tracks and the dry belts, radians.
const STORM_TRACK_LAT: f32 = 0.95;
const DRY_BELT_LAT: f32 = 0.42;
/// Eastward steering of the fallback synoptic frame, radians per day.
pub fn steering() -> f64 { hills().steering_rad_per_day }
fn hills() -> &'static engine_content::climate::HillsSynopticDials { &engine_content::climate::storm_dials().hills }
/// Distance the noise domain travels per day, so systems form and decay as they drift.
const MORPH_PER_DAY: f32 = 0.12;
/// Area-weighted mean of the raw cover bias, removed so the day's dial stays the planet mean.
const COVER_MEAN: f32 = 0.0;

/// Synoptic state at one point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Regional {
    /// Added to the day's coverage dial, about -0.5..0.6.
    pub cover: f32,
    /// Cumulonimbus share of a cyclone core, 0..1; the day's own share is the floor.
    pub storm: f32,
}

/// Weather clock in days from the saved, replicated day counter and time of day.
pub fn days(day: &crate::world::DayNightCycle) -> f64 {
    f64::from(day.day_index()) + f64::from(day.time_of_day())
}

/// Unit body direction of a world (chart) position in metres.
pub fn body_dir(world_xz_m: DVec2) -> DVec3 {
    atlas::to_body(DVec3::new(world_xz_m.x, 0.0, world_xz_m.y)).normalize()
}

fn gauss(x: f32, width: f32) -> f32 {
    (-(x / width) * (x / width)).exp()
}

/// Latitude-only climate: wet equator, dry subtropics, wet storm tracks, polar cloud.
pub fn zonal(lat: f32) -> f32 {
    let a = lat.abs();
    0.18 * gauss(lat, 0.10) - 0.32 * gauss(a - DRY_BELT_LAT, 0.13) + 0.26 * gauss(a - STORM_TRACK_LAT, 0.26)
        + 0.10 * ((a - 1.25) / 0.15).clamp(0.0, 1.0)
}

fn fbm3(p: Vec3, seed: u32) -> f32 {
    let mut value = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for octave in 0..4u32 {
        let q = p * freq;
        value += value_noise3(q.x, q.y, q.z, seed ^ (octave + 1).wrapping_mul(0x9E37_79B9)) * amp;
        norm += amp;
        freq *= 2.1;
        amp *= 0.5;
    }
    value / norm
}

/// Centre of cyclone `i` in its `epoch`, on a storm track in the steered frame.
fn cyclone_centre(seed: u32, i: u32, epoch: i64) -> DVec3 {
    let key = seed ^ i.wrapping_mul(0x51ED_27D5) ^ (epoch as u32).wrapping_mul(0x2C1B_3C6D);
    let lon = f64::from(hash01(key)) * std::f64::consts::TAU;
    let hemi = if i % 2 == 0 { 1.0 } else { -1.0 };
    let lat = hemi * (f64::from(STORM_TRACK_LAT) + (f64::from(hash01(key ^ 0x2545_F491)) - 0.5) * 0.3);
    let east = DVec3::X;
    let up = NORTH.cross(east).normalize();
    (east * lon.cos() + up * lon.sin()) * lat.cos() + NORTH * lat.sin()
}

/// Epoch, centre and 0..1 strength of cyclone `i` at `days`. Strength is zero at
/// both ends of a life, so a cyclone never jumps when it respawns elsewhere.
fn cyclone_state(seed: u32, i: u32, days: f64) -> (DVec3, f32) {
    let t = days / hills().life_days + f64::from(hash01(seed ^ i.wrapping_mul(0x9E37_79B9) ^ 0xC1C1));
    let epoch = t.floor() as i64;
    let life = (std::f64::consts::PI * (t - t.floor())).sin();
    (cyclone_centre(seed, i, epoch), (life * life) as f32)
}

/// Cyclone eye strength and spiral band cover at `spun`, a steered-frame direction.
fn cyclones(spun: DVec3, seed: u32, days: f64) -> (f32, f32) {
    let (mut core, mut band) = (0.0f32, 0.0f32);
    let radius = hills().radius_rad;
    for i in 0..hills().cyclones {
        let (c, strength) = cyclone_state(seed, i, days);
        let d = spun.dot(c).clamp(-1.0, 1.0).acos() as f32;
        if strength <= 0.0 || d >= radius * 2.0 {
            continue;
        }
        let east = NORTH.cross(c).normalize();
        let north = c.cross(east);
        let rel = spun - c * spun.dot(c);
        let theta = rel.dot(north).atan2(rel.dot(east)) as f32;
        // Arms wind the opposite way in each hemisphere.
        let hemi = if c.dot(NORTH) >= 0.0 { 1.0 } else { -1.0 };
        let arms = 0.5 + 0.5 * (2.0 * theta - hemi * d * 28.0).cos();
        core = core.max(strength * gauss(d, radius * 0.35));
        band += strength * gauss(d, radius) * arms;
    }
    (core, band.min(1.0))
}

fn rotate_about_north(v: DVec3, angle: f64) -> DVec3 {
    let (s, c) = angle.sin_cos();
    let along = NORTH * v.dot(NORTH);
    let rest = v - along;
    along + rest * c + NORTH.cross(rest) * s
}

/// Planet world whose weather is the shell's storm catalogue.
pub type Planet = Option<engine_voxel::WorldSampleConfig>;

fn planet_sky(planet: Planet, days: f64, seed: u32) -> Option<std::sync::Arc<engine_worldgen::sphere::CloudSky>> {
    planet.filter(engine_worldgen::planetary::active).map(|p| super::planet_sky(seed, Some(p), days))
}

/// Mean of `CloudSky::density` over the sphere, removed so the day's dial stays the planet mean.
const SKY_DENSITY_MEAN: f32 = 0.25;

/// Synoptic state at body direction `dir`: the planet sky's storms on a planet
/// world, the fallback cyclones elsewhere.
pub fn regional_at(planet: Planet, dir: DVec3, days: f64, seed: u32) -> Regional {
    match planet_sky(planet, days, seed) {
        Some(sky) => Regional {
            cover: 1.1 * (sky.density(dir) - SKY_DENSITY_MEAN),
            storm: sky.storm_share(dir) as f32,
        },
        None => regional(dir, days, seed),
    }
}

/// Closeness to a tornado at `world_xz_m`, 0..1 (one at a core); zero without a planet sky.
pub fn tornado_at(planet: Planet, world_xz_m: DVec2, days: f64, seed: u32) -> f32 {
    planet_sky(planet, days, seed).map_or(0.0, |sky| sky.tornado_share(body_dir(world_xz_m)) as f32)
}

/// Fallback synoptic state at body direction `dir` on weather day `days`.
pub fn regional(dir: DVec3, days: f64, seed: u32) -> Regional {
    let lat = dir.dot(NORTH).clamp(-1.0, 1.0).asin() as f32;
    // The whole synoptic frame turns east; f64 keeps long clocks exact.
    let spun = rotate_about_north(dir, -(days * steering()).rem_euclid(std::f64::consts::TAU));
    let morph = Vec3::new(0.0, (days * f64::from(MORPH_PER_DAY)) as f32, 0.0);
    let s = spun.as_vec3();
    let large = fbm3(s * LARGE_SCALE + morph, seed ^ 0xC10D_0001);
    let streaks = fbm3(Vec3::new(s.x * 0.6, s.y * 0.6, s.z * 3.0) * LARGE_SCALE + morph, seed ^ 0xC10D_0002);
    let systems = fbm3(s * LARGE_SCALE * 3.0 + morph * 2.0, seed ^ 0xC10D_0003);
    // Belts wander with the streak noise so no edge runs straight along a parallel.
    let lat = lat + 0.2 * (streaks - 0.5);
    let (core, band) = cyclones(spun, seed, days);
    let cover = zonal(lat) * (0.6 + 0.8 * large) + 0.55 * (large - 0.5) + 0.3 * (systems - 0.5) + 0.5 * band
        - COVER_MEAN;
    Regional { cover, storm: (1.2 * core).min(1.0) }
}

/// The day's controls adjusted by the synoptic state at one point.
///
/// Only coverage and cumulonimbus change. Cumulonimbus never drops below the
/// day's share, so precipitation outside cyclone cores is exactly what the day
/// gives; storm cores always carry the cloud they rain from.
pub fn apply(controls: CloudControls, r: Regional) -> CloudControls {
    let cumulonimbus = controls.cumulonimbus.max(r.storm);
    let coverage = (controls.coverage + r.cover).max(cumulonimbus * 0.9).clamp(0.02, 0.98);
    CloudControls { coverage, cumulonimbus, ..controls }
}

/// Controls in force at world position `world_xz_m` on weather day `days`.
pub fn controls_at(controls: CloudControls, planet: Planet, world_xz_m: DVec2, days: f64, seed: u32) -> CloudControls {
    apply(controls, regional_at(planet, body_dir(world_xz_m), days, seed))
}

#[cfg(test)]
#[path = "synoptic_tests.rs"]
mod tests;
