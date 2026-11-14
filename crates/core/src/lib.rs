//! Shared primitives with no engine dependencies.
//!
//! Coordinate spaces used across the workspace:
//! - **voxel** (`IVec3`): integer cell index; one cell is `VoxelScalePreset::meters()`
//!   metres on a side.
//! - **chunk** (`IVec3`): `CHUNK_SIZE`-cell cubes; `chunk_coord_from_world_block`
//!   maps voxel to chunk with floor division so negative axes stay contiguous.
//! - **face cell** (`FaceCell`): cube-sphere column `(u, height, v)` on one of six faces.
//! - **metres** (`Vec3`): player, camera and physics positions; voxel = metres / scale.
pub mod coords;
pub mod cube_sphere;
pub mod environment;
pub mod fast_hash;
pub mod hash;
pub mod material;
pub mod planet_frame;
pub mod reaction;
pub mod scale;
pub mod time;
pub mod work;
pub mod lod;

pub use coords::{CHUNK_SIZE, PADDED_CHUNK_SIZE, chunk_coord_from_world_block, floor_div};
pub use scale::VoxelScalePreset;
pub use time::Budget;

pub mod modifiers;
