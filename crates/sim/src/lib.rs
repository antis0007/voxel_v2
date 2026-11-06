//! The GPU-resident cellular world: 8³ bricks inside 32³ chunks, and the CPU reference kernel.
//!
//! Chunks stay the unit of streaming, saves and authority; a brick is the unit the GPU allocates,
//! flags dirty and dispatches. `step` advances the stored world one tick with 2×2×2 Margolus
//! blocks, driven only by the packed material table. `pinned` holds every definition that fixes
//! the result. See docs/adr/0007-bricks-and-scale.md and docs/adr/0008-ca-determinism.md.
mod anchors;
pub mod environment;
mod cell;
mod codec;
mod dam_break;
mod digest;
mod fine;
mod matter;
mod matter_jobs;
mod kernel;
mod offload;
mod pinned;
mod react;
mod replay;
mod rules;
mod store;
#[cfg(test)]
mod dam_break_tests;
#[cfg(test)]
mod digest_tests;
#[cfg(test)]
mod fine_tests;
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
pub use codec::CheckpointError;
pub use dam_break::{DAM_BREAK_200K, DamBreak};
pub use digest::{RegionDigest, brick_hash, step_tracked};
pub use fine::{
    BLOCK_UNITS, Collapse, FINE_CELLS, FINE_SIZE, FineStore, SUB_BRICK_BYTES, collapsed,
};
pub use matter::{
    BlockView, DORMANT_CHECKS_PER_TICK, FINE_RADII, FineGovernor, FineStepStats, MAX_FINE_AWAKE, MatterError,
    MatterMemory, MatterStepStats, MatterStore, RECOVER_TICKS,
};
pub use matter_jobs::{FineJob, FineJobs, FineJobStats, FineRegion};
pub use replay::{Action, RunCheckpoint, decode_log, encode_log, run_log};
pub use store::{BrickStore, NO_SLOT, PoolFull};
pub use kernel::{StepStats, set_kernel_workers, step, step_skipping, step_with};
pub use offload::{PhaseBlocks, gather, scatter};
pub use pinned::{
    Dice, HEAT_DEADBAND, HEAT_RATE, HEAT_TICK_INTERVAL, PHASE_OFFSETS, block_base, block_hash, flow_gate, is_heat_phase,
    is_heat_tick, mul_div_rem, mulhi,
};
pub use rules::{Outcome, block_rule, idx};
