//! Shared planetary woodland envelope, sampled in body metres across charts.
//! Stand occupancy, regeneration and the orbital canopy use the same climate.
use super::{atlas, noise, smooth};
use glam::DVec3;

pub struct Stand { pub canopy: f32, pub regeneration: f32, pub maturity: f32 }

pub fn potential(annual_c: f32, wet: f32, slope: f32) -> f32 {
    (smooth(-4.0, 8.0, annual_c.into()) * smooth(0.22, 0.68, wet.into())
        * (1.0 - smooth(0.35, 0.85, slope.into()))) as f32
}

pub fn stand(seed: u32, dir: DVec3, annual: f32, wet: f32, slope: f32, riparian: f32) -> Stand {
    let p = dir * atlas::RADIUS_M;
    let broad = noise(seed, p / 420.0, 286_011);
    let edge = noise(seed, p / 95.0, 286_017);
    let disturbance = noise(seed, p / 47.0, 286_031);
    let regeneration = smooth(0.65, 0.82, disturbance) as f32;
    let mass = smooth(0.32, 0.68, broad * 0.70 + edge * 0.30 + f64::from(riparian) * 0.18) as f32;
    Stand {
        canopy: potential(annual, wet, slope) * mass * (1.0 - 0.88 * regeneration),
        regeneration,
        maturity: (0.35 + 0.65 * broad) as f32,
    }
}
