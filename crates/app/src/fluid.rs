//! Scale presets and the active conservative world simulation.

pub mod ripples;

pub use engine_core::VoxelScalePreset;

#[derive(Clone, Copy, Debug, Default)]
pub struct FluidStats {
    pub source_bytes: usize,
    pub source_failed: bool,
    pub writeback_pressure: bool,
    pub recovering: bool,
    pub active_chunks: usize,
    pub inflight_chunks: usize,
    pub pending_updates: usize,
    pub simulated_chunks_last_step: usize,
    pub applied_updates_last_step: usize,
}
