//! Annual climate on the planet grid.
//!
//! Latitude comes from the planet's rotation axis, prevailing winds from the
//! three circulation cells, and rain from moisture carried off the ocean that
//! falls where the wind climbs, which leaves rain shadows behind ranges.

use super::grid::Grid;
use super::noise::{bump, smoothstep};
use super::{PlanetParams, SEA_M};

pub struct Climate {
    /// Mean annual air temperature, degrees Celsius.
    pub maat: Vec<f32>,
    /// Precipitation mapped to [0, 1).
    pub wet: Vec<f32>,
    /// Half the gap between summer and winter means, degrees Celsius.
    pub season: Vec<f32>,
}

/// Sine of the climate latitude for a face point; the face is the +y side of the planet cube.
///
/// One face spans 90 degrees of arc, far more climate than a few kilometres of
/// play should cross, so the deviation from the face centre is scaled by
/// `climate_span`.
pub fn sin_latitude(p: &PlanetParams, x: f64, z: f64) -> f64 {
    let half = p.face_edge_m * 0.5;
    let (u, v) = (x / half, z / half);
    let len = (u * u + 1.0 + v * v).sqrt();
    let s = (u * p.axis[0] + p.axis[1] + v * p.axis[2]) / len;
    p.axis[1] + (s - p.axis[1]) * p.climate_span
}

/// Sea-level mean temperature from latitude alone.
pub fn sea_level_c(sin_lat: f64) -> f64 {
    let s2 = sin_lat * sin_lat;
    27.0 - 30.0 * s2 - 12.0 * s2 * s2
}

fn wind(p: &PlanetParams, x: f64, z: f64, cell: f64) -> [f64; 2] {
    let s = sin_latitude(p, x, z);
    let ds = [sin_latitude(p, x + cell, z) - s, sin_latitude(p, x, z + cell) - s];
    let len = (ds[0] * ds[0] + ds[1] * ds[1]).sqrt().max(1e-12);
    let north = [ds[0] / len, ds[1] / len];
    let east = [-north[1], north[0]];
    let a = s.abs();
    let poleward = if s >= 0.0 { 1.0 } else { -1.0 };
    let westerly = smoothstep(0.42, 0.58, a) - smoothstep(0.80, 0.92, a);
    let zonal = -1.0 + 2.0 * westerly;
    let meridional = (-0.3 + 0.55 * westerly) * poleward;
    let w = [zonal * east[0] + meridional * north[0], zonal * east[1] + meridional * north[1]];
    let n = (w[0] * w[0] + w[1] * w[1]).sqrt().max(1e-12);
    [w[0] / n, w[1] / n]
}

/// Rain factor of the circulation band: wet tropics and mid-latitudes, dry subtropics and poles.
fn band(sin_lat: f64) -> f64 {
    let a = sin_lat.abs();
    (0.35 + 1.0 * bump(a, 0.3) + 0.6 * bump((a - 0.74).abs(), 0.18)).max(0.25)
}

pub fn solve(grid: &Grid, height_m: &[f64], land: &[bool], p: &PlanetParams) -> Climate {
    let len = grid.len();
    let h: Vec<f32> = height_m.iter().map(|&v| v.max(SEA_M) as f32).collect();
    let winds: Vec<[f64; 2]> = (0..len)
        .map(|i| {
            let [x, z] = grid.at(i);
            wind(p, x, z, grid.cell_m)
        })
        .collect();
    let bands: Vec<f64> = (0..len)
        .map(|i| {
            let [x, z] = grid.at(i);
            band(sin_latitude(p, x, z))
        })
        .collect();
    let mut moist: Vec<f32> = land.iter().map(|&l| if l { 0.0 } else { 1.0 }).collect();
    let mut next = moist.clone();
    let mut rain = vec![0.0f64; len];
    // Each pass carries moisture one cell downwind; enough passes cross the face.
    for _ in 0..grid.n + grid.n / 2 {
        for i in 0..len {
            let base = 0.018 * bands[i];
            if !land[i] {
                next[i] = 1.0;
                rain[i] = base;
                continue;
            }
            let [x, z] = grid.at(i);
            let w = winds[i];
            let (ux, uz) = (x - w[0] * grid.cell_m, z - w[1] * grid.cell_m);
            let up = grid.bilinear(&moist, ux, uz);
            let climb = (h[i] as f64 - grid.bilinear(&h, ux, uz)) / grid.cell_m;
            let mut rate = base + 1.5 * climb.max(0.0);
            if climb < 0.0 {
                rate *= 0.6;
            }
            let rate = rate.min(0.6);
            rain[i] = up * rate;
            // Plants and soil return over half the rain to the air, and storms keep
            // a floor of moisture even far inland.
            next[i] = (up - rain[i] * 0.45).max(0.12) as f32;
        }
        std::mem::swap(&mut moist, &mut next);
    }
    let mut maat = vec![0.0f32; len];
    let mut wet = vec![0.0f32; len];
    let mut season = vec![0.0f32; len];
    for i in 0..len {
        let [x, z] = grid.at(i);
        let s = sin_latitude(p, x, z);
        let t = sea_level_c(s) - p.lapse_c_per_m * (height_m[i] - SEA_M).max(0.0);
        maat[i] = t as f32;
        let k = rain[i] / 0.01;
        wet[i] = (k / (1.0 + k)) as f32;
        let inland = 1.0 - moist[i] as f64;
        season[i] = (2.0 + 32.0 * s.abs() * (p.tilt_sin / 0.3978) * (0.35 + 0.65 * inland)) as f32;
    }
    Climate { maat, wet, season }
}
