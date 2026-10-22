//! The cell record, brick coordinates and the index maths between them.
use bytemuck::{Pod, Zeroable};
use engine_core::{CHUNK_SIZE, floor_div};
use glam::IVec3;

/// Cells per brick edge: the GPU allocation, dirty-flag and dispatch unit.
pub const BRICK_SIZE: i32 = 8;
pub const BRICK_CELLS: usize = (BRICK_SIZE * BRICK_SIZE * BRICK_SIZE) as usize;
/// Bricks per chunk edge; one dirty bit per brick fits a `u64` per chunk.
pub const BRICKS_PER_AXIS: i32 = CHUNK_SIZE / BRICK_SIZE;
pub const BRICKS_PER_CHUNK: usize = (BRICKS_PER_AXIS * BRICKS_PER_AXIS * BRICKS_PER_AXIS) as usize;
pub const CHUNK_CELLS: usize = BRICK_CELLS * BRICKS_PER_CHUNK;
/// A cell holding this much of its material is full. Water already uses 0..=8.
pub const VOLUME_FULL: u8 = 8;
/// 293.15 K in tenths of a kelvin: the temperature of a cell nothing has heated.
pub const AMBIENT_DECIKELVIN: u16 = 2932;

/// One cell, 8 bytes. All integers (docs/adr/0008-ca-determinism.md); a brick is 4 KiB.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct Cell {
    /// Index into the packed material table.
    pub material: u8,
    /// How much of the cell the material fills, 0..=`VOLUME_FULL`.
    pub volume: u8,
    /// Tenths of a kelvin.
    pub temperature: u16,
    pub pressure: i16,
    pub charge: i16,
}

const _: () = assert!(size_of::<Cell>() == 8);

impl Cell {
    /// Empty space at ambient temperature. Bricks holding only this are not stored.
    pub const EMPTY: Self =
        Self { material: 0, volume: 0, temperature: AMBIENT_DECIKELVIN, pressure: 0, charge: 0 };

    /// A cell full of `material` at ambient temperature.
    pub const fn filled(material: u8) -> Self {
        Self { material, volume: VOLUME_FULL, ..Self::EMPTY }
    }
}

/// Octree depth of a brick: it covers 2^depth cells per axis. Reserved by
/// docs/adr/0007-bricks-and-scale.md so mixed scales need no save migration;
/// only `FINEST` is produced today.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Scale(pub u8);

impl Scale {
    pub const FINEST: Self = Self(0);
}

/// Which brick: a chunk, the brick's index inside it (0..64) and its scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BrickCoord {
    pub chunk: IVec3,
    pub brick: u8,
    pub scale: Scale,
}

/// Chunk coordinate and chunk-local cell (each axis 0..32) of a voxel.
pub fn split_voxel(voxel: IVec3) -> (IVec3, IVec3) {
    let chunk = IVec3::new(
        floor_div(voxel.x, CHUNK_SIZE),
        floor_div(voxel.y, CHUNK_SIZE),
        floor_div(voxel.z, CHUNK_SIZE),
    );
    (chunk, voxel - chunk * CHUNK_SIZE)
}

/// Brick index (x + 4(z + 4y)) and cell index (x + 8(z + 8y)) of a chunk-local cell.
pub fn brick_and_cell(local: IVec3) -> (u8, usize) {
    // A real assert: out of range would otherwise alias another cell silently in release.
    // One compare: a negative axis sets the top bit, and any axis >= 32 sets a bit above bit 4.
    assert!(((local.x | local.y | local.z) as u32) < CHUNK_SIZE as u32, "local cell {local} outside the chunk");
    let b = local / BRICK_SIZE;
    let c = local % BRICK_SIZE;
    (
        (b.x + BRICKS_PER_AXIS * (b.z + BRICKS_PER_AXIS * b.y)) as u8,
        (c.x + BRICK_SIZE * (c.z + BRICK_SIZE * c.y)) as usize,
    )
}

/// Inverse of `brick_and_cell`.
pub fn local_of(brick: u8, cell: usize) -> IVec3 {
    let (b, c) = (brick as i32, cell as i32);
    let brick_origin = IVec3::new(b % BRICKS_PER_AXIS, b / (BRICKS_PER_AXIS * BRICKS_PER_AXIS), (b / BRICKS_PER_AXIS) % BRICKS_PER_AXIS);
    let in_brick = IVec3::new(c % BRICK_SIZE, c / (BRICK_SIZE * BRICK_SIZE), (c / BRICK_SIZE) % BRICK_SIZE);
    brick_origin * BRICK_SIZE + in_brick
}
