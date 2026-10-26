//! Cube-sphere planet coordinates: six faces, each a 2D grid of columns.
//!
//! A column is addressed by `(u, height, v)` along its face basis, so the
//! height axis is the face normal and flat-world voxel code applies per face.
use crate::coords::CHUNK_SIZE;
use glam::IVec3;

pub mod distortion;
mod edges;
mod projection;
pub use edges::{QuarterTurns, Step};
pub use projection::{Projection, local_frame, pick_face};

/// One of the six cube faces, named by its outward normal.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[repr(u8)]
pub enum Face {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

/// Integer axes of a face: `u` and `v` span the grid, `n` is up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceBasis {
    pub u: IVec3,
    pub n: IVec3,
    pub v: IVec3,
}

impl Face {
    pub const ALL: [Face; 6] =
        [Face::PosX, Face::NegX, Face::PosY, Face::NegY, Face::PosZ, Face::NegZ];

    /// Returns the face axes; right-handed, `u x n == v`, as world `x x y == z`.
    pub const fn basis(self) -> FaceBasis {
        // PosY's (X, Y, Z) after a proper rotation (det +1) taking Y to the face normal.
        let (u, n, v) = match self {
            Face::PosX => (IVec3::new(0, -1, 0), IVec3::new(1, 0, 0), IVec3::new(0, 0, 1)),
            Face::NegX => (IVec3::new(0, 1, 0), IVec3::new(-1, 0, 0), IVec3::new(0, 0, 1)),
            Face::PosY => (IVec3::new(1, 0, 0), IVec3::new(0, 1, 0), IVec3::new(0, 0, 1)),
            Face::NegY => (IVec3::new(1, 0, 0), IVec3::new(0, -1, 0), IVec3::new(0, 0, -1)),
            Face::PosZ => (IVec3::new(1, 0, 0), IVec3::new(0, 0, 1), IVec3::new(0, -1, 0)),
            Face::NegZ => (IVec3::new(1, 0, 0), IVec3::new(0, 0, -1), IVec3::new(0, 1, 0)),
        };
        FaceBasis { u, n, v }
    }

    /// Returns the face whose outward normal is the axis unit vector `n`.
    pub fn from_normal(n: IVec3) -> Option<Face> {
        Face::ALL.into_iter().find(|f| f.basis().n == n)
    }
}

/// A voxel cell on one face: `x` along `u`, `y` height along `n`, `z` along `v`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FaceCell {
    pub face: Face,
    pub cell: IVec3,
}

/// Square extent of a face in cells, centred on the face axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceGrid {
    half: i32,
}

/// Largest half extent whose doubled coordinates fit in `i32`.
///
/// Edge stepping doubles coordinates and adds one, so `2 * half + 1` must fit.
pub const MAX_HALF: i32 = i32::MAX / 2;

impl FaceGrid {
    /// Builds a grid of `2 * half_cells` per side; `None` unless aligned and `<= MAX_HALF`.
    pub fn new(half_cells: i32) -> Option<FaceGrid> {
        let valid = half_cells > 0 && half_cells <= MAX_HALF && half_cells % CHUNK_SIZE == 0;
        valid.then_some(FaceGrid { half: half_cells })
    }

    /// Returns the half edge length in cells.
    pub fn half(self) -> i32 {
        self.half
    }

    /// Reports whether `cell` lies inside the grid; height is unbounded.
    pub fn contains(self, cell: IVec3) -> bool {
        (-self.half..self.half).contains(&cell.x) && (-self.half..self.half).contains(&cell.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bases_are_right_handed_unit_and_distinct() {
        let mut normals = std::collections::BTreeSet::new();
        for f in Face::ALL {
            let b = f.basis();
            assert_eq!(b.u.cross(b.n), b.v, "{f:?}");
            for a in [b.u, b.n, b.v] {
                assert_eq!(a.abs().element_sum(), 1, "{f:?}");
            }
            assert_eq!(Face::from_normal(b.n), Some(f));
            normals.insert((b.n.x, b.n.y, b.n.z));
        }
        assert_eq!(normals.len(), 6);
    }

    #[test]
    fn top_face_matches_flat_world_axes() {
        let b = Face::PosY.basis();
        assert_eq!((b.u, b.n, b.v), (IVec3::X, IVec3::Y, IVec3::Z));
    }

    #[test]
    fn face_grid_rejects_half_above_max_half() {
        let largest = MAX_HALF - MAX_HALF % CHUNK_SIZE;
        assert!(FaceGrid::new(largest).is_some());
        // 2^30 is chunk-aligned, but its doubled coordinate 2^31 overflows i32.
        assert_eq!(MAX_HALF + 1, 1 << 30);
        assert!(FaceGrid::new(MAX_HALF + 1).is_none());
        assert!(FaceGrid::new(i32::MAX - i32::MAX % CHUNK_SIZE).is_none());
    }

    #[test]
    fn face_grid_rejects_half_not_multiple_of_chunk() {
        assert!(FaceGrid::new(16_000).is_some());
        assert!(FaceGrid::new(16_001).is_none());
        assert!(FaceGrid::new(0).is_none());
        assert!(FaceGrid::new(-32).is_none());
        let g = FaceGrid::new(64).unwrap();
        assert!(g.contains(IVec3::new(-64, 999, 63)));
        assert!(!g.contains(IVec3::new(64, 0, 0)));
    }
}
