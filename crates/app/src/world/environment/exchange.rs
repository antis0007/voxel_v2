//! Conservative fluid and grain admission and exchange.
use super::*;
impl State {
    pub fn can_transport(&self, from: IVec3, to: IVec3, impact: &FluidImpact) -> bool {
        self.can_wake(impact.positions())
            && (!self.sediment.contains_key(&key(from))
                || self.sediment.contains_key(&key(to))
                || self.sediment.len() < MAX_SEDIMENT)
            && (!impact.record_work
                || self.hydraulic_work.contains_key(&key(to))
                || self.hydraulic_work.len() < MAX_EVENTS)
    }

    #[cfg(test)]
    pub fn free_water_capacity(&self, p: IVec3) -> u8 {
        let volume = 1_000_000_000 / (self.cells_per_metre.max(1) as u64).pow(3);
        let residual = self.patches.get(&key(p)).map_or(0, |patch| patch.soil.water);
        let occupied = engine_core::environment::sediment_volume(self.sediment_at(p)) + residual;
        (volume.saturating_sub(occupied) / (volume / 8)).min(8) as u8
    }

    pub fn transfer_fits(&self, from: IVec3, to: IVec3, source: u8, target: u8, moved: u8) -> bool {
        let source_ul = self.exact_water(from, source);
        let moved_ul = source_ul * u64::from(moved) / u64::from(source.max(1));
        let grain = self.sediment_at(to)
            + engine_sim::environment::advected_quantity(
                self.sediment.get(&key(from)).map_or(0, |s| s.suspended),
                source_ul,
                moved_ul,
            );
        self.exact_water(to, target) + moved_ul <= self.capacity_with_grains(to, grain)
            && self.hydraulic_work.get(&key(to)).copied().unwrap_or(0) < 999_000_000_000_000
    }
    /// Invoke after the admitted water pair commits.
    pub fn transport_exact(
        &mut self,
        from: IVec3,
        to: IVec3,
        source: u64,
        target: u64,
        moved: u64,
        impact: &FluidImpact,
    ) {
        let a = key(from);
        let b = key(to);
        // Change in the pair's gravitational potential, including partial-cell
        // centroids. E = mass * g * head; 981/100 m/s2, integer millijoules.
        let n = self.cells_per_metre.max(1) as u64;
        let volume = 1_000_000_000 / n.pow(3);
        let head = ((i128::from(from.y) - i128::from(to.y)) * i128::from(volume) + i128::from(source)
            - i128::from(target)
            - i128::from(moved))
        .max(0) as u128;
        let work = (moved as u128 * 981 * head / (100_000 * n * volume) as u128) as u64;
        if work > 0 && impact.record_work {
            *self.hydraulic_work.entry(b).or_default() += work;
        }
        if let Some(stock) = self.sediment.get(&a).copied() {
            let transfer = engine_sim::environment::advected_quantity(stock.suspended, source, moved);
            if transfer > 0 {
                if transfer == stock.mass() {
                    self.sediment.remove(&a);
                } else {
                    self.sediment.get_mut(&a).unwrap().suspended -= transfer;
                }
                self.sediment.entry(b).or_default().suspended += transfer;
            }
        }
        for &p in impact.positions() {
            self.wake(p);
        }
    }
}
