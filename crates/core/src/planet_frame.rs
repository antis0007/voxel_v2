//! Planet-local latitude, insolation and baseline climate.
//!
//! Every climate input is a function of the direction from the planet centre
//! measured against the spin axis, never of world axes, cube faces or the
//! camera. A rotated axis therefore rotates the climate belts with it.
use glam::DVec3;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// Spin axis and obliquity of one planet, in its body frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanetAxis {
    /// Unit north pole direction.
    pub north: DVec3,
    /// Angle between the spin axis and the orbit normal, radians.
    pub obliquity: f64,
}

impl PlanetAxis {
    /// Earth-like default: +Y north, 23.4 degree obliquity.
    pub const EARTHLIKE: Self = Self { north: DVec3::Y, obliquity: 23.4 * PI / 180.0 };

    /// Builds an axis, normalising `north`; a zero vector falls back to +Y.
    pub fn new(north: DVec3, obliquity: f64) -> Self {
        Self { north: north.try_normalize().unwrap_or(DVec3::Y), obliquity }
    }

    /// Geographic latitude in radians of a direction from the planet centre.
    pub fn latitude(&self, dir: DVec3) -> f64 {
        let r = dir.try_normalize().unwrap_or(self.north);
        r.dot(self.north).clamp(-1.0, 1.0).asin()
    }

    /// Latitude of the subsolar point at `year_fraction` (0 = northern spring equinox).
    pub fn subsolar_latitude(&self, year_fraction: f64) -> f64 {
        (self.obliquity.sin() * (TAU * year_fraction).sin()).asin()
    }

    /// Annual mean top-of-atmosphere insolation relative to the planet mean.
    ///
    /// Second-order Legendre fit (North 1975): exact to a few percent for
    /// obliquities below about 55 degrees, and it integrates to 1 over the sphere.
    pub fn annual_insolation(&self, latitude: f64) -> f64 {
        let p2 = |x: f64| 0.5 * (3.0 * x * x - 1.0);
        let s2 = -0.625 * p2(self.obliquity.cos());
        1.0 + s2 * p2(latitude.sin())
    }

    /// Daily mean insolation relative to the solar constant, given the subsolar latitude.
    pub fn daily_insolation(latitude: f64, subsolar: f64) -> f64 {
        let (sp, cp) = latitude.sin_cos();
        let (sd, cd) = subsolar.sin_cos();
        // Hour angle of sunset; clamped to polar night (0) and polar day (pi).
        let h0 = if cp.abs() < 1e-12 || cd.abs() < 1e-12 {
            if sp * sd > 0.0 { PI } else { 0.0 }
        } else {
            (-(sp / cp) * (sd / cd)).clamp(-1.0, 1.0).acos()
        };
        ((h0 * sp * sd + cp * cd * h0.sin()) / PI).max(0.0)
    }
}

/// Tunable linear response of surface temperature to insolation and height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClimateModel {
    /// Sea-level annual mean at the equator, degrees C.
    pub equator_c: f64,
    /// Sea-level annual mean at the poles, degrees C, for the reference obliquity.
    pub pole_c: f64,
    /// Cooling per metre of height above sea level.
    pub lapse_c_per_m: f64,
    /// Seasonal swing per unit of daily insolation anomaly, degrees C.
    pub seasonal_c: f64,
}

impl ClimateModel {
    /// Earth-like tuning: 27 C equator, -25 C poles, 6.5 C per km.
    pub const EARTHLIKE: Self = Self { equator_c: 27.0, pole_c: -25.0, lapse_c_per_m: 0.0065, seasonal_c: 38.0 };

    /// Linear fit through the reference axis so the end points stay where tuned.
    fn response(&self) -> (f64, f64) {
        let reference = PlanetAxis::EARTHLIKE;
        let s_eq = reference.annual_insolation(0.0);
        let s_pole = reference.annual_insolation(FRAC_PI_2);
        let gain = (self.equator_c - self.pole_c) / (s_eq - s_pole);
        (self.equator_c - gain * s_eq, gain)
    }

    /// Annual mean temperature at a direction and height above sea level.
    pub fn annual_c(&self, axis: &PlanetAxis, dir: DVec3, height_m: f64) -> f64 {
        let (offset, gain) = self.response();
        offset + gain * axis.annual_insolation(axis.latitude(dir)) - self.lapse_c_per_m * height_m.max(0.0)
    }

    /// Temperature at `year_fraction`: annual mean plus the daily insolation anomaly.
    pub fn seasonal_c(&self, axis: &PlanetAxis, dir: DVec3, height_m: f64, year_fraction: f64) -> f64 {
        let lat = axis.latitude(dir);
        let now = PlanetAxis::daily_insolation(lat, axis.subsolar_latitude(year_fraction));
        // Mean of the daily value over the year, sampled so the anomaly sums to zero.
        const SAMPLES: usize = 24;
        let mean = (0..SAMPLES)
            .map(|i| PlanetAxis::daily_insolation(lat, axis.subsolar_latitude(i as f64 / SAMPLES as f64)))
            .sum::<f64>()
            / SAMPLES as f64;
        self.annual_c(axis, dir, height_m) + self.seasonal_c * (now - mean)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deg(d: f64) -> f64 { d * PI / 180.0 }
    fn at_latitude(axis: &PlanetAxis, lat_deg: f64) -> DVec3 {
        let east = axis.north.any_orthonormal_vector();
        east * deg(lat_deg).cos() + axis.north * deg(lat_deg).sin()
    }

    #[test]
    fn both_poles_are_cold_and_the_equator_is_warmest() {
        let (axis, model) = (PlanetAxis::EARTHLIKE, ClimateModel::EARTHLIKE);
        let north = model.annual_c(&axis, DVec3::Y, 0.0);
        let south = model.annual_c(&axis, DVec3::NEG_Y, 0.0);
        let equator = model.annual_c(&axis, DVec3::X, 0.0);
        assert!((north - model.pole_c).abs() < 1e-9 && (south - model.pole_c).abs() < 1e-9);
        assert!((equator - model.equator_c).abs() < 1e-9);
        let mut last = equator;
        for lat in (10..=90).step_by(10) {
            let t = model.annual_c(&axis, at_latitude(&axis, f64::from(lat)), 0.0);
            assert!(t < last, "not cooling toward the pole at {lat}");
            last = t;
        }
    }

    #[test]
    fn annual_insolation_averages_to_one_over_the_sphere() {
        let axis = PlanetAxis::EARTHLIKE;
        // Area weight is cos(latitude); integrate in sin(latitude).
        let n = 2000;
        let mean = (0..n).map(|i| {
            let x = -1.0 + (i as f64 + 0.5) * 2.0 / n as f64;
            axis.annual_insolation(x.asin())
        }).sum::<f64>() / n as f64;
        assert!((mean - 1.0).abs() < 1e-6, "{mean}");
    }

    #[test]
    fn hemispheres_have_opposite_seasons() {
        let (axis, model) = (PlanetAxis::EARTHLIKE, ClimateModel::EARTHLIKE);
        let n = at_latitude(&axis, 45.0);
        let s = at_latitude(&axis, -45.0);
        let north_summer = model.seasonal_c(&axis, n, 0.0, 0.25);
        let north_winter = model.seasonal_c(&axis, n, 0.0, 0.75);
        assert!(north_summer > north_winter + 10.0);
        // Half a year later the southern hemisphere has the same season.
        assert!((model.seasonal_c(&axis, s, 0.0, 0.75) - north_summer).abs() < 1e-9);
        assert!((model.seasonal_c(&axis, s, 0.0, 0.25) - north_winter).abs() < 1e-9);
    }

    #[test]
    fn height_cools_at_the_lapse_rate() {
        let (axis, model) = (PlanetAxis::EARTHLIKE, ClimateModel::EARTHLIKE);
        let dir = at_latitude(&axis, 40.0);
        let drop = model.annual_c(&axis, dir, 0.0) - model.annual_c(&axis, dir, 3000.0);
        assert!((drop - 19.5).abs() < 1e-9);
        assert_eq!(model.annual_c(&axis, dir, -200.0), model.annual_c(&axis, dir, 0.0));
    }

    #[test]
    fn rotated_axis_rotates_the_climate() {
        let model = ClimateModel::EARTHLIKE;
        let tilted = PlanetAxis::new(DVec3::new(0.0, 1.0, -1.0), deg(23.4));
        let rotation = glam::DQuat::from_rotation_arc(DVec3::Y, tilted.north);
        for lat in [-80.0, -30.0, 0.0, 30.0, 80.0] {
            let local = at_latitude(&PlanetAxis::EARTHLIKE, lat);
            let a = model.annual_c(&PlanetAxis::EARTHLIKE, local, 0.0);
            let b = model.annual_c(&tilted, rotation * local, 0.0);
            assert!((a - b).abs() < 1e-9, "latitude {lat}");
        }
        // World +Y is no longer a pole on the tilted planet.
        assert!(model.annual_c(&tilted, DVec3::Y, 0.0) > model.pole_c + 10.0);
    }

    #[test]
    fn climate_is_continuous_across_cube_face_edges() {
        let (axis, model) = (PlanetAxis::EARTHLIKE, ClimateModel::EARTHLIKE);
        // Directions straddling the +X/+Z and +X/+Y face edges, one micro-radian apart.
        for edge in [DVec3::new(1.0, 0.3, 1.0), DVec3::new(1.0, 1.0, -0.4), DVec3::new(-1.0, -1.0, 0.7)] {
            let side = edge.normalize().any_orthonormal_vector() * 1e-6;
            let a = model.seasonal_c(&axis, edge - side, 0.0, 0.3);
            let b = model.seasonal_c(&axis, edge + side, 0.0, 0.3);
            assert!((a - b).abs() < 1e-3);
        }
    }

    #[test]
    fn polar_night_and_polar_day_are_bounded() {
        let summer = PlanetAxis::EARTHLIKE.subsolar_latitude(0.25);
        assert_eq!(PlanetAxis::daily_insolation(deg(-89.0), summer), 0.0);
        let pole_day = PlanetAxis::daily_insolation(deg(90.0), summer);
        assert!((pole_day - summer.sin()).abs() < 1e-9);
        for lat in (-90..=90).step_by(5) {
            for f in 0..12 {
                let q = PlanetAxis::daily_insolation(deg(f64::from(lat)), PlanetAxis::EARTHLIKE.subsolar_latitude(f64::from(f) / 12.0));
                assert!(q.is_finite() && (0.0..=1.0).contains(&q));
            }
        }
    }
}
