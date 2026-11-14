//! Local ambient temperature for the sparse thermal lattice. A cell without a
//! record is at this temperature; a recorded cell relaxes toward it.
use crate::fast_hash::FastMap;
use crate::world::BlockId;
use engine_worldgen::{WorldSampleConfig, WorldType, ecology, landscape};
use glam::IVec3;
use std::cell::RefCell;

/// Ambient where no climate applies: superflat worlds and bare sandboxes.
pub const FLAT_C: f32 = 20.0;
/// Natural water stays liquid under its ice; only a colder neighbour freezes it.
pub const WATER_FLOOR_C: f32 = 1.0;
/// Below the natural surface, °C per metre.
const GEOTHERMAL_C_PER_M: f32 = 0.03;
/// The day/night swing falls by 1/e per this many metres of ground.
const SKIN_M: f32 = 1.0;
/// Time of day (0 midnight, 0.5 noon) of the warmest air.
const WARMEST: f32 = 0.625;
const COLUMNS: usize = 16_384;

#[derive(Clone, Copy)]
struct Column {
    /// Lowland climate heat and wetness, 0..1.
    heat: f32,
    wet: f32,
    /// Top of the generated ground, metres.
    surface_m: f32,
    season_c: f32,
}

/// Columns are a pure function of the config, so evicting them never changes
/// a result.
pub struct Climate {
    cfg: Option<WorldSampleConfig>,
    /// Ambient when `cfg` is `None`.
    flat_c: f32,
    time_of_day: f32,
    cloud: f32,
    day_index: u32,
    columns: RefCell<FastMap<(i32, i32), Column>>,
}

impl Default for Climate {
    fn default() -> Self {
        Self::flat(FLAT_C)
    }
}

impl Climate {
    /// The same ambient everywhere, at every hour.
    pub fn flat(celsius: f32) -> Self {
        Self { cfg: None, flat_c: celsius, time_of_day: 0.0, cloud: 0.0, day_index: 0, columns: RefCell::default() }
    }

    /// Called once per fixed step, before the thermal pass.
    pub fn sync(&mut self, cfg: WorldSampleConfig, time_of_day: f32, cloud_cover: f32) {
        let cfg = (cfg.world_type != WorldType::Superflat).then_some(cfg);
        if self.cfg != cfg {
            self.columns.get_mut().clear();
            self.cfg = cfg;
        }
        self.time_of_day = time_of_day.rem_euclid(1.0);
        self.cloud = if cloud_cover.is_finite() { cloud_cover.clamp(0.0, 1.0) } else { 0.0 };
    }

    /// Time of day from the last `sync`: midnight 0.0, noon 0.5.
    pub fn time_of_day(&self) -> f32 {
        self.time_of_day
    }

    pub fn set_day_index(&mut self, day_index: u32) { self.day_index = day_index; }

    /// Air or ground temperature at `p`, °C.
    pub fn ambient_c(&self, p: IVec3) -> f32 {
        let Some(cfg) = self.cfg else { return self.flat_c };
        let column = self.column(&cfg, p.x, p.z);
        let y = cfg.world_y(p.y);
        let depth = (column.surface_m - y).max(0.0);
        let heat = landscape::lapse_heat(&cfg, column.heat, y.max(column.surface_m));
        // Dry air swings further; cloud cover holds the night warmth in.
        let swing = (3.0 + 5.0 * (1.0 - column.wet)) * (1.0 - 0.6 * self.cloud);
        let day = (std::f32::consts::TAU * (self.time_of_day - WARMEST)).cos();
        // The existing sky calendar has four five-day seasons; summer peaks
        // midway through its second season. Deep ground damps seasonal swings.
        let year = ((self.day_index % 20) as f32 + self.time_of_day - 7.5) / 20.0;
        let seasonal = column.season_c * (std::f32::consts::TAU * year).cos() * (-depth / 8.0).exp();
        ecology::heat_celsius(heat) + seasonal + GEOTHERMAL_C_PER_M * depth + swing * day * (-depth / SKIN_M).exp()
    }

    /// `ambient_c` as seen by `block`: liquid water does not sit below its floor.
    pub fn ambient_for(&self, p: IVec3, block: BlockId) -> f32 {
        let t = self.ambient_c(p);
        if block == BlockId::Water { t.max(WATER_FLOOR_C) } else { t }
    }

    fn column(&self, cfg: &WorldSampleConfig, x: i32, z: i32) -> Column {
        if let Some(c) = self.columns.borrow().get(&(x, z)) {
            return *c;
        }
        let (heat, wet) = landscape::lowland_climate(cfg, cfg.world_x(x), cfg.world_z(z));
        let surface_m = cfg.world_y(engine_worldgen::terrain_height(cfg, x, z) + 1);
        let season_c = if engine_worldgen::planetary::active(cfg) {
            let d = engine_worldgen::planetary::direction(cfg, f64::from(cfg.world_x(x)), f64::from(cfg.world_z(z)));
            let c = engine_worldgen::planetary::climate_at(cfg, d);
            (c.annual_c - c.winter_c).max(0.0) as f32
        } else if engine_worldgen::geo::integrated(cfg) {
            let p = engine_worldgen::geo::planet_for(cfg);
            p.grid.bilinear(&p.season, cfg.world_x(x) as f64, cfg.world_z(z) as f64) as f32
        } else { 0.0 };
        let c = Column { heat, wet, surface_m, season_c };
        let mut columns = self.columns.borrow_mut();
        if columns.len() >= COLUMNS {
            columns.clear();
        }
        columns.insert((x, z), c);
        c
    }
}

#[cfg(test)]
#[path = "climate_tests.rs"]
mod tests;
