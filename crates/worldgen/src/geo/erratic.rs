//! Erratics: boulders carried by ice and left on ground it once covered.
//!
//! Each 40 m lattice cell may hold one; the chance follows former ice cover,
//! so they gather on moraines and trough floors and thin out at the margin.
//! They are foreign rock, so their material never matches the ground below.
use super::{Planet, SEA_M, noise::unit};

const SPACING_M: f64 = 40.0;
const MAX_SLOPE: f64 = 0.35;

/// Height of an erratic above the ground at `(x, z)`, metres; zero for none.
pub fn height(p: &Planet, seed: u32, x: f64, z: f64) -> f64 {
    let (cx, cz) = ((x / SPACING_M).floor() as i64, (z / SPACING_M).floor() as i64);
    let mut best: f64 = 0.0;
    for dz in -1..=1 {
        for dx in -1..=1 {
            let (a, b) = (cx + dx, cz + dz);
            let sx = (a as f64 + 0.2 + 0.6 * unit(a, b, seed ^ 0xe7a1)) * SPACING_M;
            let sz = (b as f64 + 0.2 + 0.6 * unit(a, b, seed ^ 0xe7a2)) * SPACING_M;
            let radius = 1.4 + 3.0 * unit(a, b, seed ^ 0xe7a3).powi(2);
            let (ox, oz) = (x - sx, z - sz);
            // Long axes lie at hashed bearings; ice rarely leaves a sphere.
            let turn = unit(a, b, seed ^ 0xe7a4) * std::f64::consts::PI;
            let (u, v) = (ox * turn.cos() + oz * turn.sin(), (oz * turn.cos() - ox * turn.sin()) * 1.5);
            let k2 = (u * u + v * v) / (radius * radius);
            if k2 >= 1.0 {
                continue;
            }
            let g = &p.grid;
            let cover = g.bilinear(&p.ice, sx, sz);
            if cover <= 0.05 || unit(a, b, seed ^ 0xe7a0) > 0.35 * cover {
                continue;
            }
            if g.bilinear(&p.height, sx, sz) <= SEA_M + 1.0 || super::detail::planet_slope(p, sx, sz) >= MAX_SLOPE {
                continue;
            }
            // Blunt top and steep flanks, as a weathered block sits.
            best = best.max(0.8 * radius * (1.0 - k2 * k2).sqrt());
        }
    }
    best
}
