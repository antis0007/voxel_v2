//! Copied building regions: a box of block ids that can turn and mirror.
//!
//! Coordinates inside a blueprint are local cells from its min corner; the
//! world side is plain voxel `IVec3`, so chunk borders need no special case.
mod apply;
mod capture;
mod orient;
pub mod format;
pub mod library;
#[cfg(test)]
mod tests;

pub use apply::{Change, History, Job, Sink};
pub use capture::Capture;
pub use orient::{mirror_block, rotate_block};

use super::BlockId;
use glam::IVec3;

/// Longest blueprint side in cells.
pub const MAX_SIDE: i32 = 256;
/// Largest blueprint volume in cells (128³).
pub const MAX_VOLUME: usize = 1 << 21;

/// Whether a box of this size can be held as a blueprint.
pub fn valid_size(size: IVec3) -> bool {
    size.cmpge(IVec3::ONE).all()
        && size.cmple(IVec3::splat(MAX_SIDE)).all()
        && volume_of(size) <= MAX_VOLUME as u64
}

fn volume_of(size: IVec3) -> u64 {
    size.as_i64vec3().max(glam::I64Vec3::ZERO).element_product() as u64
}

/// A box of cells, x fastest, then z, then y, so index order climbs upward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blueprint {
    size: IVec3,
    cells: Vec<BlockId>,
}

impl Blueprint {
    /// Wraps `cells`; `None` when the size is out of range or the length disagrees.
    pub fn new(size: IVec3, cells: Vec<BlockId>) -> Option<Self> {
        (valid_size(size) && cells.len() as u64 == volume_of(size)).then_some(Self { size, cells })
    }

    pub fn filled(size: IVec3, block: BlockId) -> Option<Self> {
        valid_size(size).then(|| Self { size, cells: vec![block; volume_of(size) as usize] })
    }

    pub fn size(&self) -> IVec3 {
        self.size
    }

    pub fn cells(&self) -> &[BlockId] {
        &self.cells
    }

    pub fn volume(&self) -> usize {
        self.cells.len()
    }

    /// Non-air cells.
    pub fn solid_count(&self) -> usize {
        self.cells.iter().filter(|b| **b != BlockId::Air).count()
    }

    pub fn index(&self, p: IVec3) -> usize {
        (p.x + self.size.x * (p.z + self.size.z * p.y)) as usize
    }

    pub fn local(&self, index: usize) -> IVec3 {
        let i = index as i32;
        let (sx, sz) = (self.size.x, self.size.z);
        IVec3::new(i % sx, i / (sx * sz), (i / sx) % sz)
    }

    pub fn get(&self, p: IVec3) -> BlockId {
        self.cells[self.index(p)]
    }

    pub fn set(&mut self, p: IVec3, block: BlockId) {
        let i = self.index(p);
        self.cells[i] = block;
    }

    /// A quarter turn about +Y: local `(x, y, z)` moves to `(sz-1-z, y, x)`.
    pub fn rotated(&self) -> Self {
        let size = IVec3::new(self.size.z, self.size.y, self.size.x);
        let mut out = Self { size, cells: vec![BlockId::Air; self.cells.len()] };
        for (i, &b) in self.cells.iter().enumerate() {
            let p = self.local(i);
            out.set(IVec3::new(self.size.z - 1 - p.z, p.y, p.x), rotate_block(b));
        }
        out
    }

    /// Mirrored across the YZ plane: local `x` becomes `sx-1-x`.
    pub fn mirrored(&self) -> Self {
        let mut out = Self { size: self.size, cells: vec![BlockId::Air; self.cells.len()] };
        for (i, &b) in self.cells.iter().enumerate() {
            let p = self.local(i);
            out.set(IVec3::new(self.size.x - 1 - p.x, p.y, p.z), mirror_block(b));
        }
        out
    }
}

/// Inclusive box spanned by two corner cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub min: IVec3,
    pub max: IVec3,
}

impl Region {
    pub fn from_corners(a: IVec3, b: IVec3) -> Self {
        Self { min: a.min(b), max: a.max(b) }
    }

    pub fn size(&self) -> IVec3 {
        self.max - self.min + IVec3::ONE
    }

    pub fn volume(&self) -> u64 {
        volume_of(self.size())
    }
}

/// Paste origin that centres the blueprint's floor on `target`.
pub fn origin_at(target: IVec3, size: IVec3) -> IVec3 {
    target - IVec3::new(size.x / 2, 0, size.z / 2)
}
