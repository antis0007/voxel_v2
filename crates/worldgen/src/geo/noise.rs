//! Hashed value noise in f64 built from IEEE-exact operations only.
//!
//! Every machine in a multiplayer world must solve the same planet bit for bit,
//! so nothing in `geo` calls a transcendental function: lattice hashes, a
//! polynomial fade, products and sums.

use engine_core::hash::hash_u32;

fn lattice(x: i64, z: i64, seed: u32) -> f64 {
    let h = hash_u32((x as u32) ^ hash_u32((z as u32) ^ hash_u32(seed)));
    h as f64 / u32::MAX as f64
}

fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Hashed value in [0, 1] for an integer pair.
pub fn unit(a: i64, b: i64, seed: u32) -> f64 {
    lattice(a, b, seed)
}

/// Value noise in [0, 1] with unit lattice spacing.
pub fn value(x: f64, z: f64, seed: u32) -> f64 {
    let (fx, fz) = (x.floor(), z.floor());
    let (ix, iz) = (fx as i64, fz as i64);
    let (tx, tz) = (fade(x - fx), fade(z - fz));
    let a = lattice(ix, iz, seed);
    let b = lattice(ix + 1, iz, seed);
    let c = lattice(ix, iz + 1, seed);
    let d = lattice(ix + 1, iz + 1, seed);
    let top = a + (b - a) * tx;
    let bottom = c + (d - c) * tx;
    top + (bottom - top) * tz
}

/// Fractal value noise in [0, 1]; each octave halves the wavelength.
pub fn fbm(x: f64, z: f64, seed: u32, octaves: u32) -> f64 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 1.0, 0.0, 1.0);
    for o in 0..octaves {
        sum += value(x * f, z * f, seed.wrapping_add(o.wrapping_mul(0x9e37_79b9))) * amp;
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    sum / norm
}

pub fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Compact bump: 1 at 0, 0 from `width` on, flat at both ends.
pub fn bump(d: f64, width: f64) -> f64 {
    let t = (d / width).min(1.0);
    let s = 1.0 - t * t;
    s * s
}
