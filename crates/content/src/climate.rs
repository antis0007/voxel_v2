//! `assets/climate.ron`: dials for the synoptic storm catalogue (tropical
//! cyclones and tornadoes) in `engine_worldgen::sphere::weather::storms`.
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const SOURCE: &str = include_str!("../../../assets/climate.ron");

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TropicalDials {
    /// Storm slots per hemisphere and the period, in days, each slot repeats over.
    pub slots: u32,
    pub period_days: f64,
    /// Chance a slot generation tries genesis at all.
    pub genesis_chance: f64,
    /// Candidate genesis points tried per generation; the first over warm open ocean wins.
    pub genesis_attempts: u32,
    pub genesis_lat_deg: (f64, f64),
    /// Sea surface (annual mean) needed for genesis and for intensification, degrees C.
    pub sst_genesis_c: f64,
    /// Below this SST the core loses strength.
    pub sst_decay_c: f64,
    /// Land fraction above which genesis is refused.
    pub max_genesis_land: f64,
    pub lifetime_days: (f64, f64),
    /// Intensity at genesis, in `0..=1` of the storm's potential.
    pub initial_intensity: f64,
    /// Intensity gain and losses per day.
    pub intensify_per_day: f64,
    pub land_decay_per_day: f64,
    pub cold_decay_per_day: f64,
    /// Potential maximum sustained wind range, m/s.
    pub potential_wind_ms: (f64, f64),
    /// Template radius range, radians of arc.
    pub radius_rad: (f64, f64),
    /// Radius of maximum wind as a fraction of the template radius, at full and zero strength.
    pub eyewall_frac: (f64, f64),
    /// Holland profile shape parameter range.
    pub holland_b: (f64, f64),
    /// Surface inflow angle across the isobars, degrees.
    pub inflow_deg: f64,
    /// Spiral band wind modulation amplitude and winding.
    pub band_gain: f64,
    pub band_wrap: f64,
    /// Steering: beta drift poleward, trade-wind westward and westerly eastward motion, degrees per day.
    pub beta_deg_per_day: f64,
    pub trade_deg_per_day: f64,
    pub westerly_deg_per_day: f64,
    /// Latitude of the subtropical ridge where the track recurves.
    pub recurve_lat_deg: f64,
    /// Saffir-Simpson thresholds for categories 1..=5, m/s.
    pub category_ms: [f64; 5],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TornadoDials {
    /// Time bucket a supercell cell may spawn one tornado in, days.
    pub bucket_days: f64,
    /// Supercell cells per side of the warm-sector grid of each frontal storm.
    pub cells: u32,
    /// Spawn chance per cell and bucket at full convective index.
    pub chance: f64,
    /// Convective index (instability x shear) a storm needs before any tornado.
    pub min_convective: f64,
    /// Warm-sector annual temperature mapping to zero and full instability, degrees C.
    pub instability_c: (f64, f64),
    pub lifetime_days: (f64, f64),
    /// Core radius range, radians of arc.
    pub radius_rad: (f64, f64),
    pub max_wind_ms: (f64, f64),
    /// Motion relative to the parent storm, m/s.
    pub translate_ms: f64,
    /// Outer cut-off of the vortex wind in core radii.
    pub reach_cores: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StormDials {
    /// Flow units (radians per model unit) per m/s; maps physical winds into `CloudSky::wind`.
    pub flow_per_ms: f64,
    pub tropical: TropicalDials,
    pub tornado: TornadoDials,
    pub ground: GroundWindDials,
    pub hills: HillsSynopticDials,
}

/// Ground wind as play feels it: `world::wind_at` in m/s and its consumers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroundWindDials {
    /// m/s per unit of the day's authored `wind_speed` dial (0..1); also the calm floor on planets.
    pub day_wind_ms: f64,
    /// Multiplier on the planetary flow converted to m/s.
    pub planet_gain: f64,
    /// Extra gust share under a full cumulonimbus share.
    pub storm_gust: f64,
    /// Hard ceiling of ground wind, m/s (tornado cores reach it).
    pub max_ms: f64,
    /// Positional gust cell size, metres, and +- gust share.
    pub gust_cell_m: f64,
    pub gust_range: f64,
    /// Quadratic drag on a walking body, 1/m; acceleration = k |w - v| (w - v).
    pub push_ground: f64,
    /// Same for an airborne body that is not gliding.
    pub push_air: f64,
    /// Relative wind below this does not push a body, m/s.
    pub push_min_ms: f64,
    /// Ceiling of the wind fed to the glider frame, m/s.
    pub glide_max_ms: f64,
    /// Updraft at a tornado core, m/s, falling to zero at its reach.
    pub tornado_lift_ms: f64,
    /// Rain streak slant per m/s and snow drift share of the wind.
    pub rain_drift: f64,
    pub snow_drift: f64,
}

/// Fallback synoptic cyclones of worlds without a planet sky (Hills and flat maps).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HillsSynopticDials {
    pub cyclones: u32,
    /// Half-width of a cyclone, radians.
    pub radius_rad: f32,
    pub life_days: f64,
    /// Eastward steering of the synoptic frame, radians per day.
    pub steering_rad_per_day: f64,
}

/// The parsed `assets/climate.ron`, loaded once.
pub fn storm_dials() -> &'static StormDials {
    static TABLE: OnceLock<StormDials> = OnceLock::new();
    TABLE.get_or_init(|| {
        let d: StormDials = ron::from_str(SOURCE).unwrap_or_else(|e| panic!("assets/climate.ron: {e}"));
        let t = &d.tropical;
        let n = &d.tornado;
        assert!(d.flow_per_ms > 0.0, "assets/climate.ron: flow_per_ms must be positive");
        assert!(t.slots > 0 && t.period_days > t.lifetime_days.1 && t.genesis_attempts > 0, "assets/climate.ron: tropical slots");
        assert!(t.category_ms.windows(2).all(|w| w[0] < w[1]), "assets/climate.ron: category thresholds must rise");
        assert!(t.eyewall_frac.0 > 0.0 && t.holland_b.0 > 0.0, "assets/climate.ron: tropical profile");
        assert!(n.cells > 0 && n.bucket_days > n.lifetime_days.1 && n.radius_rad.0 > 0.0, "assets/climate.ron: tornado");
        assert!(n.instability_c.1 > n.instability_c.0, "assets/climate.ron: instability range");
        d
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn climate_ron_round_trips() {
        let d = super::storm_dials();
        let text = ron::to_string(d).unwrap();
        let back: super::StormDials = ron::from_str(&text).unwrap();
        assert_eq!(&back, d);
    }
}
