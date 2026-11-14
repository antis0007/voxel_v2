//! Equal-area, diffusive energy balance, with temperature-dependent ice albedo.
//! Equations follow the Budyko/Sellers EBM used by CLIMLAB's model/ebm.py.
//! Independent Rust implementation. Radiative forcing is a reduced boundary
//! condition in legacy worlds; deep time supplies weathering/outgassing feedback.
//! Neither branch resolves atmospheric circulation.
use super::{Mesh, SEA};
use crate::planetary::smooth;

const N: usize = 24;

#[derive(Clone)]
pub(super) struct Climate {
    anomaly: [f64; N],
}

impl Climate {
    pub fn anomaly(&self, latitude_sine: f64) -> f64 {
        let x = ((latitude_sine + 1.0) * 0.5 * N as f64 - 0.5).clamp(0.0, (N - 1) as f64);
        let i = (x as usize).min(N - 2);
        self.anomaly[i] + (self.anomaly[i + 1] - self.anomaly[i]) * (x - i as f64)
    }
}

pub(super) fn solve(mesh: &Mesh, height: &[f64], seed: u32, myr: f64, duration: u16, forcing: Option<f64>) -> Climate {
    let mut area = [0.0; N];
    let mut land = [0.0; N];
    for (i, &h) in height.iter().enumerate() {
        let band = (((mesh.directions[i].y + 1.0) * 0.5 * N as f64) as usize).min(N - 1);
        area[band] += mesh.area[i];
        if h > SEA {
            land[band] += mesh.area[i];
        }
    }
    let fraction = std::array::from_fn(|i| land[i] / area[i].max(1.0));
    // Geological greenhouse forcing and obliquity are inputs to the energy
    // equation. The present boundary is shared with the playable world.
    let age = f64::from(duration) - myr.min(f64::from(duration));
    let period = 48.0 + f64::from(seed % 23);
    let carbon = forcing.unwrap_or_else(|| -3.2 * (1.0 - (age * std::f64::consts::TAU / period).cos()));
    let obliquity = 0.018 * (age * std::f64::consts::TAU / 0.041).sin();
    let reference = equilibrium(&fraction, 0.0, 0.0);
    let current = equilibrium(&fraction, carbon, obliquity);
    let anomaly = std::array::from_fn(|i| current[i] - reference[i]);
    Climate { anomaly }
}

fn equilibrium(land: &[f64; N], forcing: f64, solar_shape: f64) -> [f64; N] {
    let dx = 2.0 / N as f64;
    let mut temperature = std::array::from_fn(|i| {
        let x = -1.0 + (i as f64 + 0.5) * dx;
        12.0 - 40.0 * (1.5 * x * x - 0.5)
    });
    // Implicit diffusion avoids a grid-dependent stability timestep. Solve
    // radiation and the current ice boundary together by damped iteration.
    for _ in 0..96 {
        let mut upper = [0.0; N];
        let mut rhs = [0.0; N];
        for i in 0..N {
            let x = -1.0 + (i as f64 + 0.5) * dx;
            let p2 = 1.5 * x * x - 0.5;
            let ice = smooth(4.0, 14.0, -temperature[i]);
            let bare = 0.29 + 0.025 * land[i] + 0.078 * p2;
            let albedo = bare + (0.55 - bare) * ice;
            let solar = 1365.2 * 0.25 * (1.0 + (-0.48 + solar_shape) * p2);
            let left = if i == 0 { 0.0 } else { 0.555 * (1.0 - (x - dx * 0.5).powi(2)) / (dx * dx) };
            let right = if i + 1 == N { 0.0 } else { 0.555 * (1.0 - (x + dx * 0.5).powi(2)) / (dx * dx) };
            let diagonal = 2.0 + left + right - if i > 0 { left * upper[i - 1] } else { 0.0 };
            upper[i] = right / diagonal;
            rhs[i] = (solar * (1.0 - albedo) - 210.0 + forcing
                + if i > 0 { left * rhs[i - 1] } else { 0.0 }) / diagonal;
        }
        for i in (0..N - 1).rev() { rhs[i] += upper[i] * rhs[i + 1]; }
        for i in 0..N { temperature[i] += (rhs[i] - temperature[i]) * 0.35; }
    }
    temperature
}

mod snapshot;
