//! Stages authoritative edits without exposing a partially applied frame.
use super::{
    BlockId, ChunkData, World, block_light, chunk_coord_from_world_block, environment,
    mark_chunk_and_neighbors_dirty, relight_bounds,
};
use glam::IVec3;
use std::{collections::HashSet, mem::size_of, sync::Arc};

pub(crate) struct StagedChunk {
    coord: IVec3,
    data: Arc<ChunkData>,
    changed: Vec<(IVec3, BlockId)>,
    light: Option<(IVec3, IVec3)>,
}

impl StagedChunk {
    pub(crate) fn new(coord: IVec3, base: Arc<ChunkData>) -> Self {
        Self {
            coord,
            data: base,
            changed: Vec::new(),
            light: None,
        }
    }

    pub(crate) fn edit(&mut self, p: IVec3, id: BlockId) {
        debug_assert_eq!(chunk_coord_from_world_block(p), self.coord);
        let index = ChunkData::local_index(p.x, p.y, p.z);
        let old = self.data.blocks[index];
        if old == id {
            return;
        }
        let data = Arc::make_mut(&mut self.data);
        data.blocks[index] = id;
        data.note_modified(p, old, id);
        if id.is_light_source() {
            data.torches.insert(p);
        } else {
            data.torches.remove(&p);
        }
        if block_light::affects(old, id) {
            self.light = Some(self.light.map_or((p, p), |(lo, hi)| (lo.min(p), hi.max(p))));
        }
        // Retain intermediate writes so water cleanup matches direct replication.
        self.changed.push((p, id));
    }

    pub(crate) fn bytes(&self) -> usize {
        size_of::<Self>()
            + size_of::<ChunkData>()
            + 2 * size_of::<usize>()
            + chunk_bytes(&self.data)
            + self.changed.capacity() * size_of::<(IVec3, BlockId)>()
    }
}

fn chunk_bytes(data: &ChunkData) -> usize {
    data.blocks.len() * size_of::<BlockId>()
        + data.light.len()
        + data.torches.capacity() * (size_of::<IVec3>() + 1) * 2
}

fn admit(
    world: &World,
    staged: &[StagedChunk],
    byte_limit: usize,
    can_admit_dirty: impl FnOnce(&World, usize) -> bool,
) -> anyhow::Result<()> {
    let mut coords = HashSet::new();
    for chunk in staged {
        anyhow::ensure!(
            coords.insert(chunk.coord),
            "Replica frame contains a duplicate chunk"
        );
    }
    let current = world.edited_resident_bytes();
    let (mut removed, mut incoming) = (0usize, 0usize);
    let mut additional = HashSet::new();
    for chunk in staged {
        let old = world.chunks.get(&chunk.coord);
        if chunk.changed.is_empty() && old.is_some() {
            continue;
        }
        removed = removed.saturating_add(old.map_or(0, |data| chunk_bytes(data)));
        incoming = incoming.saturating_add(chunk_bytes(&chunk.data));
        if !world.regions.is_dirty(chunk.coord) {
            additional.insert(chunk.coord);
        }
        if let Some((lo, hi)) = chunk.light {
            // Seed scanning extends two reaches; each seed can spread one more.
            let reach = IVec3::splat(block_light::REACH * 3);
            let c0 = chunk_coord_from_world_block(lo - reach);
            let c1 = chunk_coord_from_world_block(hi + reach);
            for y in c0.y..=c1.y {
                for z in c0.z..=c1.z {
                    for x in c0.x..=c1.x {
                        let c = IVec3::new(x, y, z);
                        if (world.chunks.contains_key(&c) || coords.contains(&c))
                            && !world.regions.is_dirty(c)
                        {
                            additional.insert(c);
                        }
                    }
                }
            }
        }
    }
    let projected = current.saturating_sub(removed).saturating_add(incoming);
    anyhow::ensure!(
        projected <= current || projected <= byte_limit,
        "Replica resident allocation would grow from {current} to {projected} bytes past {byte_limit}-byte limit"
    );
    anyhow::ensure!(
        can_admit_dirty(world, additional.len()),
        "Replica dirty backlog cannot admit {} additional chunks",
        additional.len()
    );
    Ok(())
}

/// Publishes one prepared frame before updating any cross-chunk light.
pub(crate) fn publish(
    world: &mut World,
    staged: Vec<StagedChunk>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> anyhow::Result<()> {
    let byte_limit = super::regions::edited_budget_mib() * 4 * 1024 * 1024;
    publish_with_admission(
        world,
        staged,
        blocks,
        chunks,
        byte_limit,
        |world, additional| world.regions.can_admit_replica(additional),
    )
}

fn publish_with_admission(
    world: &mut World,
    staged: Vec<StagedChunk>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
    byte_limit: usize,
    can_admit_dirty: impl FnOnce(&World, usize) -> bool,
) -> anyhow::Result<()> {
    if let Err(error) = admit(world, &staged, byte_limit, can_admit_dirty) {
        world.request_simulation_writeback();
        return Err(error);
    }
    let mut lights = Vec::new();
    for StagedChunk {
        coord,
        data,
        changed,
        light,
    } in staged
    {
        if changed.is_empty() {
            if !world.chunks.contains_key(&coord) {
                // Procedural water amounts must not replace authoritative sidecars.
                world.chunks.insert(coord, data);
                world.regions.mark_dirty(coord);
                world.invalidate_snapshot_at(coord);
                chunks.insert(coord);
            }
            continue;
        }
        world.chunks.insert(coord, data);
        let committed = changed.len() as u64;
        for (p, id) in changed {
            if id != BlockId::Water {
                world.water_levels.remove(&p);
                world.water_levels.remove_velocity(&p);
                world.environment.water_deficit.remove(&environment::key(p));
            }
            world.changed_cells.push(p);
            blocks.insert(p);
            mark_chunk_and_neighbors_dirty(chunks, p);
        }
        world.regions.mark_dirty(coord);
        world.invalidate_snapshot_at(coord);
        world.revision = world.revision.wrapping_add(committed);
        if let Some(bounds) = light {
            lights.push(bounds);
        }
    }
    // Installing another staged chunk after this could erase propagated light.
    for (lo, hi) in lights {
        relight_bounds(world, lo, hi);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
