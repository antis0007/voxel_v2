//! Bounded grouped preparation; readiness is never inferred from scheduling.
use super::*;
impl ReadOnlyPicker {
    pub const MAX_GROUP_POINTS: usize = MAX_BATCH;

    /// Reads current authority without scheduling missing sources.
    pub fn lookup_ready(&mut self, world: &World, p: IVec3) -> Readiness<BlockId> {
        if let Err(error) = self.prepare(world) { return Readiness::Failed(error); }
        self.read(world, p)
    }

    fn collect_group(&mut self, world: &World, points: &[IVec3])
        -> Result<Vec<Readiness<BlockId>>, String> {
        if points.len() > Self::MAX_GROUP_POINTS {
            return Err("Read-only group exceeds its bounded point count".into());
        }
        self.prepare(world)?;
        let mut seen = std::collections::HashSet::new();
        let mut bytes = 0usize;
        let mut missing = Vec::new();
        let mut values = Vec::with_capacity(points.len());
        for &p in points {
            let value = self.read(world, p);
            // Count the complete retained footprint, not only this poll's misses.
            // Resident authority and exact procedural outside-world Air need no cache.
            if world.resident_block(p).is_none() {
                let key = Key::at_position(world, p);
                let outside = key.record.is_none()
                    && (p.y < key.cfg.min_world_y() || p.y >= key.cfg.max_world_y());
                if !outside && seen.insert(key.address()) {
                    bytes = bytes.checked_add(key.voxels() * std::mem::size_of::<BlockId>())
                        .ok_or_else(|| "Read-only group footprint overflow".to_string())?;
                    if bytes > CACHE_BYTES {
                        return Err("Read-only group exceeds its bounded cache footprint".into());
                    }
                    if matches!(value, Readiness::Pending) { missing.push(key); }
                }
            }
            values.push(value);
        }
        // Deeper failed speculative cells must not starve earlier missing cells.
        if !missing.is_empty() {
            if let Readiness::Failed(error) = self.request(world, missing) { return Err(error); }
        }
        Ok(values)
    }

    /// Schedules bounded missing inputs without surfacing speculative source failures.
    /// Ok means scheduling succeeded, never that the inputs are ready.
    pub fn prepare_group(&mut self, world: &World, points: &[IVec3]) -> Result<(), String> {
        self.collect_group(world, points).map(|_| ())
    }

    /// Returns ordered values only when every requested cell is ready.
    pub fn read_group(&mut self, world: &World, points: &[IVec3]) -> Readiness<Vec<BlockId>> {
        let values = match self.collect_group(world, points) {
            Ok(values) => values, Err(error) => return Readiness::Failed(error),
        };
        let mut blocks = Vec::with_capacity(values.len());
        let mut pending = false;
        for value in values {
            match value {
                Readiness::Ready(block) => blocks.push(block),
                Readiness::Pending => pending = true,
                Readiness::Failed(error) => return Readiness::Failed(error),
            }
        }
        if pending { Readiness::Pending } else { Readiness::Ready(blocks) }
    }
}
