//! Per-planet dials derived from the seed: how wet, warm, rough and ruined a
//! world is. Consumers read the dials instead of hashing the seed themselves,
//! so a later owner-facing planet screen can override one dial without
//! touching every generator.
//!
//! This remains the legacy seed profile. Generator 136 consumes the saved
//! `WorldSampleConfig::planet_settings()` overrides; older seeds retain these dials.
use crate::hash2;
use crate::sphere::PlanetClass;

/// One planet's dials, each in `0.0..=1.0`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanetProfile {
    /// Share of the surface that drains to an ocean. Below `ICY_OCEAN` the
    /// class is airless, between that and `WET_OCEAN` it is icy.
    pub ocean: f32,
    /// Rainfall; feeds vegetation and river density once those read it.
    pub rainfall: f32,
    /// Mean surface temperature, cold to hot.
    pub temperature: f32,
    /// Height scale of the land, flat to alpine.
    pub relief: f32,
    /// Density of post-fall ruins: offices, homesteads, villages.
    pub ruins: f32,
    /// Density of stratum III works: the Surveyors' fused-stone monuments.
    pub surveyor_works: f32,
    /// Share of the underground that carries hazards (blight, magma, flood).
    pub hazard: f32,
}

const WET_OCEAN: f32 = 0.45;
const ICY_OCEAN: f32 = 0.2;

/// Salt per dial so one seed yields independent values.
const SALTS: [u32; 7] = [0x5e11_0001, 0x5e11_0002, 0x5e11_0003, 0x5e11_0004, 0x5e11_0005, 0x5e11_0006, 0x5e11_0007];

fn dial(seed: u32, salt: u32) -> f32 {
    hash2(0x2b, 0x47, seed.wrapping_mul(0x9e37_79b9) ^ salt)
}

/// Two hashes averaged so the dial clusters toward the middle, as RimWorld's
/// planet sliders do by default; extreme worlds still exist but are rare.
fn centred(seed: u32, salt: u32) -> f32 {
    0.5 * (dial(seed, salt) + dial(seed, salt.rotate_left(7)))
}

pub fn profile(seed: u32) -> PlanetProfile {
    PlanetProfile {
        ocean: centred(seed, SALTS[0]),
        rainfall: centred(seed, SALTS[1]),
        temperature: centred(seed, SALTS[2]),
        relief: centred(seed, SALTS[3]),
        ruins: dial(seed, SALTS[4]),
        surveyor_works: dial(seed, SALTS[5]),
        hazard: dial(seed, SALTS[6]),
    }
}

impl PlanetProfile {
    pub fn class(&self) -> PlanetClass {
        if self.ocean >= WET_OCEAN {
            PlanetClass::Wet
        } else if self.ocean >= ICY_OCEAN {
            PlanetClass::Icy
        } else {
            PlanetClass::Airless
        }
    }

    /// Expected count of Surveyor monuments per placement cell, `0.0..=1.0`.
    pub fn surveyor_chance(&self) -> f32 {
        0.12 + 0.5 * self.surveyor_works
    }

    /// Dial rows as percentages, for a planet summary screen.
    pub fn rows(&self) -> [(&'static str, u8); 7] {
        let pct = |v: f32| (v * 100.0).round() as u8;
        [
            ("Ocean", pct(self.ocean)),
            ("Rainfall", pct(self.rainfall)),
            ("Temperature", pct(self.temperature)),
            ("Relief", pct(self.relief)),
            ("Ruins", pct(self.ruins)),
            ("Surveyor works", pct(self.surveyor_works)),
            ("Hazard", pct(self.hazard)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dials_stay_in_range_and_repeat() {
        for seed in [0u32, 1, 7, 424_242, u32::MAX] {
            let p = profile(seed);
            for (name, v) in p.rows() {
                assert!((0..=100).contains(&v), "seed {seed} {name} {v}");
            }
            assert_eq!(p, profile(seed));
        }
    }

    #[test]
    fn seeds_spread_across_classes() {
        let mut seen = [false; 3];
        for seed in 0..512u32 {
            let i = match profile(seed).class() {
                PlanetClass::Wet => 0,
                PlanetClass::Icy => 1,
                PlanetClass::Airless => 2,
            };
            seen[i] = true;
        }
        assert_eq!(seen, [true; 3], "512 seeds should cover every class");
    }

    #[test]
    fn dials_are_independent() {
        let seeds = 0..256u32;
        let same = seeds.filter(|&s| {
            let p = profile(s);
            (p.ruins - p.surveyor_works).abs() < 0.02
        }).count();
        assert!(same < 24, "ruins and surveyor works track each other in {same} of 256 seeds");
    }
}
