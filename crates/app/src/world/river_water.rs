//! One-time initial stock for newly generated river cells; saved stocks win.
use super::*;
impl World {
    /// Call only for an existing water voxel. Empty or edited solid cells
    /// are never refilled by the steady river presentation.
    pub(crate) fn is_steady_surface_water(&self, p: IVec3) -> bool {
        let cfg = self.sample_config();
        p.y > cfg.water_level()
            && engine_worldgen::steady_water_column(&cfg, p.x, p.z)
                .is_some_and(|(bed, head)| p.y > bed && (p.y as f32) < head)
    }

    pub(crate) fn generated_river_amounts(&self, c: IVec3, data: &ChunkData) -> Vec<(IVec3,u64)> {
        if self.generator_version < 79 || data.modified_bounds.is_some()
            || self.regions.view.records.contains_key(&c) { return Vec::new(); }
        let cfg = self.sample_config();
        let base = c * CHUNK_SIZE;
        data.blocks.iter().enumerate().filter_map(|(i, &b)| {
            if b != BlockId::Water { return None; }
            let p = base + IVec3::new((i % 32) as i32, (i / 1024) as i32, ((i / 32) % 32) as i32);
            engine_worldgen::river_water_amount(&cfg,p).map(|v|(p,v))
        }).collect()
    }
    pub(crate) fn install_river_amounts(&mut self, amounts: &[(IVec3,u64)]) {
        debug_assert!(self.environment.can_encode(amounts));
        let quantum = self.environment.quantum();
        for &(p,v) in amounts {
            let coarse = v.div_ceil(quantum) as u8;
            if coarse < 8 { self.water_levels.insert(p,coarse); }
            self.environment.encode_water(p,v);
            if !self.is_steady_surface_water(p) {
                self.activity.wake(p);
            }
        }
    }
}
