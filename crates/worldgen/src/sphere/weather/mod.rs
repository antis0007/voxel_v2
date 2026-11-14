//! Planet-scale weather on the cube-sphere grid: pressure, wind, humidity, cloud.
//!
//! A reduced one-layer model, not a general circulation model. Pressure
//! relaxes toward the Hadley/Ferrel belt profile (equatorial low, subtropical
//! highs, subpolar lows, polar highs) plus thermal lows over sun-warmed land.
//! Wind follows the pressure gradient, Coriolis force and drag, so highs and
//! lows turn the right way in each hemisphere and trade winds and westerlies
//! appear without being painted in. Transient depressions kicked onto the
//! storm tracks are carried east by the jet and spiral up as cyclones. Humidity
//! evaporates from ocean, is advected, and condenses into cloud where flow
//! converges, rises over relief or cools. Time is in units of 1/Omega of an
//! Earth-like spin; nothing here reads a clock.

mod dynamics;
pub mod clouds;
mod forcing;
mod moisture;
mod ops;
pub mod storms;

use engine_core::Budget;
use engine_core::planet_frame::PlanetAxis;
use glam::DVec3;
use std::sync::Arc;

use super::SphereGrid;
use ops::Operators;

/// Seconds-free integration step.
pub const DT: f64 = 0.05;
/// Squared gravity-wave speed; sets the Rossby deformation radius near 0.1 rad.
const WAVE_SPEED_SQ: f64 = 0.0225;

/// Per-cell surface inputs, by dense `SphereGrid` index.
#[derive(Clone, Debug)]
pub struct Surface {
    /// Land fraction in `0..=1`; zero is open ocean.
    pub land: Vec<f32>,
    /// Terrain height above sea level scaled to `0..=1` over the tallest relief.
    pub relief: Vec<f32>,
    /// Long-term land moisture controlling evaporation; neutral at 0.5.
    pub land_moisture: Vec<f32>,
}

impl Surface {
    /// A planet that is all ocean.
    pub fn ocean(len: usize) -> Self {
        Self { land: vec![0.0; len], relief: vec![0.0; len], land_moisture: vec![0.5; len] }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct WeatherParams {
    pub seed: u32,
    pub axis: PlanetAxis,
    /// Year phase for the sun's declination, 0 = northern spring equinox.
    pub year_fraction: f64,
    /// Spin rate relative to Earth; faster means narrower belts and more of them.
    pub spin: f64,
    /// Global moisture dial in `0..=1`; 0.5 is Earth-like.
    pub wetness: f64,
}

impl WeatherParams {
    pub fn earthlike(seed: u32) -> Self {
        Self { seed, axis: PlanetAxis::EARTHLIKE, year_fraction: 0.15, spin: 1.0, wetness: 0.5 }
    }
}

pub struct WeatherSim {
    ops: Arc<Operators>,
    surface: Surface,
    params: WeatherParams,
    /// Pressure anomaly; high where positive.
    h: Vec<f64>,
    v: Vec<DVec3>,
    temp: Vec<f64>,
    humidity: Vec<f64>,
    cloud: Vec<f64>,
    div: Vec<f64>,
    /// Pressure the cell relaxes toward, rebuilt as the year phase changes.
    h_eq: Vec<f64>,
    temp_eq: Vec<f64>,
    /// Terrain slope per cell, for upslope lift.
    slope: Vec<DVec3>,
    time: f64,
    steps: u64,
    scratch: dynamics::Scratch,
}

/// Immutable cloud samples for a bake; shares only the grid with the solver.
pub struct CloudCover {
    ops: Arc<Operators>,
    values: Vec<f32>,
    mean: f32,
}

impl CloudCover {
    pub fn mean(&self) -> f32 {
        self.mean
    }

    pub fn sample(&self, dir: DVec3, hint: &mut usize) -> f32 {
        sample_field(&self.ops, &self.values, dir, hint)
    }
}

impl WeatherSim {
    /// Builds the model at rest, in thermal equilibrium; `None` if `surface` does not match `grid`.
    pub fn new(grid: &SphereGrid, surface: Surface, params: WeatherParams) -> Option<Self> {
        let len = grid.len();
        if surface.land.len() != len || surface.relief.len() != len || surface.land_moisture.len() != len {
            return None;
        }
        let ops = Operators::new(grid);
        let relief: Vec<f64> = surface.relief.iter().map(|&r| f64::from(r)).collect();
        let mut slope = vec![DVec3::ZERO; len];
        ops.gradient(&relief, &mut slope);
        let coriolis = ops.centre.iter().map(|n|
            (-(2.0 * params.spin) * n.dot(params.axis.north) * DT).sin_cos()
        ).collect();
        let mut sim = Self {
            ops: Arc::new(ops),
            surface,
            params,
            h: vec![0.0; len],
            v: vec![DVec3::ZERO; len],
            temp: vec![0.0; len],
            humidity: vec![0.0; len],
            cloud: vec![0.0; len],
            div: vec![0.0; len],
            h_eq: vec![0.0; len],
            temp_eq: vec![0.0; len],
            slope,
            time: 0.0,
            steps: 0,
            scratch: dynamics::Scratch {
                sources: Vec::with_capacity(len),
                gradient: vec![DVec3::ZERO; len],
                scalar_in: vec![0.0; len],
                scalar_out: vec![0.0; len],
                wind: vec![DVec3::ZERO; len],
                coriolis,
            },
        };
        sim.rebuild_forcing();
        sim.h.clone_from(&sim.h_eq);
        sim.temp.clone_from(&sim.temp_eq);
        for i in 0..len {
            sim.humidity[i] = 0.6 * moisture::saturation(sim.temp[i]);
        }
        Some(sim)
    }

    /// Moves the sun to a new year phase; the fields adjust over the next few time units.
    pub fn set_year_fraction(&mut self, year_fraction: f64) {
        self.params.year_fraction = year_fraction;
        self.rebuild_forcing();
    }

    /// Simulated time in model units.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Advances one fixed step.
    pub fn step(&mut self) {
        self.step_dynamics();
        self.step_moisture();
        self.time += DT;
        self.steps += 1;
    }

    /// Steps until `units` of model time pass or the budget runs out; returns steps taken.
    pub fn advance(&mut self, units: f64, budget: Budget) -> u64 {
        let target = self.time + units;
        let mut taken = 0u64;
        while self.time < target {
            if !budget.has_time() {
                break;
            }
            self.step();
            taken += 1;
        }
        taken
    }

    pub fn len(&self) -> usize {
        self.h.len()
    }

    pub fn is_empty(&self) -> bool {
        self.h.is_empty()
    }

    /// Pressure anomaly per cell.
    pub fn pressure(&self) -> &[f64] {
        &self.h
    }

    /// Tangent wind per cell, radii per time unit.
    pub fn wind(&self) -> &[DVec3] {
        &self.v
    }

    pub fn centre(&self, i: usize) -> DVec3 {
        self.ops.centre[i]
    }

    /// Cloud cover per cell in `0..=1`.
    pub fn cover(&self) -> Vec<f32> {
        let gain = 0.8 + 1.2 * self.params.wetness;
        self.cloud.iter().map(|c| (1.0 - (-gain * c).exp()).clamp(0.0, 1.0) as f32).collect()
    }

    pub fn cover_snapshot(&self) -> CloudCover {
        let values = self.cover();
        let mean = values.iter().sum::<f32>() / values.len().max(1) as f32;
        CloudCover { ops: Arc::clone(&self.ops), values, mean }
    }

    /// Smooth value of a per-cell `field` at unit direction `dir`.
    ///
    /// `hint` is the cell found by the previous call; neighbouring samples
    /// then resolve in a step or two. Reconstruction is linear and limited to
    /// the neighbouring values, so it never overshoots.
    pub fn sample(&self, field: &[f32], dir: DVec3, hint: &mut usize) -> f32 {
        sample_field(&self.ops, field, dir, hint)
    }
}

fn sample_field(ops: &Operators, field: &[f32], dir: DVec3, hint: &mut usize) -> f32 {
    let cell = ops.locate((*hint).min(ops.len() - 1), dir);
    *hint = cell;
    let centre = f64::from(field[cell]);
    let (mut lo, mut hi) = (centre, centre);
    let mut g = DVec3::ZERO;
    for (k, &j) in ops.nb[cell].iter().enumerate() {
        let value = f64::from(field[j as usize]);
        lo = lo.min(value);
        hi = hi.max(value);
        g += ops.coef_at(cell, k) * (value - centre);
    }
    (centre + g.dot(dir - ops.centre[cell])).clamp(lo, hi) as f32
}
