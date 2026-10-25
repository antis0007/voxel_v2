//! Construction displaces existing water/grains through real neighboring
//! openings. It reserves the entire placement before moving either stock.
//! A sealed wet cell rejects construction; it cannot destroy the flood.
use super::*;
use crate::world::{World, chunk_coord_from_world_block as chunk};

impl World {
    pub(in crate::world) fn displace_for_construction(
        &mut self,
        p: IVec3,
        old: BlockId,
        new: BlockId,
    ) -> bool {
        if !new.is_solid() {
            return true;
        }
        let Some(definition) = engine_content::environment::definition() else {
            return false;
        };
        let cfg = self.sample_config();
        if definition.soil(old).is_some()
            && !self.environment.create_patch(cfg, p, old, self.get(p + IVec3::Y) == BlockId::Air, true)
        {
            return false;
        }
        let soil_before = self.environment.patches.get(&key(p)).map_or(0, |p| p.soil.water);
        let free_before = self.water_microlitres(p);
        let retain = definition
            .soil(new)
            .map_or(0, |law| law.capacity(self.voxel_scale))
            .min(soil_before + free_before);
        let mut water = soil_before + free_before - retain;
        let mut grains = self.environment.sediment_at(p);
        let mut plan = Vec::new();
        for d in [IVec3::Y, IVec3::X, IVec3::Z, IVec3::NEG_X, IVec3::NEG_Z, IVec3::NEG_Y] {
            if water == 0 && grains == 0 {
                break;
            }
            let q = p + d;
            if q.y < cfg.min_world_y()
                || q.y >= cfg.max_world_y()
                || !self.chunks.contains_key(&chunk(q))
                || !matches!(self.get(q), BlockId::Air | BlockId::Water)
                || self
                    .factory
                    .nodes
                    .contains_key(&super::super::factory::anchor(q, super::super::factory::step(self)))
            {
                continue;
            }
            let existing = self.water_microlitres(q);
            let room = self.environment.water_capacity_ul(q).saturating_sub(existing);
            let volume = water + engine_core::environment::sediment_volume(grains);
            let part = room.min(volume);
            if part == 0 {
                continue;
            }
            let mut wet = (water as u128 * part as u128 / volume as u128) as u64;
            let dry = (grains as u128 * part as u128 / volume as u128) as u64;
            let grain_space =
                engine_core::environment::sediment_volume(self.environment.sediment_at(q) + dry)
                    - engine_core::environment::sediment_volume(self.environment.sediment_at(q));
            wet = wet.min(room.saturating_sub(grain_space));
            if wet + dry == 0 {
                continue;
            }
            plan.push((q, existing, wet, dry));
            water -= wet;
            grains -= dry;
        }
        if water + grains > 0 {
            return false;
        }
        let positions: Vec<_> =
            std::iter::once(p).chain(plan.iter().flat_map(|(q, _, _, _)| [*q, *q - IVec3::Y])).collect();
        let amounts: Vec<_> = plan.iter().map(|(q, existing, wet, _)| (*q, existing + wet)).collect();
        if !self.environment.can_encode(&amounts) || !self.environment.can_wake(&positions) {
            return false;
        }
        let extra = plan
            .iter()
            .filter(|(q, _, _, dry)| *dry > 0 && !self.environment.sediment.contains_key(&key(*q)))
            .count();
        if self.environment.sediment.len() + extra > MAX_SEDIMENT {
            return false;
        }
        if definition.soil(new).is_some() && !self.environment.create_patch(cfg, p, new, true, false) {
            return false;
        }
        for &(q, _, _, _) in &plan {
            if self.resident_edit(chunk(q)).is_err() {
                return false;
            }
        }
        // Admission is complete. No user input or other writer can interleave
        // these operations; all target quantities and geometry remain valid.
        for &(q, existing, wet, dry) in &plan {
            let accepted = self.set_water_microlitres(q, existing + wet);
            debug_assert!(accepted);
            if dry > 0 {
                self.environment.sediment.entry(key(q)).or_default().suspended += dry;
            }
        }
        if free_before > 0 {
            let accepted = self.set_water_microlitres(p, 0);
            debug_assert!(accepted);
        }
        self.environment.sediment.remove(&key(p));
        if let Some(patch) = self.environment.patches.get_mut(&key(p)) {
            patch.soil.water = retain;
            if definition.soil(new).is_some() {
                patch.material = new;
            }
            patch.revision += 1;
        }
        true
    }
}
