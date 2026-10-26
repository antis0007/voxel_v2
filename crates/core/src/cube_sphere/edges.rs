//! Edge stepping between cube faces, derived from the face bases.
//!
//! Topology is model R: a column leaving a face edge continues on the
//! neighbour face at the same height, and three faces meet at each corner.
use super::{Face, FaceCell, FaceGrid};
use crate::coords::CHUNK_SIZE;
use glam::{IVec2, IVec3};

/// One unit move along a face's tangent axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    PosU,
    NegU,
    PosV,
    NegV,
}

impl Step {
    /// Returns the move as `(du, dv)`.
    pub(crate) fn delta(self) -> IVec2 {
        match self {
            Step::PosU => IVec2::X,
            Step::NegU => IVec2::NEG_X,
            Step::PosV => IVec2::Y,
            Step::NegV => IVec2::NEG_Y,
        }
    }
}

/// Rotation carrying source-face tangent directions onto the destination face.
///
/// `apply` turns a `(du, dv)` direction `k` quarter turns from `+u` toward `+v`,
/// one turn being `(du, dv) -> (-dv, du)`. The count is always in `0..=3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QuarterTurns(u8);

impl QuarterTurns {
    /// The rotation that changes nothing.
    pub const IDENTITY: QuarterTurns = QuarterTurns(0);

    /// Returns `k` quarter turns, reduced modulo 4.
    pub const fn new(k: u8) -> QuarterTurns {
        QuarterTurns(k % 4)
    }

    /// Returns the number of quarter turns, in `0..=3`.
    pub const fn get(self) -> u8 {
        self.0
    }

    /// Rotates a source-face direction into destination-face axes.
    pub fn apply(self, d: IVec2) -> IVec2 {
        (0..self.0).fold(d, |d, _| IVec2::new(-d.y, d.x))
    }

    /// Returns the rotation of `self` followed by `other`.
    pub fn compose(self, other: QuarterTurns) -> QuarterTurns {
        QuarterTurns((self.0 + other.0) % 4)
    }

    /// Returns the rotation that undoes `self`.
    pub fn inverse(self) -> QuarterTurns {
        QuarterTurns((4 - self.0) % 4)
    }
}

impl FaceGrid {
    /// Moves one cell, crossing onto the neighbour face at an edge.
    ///
    /// Height `y` never changes. Inputs outside the grid are a caller error
    /// and come back unchanged with `QuarterTurns::IDENTITY`.
    pub fn step_cell(self, at: FaceCell, step: Step) -> (FaceCell, QuarterTurns) {
        step_on(self.half, at, step)
    }

    /// Moves one chunk; `at.cell` is a chunk coordinate on this grid.
    pub fn step_chunk(self, at: FaceCell, step: Step) -> (FaceCell, QuarterTurns) {
        step_on(self.half / CHUNK_SIZE, at, step)
    }
}

/// Steps on a square grid of half extent `half`, whatever its unit.
fn step_on(half: i32, at: FaceCell, step: Step) -> (FaceCell, QuarterTurns) {
    let grid = FaceGrid { half };
    let unchanged = (at, QuarterTurns::IDENTITY);
    if !grid.contains(at.cell) {
        debug_assert!(false, "step from {at:?} outside half {half}");
        return unchanged;
    }
    let d = step.delta();
    let moved = at.cell + IVec3::new(d.x, 0, d.y);
    if grid.contains(moved) {
        return (FaceCell { face: at.face, cell: moved }, QuarterTurns::IDENTITY);
    }
    let a = at.face.basis();
    // The outward tangent at the edge is the neighbour face's normal.
    let t = d.x * a.u + d.y * a.v;
    let Some(face) = Face::from_normal(t) else {
        debug_assert!(false, "tangent {t} is not a face normal");
        return unchanged;
    };
    let b = face.basis();
    // Doubled coordinates keep cell centres integral: centres are odd, the surface is 2H.
    let two_h = 2 * half;
    let p = two_h * a.n + (2 * at.cell.x + 1) * a.u + (2 * at.cell.z + 1) * a.v;
    let along_edge = p - p.dot(a.n) * a.n - p.dot(t) * t;
    // The walk continues half a cell down the neighbour, toward -n_A.
    let p2 = two_h * t + along_edge + (two_h - 1) * a.n;
    let cell = IVec3::new((p2.dot(b.u) - 1) / 2, at.cell.y, (p2.dot(b.v) - 1) / 2);
    // The edge fold sends t to -n_A and fixes the edge axis.
    let fold = |w: IVec3| w - w.dot(t) * t - w.dot(t) * a.n;
    let on_b = |w: IVec3| {
        let f = fold(w);
        IVec2::new(f.dot(b.u), f.dot(b.v))
    };
    let turn = match on_b(a.u) {
        IVec2::X => QuarterTurns::IDENTITY,
        IVec2::Y => QuarterTurns::new(1),
        IVec2::NEG_X => QuarterTurns::new(2),
        _ => QuarterTurns::new(3),
    };
    // Every face basis has u x v == -n, so the fold is a proper rotation in (u, v).
    debug_assert_eq!(turn.apply(IVec2::Y), on_b(a.v));
    (FaceCell { face, cell }, turn)
}

#[cfg(test)]
#[path = "edges_tests.rs"]
mod tests;
