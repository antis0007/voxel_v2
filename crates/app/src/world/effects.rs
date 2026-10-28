//! Synchronous, admitted effects over the live legacy world. Callers prepare
//! residency first; admission never initiates I/O or writes to a CA mirror.
//! A commit receipt is consumed immediately to publish its mesh dependency halo.
use super::{
    BlockId, World, chunk_coord_from_world_block, environment, factory,
    mark_chunk_and_neighbors_dirty,
};
use glam::IVec3;
use std::collections::HashSet;

/// Existing machine-water unit: 1/512 cubic metre, expressed in microlitres.
pub const WATER_UNIT_UL: u64 = 1_953_125;

#[derive(Clone, Copy, Debug)]
pub enum Failure {
    /// Residency or bounded admission can be retried without consuming supply.
    Deferred,
    /// Caller observed a different material; re-read before deciding output.
    Conflict,
    /// Wrong authority, assembly ownership, bounds, or invalid buffer state.
    Rejected,
    /// Empty source, full destination, or less than one representable unit.
    NoChange,
}

/// Not Clone/Copy: publishing consumes the receipt. This is a local commit
/// result, not a durable transaction ID or an asynchronous world-change journal.
#[must_use = "Publish committed effects to their derived geometry consumers"]
pub struct Commit<T> {
    position: IVec3,
    revision: u64,
    value: T,
}

impl<T> Commit<T> {
    /// Adventure publishes changed cells through its existing per-frame list;
    /// the simulation consumer expands these into the same mesh dependency halo.
    pub fn publish_voxel(self, world: &World, cells: &mut Vec<IVec3>) -> T {
        debug_assert_eq!(self.revision, world.revision());
        cells.push(self.position);
        self.value
    }

    pub fn publish(
        self,
        world: &World,
        blocks: &mut HashSet<IVec3>,
        chunks: &mut HashSet<IVec3>,
    ) -> T {
        // The producer owns &mut World through commit and immediate publication.
        debug_assert_eq!(self.revision, world.revision());
        blocks.insert(self.position);
        mark_chunk_and_neighbors_dirty(chunks, self.position);
        self.value
    }
}

pub(super) fn admit(world: &World, p: IVec3) -> Result<(), Failure> {
    admit_cells(world, &[p])
}

fn admit_cells(world: &World, cells: &[IVec3]) -> Result<(), Failure> {
    if cells.is_empty() || cells.len() > 2 {
        return Err(Failure::Rejected);
    }
    let cfg = world.sample_config();
    let mut owners = [IVec3::ZERO; 22];
    let mut count = 0;
    for &p in cells {
        if world.ca_view.is_some()
            || p.y < cfg.min_world_y()
            || p.y >= cfg.max_world_y()
            || world
                .factory
                .nodes
                .contains_key(&factory::anchor(p, factory::step(world)))
        {
            return Err(Failure::Rejected);
        }
        if !world.chunks.contains_key(&chunk_coord_from_world_block(p)) {
            return Err(Failure::Deferred);
        }
        // Check capacity for the same cellular dependency neighborhood awakened
        // by World::wake_acting; queue saturation must not strand a committed edit.
        for offset in std::iter::once(IVec3::ZERO).chain(super::activity::WAKE_NEIGHBORS) {
            let owner = chunk_coord_from_world_block(p + offset);
            if !owners[..count].contains(&owner) {
                owners[count] = owner;
                count += 1;
            }
        }
    }
    if !world.activity.can_wake(&owners[..count]) {
        return Err(Failure::Deferred);
    }
    Ok(())
}

/// One conditional terrain edit. The existing setter remains the sole material
/// authority, including environmental admission and protected-material rules.
pub fn replace(
    world: &mut World,
    p: IVec3,
    expected: BlockId,
    replacement: BlockId,
) -> Result<Commit<BlockId>, Failure> {
    admit(world, p)?;
    let old = world.get(p);
    if old != expected {
        return Err(Failure::Conflict);
    }
    if old == replacement {
        return Err(Failure::NoChange);
    }
    if matches!(old, BlockId::Bedrock | BlockId::TombStone) {
        return Err(Failure::Rejected);
    }
    world.set(p, replacement);
    if world.get(p) != replacement {
        return Err(Failure::Deferred);
    }
    Ok(Commit {
        position: p,
        revision: world.revision(),
        value: old,
    })
}

/// Place terrain and charge the selected stack only after commit.
pub fn place(
    world: &mut World,
    p: IVec3,
    expected: BlockId,
    replacement: BlockId,
    survival_slot: Option<usize>,
    blocks: &mut HashSet<IVec3>,
    chunks: &mut HashSet<IVec3>,
) -> Result<(), Failure> {
    let supply = if let Some(slot) = survival_slot {
        let item = super::inventory::Item::Block(
            super::inventory::Item::canonical_block(replacement).ok_or(Failure::Rejected)?,
        );
        if !world
            .inventory
            .hotbar
            .get(slot)
            .and_then(|stack| *stack)
            .is_some_and(|stack| stack.item == item && stack.count > 0)
        {
            return Err(Failure::Rejected);
        }
        Some((slot, item))
    } else {
        None
    };
    let commit = replace(world, p, expected, replacement)?;
    commit.publish(world, blocks, chunks);
    if let Some((slot, item)) = supply {
        let consumed = world.inventory.consume_from_hotbar(slot, item);
        debug_assert!(consumed);
        world.sync_from_inventory();
    }
    Ok(())
}

mod water;
pub use water::{WaterDirection, exchange_water};
