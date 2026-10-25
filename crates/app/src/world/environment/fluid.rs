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

impl super::super::World {
    pub fn water_fill(&self, p: IVec3) -> f32 {
        self.water_microlitres(p) as f32 / engine_core::environment::cell_volume(self.voxel_scale) as f32
    }
    pub fn water_microlitres(&self, p: IVec3) -> u64 {
        let coarse = self.water_volume(p);
        if State::enabled(self.generator_version) {
            self.environment.exact_water(p, coarse)
        } else {
            u64::from(coarse) * engine_core::environment::cell_volume(self.voxel_scale) / 8
        }
    }

    /// Changes free water only. Callers own the equal opposite exchange in a
    /// soil, machine or fluid transaction. Admission failure changes nothing.
    pub fn set_water_microlitres(&mut self, p: IVec3, amount: u64) -> bool {
        if !State::enabled(self.generator_version) {
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
