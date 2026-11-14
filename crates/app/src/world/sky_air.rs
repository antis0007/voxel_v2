//! Daily aerosol regimes from `assets/sky_air.ron`.
//!
//! A regime is a set of physical coefficients applied to the atmosphere
//! preset, never a colour. It is a pure function of (weather seed, day index,
//! time of day), so it never touches a save and every player on one seed sees
//! the same sky. Regional air at the player (desert dust, dry cold) layers on.
use super::{LightingState, hash01, smoothstep};
use std::sync::LazyLock;

const TABLE: &str = include_str!("../../../../assets/sky_air.ron");
/// Tomorrow's regime fades in over the last tenth of the cycle (deep night).
const HANDOVER_START: f32 = 0.90;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Regime {
    pub id: String,
    pub weight: f32,
    pub cirrus: f32,
    pub stratus: f32,
    pub density: f32,
    pub height: f32,
    pub angstrom: f32,
    pub absorption: (f32, f32, f32),
    pub g: f32,
    pub ozone: f32,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Region {
    pub density: f32,
    pub height: f32,
    pub angstrom: f32,
    pub absorption: (f32, f32, f32),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Regions {
    pub desert: Region,
    pub snow: Region,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub regimes: Vec<Regime>,
    pub regions: Regions,
}

static TABLE_DATA: LazyLock<Option<Table>> = LazyLock::new(|| ron::from_str(TABLE).ok());

/// Review hook: `VOXEL_SKY_AIR=<id>` pins every day to one regime.
static PINNED: LazyLock<Option<String>> =
    LazyLock::new(|| std::env::var("VOXEL_SKY_AIR").ok().filter(|s| !s.is_empty()));

/// Aerosol state for one frame, applied to the preset by the renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyAir {
    pub density: f32,
    pub height: f32,
    pub angstrom: f32,
    pub absorption: [f32; 3],
    pub g: f32,
    pub ozone: f32,
}

impl SkyAir {
    /// The shipped preset unchanged.
    pub const NEUTRAL: Self = Self { density: 1.0, height: 1.0, angstrom: 0.0, absorption: [0.0; 3], g: 0.76, ozone: 1.0 };

    fn from_row(r: &Regime) -> Self {
        Self {
            density: r.density.clamp(0.2, 4.0),
            height: r.height.clamp(0.5, 4.0),
            angstrom: r.angstrom.clamp(-0.5, 2.5),
            absorption: [r.absorption.0, r.absorption.1, r.absorption.2].map(|a| a.clamp(0.0, 5.0e-6)),
            g: r.g.clamp(0.5, 0.9),
            ozone: r.ozone.clamp(0.5, 2.0),
        }
    }

    fn lerp(self, o: Self, t: f32) -> Self {
        let f = |a: f32, b: f32| a + (b - a) * t;
        Self {
            density: f(self.density, o.density),
            height: f(self.height, o.height),
            angstrom: f(self.angstrom, o.angstrom),
            absorption: [0, 1, 2].map(|i| f(self.absorption[i], o.absorption[i])),
            g: f(self.g, o.g),
            ozone: f(self.ozone, o.ozone),
        }
    }

    /// Mie scattering multiplier per channel at 680/550/440 nm.
    pub fn spectral_mie(self) -> [f32; 3] {
        [680.0_f32, 550.0, 440.0].map(|nm| self.density * (nm / 550.0).powf(-self.angstrom))
    }

    /// True when the transport tables must be re-solved for `other`.
    pub fn differs(self, other: Self) -> bool {
        let rel = |a: f32, b: f32| (a - b).abs() > 0.02 * a.abs().max(b.abs()).max(1e-3);
        rel(self.density, other.density)
            || rel(self.height, other.height)
            || (self.angstrom - other.angstrom).abs() > 0.02
            || (self.g - other.g).abs() > 0.002
            || rel(self.ozone, other.ozone)
            || (0..3).any(|i| (self.absorption[i] - other.absorption[i]).abs() > 1.0e-8)
    }
}

/// Index of the regime for `day_index`, weighted by that day's cirrus and stratus.
pub fn pick(regimes: &[Regime], day_index: u32, weather_seed: u32) -> Option<usize> {
    if let Some(id) = PINNED.as_deref() {
        return regimes.iter().position(|r| r.id == id);
    }
    let weather = super::weather_profile_for_day(day_index, weather_seed);
    let weight = |r: &Regime| {
        (r.weight * (1.0 + r.cirrus * weather.cirrus + r.stratus * weather.stratus)).max(0.0)
    };
    let total: f32 = regimes.iter().map(weight).sum();
    if total <= 0.0 {
        return None;
    }
    let mut roll = hash01(weather_seed ^ day_index.wrapping_mul(0x2C1B_3C6D) ^ 0x5EB1_A70D) * total;
    for (i, r) in regimes.iter().enumerate() {
        roll -= weight(r);
        if roll < 0.0 {
            return Some(i);
        }
    }
    Some(regimes.len() - 1)
}

fn air_for_day(table: &Table, day_index: u32, weather_seed: u32) -> SkyAir {
    pick(&table.regimes, day_index, weather_seed)
        .and_then(|i| table.regimes.get(i))
        .map_or(SkyAir::NEUTRAL, SkyAir::from_row)
}

/// The regime at this hour, with tomorrow's fading in during deep night.
pub fn for_time(day_index: u32, weather_seed: u32, time_of_day: f32) -> SkyAir {
    let Some(table) = TABLE_DATA.as_ref() else {
        return SkyAir::NEUTRAL;
    };
    let today = air_for_day(table, day_index, weather_seed);
    let t = smoothstep(HANDOVER_START, 1.0, time_of_day.rem_euclid(1.0));
    if t <= 0.0 {
        return today;
    }
    today.lerp(air_for_day(table, day_index.wrapping_add(1), weather_seed), t)
}

/// `apply_region` with the biome blend under `eye_m`. Hell keeps its own air.
pub fn apply_region_at(lighting: &mut LightingState, cfg: &super::WorldSampleConfig, eye_m: glam::Vec3) {
    if cfg.is_hell() || lighting.hell {
        return;
    }
    let scale = cfg.voxel_meters().max(1e-3);
    let w = super::biome_weights(cfg, (eye_m.x / scale).floor() as i32, (eye_m.z / scale).floor() as i32);
    apply_region(lighting, w.desert, w.snow);
}

/// Blend regional air at the player's biome into `lighting.sky_air`.
pub fn apply_region(lighting: &mut LightingState, desert: f32, snow: f32) {
    let Some(table) = TABLE_DATA.as_ref() else {
        return;
    };
    let mut a = lighting.sky_air;
    for (region, w) in [(&table.regions.desert, desert), (&table.regions.snow, snow)] {
        let w = w.clamp(0.0, 1.0);
        if w <= 0.0 {
            continue;
        }
        a.density *= 1.0 + (region.density - 1.0) * w;
        a.height *= 1.0 + (region.height - 1.0) * w;
        a.angstrom += (region.angstrom - a.angstrom) * w;
        let r = [region.absorption.0, region.absorption.1, region.absorption.2];
        a.absorption = [0, 1, 2].map(|i| a.absorption[i] + r[i] * w);
    }
    a.density = a.density.clamp(0.2, 4.0);
    a.height = a.height.clamp(0.5, 4.0);
    lighting.sky_air = a;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        ron::from_str(TABLE).expect("sky_air.ron parses")
    }

    #[test]
    fn sky_air_ron_round_trips() {
        let t = table();
        let text = ron::ser::to_string(&t).expect("serialise");
        assert_eq!(ron::from_str::<Table>(&text).expect("reparse"), t);
    }

    #[test]
    fn regimes_are_unique_and_physical() {
        let t = table();
        assert!(t.regimes.len() >= 4);
        for (i, r) in t.regimes.iter().enumerate() {
            assert!(t.regimes[..i].iter().all(|o| o.id != r.id), "duplicate {}", r.id);
            assert!(r.weight > 0.0 && r.density > 0.0 && r.height > 0.0, "{}", r.id);
            assert!([r.absorption.0, r.absorption.1, r.absorption.2].iter().all(|a| (0.0..5e-6).contains(a)), "{}", r.id);
        }
    }

    #[test]
    fn every_regime_appears_across_a_long_calendar() {
        let t = table();
        let mut seen = vec![0u32; t.regimes.len()];
        for day in 0..2000 {
            if let Some(i) = pick(&t.regimes, day, 0xA341_316C) {
                seen[i] += 1;
            }
        }
        for (r, n) in t.regimes.iter().zip(&seen) {
            assert!(*n > 10, "{} picked {n} times in 2000 days", r.id);
        }
    }

    #[test]
    fn fine_aerosol_scatters_blue_more_than_red() {
        let fine = SkyAir { angstrom: 1.9, ..SkyAir::NEUTRAL }.spectral_mie();
        assert!(fine[2] > fine[1] && fine[1] > fine[0]);
        let coarse = SkyAir { angstrom: 0.0, ..SkyAir::NEUTRAL }.spectral_mie();
        assert!((coarse[0] - coarse[2]).abs() < 1e-6);
    }

    #[test]
    fn air_is_stable_through_the_day_and_hands_over_at_night() {
        assert_eq!(for_time(7, 99, 0.50), for_time(7, 99, 0.75));
        assert!((for_time(7, 99, 0.9999).density - for_time(8, 99, 0.0).density).abs() < 1e-3);
    }
}
