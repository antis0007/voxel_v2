//! Planet terrain from a drainage solve (generator 73, ADR 0038).
//!
//! Plates set uplift, stream-power erosion carves ranges, spurs and valleys,
//! climate follows latitude, wind and relief, and rivers are the solve's own
//! flow network. Per-column sampling only interpolates and adds bounded detail.
//! The solve uses IEEE-exact arithmetic in a fixed order, so every machine in a
//! multiplayer world derives the same planet from the seed; `checksum` proves it.

mod climate;
pub mod corridor;
mod detail;
pub(crate) mod erosion;
mod erratic;
pub(crate) mod grid;
mod glacial;
pub mod history;
mod mountain;
mod noise;
mod periglacial;
mod pingo;
mod planet;
mod rivers;
mod tectonics;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod glacial_tests;

use std::cell::RefCell;
use std::sync::Arc;

use engine_content::BlockId;
use engine_voxel::{WATER_LEVEL_METERS, WorldSampleConfig, WorldType};

pub use planet::{Entry, Planet, Preview, Progress, Stage};

pub const GEO_GENERATOR: u32 = 73;
pub const INTEGRATED_GENERATOR: u32 = 76;
/// Opt-in Hills/Ridged review; existing saves retain their original landscape.
pub const MOUNTAIN_EROSION_GENERATOR: u32 = 252;
/// Fractured channel walls and natural relief at former stack landmarks.
pub const RAVINE_GENERATOR: u32 = 253;

fn mountain_erosion_active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= MOUNTAIN_EROSION_GENERATOR
        && matches!(cfg.world_type, WorldType::Hills | WorldType::Ridged)
}

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
    /// Drainage-aligned glacial erosion; historical solves leave this disabled.
    pub glacial_erosion: bool,
    /// Cirques, horns, moraines and erratics on top of glacial erosion (gate 129).
    pub glacial_landforms: bool,
    /// Run drainage and slope transport after conversion to physical metres.
    pub mountain_erosion: bool,
    pub fractured_ravines: bool,
    pub ocean_bias: f64,
    pub continent_scale: f64,
    pub erosion: f64,
    pub temperature_bias: f64,
    pub rainfall: f64,
    pub lake_depth_m: f64,
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
            self.glacial_erosion as u64,
            self.glacial_landforms as u64,
            self.ocean_bias.to_bits(), self.continent_scale.to_bits(), self.erosion.to_bits(),
            self.temperature_bias.to_bits(), self.rainfall.to_bits(), self.lake_depth_m.to_bits(),
        ];
        let legacy = words.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, w| (h ^ w).wrapping_mul(0x0100_0000_01b3));
        // Preserve every historical cache key, including custom planets.
        let mountain = if self.mountain_erosion { (legacy ^ MOUNTAIN_EROSION_GENERATOR as u64).wrapping_mul(0x0100_0000_01b3) } else { legacy };
        if self.fractured_ravines { (mountain ^ RAVINE_GENERATOR as u64).wrapping_mul(0x0100_0000_01b3) } else { mountain }
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
    glacial_erosion: false,
    glacial_landforms: false,
    mountain_erosion: false,
    fractured_ravines: false,
    ocean_bias: 0.0, continent_scale: 1.0, erosion: 1.0,
    temperature_bias: 0.0, rainfall: 1.0, lake_depth_m: 30.0,
};

pub const INTEGRATED_HOME: PlanetParams = PlanetParams { glacial_erosion: true, ..HOME };

/// Compressed ranges and drainage basins for ordinary exploration.
pub const COMPACT_HOME: PlanetParams = PlanetParams {
    face_edge_m: 24576.0, relief_m: 380.0, ..INTEGRATED_HOME
};
/// Walkable regional variety; large planet dimensions must not dictate trip length.
pub const EXPLORATION_HOME: PlanetParams = PlanetParams {
    face_edge_m: 10240.0, relief_m: 360.0, ..INTEGRATED_HOME
};

/// Shorter journeys between drainage basins, with less vertical travel.
pub const SETTLED_HOME: PlanetParams = PlanetParams {
    face_edge_m: 6144.0, relief_m: 240.0, ..INTEGRATED_HOME
};

/// The planet's spawn cube face: one atlas face edge, full continental relief.
pub const PLANET_FACE: PlanetParams = PlanetParams {
    face_edge_m: 2.0 * engine_core::cube_sphere::atlas::HALF_M, relief_m: 520.0, ..INTEGRATED_HOME
};

/// Gate 129: glacial landforms and erratics on the planet face. Hills faces
/// are too low to hold glaciers, so they keep their solve.
pub const LANDFORMS_VERSION: u32 = 129;
const PLANET_FACE_GLACIATED: PlanetParams = PlanetParams { glacial_landforms: true, ..PLANET_FACE };

/// Edge of the solved square in metres; the loading globe maps it onto one face.
pub fn face_edge_m(cfg: &WorldSampleConfig) -> f64 {
    params(cfg).face_edge_m
}

pub fn integrated(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= INTEGRATED_GENERATOR && active(cfg)
}

fn params(cfg: &WorldSampleConfig) -> PlanetParams {
    let mut p = *legacy_params(cfg);
    p.mountain_erosion = mountain_erosion_active(cfg);
    p.fractured_ravines = p.mountain_erosion && cfg.generator_version >= RAVINE_GENERATOR;
    if let Some(s) = cfg.planet_settings() {
        p.ocean_bias = (s.unit(0) - 0.5) * 0.95;
        p.continent_scale = 2.0_f64.powf((0.5 - s.unit(1)) * 2.0);
        p.relief_m = 80.0 + s.unit(2) * 400.0;
        p.erosion = 0.25 + s.unit(3) * 1.5;
        p.temperature_bias = (s.unit(4) - 0.5) * 40.0;
        p.rainfall = s.unit(5) * 2.0;
        p.tilt_sin = (s.unit(6) * 47.0).to_radians().sin();
        p.lake_depth_m = 2.0 + s.unit(11) * 56.0;
    }
    p
}

fn legacy_params(cfg: &WorldSampleConfig) -> &'static PlanetParams {
    let landforms = cfg.generator_version >= LANDFORMS_VERSION;
    if cfg.world_type == WorldType::Planet { if landforms { &PLANET_FACE_GLACIATED } else { &PLANET_FACE } }
    else if integrated(cfg) && cfg.generator_version >= 92 { &SETTLED_HOME }
    else if integrated(cfg) && cfg.generator_version >= 91 { &EXPLORATION_HOME }
    else if integrated(cfg) && cfg.generator_version >= 90 { &COMPACT_HOME }
    else if integrated(cfg) { &INTEGRATED_HOME } else { &HOME }
}

pub fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= GEO_GENERATOR && matches!(cfg.world_type, WorldType::Hills | WorldType::Ridged)
        || cfg.world_type == WorldType::Planet && cfg.generator_version >= crate::basin::VERSION
}

/// First wet nodes of solved catchments. Downstream reaches and closed lakes
/// are never independently refilled; the runtime accounts for this runoff.
pub fn river_sources(cfg: &WorldSampleConfig, center: glam::Vec2, radius: f32) -> Vec<crate::drainage::Spring> {
    if crate::planetary::active(cfg) { return crate::planetary::river_sources(cfg, center, radius); }
    let planet = planet_for(cfg);
    let mut wet_parent = vec![false; planet.height.len()];
    for (i, &area) in planet.discharge.iter().enumerate() {
        let r = planet.receiver[i] as usize;
        if r != i && area >= rivers::SOURCE_AREA_M2 && planet.land[i] { wet_parent[r] = true; }
    }
    let mut result = Vec::new();
    for (i, &area) in planet.discharge.iter().enumerate() {
        if area < rivers::SOURCE_AREA_M2 || wet_parent[i] || !planet.land[i]
            || planet.receiver[i] as usize == i || planet.lake[i] > f32::MIN { continue; }
        let [x, z] = planet.node[i];
        if (glam::Vec2::new(x, z) - center).length_squared() > radius * radius { continue; }
        let ground = ground(cfg, x, z, cfg.voxel_meters());
        if let Some(head) = ground.water_m.filter(|h| *h > WATER_LEVEL_METERS + 1.0) {
            result.push(crate::drainage::Spring {
                position_m: glam::Vec3::new(x, head, z),
                // About 0.63 m/year of effective runoff over solved wet area.
                microlitres_per_second: (area as f64 * 20.0).clamp(8_000_000.0, 240_000_000.0) as u64,
            });
        }
    }
    result
}

thread_local! {
    static INTEGRATED_HOT: RefCell<Option<((u32, u64), Arc<Planet>)>> = const { RefCell::new(None) };
}

pub fn planet_for(cfg: &WorldSampleConfig) -> Arc<Planet> {
    if crate::planetary::active(cfg) { return crate::planetary::regional_view(cfg); }
    if !integrated(cfg) { return planet(cfg.seed); }
    let key = (cfg.seed, params(cfg).fingerprint());
    if let Some(p) = INTEGRATED_HOT.with(|h| h.borrow().as_ref().filter(|(s, _)| *s == key).map(|(_, p)| Arc::clone(p))) {
        return p;
    }
    let p = planet::get(cfg.seed, &params(cfg));
    INTEGRATED_HOT.with(|h| *h.borrow_mut() = Some((key, Arc::clone(&p))));
    p
}

pub fn entry_for(cfg: &WorldSampleConfig) -> Arc<Entry> {
    if crate::planetary::active(cfg) { return crate::planetary::entry(cfg); }
    planet::entry(cfg.seed, &params(cfg))
}

pub fn prewarm_for(cfg: WorldSampleConfig) {
    if crate::planetary::active(&cfg) {
        crate::planetary::prewarm(cfg);
        return;
    }
    let e = entry_for(&cfg);
    if !e.ready() && e.claim_prewarm() {
        let retry = Arc::clone(&e);
        if std::thread::Builder::new().name("planet-solve".into()).spawn(move || drop(planet_for(&cfg))).is_err() {
            retry.release_prewarm();
        }
    }
}

/// Coarse lake head for far meshes, without building local erosion tiles.
pub fn lake_level(cfg: &WorldSampleConfig, x: f32, z: f32) -> Option<f32> {
    if crate::planetary::active(cfg) { return crate::planetary::ground(cfg, x, z, cfg.voxel_meters()).water_m; }
    let p = planet_for(cfg);
    let (x, z) = (x as f64, z as f64);
    rivers::lake(&p, x, z).filter(|&h| p.grid.bilinear(&p.height, x, z) < h - 0.05).map(|h| h as f32)
}

/// Stable, metre-aligned floor for authored surface structures. No site recursion.
pub fn site_floor(cfg: &WorldSampleConfig, x: f32, z: f32) -> f32 {
    if !integrated(cfg) { return 0.0; }
    type Key = (u32, u32, u64, u32, u32);
    thread_local! {
        static FLOORS: RefCell<Vec<(Key, f32)>> = const { RefCell::new(Vec::new()) };
    }
    let key = (cfg.seed, cfg.generator_version, params(cfg).fingerprint(), x.to_bits(), z.to_bits());
    if let Some(y) = FLOORS.with(|c| c.borrow().iter().find(|(k, _)| *k == key).map(|(_, y)| *y)) { return y; }
    let g = ground(cfg, x, z, 1.0);
    let y = g.top_m.max(g.water_m.unwrap_or(WATER_LEVEL_METERS) + 1.0).ceil();
    FLOORS.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= 32 { c.clear(); }
        c.push((key, y));
    });
    y
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
    /// Surface deposit added after erosion, metres; zero on historical worlds.
    pub(crate) deposit_m: f32,
    /// Material of the derived surface deposit, independent of scree.
    pub(crate) deposit_material: crate::BlockId,
    /// Eroded bed slope before the surface deposit; material rules use this datum.
    pub(crate) deposit_bed_slope: f32,
    /// Exact pre-addition bed datum; never recover it by subtracting f32 heights.
    pub(crate) deposit_bed_top: f32,
    /// Top of the ground in metres.
    pub top_m: f32,
    pub slope: f32,
    /// Mean annual temperature mapped from [-15, 30] C to [0, 1].
    pub temperature: f32,
    pub moisture: f32,
    pub sediment: f32,
    pub(crate) placer: f32,
    pub(crate) river_gravel: bool,
    pub river_bedrock: bool,
    pub maat_c: f32,
    pub permafrost: bool,
    /// Surface of a river, lake or pond standing on this column.
    pub water_m: Option<f32>,
    /// Gate 136 riparian strength 0..1 beside a sizeable channel.
    pub riparian: f32,
    /// Strength of the depositional lobe at a spherical river mouth.
    pub delta: f32,
    /// Fine fraction of a solved coastal deposit.
    pub delta_mud: f32,
    /// Optical density of fines still suspended in the solved coastal water.
    pub suspended: f32,
    /// Exposure of a coastal deposit that repeatedly flooded and drained.
    pub intertidal: f32,
    /// Local thickness of scree beneath a sedimentary escarpment in metres.
    pub(crate) scree_m: f32,
    /// Ground kept open at a surveyed coastal outlook.
    pub(crate) outlook_clearing: f32,
}

pub(crate) fn cane_bank(cfg: &WorldSampleConfig, x: f32, z: f32) -> bool {
    let planet = planet_for(cfg);
    let near_river = rivers::nearest(&planet, f64::from(x), f64::from(z))
        .filter(|c| (0.0..=2.5).contains(&(c.distance - c.half_width)) && c.water_m > SEA_M + 0.25);
    // Small Hills basins may have no channel above the main river threshold.
    // The shallow margins of solved inland lakes are also cane habitat.
    let mut lake_edges = [(0.0, 0.0); 8];
    let mut count = 0;
    for d in [1.5, 3.0] {
        for (sx, sz) in [(x-d, z), (x+d, z), (x, z-d), (x, z+d)] {
            if rivers::lake(&planet, f64::from(sx), f64::from(sz)).is_some_and(|y| y > SEA_M + 0.25) {
                lake_edges[count] = (sx, sz);
                count += 1;
            }
        }
    }
    if near_river.is_none() && count == 0 {
        return false;
    }
    let bank = ground(cfg, x, z, cfg.voxel_meters());
    if bank.water_m.is_some() || bank.temperature < 0.55 || bank.permafrost || bank.slope > 0.45 {
        return false;
    }
    let bank_above = |water| (0.0..=1.8).contains(&(f64::from(bank.top_m) - water));
    near_river.is_some_and(|c| bank_above(c.water_m)) || lake_edges[..count].iter().any(|&(sx, sz)| {
        let shore = ground(cfg, sx, sz, cfg.voxel_meters());
        shore.water_m.is_some_and(|water| f64::from(water) > SEA_M + 0.25
            && water - shore.top_m <= 1.6 && bank_above(f64::from(water)))
    })
}

/// Ground at a world point. Above 32 m, omit local detail; gate 138 retains
/// footprint-filtered drainage so the regional view still describes walked land.
pub fn ground(cfg: &WorldSampleConfig, wx: f32, wz: f32, footprint: f32) -> Ground {
    if crate::planetary::active(cfg) { return crate::planetary::ground(cfg, wx, wz, footprint); }
    use noise::{fbm, smoothstep};
    let pl = planet_for(cfg);
    let (x, z) = (wx as f64, wz as f64);
    let g = &pl.grid;
    let fine = footprint <= 32.0;
    let mountains = mountain_erosion_active(cfg);
    // The solved massif is shared at all LODs. Sub-grid relief fades before
    // the legacy 32 m cutoff, so a ravine cannot abruptly refill at that edge.
    let detail_filter = if mountains { 1.0 - smoothstep(8.0, 32.0, footprint as f64) } else { 1.0 };
    let valley_weight = if fine { 1.0 } else if corridor::lod_active(cfg) || mountains {
        corridor::lod_weight(footprint)
    } else { 0.0 };
    let nearest = if !(fine || valley_weight > 0.0) {
        None
    } else if corridor::active(cfg) {
        rivers::nearest_on(&pl, Some(&corridor::incised_levels(&pl)), x, z)
    } else {
        rivers::nearest(&pl, x, z)
    };
    let river = nearest.filter(|c| c.distance < c.half_width * 4.0 + 12.0);
    let reach = nearest.filter(|c| corridor::active(cfg) && c.distance < corridor::REACH_M);
    let h0 = g.bilinear(&pl.height, x, z);
    // A lake fills its basin up to the smooth planet surface's contour, so the
    // shore follows the basin instead of the cell grid or a detail gully.
    let lake = rivers::lake(&pl, x, z).filter(|&l| h0 < l - 0.05);
    // Detail fades toward a channel so its banks keep the solved valley floor,
    // and lies flat under a lake.
    let calm = river.map_or(1.0, |c| {
        let calm = smoothstep(c.half_width + 4.0, c.half_width * 2.0 + 24.0, c.distance);
        if fine { calm } else { 1.0 - (1.0 - calm) * corridor::bed_weight(&c, footprint) }
    })
        * if lake.is_some() { 0.0 } else { 1.0 }
        * reach.map_or(1.0, |c| if fine { corridor::calm(&c) } else { 1.0 - (1.0 - corridor::calm(&c)) * valley_weight });
    let surface = |x: f64, z: f64| {
        let s = g.bilinear(&pl.height, x, z);
        let s = crate::vistas::height(cfg,x as f32,z as f32,s);
        if fine && detail_filter > 0.0 && s > SEA_M {
            let delta = detail::offset(&pl, cfg.seed, x, z)*crate::vistas::detail_weight(cfg,x as f32,z as f32);
            s + (if integrated(cfg) { delta.min(0.0) } else { delta }) * calm * detail_filter
        } else { s }
    };
    let base = surface(x, z);
    let probe = (footprint as f64).max(2.0);
    let sx = (surface(x + probe, z) - surface(x - probe, z)) / (2.0 * probe);
    let sz = (surface(x, z + probe) - surface(x, z - probe)) / (2.0 * probe);
    let slope = (sx * sx + sz * sz).sqrt();
    let wet = g.bilinear(&pl.wet, x, z);
    let mut top = base;
    if integrated(cfg) && fine && base > SEA_M + 2.0 {
        top += periglacial::slump(&pl, cfg.seed, x, z) * calm * detail_filter;
    }
    if fine && base > SEA_M {
        top += (fbm(x / 7.0, z / 7.0, cfg.seed ^ 0x77, 2) - 0.5) * (0.4 + slope.min(1.5)) * 0.9 * calm * detail_filter;
    }
    let maat = g.bilinear(&pl.maat, x, z) - HOME.lapse_c_per_m * (top - h0);
    let permafrost = maat < PERMAFROST_C;
    let mut water: Option<f64> = None;
    if fine && permafrost && slope < 0.12 && top > SEA_M + 1.0 && calm > 0.99 {
        let poly = periglacial::polygon(cfg.seed, x, z, maat, wet);
        water = poly.water.map(|w| top + w * detail_filter);
        top += poly.ground * detail_filter;
    }
    if fine && water.is_none() && params(cfg).glacial_landforms {
        top += erratic_at(&pl, cfg.seed, x, z) * detail_filter;
    }
    if fine && let Some(p) = pingo_at(cfg, &pl, x, z) {
        top += p.height * detail_filter;
    }
    if let Some(c) = reach {
        let carved = top.min(corridor::ceiling_m(&c));
        top = if fine { carved } else { top + (carved - top) * valley_weight };
    }
    if let Some(c) = river {
        let profile = if c.distance < c.half_width {
            let k = c.distance / c.half_width;
            c.water_m - c.depth * (1.0 - k * k)
        } else {
            c.water_m + 0.4 + (c.distance - c.half_width) * 0.6
        };
        let before = top;
        let weight = if fine { 1.0 } else { corridor::bed_weight(&c, footprint) };
        let carved = top.min(profile);
        top = if fine { carved } else { top + (carved - top) * weight };
        if c.distance >= c.half_width && c.distance < c.half_width + 2.0 {
            let bank = top.max((c.water_m + 0.3).min(before + 2.0));
            top = if fine { bank } else { top + (bank - top) * weight };
        }
        if c.distance < c.half_width && weight > 0.0 {
            water = Some(water.map_or(c.water_m, |w| w.max(c.water_m)));
        }
    }
    if let Some(l) = lake {
        water = Some(water.map_or(l, |w| w.max(l)));
    }
    let water = water.filter(|w| *w > top);
    let area = g.bilinear(&pl.discharge, x, z).max(0.0);
    let sediment = ((area.sqrt() / 3000.0).min(1.0) * (1.0 - slope.min(1.0)) + (1.0 - calm) * 0.4).min(1.0);
    let riparian = reach.map_or(0.0, |c| corridor::riparian(&c) * valley_weight as f32);
    let (temperature, moisture) = if cfg.generator_version >= 93 {
        let altitude = (top - SEA_M).max(0.0);
        let sea_heat = ((maat + HOME.lapse_c_per_m * altitude + 15.0) / 45.0) as f32;
        let (heat, rain) = crate::landscape::regional_ecology(cfg,x as f32,z as f32,sea_heat,wet as f32);
        ((heat - (HOME.lapse_c_per_m * altitude / 45.0) as f32).clamp(0.0,1.0),
         (rain + (1.0-calm) as f32*0.25 + riparian*0.30).min(1.0))
    } else {
        (((maat+15.0)/45.0).clamp(0.0,1.0) as f32,(wet+(1.0-calm)*0.25).min(1.0) as f32)
    };
    if cfg.world_type == WorldType::Hills && crate::trees::rainforest::forest_floor(cfg)
        && footprint < 9.0 && water.is_none()
    {
        let tropical = crate::smoothstep(0.54, 0.62, temperature) * crate::smoothstep(0.66, 0.79, moisture);
        if tropical > 0.0 {
            let (swale, mound) = crate::trees::rainforest::hills_floor(cfg, wx, wz);
            let shift = crate::trees::rainforest::floor_relief(swale, mound, top as f32, slope as f32, footprint);
            top += f64::from(tropical * shift) * calm;
        }
    }
    Ground { deposit_material: crate::BlockId::Stone, deposit_bed_top: 0.0, deposit_bed_slope: 0.0, deposit_m: 0.0,
        top_m: top as f32,
        slope: slope as f32,
        temperature,
        moisture,
        sediment: sediment as f32,
        placer: 0.0,
        river_gravel: false,
        river_bedrock: false,
        maat_c: maat as f32,
        permafrost,
        water_m: water.map(|w| w as f32),
        riparian,
        delta: 0.0,
        delta_mud: 0.0,
        suspended: 0.0,
        intertidal: 0.0,
        scree_m: 0.0,
        outlook_clearing: 0.0,
    }
}

/// The routed channel still has a downstream direction at a level reach.
pub fn flat_river_current(cfg: &WorldSampleConfig, wx: f32, wz: f32) -> glam::Vec2 {
    if !active(cfg) || crate::planetary::active(cfg) {
        return glam::Vec2::ZERO;
    }
    let planet = planet_for(cfg);
    let (x, z) = (f64::from(wx), f64::from(wz));
    let Some(channel) = rivers::nearest(&planet, x, z) else {
        return glam::Vec2::ZERO;
    };
    if channel.distance > channel.half_width
        || rivers::lake(&planet, x, z).is_some_and(|level| level > channel.water_m + 0.05)
    {
        return glam::Vec2::ZERO;
    }
    channel.downstream * 0.5
}

/// Erratic height where no river or lake claims the ground; `ground` and
/// `subsurface` share it so a boulder's shape and rock always agree.
fn erratic_at(pl: &Planet, seed: u32, x: f64, z: f64) -> f64 {
    let h = erratic::height(pl, seed, x, z);
    if h <= 0.0 || rivers::near(pl, x, z).is_some() || rivers::lake(pl, x, z).is_some() { 0.0 } else { h }
}

/// Pingo on flat wet continuous permafrost away from water; planet fields
/// only, so `ground` and `subsurface` agree.
fn pingo_at(cfg: &WorldSampleConfig, pl: &Planet, x: f64, z: f64) -> Option<pingo::Pingo> {
    if !params(cfg).glacial_landforms {
        return None;
    }
    let g = &pl.grid;
    let p = pingo::at(cfg.seed, x, z, g.bilinear(&pl.maat, x, z))?;
    let flat_wet = g.bilinear(&pl.wet, x, z) > 0.4 && detail::planet_slope(pl, x, z) < 0.05;
    (flat_wet && rivers::near(pl, x, z).is_none() && rivers::lake(pl, x, z).is_none()).then_some(p)
}

/// Erratic rock, pingo cores and wedge ice under the cracks of flat permafrost, `depth_m` below the surface.
pub fn subsurface(cfg: &WorldSampleConfig, wx: f32, wz: f32, depth_m: f32) -> Option<BlockId> {
    if crate::planetary::active(cfg) { return None; }
    let pl = planet_for(cfg);
    let (x, z) = (wx as f64, wz as f64);
    let landforms = params(cfg).glacial_landforms;
    if landforms && depth_m < 6.0 && (depth_m as f64) < erratic_at(&pl, cfg.seed, x, z) {
        return Some(BlockId::Basalt);
    }
    // Cores sit within 1.5..21 m; deeper voxels skip the pingo lookup.
    if landforms && (1.5..21.0).contains(&depth_m) && let Some(p) = pingo_at(cfg, &pl, x, z) {
        let d = depth_m as f64;
        if d >= 1.5 && d < 1.5 + p.core {
            return Some(BlockId::Ice);
        }
    }
    if !(0.5..4.0).contains(&depth_m) {
        return None;
    }
    let maat = pl.grid.bilinear(&pl.maat, x, z);
    if maat >= PERMAFROST_C || detail::planet_slope(&pl, x, z) >= 0.12 {
        return None;
    }
    periglacial::wedge_ice(cfg.seed, x, z, depth_m as f64, maat).then_some(BlockId::Ice)
}
