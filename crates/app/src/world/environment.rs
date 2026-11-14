//! Sparse, persistent environmental authority. A patch owns the pore volume
//! of contacted substrate, addressed in THREE dimensions. Homogeneous buried
//! soil shares metre-sized records; edited/surface cells retain exact topology.
//! Unvisited solid volume allocates nothing. Patches exchange only through an
//! admitted material face; caves/roofs cannot share an XZ reservoir.
//!
//! This is process aggregation, never a second geometry/collision authority.
use super::BlockId;
use engine_core::environment::ENVIRONMENT_GENERATOR;
use engine_sim::environment::{Sediment, Soil};
use glam::IVec3;
use std::collections::{BTreeMap, BTreeSet};
mod activation;
mod construction;
pub use activation::FluidImpact;
pub(crate) mod cultivation;
mod exchange;
mod fluid;
pub(crate) use fluid::surface_step;
mod fractions;
mod geometry;
mod persistence;
pub use fractions::Fractions;
pub(crate) mod light;
pub(crate) mod sky;
pub use geometry::faces;
pub(crate) use geometry::append_face_positions;
pub(crate) mod inspect;

pub type Key = (i32, i32, i32);
pub fn key(p: IVec3) -> Key {
    (p.x, p.y, p.z)
}
pub fn position(k: Key) -> IVec3 {
    IVec3::new(k.0, k.1, k.2)
}
/// Conservative reservations include ordered-map nodes and allocator overhead.
/// New cells stop allocating at this soft limit and stay implicit instead.
pub const MAX_PATCHES: usize = 65_536;
/// Refinement before an edit may pass the soft limit so the edit still lands.
pub const PATCH_CEILING: usize = MAX_PATCHES + 8_192;
pub const MAX_EVENTS: usize = 65_536;
const EDIT_EVENT_RESERVE:usize=1024;
pub const MAX_SEDIMENT: usize = 32_768;
pub const MAX_FRACTIONS: usize = 32_768;
pub const MAX_STATE_BYTES: usize = 64 * 1024 * 1024;
const _: () = assert!(
    PATCH_CEILING * 352
        + MAX_EVENTS * 224
        + MAX_SEDIMENT * 96
        + MAX_FRACTIONS * 384
        + light::MAX_QUERIES * 192
        + sky::MAX_CHUNKS * sky::RESERVATION
        < MAX_STATE_BYTES
);

#[derive(Clone, Debug)]
pub struct Patch {
    pub material: BlockId,
    pub soil: Soil,
    pub species: u32,
    pub revision: u64,
    pub last_tick: u64,
    pub extent: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ledger {
    pub initial_soil_water: u64,
    pub evaporated_water: u64,
    pub transpired_water: u64,
    pub atmospheric_biomass: u64,
    pub respired_biomass: u64,
    pub eroded_mg: u64,
    pub deposited_mg: u64,
    pub removed_sediment_mg: u64,
    pub imported_litter_mg: u64,
    pub imported_litter_nutrient_mg: u64,
    pub edited_water_added_ul: u64,
    pub edited_water_removed_ul: u64,
}

pub struct State {
    pub patches: BTreeMap<Key, Patch>,
    /// Fine particulate stock physically located in this fluid/air cell.
    /// Dry stock remains at that position until deposited or removed.
    pub sediment: BTreeMap<Key, Sediment>,
    /// Dissipated gravitational work from admitted water transfers (mJ).
    /// It drives detachment once, then is consumed; it is not stored water.
    pub hydraulic_work: BTreeMap<Key, u64>,
    /// Exact free-water encoding: ceil(eighths) * cell_volume / 8 - deficit.
    /// This is the fractional part of the SAME fluid inventory, not soil water.
    pub water_deficit: Fractions,
    pub light: BTreeMap<Key, light::Query>,
    pub sky: BTreeMap<Key, sky::Summary>,
    pub(crate) due: BTreeSet<(u64, Key)>,
    pub(crate) scheduled: BTreeMap<Key, u64>,
    pub tick: u64,
    pub target_tick: u64,
    pub fixed_steps: u64,
    /// Transient event context. External changes enter the next target tick;
    /// an executing process schedules its dependencies after its own step.
    pub(crate) event_tick: Option<u64>,
    /// Transient foreground admission; never serialized.
    pub(crate) player_edit:bool,
    pub ledger: Ledger,
    pub serviced: u64,
    pub blocked: u64,
    pub biological_rate: u32,
    pub erosion_rate: u32,
    pub evaporation_um_s: u32,
    pub cells_per_metre: i32,
    /// Reused dependency positions for the current event service.
    pub(crate) source_positions: Vec<IVec3>,
    /// Last patch the redundancy sweep examined. Transient: a reload restarts
    /// the sweep, which only drops records that regenerate identically.
    pub(crate) prune_cursor: Option<Key>,
}

impl Default for State {
    fn default() -> Self {
        let definition = engine_content::environment::definition();
        Self {
            patches: BTreeMap::new(),
            sediment: BTreeMap::new(),
            hydraulic_work: BTreeMap::new(),
            water_deficit: Fractions::default(),
            light: BTreeMap::new(),
            sky: BTreeMap::new(),
            due: BTreeSet::new(),
            scheduled: BTreeMap::new(),
            tick: 0,
            target_tick: 0,
            fixed_steps: 0,
            event_tick: None,
            player_edit:false,
            ledger: Ledger::default(),
            serviced: 0,
            blocked: 0,
            biological_rate: definition.map_or(0, |d| d.biological_rate),
            erosion_rate: definition.map_or(0, |d| d.erosion_rate),
            evaporation_um_s: definition.map_or(0, |d| d.evaporation_um_s),
            cells_per_metre: 1,
            source_positions: Vec::new(),
            prune_cursor: None,
        }
    }
}

impl State {
    pub fn enabled(generator: u32) -> bool {
        generator >= ENVIRONMENT_GENERATOR
    }
    /// Admits every material edit; the patch table never blocks one.
    pub fn admit_material(&self, _p: IVec3, _old: BlockId, _new: BlockId) -> bool {
        true
    }
    /// Stores a fresh patch. Returns false only for a non-porous material;
    /// past `MAX_PATCHES` the cell stays implicit (natural) and true is returned.
    pub fn create_patch(
        &mut self,
        cfg: super::WorldSampleConfig,
        p: IVec3,
        material: BlockId,
        exposed: bool,
        natural: bool,
    ) -> bool {
        if self.patches.contains_key(&self.owner(p)) {
            return true;
        }
        if self.patches.len() >= MAX_PATCHES {
            return engine_content::environment::definition().is_some_and(|d| d.soil(material).is_some());
        }
        let Some(patch) = self.fresh_patch(cfg, p, material, exposed, natural) else {
            return false;
        };
        self.ledger.initial_soil_water += patch.soil.water;
        self.patches.insert(key(p), patch);
        true
    }
    /// The patch `create_patch` would store, without touching the ledger.
    pub fn fresh_patch(
        &self,
        cfg: super::WorldSampleConfig,
        p: IVec3,
        material: BlockId,
        exposed: bool,
        natural: bool,
    ) -> Option<Patch> {
        let definition = engine_content::environment::definition()?;
        let law = definition.soil(material)?;
        let metric = p.as_vec3() * cfg.voxel_meters();
        let conditions = engine_worldgen::ecology::conditions(&cfg, metric, if exposed { 0 } else { 1_000 });
        let water = if natural {
            // Climate moisture is capillary water, not a pressurised aquifer.
            // Seeding it as a fraction of porosity made ordinary excavated
            // grass/dirt release free water even far above the water table.
            if p.y <= engine_worldgen::water_table(&cfg, p.x, p.z) {
                law.capacity(cfg.voxel_scale)
            } else {
                law.retained_water(cfg.voxel_scale) * u64::from(conditions.moisture_per_mille.min(1_000)) / 1_000
            }
        } else {
            0
        };
        let species = if exposed { engine_worldgen::ecology::colonist(&cfg, metric, conditions) } else { 0 };
        let area = engine_core::environment::face_area_cm2(cfg.voxel_scale);
        let biomass = if natural && matches!(material, BlockId::Grass | BlockId::SnowGrass) {
            definition.species(species).map_or(0, |s| u64::from(s.mature_mg_m2) * area / 10_000 / 2)
        } else {
            0
        };
        let nutrient = if natural {
            engine_core::environment::cell_volume(cfg.voxel_scale) * 10_240 / 1_000_000_000
        } else {
            0
        };
        Some(Patch {
            material,
            species,
            revision: 0,
            last_tick: self.event_tick.map_or(self.target_tick, |tick| tick.saturating_sub(1)),
            extent: 1,
            soil: Soil { water, nutrient, biomass, plant_nutrient: biomass / 40, ..Default::default() },
        })
    }
    /// Repair only pristine fine-cell records matching the old climate seed.
    /// Irrigated/evolved soil and groundwater keep their conserved stocks.
    pub(crate) fn repair_initial_moisture(&mut self, cfg: super::WorldSampleConfig, p: IVec3, patch: &mut Patch) {
        if patch.revision != 0 || patch.extent != 1 { return; }
        let Some(law) = engine_content::environment::definition().and_then(|d| d.soil(patch.material)) else { return };
        if patch.soil.water <= law.retained_water(cfg.voxel_scale)
            || p.y <= engine_worldgen::water_table(&cfg, p.x, p.z) { return; }
        for exposed in [false, true] {
            let Some(mut fresh) = self.fresh_patch(cfg, p, patch.material, exposed, true) else { continue };
            let conditions = engine_worldgen::ecology::conditions(&cfg, p.as_vec3() * cfg.voxel_meters(), if exposed { 0 } else { 1_000 });
            let corrected = fresh.soil.water;
            fresh.soil.water = law.capacity(cfg.voxel_scale) * u64::from(conditions.moisture_per_mille) / 1_000;
            if patch.species == fresh.species && patch.soil == fresh.soil && patch.soil.water > corrected {
                let excess = patch.soil.water - corrected;
                patch.soil.water = corrected;
                patch.revision += 1;
                self.ledger.initial_soil_water = self.ledger.initial_soil_water.saturating_sub(excess);
                self.patches.insert(key(p), patch.clone());
                return;
            }
        }
    }
    pub(crate) fn event_limit(&self)->usize {
        if self.player_edit {MAX_EVENTS}else{MAX_EVENTS-EDIT_EVENT_RESERVE}
    }
    /// Under queue pressure distant processes may sleep; their physical
    /// patch stocks remain stored. Contact/weather wakes them again. Never
    /// let a full background frontier make foreground edits permanently fail.
    pub(crate) fn make_player_room(&mut self,p:IVec3) {
        let needed=self.scheduled.len().saturating_sub(MAX_EVENTS-EDIT_EVENT_RESERVE);
        if needed==0{return;}
        let keep=self.cells_per_metre.max(1)*32;
        let sleepers:Vec<_>=self.due.iter().rev().take(4096)
            .filter(|(_,k)|(position(*k)-p).abs().max_element()>keep)
            .take(needed).copied().collect();
        for (due,k) in sleepers {self.due.remove(&(due,k));self.scheduled.remove(&k);}
    }
    pub fn can_wake(&self, positions: &[IVec3]) -> bool {
        let mut owners = [(0, 0, 0); 128];
        let mut count = 0;
        for &p in positions {
            let owner = self.owner(p);
            if self.scheduled.contains_key(&owner) || owners[..count].contains(&owner) {
                continue;
            }
            if count == owners.len() {
                return false;
            }
            owners[count] = owner;
            count += 1;
        }
        self.scheduled.len() + count <= self.event_limit()
    }
    pub fn wake(&mut self, p: IVec3) -> bool {
        self.wake_after(p, 1)
    }
    pub fn wake_after(&mut self, p: IVec3, seconds: u64) -> bool {
        let base = self.event_tick.unwrap_or(self.target_tick).max(self.tick);
        let owner = self.owner(p);
        let dormant = !self.scheduled.contains_key(&owner);
        if !self.schedule(owner, base.saturating_add(seconds)) {
            return false;
        }
        if dormant && let Some(patch) = self.patches.get_mut(&owner) {
            // Start a new eligible interval, without evolving the time spent asleep.
            patch.last_tick = base;
        }
        true
    }
    pub(crate) fn schedule(&mut self, p: Key, due: u64) -> bool {
        if let Some(&old) = self.scheduled.get(&p) {
            if old <= due {
                return true;
            }
            self.due.remove(&(old, p));
        } else if self.scheduled.len() >= self.event_limit() {
            return false;
        }
        self.scheduled.insert(p, due);
        self.due.insert((due, p));
        true
    }
    pub(crate) fn complete_event(&mut self, due: u64, p: Key) {
        self.due.remove(&(due, p));
        self.scheduled.remove(&p);
        self.serviced += 1;
    }
    pub fn sediment_at(&self, p: IVec3) -> u64 {
        self.sediment.get(&key(p)).map_or(0, |s| s.mass())
    }
    pub fn lag_seconds(&self) -> u64 {
        self.target_tick.saturating_sub(self.tick)
    }
    pub fn reserved_bytes(&self) -> usize {
        self.patches.len() * 352
            + self.scheduled.len() * 160
            + self.sediment.len() * 96
            + self.hydraulic_work.len() * 64
            + self.water_deficit.len() * 384
            + self.light.len() * 192
            + self.sky.len() * sky::RESERVATION
    }

    pub fn disturbed(&mut self, p: IVec3) {
        if let Some(patch) = self.patches.get_mut(&self.owner(p)) {
            patch.soil.litter += patch.soil.biomass;
            patch.soil.litter_nutrient += patch.soil.plant_nutrient;
            patch.soil.biomass = 0;
            patch.soil.plant_nutrient = 0;
            patch.revision += 1;
        }
    }
}

#[cfg(test)]
mod fluid_tests;
#[cfg(test)]
mod tests;
