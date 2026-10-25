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
    let cfg = world.sample_config();
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
    let mut owners = [chunk_coord_from_world_block(p); 11];
    let mut count = 1;
    for offset in super::activity::WAKE_NEIGHBORS {
        let owner = chunk_coord_from_world_block(p + offset);
        if !owners[..count].contains(&owner) {
            owners[count] = owner;
            count += 1;
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

#[derive(Clone, Copy)]
pub enum WaterDirection {
    IntoBuffer,
    IntoWorld,
}

/// Transfer up to `limit` machine units. The exclusively borrowed buffer and
/// cell commit synchronously; a failed admission changes neither quantity.
/// Legacy worlds move whole cell packets, exact worlds preserve subpacket water.
/// This transfers free water only; sediment remains in the intake cell.
pub fn exchange_water(
    world: &mut World,
    p: IVec3,
    buffer: &mut u32,
    capacity: u32,
    limit: u32,
    direction: WaterDirection,
) -> Result<Commit<u32>, Failure> {
    admit(world, p)?;
    if *buffer > capacity || !matches!(world.get(p), BlockId::Air | BlockId::Water) {
        return Err(Failure::Rejected);
    }
    let exact = environment::State::enabled(world.generator_version);
    let before = world.water_microlitres(p);
    let cell_capacity = if exact {
        world.environment.water_capacity_ul(p)
    } else {
        engine_core::environment::cell_volume(world.voxel_scale)
    };
    let available = match direction {
        WaterDirection::IntoBuffer => (before / WATER_UNIT_UL).min(u64::from(capacity - *buffer)),
        WaterDirection::IntoWorld => {
            ((cell_capacity.saturating_sub(before)) / WATER_UNIT_UL).min(u64::from(*buffer))
        }
    };
    let mut amount = available.min(u64::from(limit)) as u32;
    let packet =
        (engine_core::environment::cell_volume(world.voxel_scale) / 8 / WATER_UNIT_UL) as u32;
    if !exact {
        amount -= amount % packet;
    }
    if amount == 0 {
        return Err(Failure::NoChange);
    }
    let delta = u64::from(amount) * WATER_UNIT_UL;
    let after = match direction {
        WaterDirection::IntoBuffer => before - delta,
        WaterDirection::IntoWorld => before + delta,
    };
    if exact {
        if !world.set_water_microlitres(p, after) {
            return Err(Failure::Deferred);
        }
    } else {
        world.set_water_volume(p, (after / (u64::from(packet) * WATER_UNIT_UL)) as u8);
        if world.water_microlitres(p) != after {
            return Err(Failure::Deferred);
        }
    }
    // Admission above guarantees these operations cannot overflow or underflow.
    match direction {
        WaterDirection::IntoBuffer => *buffer += amount,
        WaterDirection::IntoWorld => *buffer -= amount,
    }
    Ok(Commit {
        position: p,
        revision: world.revision(),
        value: amount,
    })
}
