//! Installs host chunks into a joiner's world as whole chunks.
//! The host already admitted every cell, so the replica skips gameplay admission
//! (environment budgets, material checks) and per-cell worldgen sampling.
use crate::world::{self, BlockId, World, regions::DiskChunk};
use glam::IVec3;
use std::{
    collections::HashSet,
    sync::Arc,
};

const SIDE: usize = 32;
mod transaction;
pub(super) use transaction::Queue;

/// Replaces chunk `coord` with the run-length cells; returns false when the chunk cannot be
/// made resident. Changed cells are added to `changed`. Runs must already be validated to
/// cover exactly one chunk.
pub fn install_chunk(world: &mut World, coord: IVec3, runs: &[(u16, u32)], changed: &mut HashSet<IVec3>) -> bool {
    // Loads or generates the base once, for its light and any cells the host left procedural.
    if !world.admit_edit_chunk(coord) {
        return false;
    }
    let Some(current) = world.chunks.get(&coord) else { return false };
    let mut data = world::ChunkData::clone(current);
    let base = coord * SIDE as i32;
    let mut light_lo: Option<IVec3> = None;
    let mut light_hi: Option<IVec3> = None;
    let mut index = 0usize;
    for &(count, block) in runs {
        let id = BlockId::from_u32(block);
        for _ in 0..count {
            let old = data.blocks[index];
            if old != id {
                // ChunkData::local_index order: x + z*32 + y*32*32.
                let p = base + IVec3::new((index % SIDE) as i32, (index / (SIDE * SIDE)) as i32, ((index / SIDE) % SIDE) as i32);
                data.blocks[index] = id;
                data.note_modified(p, old, id);
                if id.is_light_source() {
                    data.torches.insert(p);
                } else {
                    data.torches.remove(&p);
                }
                if world::block_light::affects(old, id) {
                    light_lo = Some(light_lo.map_or(p, |lo| lo.min(p)));
                    light_hi = Some(light_hi.map_or(p, |hi| hi.max(p)));
                }
                changed.insert(p);
            }
            index += 1;
        }
    }
    // Removed first: installing never replaces a resident chunk.
    world.chunks.remove(&coord);
    world.install_simulation_chunk(coord, DiskChunk { chunk: Arc::new(data), water: Arc::default() });
    if let (Some(lo), Some(hi)) = (light_lo, light_hi) {
        world::relight_bounds(world, lo, hi);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fluid::VoxelScalePreset, world::WorldType};

    #[test]
    fn whole_chunks_install_without_admission() {
        let mut world = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7);
        let coord = IVec3::new(1, 2, -1);
        // Half water, half stone, by local index.
        let half = (SIDE * SIDE * SIDE / 2) as u16;
        let runs = [(half, BlockId::Water.as_u32()), (half, BlockId::Stone.as_u32())];
        let mut changed = HashSet::new();
        assert!(install_chunk(&mut world, coord, &runs, &mut changed));
        let base = coord * 32;
        assert_eq!(world.get(base), BlockId::Water);
        assert_eq!(world.get(base + IVec3::new(31, 15, 31)), BlockId::Water);
        assert_eq!(world.get(base + IVec3::new(0, 16, 0)), BlockId::Stone);
        assert_eq!(world.get(base + IVec3::new(31, 31, 31)), BlockId::Stone);
        assert!(!changed.is_empty());
        // A second install of the same cells changes nothing.
        let mut again = HashSet::new();
        assert!(install_chunk(&mut world, coord, &runs, &mut again));
        assert!(again.is_empty());
    }
}
