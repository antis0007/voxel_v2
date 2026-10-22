//! Shared primitives with no engine dependencies.
//!
//! Coordinate spaces used across the workspace:
//! - **voxel** (`IVec3`): integer cell index; one cell is `VoxelScalePreset::meters()`
//!   metres on a side.
//! - **chunk** (`IVec3`): `CHUNK_SIZE`-cell cubes; `chunk_coord_from_world_block`
//!   maps voxel to chunk with floor division so negative axes stay contiguous.
//! - **metres** (`Vec3`): player, camera and physics positions; voxel = metres / scale.
pub mod coords;
pub mod fast_hash;
pub mod hash;
pub mod material;
pub mod scale;
pub mod time;

pub use coords::{CHUNK_SIZE, PADDED_CHUNK_SIZE, chunk_coord_from_world_block, floor_div};
pub use scale::VoxelScalePreset;
pub use time::Budget;
