//! Planet shell climate measured against the spin axis, in degrees Celsius.
//!
//! Temperature comes from annual insolation at the latitude, rain from the
//! three circulation belts, and cover keeps snowpack, ice sheets, polar desert
//! and sea ice apart instead of one white threshold.
use crate::BlockId;
use crate::planet_profile::profile;
use engine_core::planet_frame::{ClimateModel, PlanetAxis};
use glam::DVec3;
use std::sync::OnceLock;

/// Axis climate and the cover split. Older planets keep the noise-weighted field.
pub const VERSION: u32 = 131;

/// Shell relief tops out near 560 m, a tenth of Earth's, so the lapse is
/// steepened until high ranges hold glaciers below the polar belt.
pub const LAPSE_C_PER_M: f64 = 0.03;

/// Earth's poles average about -15 C at sea level once ocean heat transport is
/// counted; planet_frame's -25 C default has no transport term.
const MODEL: ClimateModel = ClimateModel { pole_c: -15.0, ..ClimateModel::EARTHLIKE };

/// The model's swing fits a damped ocean; land winters at 45 degrees reach about -2 C with this.
const WINTER_GAIN: f64 = 2.0;

/// Sea-level climate of one direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Climate {
    /// Annual mean, degrees C.
    pub annual_c: f64,
    /// Mean of the coldest part of the year, degrees C.
    pub winter_c: f64,
    /// Precipitation in `0.0..=1.0`.
    pub wet: f64,
}

impl Climate {
    /// The same climate `lift` metres above the shore.
    pub fn lifted(self, lift: f64) -> Climate {
        let cool = LAPSE_C_PER_M * lift.max(0.0);
        Climate { annual_c: self.annual_c - cool, winter_c: self.winter_c - cool, ..self }
    }

    /// Legacy 0..1 heat used by relief and woodland rules: -25 C to 27 C.
    pub fn heat01(self) -> f64 {
        ((self.annual_c + 25.0) / 52.0).clamp(0.0, 1.0)
    }
}

/// Coldest seasonal anomaly per whole degree of latitude, -90..=90.
fn winter_anomaly(latitude: f64) -> f64 {
    static TABLE: OnceLock<Vec<f64>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let axis = PlanetAxis::EARTHLIKE;
        let east = axis.north.any_orthonormal_vector();
        (-90..=90)
            .map(|deg| {
                let lat = f64::from(deg).to_radians();
                let dir = east * lat.cos() + axis.north * lat.sin();
                let annual = MODEL.annual_c(&axis, dir, 0.0);
                (0..24)
                    .map(|i| MODEL.seasonal_c(&axis, dir, 0.0, f64::from(i) / 24.0) - annual)
                    .fold(0.0, f64::min)
            })
            .collect()
    });
    let x = (latitude.to_degrees() + 90.0).clamp(0.0, 180.0);
    let i = (x.floor() as usize).min(179);
    let f = x - i as f64;
    table[i] * (1.0 - f) + table[i + 1] * f
}

/// Rain belts by latitude: wet equator, dry subtropics, storm tracks near 55 degrees, dry poles.
fn belt_wet(latitude_deg: f64) -> f64 {
    let a = latitude_deg.abs();
    let bump = |centre: f64, width: f64| (-((a - centre) / width).powi(2)).exp();
    0.30 + 0.45 * bump(0.0, 12.0) - 0.22 * bump(27.0, 9.0) + 0.25 * bump(55.0, 14.0) - 0.22 * bump(90.0, 18.0)
}

/// Sea-level climate at a unit direction from the planet centre.
///
/// `land` is the continental signal above the sea line; interiors dry out and swing harder.
pub fn at(seed: u32, axis: &PlanetAxis, dir: DVec3, land: f64) -> Climate {
    let dials = profile(seed);
    let noise = |f: f64, salt: u32| crate::planet::noise_at(dir * f, seed.wrapping_add(salt)) * 2.0 - 1.0;
    let lat = axis.latitude(dir);
    // Regional terms are a few degrees at continental scale, never patch-sized.
    let regional = noise(2.6, 11) * 3.0 + f64::from(dials.temperature - 0.5) * 10.0;
    let interior = land.clamp(0.0, 1.0);
    let annual_c = MODEL.annual_c(axis, dir, 0.0) + regional - interior * 2.0;
    let winter_c = annual_c + winter_anomaly(lat) * WINTER_GAIN * (0.7 + 0.6 * interior);
    let wet = belt_wet(lat.to_degrees()) + noise(5.0, 29) * 0.18 + noise(15.5, 36) * 0.06
        + f64::from(dials.rainfall - 0.5) * 0.3
        - interior * 0.18;
    Climate { annual_c, winter_c, wet: wet.clamp(0.0, 1.0) }
}

/// Immutable terms for one exact seed, axis and direction. Keep the wet sum
/// unclamped: changing land can bring a formerly saturated value back in range.
pub(crate) struct ClimateBasis {
    annual_base: f64,
    winter_anomaly: f64,
    wet_base: f64,
}

impl ClimateBasis {
    pub(crate) fn new(seed: u32, axis: &PlanetAxis, dir: DVec3) -> Self {
        let dials = profile(seed);
        let noise = |f: f64, salt: u32| crate::planet::noise_at(dir * f, seed.wrapping_add(salt)) * 2.0 - 1.0;
        let lat = axis.latitude(dir);
        let regional = noise(2.6, 11) * 3.0 + f64::from(dials.temperature - 0.5) * 10.0;
        let annual_base = MODEL.annual_c(axis, dir, 0.0) + regional;
        let winter_anomaly = winter_anomaly(lat);
        let wet_base = belt_wet(lat.to_degrees()) + noise(5.0, 29) * 0.18 + noise(15.5, 36) * 0.06
            + f64::from(dials.rainfall - 0.5) * 0.3;
        Self { annual_base, winter_anomaly, wet_base }
    }

    #[inline]
    pub(crate) fn at_land(&self, land: f64) -> Climate {
        let interior = land.clamp(0.0, 1.0);
        let annual_c = self.annual_base - interior * 2.0;
        let winter_c = annual_c + self.winter_anomaly * WINTER_GAIN * (0.7 + 0.6 * interior);
        let wet = self.wet_base - interior * 0.18;
        Climate { annual_c, winter_c, wet: wet.clamp(0.0, 1.0) }
    }
}

/// Whether the sea surface above a column freezes over.
pub fn sea_ice(c: Climate) -> bool {
    c.winter_c < -8.0
}

/// Surface, filler and woodland density for a land or sea-floor column.
pub fn cover(c: Climate, metres: f64, shore: f64) -> (BlockId, BlockId, f32) {
    use BlockId::*;
    let c = c.lifted(metres - shore);
    if metres <= shore {
        return if c.annual_c < 2.0 { (Gravel, Stone, 0.0) } else if c.wet > 0.70 { (Mud, Clay, 0.10) } else { (Sand, Sandstone, 0.0) };
    }
    // Permanent ice needs both a year below freezing and snowfall to build it.
    if c.annual_c < -8.0 {
        return if c.wet > 0.22 { (Ice, Stone, 0.0) } else { (Gravel, Stone, 0.0) };
    }
    let snow = c.winter_c < -2.0 && c.wet > 0.30;
    if metres > 260.0 && !snow {
        return (Stone, Gravel, 0.03);
    }
    if c.annual_c < 0.0 {
        return match (snow, c.wet > 0.35) {
            (true, _) => (SnowGrass, Dirt, 0.04),
            (false, true) => (Grass, Dirt, 0.05),
            (false, false) => (Gravel, Dirt, 0.02),
        };
    }
    if c.annual_c < 6.0 {
        let density = (0.15 + c.wet * 0.75) as f32;
        return (if snow { SnowGrass } else { Grass }, Dirt, density);
    }
    if c.annual_c > 18.0 {
        return match c.wet {
            m if m < 0.22 => (Sand, Sandstone, 0.0),
            m if m < 0.34 => (Sandstone, Clay, 0.02),
            m if m < 0.55 => (Grass, Clay, 0.14),
            _ => (Grass, Dirt, 0.92),
        };
    }
    match c.wet {
        m if m < 0.24 => (Gravel, Stone, 0.05),
        m if m > 0.74 && metres < shore + 40.0 => (Mud, Clay, 0.45),
        m if m > 0.50 => (if snow { SnowGrass } else { Grass }, Dirt, 0.70),
        _ => (Grass, Dirt, 0.24),
    }
}

#[cfg(test)]
#[path = "planet_climate_tests.rs"]
mod tests;

#[cfg(test)]
mod climate_basis_tests {
    use super::*;

    fn bits(c: Climate) -> [u64; 3] {
        [c.annual_c, c.winter_c, c.wet].map(f64::to_bits)
    }

    #[test]
    fn immutable_basis_matches_unchanged_at_across_land_and_direction_boundaries() {
        let mut directions = vec![DVec3::X, -DVec3::X, DVec3::Y, -DVec3::Y, DVec3::Z, -DVec3::Z];
        for x in -8..=8 {
            for y in -8..=8 {
                let (a, b) = (x as f64 / 8.0, y as f64 / 8.0);
                for face in [-1.0, 1.0] {
                    directions.extend([DVec3::new(a, b, face).normalize(),
                        DVec3::new(face, a, b).normalize(), DVec3::new(b, face, a).normalize()]);
                }
            }
        }
        for seed in [0, 74, 731, u32::MAX] {
            for (i, &dir) in directions.iter().enumerate() {
                let basis = ClimateBasis::new(seed, &PlanetAxis::EARTHLIKE, dir);
                for land in [f64::NEG_INFINITY, -1.0, -0.0, 0.0, 1e-12, 0.25, 0.7,
                    1.0 - 1e-12, 1.0, 1.0 + 1e-12, 2.0, f64::INFINITY] {
                    // The original public function is deliberately untouched and
                    // neither constructs nor calls ClimateBasis for its result.
                    let expected = at(seed, &PlanetAxis::EARTHLIKE, dir, land);
                    assert_eq!(bits(basis.at_land(land)), bits(expected),
                        "seed={seed}, direction={i}, land={land}");
                }
            }
        }
    }

    #[test]
    fn immutable_basis_retains_preclamp_wet_value() {
        let basis = ClimateBasis { annual_base: 10.0, winter_anomaly: -5.0, wet_base: 1.1 };
        assert_eq!(basis.at_land(1.0).wet.to_bits(), (1.1_f64 - 0.18).clamp(0.0, 1.0).to_bits());
        let invalid_clamped_cache = (basis.at_land(0.0).wet - 0.18).clamp(0.0, 1.0);
        assert_ne!(basis.at_land(1.0).wet.to_bits(), invalid_clamped_cache.to_bits());
    }
}
