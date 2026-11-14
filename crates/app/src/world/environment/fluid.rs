//! Exact fractional encoding for the existing sparse fluid owner. Integer
//! eighths remain a compatibility/presentation view, never another inventory.
use super::*;

impl State {
    pub fn quantum(&self) -> u64 {
        1_000_000_000 / (self.cells_per_metre.max(1) as u64).pow(3) / 8
    }
    pub fn exact_water(&self, p: IVec3, coarse: u8) -> u64 {
        (u64::from(coarse) * self.quantum())
            .saturating_sub(self.water_deficit.get(&key(p)).copied().unwrap_or(0))
    }
    pub fn can_encode(&self, amounts: &[(IVec3, u64)]) -> bool {
        let additional = amounts
            .iter()
            .enumerate()
            .filter(|(i, (p, amount))| {
                !amounts[..*i].iter().any(|(q, _)| p == q)
                    && amount % self.quantum() != 0
                    && !self.water_deficit.contains_key(&key(*p))
            })
            .count();
        self.water_deficit.len() + additional <= MAX_FRACTIONS
    }
    pub fn encode_water(&mut self, p: IVec3, microlitres: u64) {
        let deficit = (self.quantum() - microlitres % self.quantum()) % self.quantum();
        if deficit == 0 {
            self.water_deficit.remove(&key(p));
        } else {
            self.water_deficit.insert(key(p), deficit);
        }
    }
    pub fn capacity_with_grains(&self, p: IVec3, grains: u64) -> u64 {
        let residual = self.patches.get(&key(p)).map_or(0, |p| p.soil.water);
        (self.quantum() * 8).saturating_sub(engine_core::environment::sediment_volume(grains) + residual)
    }
    pub fn water_capacity_ul(&self, p: IVec3) -> u64 {
        self.capacity_with_grains(p, self.sediment_at(p))
    }
}

/// Visible surface steps per cell height; a multiple of the eight coarse levels.
const SURFACE_STEPS: u64 = 32;

/// Rendered surface step of a water amount; empty is its own step.
pub(crate) fn surface_step(amount: u64, quantum: u64) -> u64 {
    if amount == 0 { u64::MAX } else { amount * SURFACE_STEPS / (quantum * 8) }
}

impl super::super::World {
    /// Conservative exact-volume flow. Both source chunks must already be
    /// resident; a fluid rule never falls back to synchronous generation/I/O.
    pub(crate) fn apply_water_transfer(&mut self, from: IVec3, to: IVec3, moved: u64) -> bool {
        if self.ca_view.is_some() || !State::enabled(self.generator_version)
            || super::super::mill::owns_water(self, from) || super::super::mill::owns_water(self, to)
            || from == to || moved == 0 {
            return false;
        }
        let cfg = self.sample_config();
        if [from, to].iter().any(|&p| p.y < cfg.min_world_y() || p.y >= cfg.max_world_y()
            || !self.chunks.contains_key(&super::super::chunk_coord_from_world_block(p))
            || self.factory.nodes.contains_key(&super::super::factory::anchor(p, super::super::factory::step(self))))
            || self.get(from) != BlockId::Water
            || !matches!(self.get(to), BlockId::Air | BlockId::Water) {
            return false;
        }
        let source = self.water_microlitres(from);
        let target = self.water_microlitres(to);
        if moved > source { return false; }
        let grains = self.environment.sediment_at(to)
            + engine_sim::environment::advected_quantity(
                self.environment.sediment.get(&key(from)).map_or(0, |s| s.suspended), source, moved);
        let impact = self.fluid_impact(from, to);
        if target + moved > self.environment.capacity_with_grains(to, grains)
            || !self.environment.can_transport(from, to, &impact)
            || !self.environment.can_encode(&[(from, source - moved), (to, target + moved)])
            || self.environment.hydraulic_work.get(&key(to)).copied().unwrap_or(0) >= 999_000_000_000_000 {
            return false;
        }
        let quantum = self.environment.quantum();
        let published = self.changed_cells.len();
        let a = self.commit_resident_water_cell(from, (source - moved).div_ceil(quantum) as u8);
        let b = self.commit_resident_water_cell(to, (target + moved).div_ceil(quantum) as u8);
        // Steady flow moves water every tick at nearly constant levels. Remesh a
        // cell only when its surface crosses a step; the mesh trails by under one.
        self.changed_cells.truncate(published);
        for (p, old, new) in [(from, source, source - moved), (to, target, target + moved)] {
            if *crate::AB_OLD || surface_step(old, quantum) != surface_step(new, quantum) {
                self.changed_cells.push(p);
            }
        }
        self.environment.transport_exact(from, to, source, target, moved, &impact);
        self.environment.encode_water(from, source - moved);
        self.environment.encode_water(to, target + moved);
        self.wake_acting(from);
        self.wake_acting(to);
        self.invalidate_snapshot_at(a);
        if a != b { self.invalidate_snapshot_at(b); }
        self.revision = self.revision.wrapping_add(1);
        #[cfg(feature="multiplayer")]
        if self.network_host { self.network_changes.insert(from); self.network_changes.insert(to); }
        true
    }

    pub fn water_fill(&self, p: IVec3) -> f32 {
        if let Some(cell) = self.ca_view.as_ref().and_then(|view| view.cell(p)) {
            return if cell.material == BlockId::Water as u8 { f32::from(cell.volume) / 64.0 } else { 0.0 };
        }
        self.water_microlitres(p) as f32 / engine_core::environment::cell_volume(self.voxel_scale) as f32
    }
    pub fn water_microlitres(&self, p: IVec3) -> u64 {
        if let Some(cell) = self.ca_view.as_ref().and_then(|view| view.cell(p)) {
            return if cell.material == BlockId::Water as u8 {
                u64::from(cell.volume) * engine_core::environment::cell_volume(self.voxel_scale) / 64
            } else { 0 };
        }
        let coarse = self.water_volume(p);
        if coarse > 0 && !self.chunks.contains_key(&crate::world::chunk_coord_from_world_block(p))
            && !self.regions.view.records.contains_key(&crate::world::chunk_coord_from_world_block(p)) {
            if let Some(amount)=engine_worldgen::river_water_amount(&self.sample_config(),p) { return amount; }
        }
        if State::enabled(self.generator_version) {
            self.environment.exact_water(p, coarse)
        } else {
            u64::from(coarse) * engine_core::environment::cell_volume(self.voxel_scale) / 8
        }
    }

    /// Changes free water only. Callers own the equal opposite exchange in a
    /// soil, machine or fluid transaction. Admission failure changes nothing.
    pub fn set_water_microlitres(&mut self, p: IVec3, amount: u64) -> bool {
        if self.ca_view.is_some() || !State::enabled(self.generator_version) {
            return false;
        }
        let old = self.water_microlitres(p);
        if old == amount {
            return true;
        }
        let impact = self.fluid_impact(p, p);
        if !matches!(self.get(p), BlockId::Air | BlockId::Water)
            || amount > self.environment.water_capacity_ul(p)
            || !self.environment.can_encode(&[(p, amount)])
            || !self.environment.can_wake(impact.positions())
        {
            return false;
        }
        let coarse = amount.div_ceil(self.environment.quantum()) as u8;
        let Some(coord) = self.write_water_cell(p, coarse) else {
            return false;
        };
        self.environment.encode_water(p, amount);
        for &q in impact.positions() {
            self.environment.wake(q);
        }
        self.wake_acting(p);
        self.invalidate_snapshot_at(coord);
        self.revision = self.revision.wrapping_add(1);
        true
    }
}
