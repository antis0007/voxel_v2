//! Allocate environmental work at a fluid/substrate contact, not throughout
//! a body of water. Unknown geometry retains a probe until it can be loaded.
use super::*;
use crate::world::{ChunkData, World, chunk_coord_from_world_block};

#[derive(Default)]
pub struct FluidImpact {
    positions: [IVec3; 4],
    count: usize,
    pub record_work: bool,
}
impl FluidImpact {
    pub fn positions(&self) -> &[IVec3] {
        &self.positions[..self.count]
    }
    fn add(&mut self, p: IVec3) {
        if !self.positions().contains(&p) {
            self.positions[self.count] = p;
            self.count += 1;
        }
    }
}

impl World {
    pub fn fluid_impact(&self, from: IVec3, to: IVec3) -> FluidImpact {
        let mut impact = FluidImpact::default();
        let definition = engine_content::environment::definition();
        let moving_grains = self.environment.sediment.get(&key(from)).is_some_and(|s| s.suspended > 0);
        for p in [from, to] {
            let stock = self.environment.sediment_at(p) > 0 || (p == to && moving_grains);
            let residue = self.environment.patches.contains_key(&self.environment.owner(p));
            let below = p - IVec3::Y;
            let material = self
                .chunks
                .get(&chunk_coord_from_world_block(below))
                .map(|c| c.blocks[ChunkData::local_index(below.x, below.y, below.z)]);
            let contact = material.is_none_or(|b| definition.is_some_and(|d| d.soil(b).is_some()));
            if contact {
                impact.add(below);
            }
            if stock || residue || (p == to && contact) {
                impact.add(p);
            }
            if p == to {
                impact.record_work = stock || contact;
            }
        }
        impact
    }
}
