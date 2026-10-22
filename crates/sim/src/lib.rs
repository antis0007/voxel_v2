//! The GPU-resident cellular world's storage: 8³ bricks inside 32³ chunks.
//!
//! Chunks stay the unit of streaming, saves and authority; a brick is the unit
//! the GPU allocates, flags dirty and dispatches. See
//! docs/adr/0007-bricks-and-scale.md for the layout and scale rules.
mod cell;
mod store;
#[cfg(test)]
mod tests;

pub use cell::{
    AMBIENT_DECIKELVIN, BRICK_CELLS, BRICK_SIZE, BRICKS_PER_AXIS, BRICKS_PER_CHUNK, BrickCoord, CHUNK_CELLS, Cell,
    Scale, VOLUME_FULL, brick_and_cell, local_of, split_voxel,
};
pub use store::{BrickStore, NO_SLOT, PoolFull};
