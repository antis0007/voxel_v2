//! Planet terrain from a drainage solve (generator 71, ADR 0038).
//!
//! Plates set uplift, stream-power erosion carves ranges, spurs and valleys,
//! climate follows latitude, wind and relief, and rivers are the solve's own
//! flow network. Per-column sampling only interpolates and adds bounded detail.
//! The solve uses IEEE-exact arithmetic in a fixed order, so every machine in a
//! multiplayer world derives the same planet from the seed; `checksum` proves it.

mod climate;
mod detail;
mod erosion;
mod grid;
mod noise;
mod periglacial;
mod planet;
mod rivers;
mod tectonics;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::sync::Arc;

use engine_content::BlockId;
use engine_voxel::{WATER_LEVEL_METERS, WorldSampleConfig, WorldType};

pub use planet::{Entry, Planet, Preview, Progress, Stage};

pub const GEO_GENERATOR: u32 = 71;

pub(crate) const SEA_M: f64 = WATER_LEVEL_METERS as f64;

/// Mean annual temperature below which the ground stays frozen.
pub const PERMAFROST_C: f64 = -1.5;

/// Body constants the solve reads; later supplied per planet by `engine_space`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanetParams {
    /// Solve cells along one face edge.
    pub cells: usize,
    /// Edge of the playable cube face in metres, centred on the origin.
    pub face_edge_m: f64,
    /// Unit rotation axis in world axes; the face normal is +y.
    pub axis: [f64; 3],
    /// Sine of the axial tilt; sets the seasonal swing.
    pub tilt_sin: f64,
    /// Height of the highest summit above sea level, metres.
    pub relief_m: f64,
    /// Cooling with altitude, degrees per metre. Steeper than Earth's because ranges are compressed.
    pub lapse_c_per_m: f64,
    /// Fraction of the face's latitude range its climate spans; 1 is geometric.
    pub climate_span: f64,
}

impl PlanetParams {
    pub fn fingerprint(&self) -> u64 {
        let words = [
            self.cells as u64,
            self.face_edge_m.to_bits(),
            self.axis[0].to_bits(),
            self.axis[1].to_bits(),
            self.axis[2].to_bits(),
            self.tilt_sin.to_bits(),
            self.relief_m.to_bits(),
            self.lapse_c_per_m.to_bits(),
            self.climate_span.to_bits(),
        ];
        words.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, w| (h ^ w).wrapping_mul(0x0100_0000_01b3))
    }
}

/// The home world: north is -z, spawn at 45 degrees; climate runs from subtropical (south edge) to subpolar (north edge).
pub const HOME: PlanetParams = PlanetParams {
    cells: 512,
    face_edge_m: 32768.0,
    axis: [0.0, std::f64::consts::FRAC_1_SQRT_2, -std::f64::consts::FRAC_1_SQRT_2],
    tilt_sin: 0.3978,
    relief_m: 440.0,
    lapse_c_per_m: 0.022,
    climate_span: 0.55,
};

pub fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= GEO_GENERATOR && cfg.world_type != WorldType::Superflat
}

thread_local! {
    static HOT: RefCell<Option<(u32, Arc<Planet>)>> = const { RefCell::new(None) };
}

/// The solved home planet for this seed, built on first use.
pub fn planet(seed: u32) -> Arc<Planet> {
    if let Some(p) = HOT.with(|h| h.borrow().as_ref().filter(|(s, _)| *s == seed).map(|(_, p)| Arc::clone(p))) {
        return p;
    }
    let p = planet::get(seed, &HOME);
    HOT.with(|h| *h.borrow_mut() = Some((seed, Arc::clone(&p))));
    p
}

/// Solve state of the home planet for a seed, for the loading screen.
pub fn entry(seed: u32) -> Arc<Entry> {
    planet::entry(seed, &HOME)
}

/// Starts the solve on its own thread unless it is done or under way.
pub fn prewarm(seed: u32) {
    let e = entry(seed);
    if !e.ready() && e.claim_prewarm() {
        std::thread::Builder::new().name("planet-solve".into()).spawn(move || drop(planet(seed))).ok();
    }
}

/// Hash of the solved planet; peers compare it to prove they generate the same world.
pub fn checksum(seed: u32) -> u64 {
    planet(seed).checksum
}

#[derive(Clone, Copy, Debug)]
pub struct Ground {
    /// Top of the ground in metres.
    pub top_m: f32,
    pub slope: f32,
    /// Mean annual temperature mapped from [-15, 30] C to [0, 1].
    pub temperature: f32,
    pub moisture: f32,
    pub sediment: f32,
    pub maat_c: f32,
    pub permafrost: bool,
    /// Surface of a river, lake or pond standing on this column.
    pub water_m: Option<f32>,
}

/// Ground at a world point. `footprint` above 32 m skips mid-scale detail and channels.
pub fn ground(cfg: &WorldSampleConfig, wx: f32, wz: f32, footprint: f32) -> Ground {
    use noise::{fbm, smoothstep};
    let pl = planet(cfg.seed);
    let (x, z) = (wx as f64, wz as f64);
    let g = &pl.grid;
    let fine = footprint <= 32.0;
    let river = if fine { rivers::near(&pl, x, z) } else { None };
    let h0 = g.bilinear(&pl.height, x, z);
    // A lake fills its basin up to the smooth planet surface's contour, so the
    // shore follows the basin instead of the cell grid or a detail gully.
    let lake = rivers::lake(&pl, x, z).filter(|&l| h0 < l - 0.05);
    // Detail fades toward a channel so its banks keep the solved valley floor,
    // and lies flat under a lake.
    let calm = river.map_or(1.0, |c| smoothstep(c.half_width + 4.0, c.half_width * 2.0 + 24.0, c.distance))
        * if lake.is_some() { 0.0 } else { 1.0 };
    let surface = |x: f64, z: f64| {
        let s = g.bilinear(&pl.height, x, z);
        if fine && s > SEA_M { s + detail::offset(&pl, cfg.seed, x, z) * calm } else { s }
    };
    let base = surface(x, z);
    let probe = (footprint as f64).max(2.0);
    let sx = (surface(x + probe, z) - surface(x - probe, z)) / (2.0 * probe);
    let sz = (surface(x, z + probe) - surface(x, z - probe)) / (2.0 * probe);
    let slope = (sx * sx + sz * sz).sqrt();
    let wet = g.bilinear(&pl.wet, x, z);
    let mut top = base;
    if fine && base > SEA_M {
        top += (fbm(x / 7.0, z / 7.0, cfg.seed ^ 0x77, 2) - 0.5) * (0.4 + slope.min(1.5)) * 0.9 * calm;
    }
    let maat = g.bilinear(&pl.maat, x, z) - HOME.lapse_c_per_m * (top - h0);
    let permafrost = maat < PERMAFROST_C;
    let mut water: Option<f64> = None;
    if fine && permafrost && slope < 0.12 && top > SEA_M + 1.0 && calm > 0.99 {
        let poly = periglacial::polygon(cfg.seed, x, z, maat, wet);
        water = poly.water.map(|w| top + w);
        top += poly.ground;
    }
    if let Some(c) = river {
        let profile = if c.distance < c.half_width {
            let k = c.distance / c.half_width;
            c.water_m - c.depth * (1.0 - k * k)
        } else {
            c.water_m + 0.4 + (c.distance - c.half_width) * 0.6
        };
        let before = top;
        top = top.min(profile);
        if c.distance >= c.half_width && c.distance < c.half_width + 2.0 {
            top = top.max((c.water_m + 0.3).min(before + 2.0));
        }
        if c.distance < c.half_width {
            water = Some(water.map_or(c.water_m, |w| w.max(c.water_m)));
        }
    }
    if let Some(l) = lake {
        water = Some(water.map_or(l, |w| w.max(l)));
    }
    let water = water.filter(|w| *w > top);
    let area = g.bilinear(&pl.discharge, x, z).max(0.0);
    let sediment = ((area.sqrt() / 3000.0).min(1.0) * (1.0 - slope.min(1.0)) + (1.0 - calm) * 0.4).min(1.0);
    Ground {
        top_m: top as f32,
        slope: slope as f32,
        temperature: ((maat + 15.0) / 45.0).clamp(0.0, 1.0) as f32,
        moisture: (wet + (1.0 - calm) * 0.25).min(1.0) as f32,
        sediment: sediment as f32,
        maat_c: maat as f32,
        permafrost,
        water_m: water.map(|w| w as f32),
    }
}

/// Wedge ice under the cracks of flat permafrost, `depth_m` below the surface.
pub fn subsurface(cfg: &WorldSampleConfig, wx: f32, wz: f32, depth_m: f32) -> Option<BlockId> {
    if !(0.5..4.0).contains(&depth_m) {
        return None;
    }
    let pl = planet(cfg.seed);
    let (x, z) = (wx as f64, wz as f64);
    let maat = pl.grid.bilinear(&pl.maat, x, z);
    if maat >= PERMAFROST_C || detail::planet_slope(&pl, x, z) >= 0.12 {
        return None;
    }
    periglacial::wedge_ice(cfg.seed, x, z, depth_m as f64, maat).then_some(BlockId::Ice)
}
