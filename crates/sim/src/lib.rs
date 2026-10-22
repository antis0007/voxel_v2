//! The GPU-resident cellular world: 8³ bricks inside 32³ chunks, and the CPU reference kernel.
//!
//! Chunks stay the unit of streaming, saves and authority; a brick is the unit the GPU allocates,
//! flags dirty and dispatches. `step` advances the stored world one tick with 2×2×2 Margolus
//! blocks, driven only by the packed material table. `pinned` holds every definition that fixes
//! the result. See docs/adr/0007-bricks-and-scale.md and docs/adr/0008-ca-determinism.md.
mod anchors;
mod cell;
mod kernel;
mod pinned;
mod react;
mod rules;
mod store;
#[cfg(test)]
mod heat_tests;
#[cfg(test)]
mod react_tests;
#[cfg(test)]
mod scenario_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub use cell::{
    AMBIENT_DECIKELVIN, BRICK_CELLS, BRICK_SIZE, BRICKS_PER_AXIS, BRICKS_PER_CHUNK, BrickCoord, CHUNK_CELLS, Cell,
    Scale, VOLUME_FULL, brick_and_cell, local_of, split_voxel,
};
pub use store::{BrickStore, NO_SLOT, PoolFull};
pub use kernel::{StepStats, step};
pub use pinned::{
    Dice, HEAT_DEADBAND, HEAT_RATE, HEAT_TICK_INTERVAL, PHASE_OFFSETS, block_base, block_hash, flow_gate, is_heat_phase,
    is_heat_tick, mul_div_rem, mulhi,
};
pub use rules::{Outcome, block_rule, idx};
