//! Equilibrium pressure and temperature, and the transient depressions.

use engine_core::hash::hash01;
use engine_core::planet_frame::PlanetAxis;
use glam::DVec3;
use std::f64::consts::TAU;

use super::WeatherSim;

/// Amplitude of the zonal pressure belts.
const BELT_AMPLITUDE: f64 = 1.0;
/// Pressure drop per unit of land-warming anomaly.
const THERMAL_LOW: f64 = 3.0;
/// Depth of a kicked depression and its half-width in radians.
const KICK_DEPTH: f64 = 0.8;
const KICK_WIDTH: f64 = 0.14;
/// Model time between kicks.
pub(super) const KICK_PERIOD: f64 = 2.0;
const TEMP_LAPSE: f64 = 0.35;

impl WeatherSim {
    pub(super) fn rebuild_forcing(&mut self) {
        let axis = self.params.axis;
        let subsolar = axis.subsolar_latitude(self.params.year_fraction);
        // Equator-to-pole belts: -cos(6 lat) is low at 0 and 60 degrees, high at 30 and 90.
        let belts = |lat: f64| -(6.0 * (lat - 0.35 * subsolar)).cos();
        for i in 0..self.ops.len() {
            let lat = axis.latitude(self.ops.centre[i]);
            let now = PlanetAxis::daily_insolation(lat, subsolar);
            let mean = mean_insolation(&axis, lat);
            let land = f64::from(self.surface.land[i]);
            let relief = f64::from(self.surface.relief[i]);
            self.h_eq[i] = BELT_AMPLITUDE * belts(lat) - THERMAL_LOW * land * (now - mean);
            let sun = (now / 0.45).clamp(0.0, 1.0).powf(0.6);
            self.temp_eq[i] = (0.08 + 0.92 * sun - TEMP_LAPSE * relief).clamp(0.0, 1.0);
        }
    }

    /// Adds storm-track lows, warm-ocean lows and broad subtropical highs.
    pub(super) fn kick(&mut self, slot: u64) {
        let seed = self.params.seed;
        let key = |salt: u32| hash01(seed ^ (slot as u32).wrapping_mul(0x9E37_79B1) ^ salt.wrapping_mul(0x85EB_CA77));
        let north = self.params.axis.north;
        let east0 = north.cross(if north.x.abs() < 0.8 { DVec3::X } else { DVec3::Y }).normalize();
        let east1 = north.cross(east0);
        let place = |lat: f64, lon: f64| (east0 * lon.cos() + east1 * lon.sin()) * lat.cos() + north * lat.sin();
        let mut spots = Vec::with_capacity(6);
        for (k, hemi) in [(0u32, 1.0), (1, -1.0)] {
            let lat = hemi * (0.6 + 0.55 * f64::from(key(10 + k)));
            spots.push((place(lat, TAU * f64::from(key(20 + k))), KICK_DEPTH));
            // A positive pressure anomaly gives a localized anticyclone,
            // rather than only the axisymmetric subtropical high belt.
            let high_lat = hemi * (0.33 + 0.20 * f64::from(key(50 + k)));
            spots.push((place(high_lat, TAU * f64::from(key(60 + k))), -0.55 * KICK_DEPTH));
        }
        for k in 0..2u32 {
            let hemi = if key(30 + 3 * k) < 0.5 { 1.0 } else { -1.0 };
            let c = place(hemi * (0.1 + 0.3 * f64::from(key(31 + 3 * k))), TAU * f64::from(key(32 + 3 * k)));
            let wet = self.ops.centre.iter().zip(&self.surface.land).filter(|(p, _)| p.dot(c) > 0.995).all(|(_, &l)| l < 0.3);
            if wet {
                spots.push((c, 0.7 * KICK_DEPTH));
            }
        }
        for (c, depth) in spots {
            for i in 0..self.ops.len() {
                let d = self.ops.centre[i].dot(c).clamp(-1.0, 1.0).acos();
                if d < 3.0 * KICK_WIDTH {
                    self.h[i] -= depth * (-(d / KICK_WIDTH) * (d / KICK_WIDTH)).exp();
                }
            }
        }
    }
}

fn mean_insolation(axis: &PlanetAxis, lat: f64) -> f64 {
    const SAMPLES: usize = 12;
    (0..SAMPLES).map(|k| PlanetAxis::daily_insolation(lat, axis.subsolar_latitude(k as f64 / SAMPLES as f64))).sum::<f64>() / SAMPLES as f64
}
