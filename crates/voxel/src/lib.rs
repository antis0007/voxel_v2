//! Voxel world data shared by generation, simulation and rendering.
//!
//! Units: `WorldSampleConfig::voxel_meters()` metres per cell; heights such as
//! `WORLD_MIN_Y_METERS` are metres and convert through `blocks_for_height`.
mod chunk;
mod config;

pub use chunk::ChunkData;
pub use config::{
    CURRENT_GENERATOR_VERSION, WATER_LEVEL_METERS, WORLD_MAX_Y_METERS, WORLD_MIN_Y_METERS, WorldSampleConfig,
    WorldType,
};
