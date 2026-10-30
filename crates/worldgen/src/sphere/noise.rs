//! Seeded height field sampled from 3D position on the unit sphere.
//!
//! Sampling the direction rather than face coordinates makes the field
//! continuous across face edges and corners by construction.

use engine_core::hash::hash_u32;
use glam::DVec3;

use super::class::PlanetParams;
use super::grid::SphereGrid;

const OCTAVES: u32 = 5;
/// Lattice cells per unit of direction for the first octave.
const BASE_FREQUENCY: f64 = 2.0;

/// Returns the height in metres at unit direction `dir`.
pub fn height_m(seed: u32, relief_m: f64, dir: DVec3) -> f64 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = BASE_FREQUENCY;
    for octave in 0..OCTAVES {
        let salt = hash_u32(seed ^ hash_u32(octave.wrapping_add(0x9e37_79b9)));
        sum += amp * value3(dir * freq, salt);
        amp *= 0.5;
        freq *= 2.0;
    }
    // The amplitudes sum to just under one, so `sum` lies in [0, 1).
    (2.0 * sum - 1.0) * relief_m
}

/// Returns the height of every cell centre of `grid`, by dense index.
pub fn elevation(grid: &SphereGrid, params: &PlanetParams) -> Vec<f64> {
    (0..grid.len()).map(|i| height_m(params.seed, params.relief_m, grid.centre(i))).collect()
}

/// Trilinear value noise in `[0, 1]` with smoothstep weights.
fn value3(p: DVec3, salt: u32) -> f64 {
    let f = p.floor();
    let (x0, y0, z0) = (f.x as i32, f.y as i32, f.z as i32);
    let w = p - f;
    let s = w * w * (DVec3::splat(3.0) - 2.0 * w);
    let corner = |dx: i32, dy: i32, dz: i32| lattice(x0 + dx, y0 + dy, z0 + dz, salt);
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let x00 = lerp(corner(0, 0, 0), corner(1, 0, 0), s.x);
    let x10 = lerp(corner(0, 1, 0), corner(1, 1, 0), s.x);
    let x01 = lerp(corner(0, 0, 1), corner(1, 0, 1), s.x);
    let x11 = lerp(corner(0, 1, 1), corner(1, 1, 1), s.x);
    lerp(lerp(x00, x10, s.y), lerp(x01, x11, s.y), s.z)
}

fn lattice(x: i32, y: i32, z: i32, salt: u32) -> f64 {
    let h = hash_u32(salt ^ hash_u32(x as u32 ^ hash_u32(y as u32 ^ hash_u32(z as u32))));
    f64::from(h) / f64::from(u32::MAX)
}
