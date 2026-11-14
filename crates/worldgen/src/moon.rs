//! Cratered surface height of an airless moon, a pure function of seed and direction.
use engine_core::hash::hash_u32;
use glam::{DVec3, IVec3};

/// Crater lattice spacing per level, in radians of arc; finer levels hold smaller craters.
const LEVELS: [f64; 5] = [0.25, 0.08, 0.025, 0.008, 0.0025];
/// Share of lattice cells that hold a crater, per level.
const DENSITY: [f64; 5] = [0.5, 0.6, 0.7, 0.7, 0.6];

fn unit(seed: u32, level: u32, cell: IVec3, salt: u32) -> f64 {
    let mut h = hash_u32(seed ^ 0x6d6f_6f6e ^ level.wrapping_mul(0x9e37_79b9));
    for v in [cell.x, cell.y, cell.z] {
        h = hash_u32(h ^ v as u32);
    }
    f64::from(hash_u32(h ^ salt)) / f64::from(u32::MAX)
}

/// Distance over radius where the rim apron reaches zero.
const REACH: f64 = 1.6;

/// Bowl below the datum inside the rim, raised rim falling off outside it; `x` is distance over radius.
fn profile(x: f64, depth: f64, rim: f64) -> f64 {
    if x < 1.0 {
        // The floor flattens once the bowl is a third deep, as on real simple craters.
        (depth * (x * x - 1.0)).max(-depth * 0.7) + rim
    } else {
        let t = ((REACH - x) / (REACH - 1.0)).clamp(0.0, 1.0);
        rim * t * t * (3.0 - 2.0 * t)
    }
}

/// Metres above the body's radius at unit direction `dir`, for a body of `radius_m`.
pub fn height(seed: u32, radius_m: f64, dir: DVec3) -> f64 {
    let dir = dir.normalize_or(DVec3::Y);
    // Broad basins and highlands, a few hundredths of the radius.
    let mut h = radius_m
        * 0.004
        * ((dir.x * 2.3 + 0.7).sin() * (dir.y * 1.9 - 0.4).sin() + 0.6 * (dir.z * 3.1 + 1.3).sin() * (dir.x * 2.7).cos());
    for (level, (&spacing, &density)) in LEVELS.iter().zip(&DENSITY).enumerate() {
        let level = level as u32;
        let base = (dir / spacing).floor().as_ivec3();
        for z in -1..=1 {
            for y in -1..=1 {
                for x in -1..=1 {
                    let cell = base + IVec3::new(x, y, z);
                    if unit(seed, level, cell, 0) > density {
                        continue;
                    }
                    let jitter = DVec3::new(unit(seed, level, cell, 1), unit(seed, level, cell, 2), unit(seed, level, cell, 3));
                    let point = (cell.as_dvec3() + jitter) * spacing;
                    // Only points near the sphere own a crater, independent of `dir`.
                    // Shell half-width plus reach stays under one spacing, so each
                    // owner of a crater touching `dir` sits in the 3x3x3 neighbourhood.
                    if (point.length() - 1.0).abs() >= spacing * 0.5 {
                        continue;
                    }
                    let centre = point.normalize_or(DVec3::Y);
                    let size = spacing * (0.1 + 0.2 * unit(seed, level, cell, 4).powi(2));
                    let angle = dir.dot(centre).clamp(-1.0, 1.0).acos();
                    let x = angle / size;
                    if x >= REACH {
                        continue;
                    }
                    let r = size * radius_m;
                    h += profile(x, r * 0.22, r * 0.05);
                }
            }
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_is_deterministic_and_bounded() {
        for i in 0..2_000 {
            let t = i as f64 * 0.731;
            let dir = DVec3::new(t.sin() * (t * 0.37).cos(), (t * 0.37).sin(), t.cos() * (t * 0.37).cos());
            let h = height(7, 4_000.0, dir);
            assert_eq!(h, height(7, 4_000.0, dir));
            assert!(h.abs() < 600.0, "height {h} at {dir}");
        }
    }

    #[test]
    fn surface_is_continuous_at_walking_scale() {
        let a = DVec3::new(0.3, 0.8, -0.5).normalize();
        let step = 0.5 / 4_000.0;
        let mut worst: f64 = 0.0;
        for i in 0..4_000 {
            let p = (a + DVec3::new(i as f64 * step, 0.0, i as f64 * step * 0.3)).normalize();
            let q = (a + DVec3::new((i + 1) as f64 * step, 0.0, (i + 1) as f64 * step * 0.3)).normalize();
            worst = worst.max((height(3, 4_000.0, p) - height(3, 4_000.0, q)).abs());
        }
        assert!(worst < 0.6, "half-metre step changed height by {worst} m");
    }

    #[test]
    fn crater_floor_sits_below_its_rim() {
        let profile_floor = profile(0.0, 10.0, 2.0);
        let rim = profile(1.0, 10.0, 2.0);
        assert!(profile_floor < rim && profile(REACH, 10.0, 2.0) == 0.0);
    }

    #[test]
    fn seeds_differ() {
        let dir = DVec3::new(0.2, 0.9, 0.1).normalize();
        let differs = (0..16).any(|s| (height(s, 4_000.0, dir) - height(s + 1, 4_000.0, dir)).abs() > 0.5);
        assert!(differs);
    }
}
