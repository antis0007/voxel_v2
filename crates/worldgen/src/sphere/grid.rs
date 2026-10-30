//! Six-face cell grid over a sphere with adjacency across face edges.

use engine_core::cube_sphere::{Face, FaceCell, FaceGrid, Projection, Step};
use glam::{DVec3, IVec3};

/// Projection for every position; its forward map uses only exact operations.
pub const PROJECTION: Projection = Projection::Spherified;

/// Neighbour slots, in the order `SphereGrid::neighbours` returns them.
const STEPS: [Step; 4] = [Step::PosU, Step::NegU, Step::PosV, Step::NegV];

/// Cells of all six faces, indexed `face * n * n + v * n + u`.
#[derive(Clone, Debug)]
pub struct SphereGrid {
    n: u32,
    radius_m: f64,
    neighbours: Vec<[u32; 4]>,
    centres: Vec<DVec3>,
    areas: Vec<f64>,
}

impl SphereGrid {
    /// Builds `n` by `n` cells per face on a sphere of `radius_m`.
    ///
    /// Edge stepping comes from core's `FaceGrid`, whose half extent must be a
    /// multiple of the chunk size, so `n` must be a positive multiple of 64.
    pub fn new(n: u32, radius_m: f64) -> Option<SphereGrid> {
        if !(n.is_multiple_of(2) && radius_m > 0.0 && radius_m.is_finite()) {
            return None;
        }
        let half = i32::try_from(n / 2).ok()?;
        let faces = FaceGrid::new(half)?;
        let per_face = n as usize * n as usize;
        let len = 6 * per_face;
        let mut neighbours = Vec::with_capacity(len);
        let mut centres = Vec::with_capacity(len);
        let mut areas = Vec::with_capacity(len);
        let h = f64::from(half);
        let r2 = radius_m * radius_m;
        for face in Face::ALL {
            for v in 0..half * 2 {
                for u in 0..half * 2 {
                    let cell = IVec3::new(u - half, 0, v - half);
                    let at = FaceCell { face, cell };
                    let mut nb = [0u32; 4];
                    for (slot, step) in nb.iter_mut().zip(STEPS) {
                        *slot = flat_index(n, half, faces.step_cell(at, step).0);
                    }
                    neighbours.push(nb);
                    let (s, t) = faces.cell_face_coords(cell);
                    centres.push(PROJECTION.to_sphere(face, s, t));
                    // Corner coordinates are integers over a power of two times half.
                    let s0 = f64::from(cell.x) / h;
                    let s1 = f64::from(cell.x + 1) / h;
                    let t0 = f64::from(cell.z) / h;
                    let t1 = f64::from(cell.z + 1) / h;
                    let p00 = PROJECTION.to_sphere(face, s0, t0);
                    let p10 = PROJECTION.to_sphere(face, s1, t0);
                    let p11 = PROJECTION.to_sphere(face, s1, t1);
                    let p01 = PROJECTION.to_sphere(face, s0, t1);
                    // Half the diagonal cross product is the area of the corner quad.
                    areas.push(0.5 * (p11 - p00).cross(p01 - p10).length() * r2);
                }
            }
        }
        Some(SphereGrid { n, radius_m, neighbours, centres, areas })
    }

    /// Returns the cells per face edge.
    pub fn n(&self) -> u32 {
        self.n
    }

    /// Returns the sphere radius in metres.
    pub fn radius_m(&self) -> f64 {
        self.radius_m
    }

    /// Returns the number of cells over all six faces.
    pub fn len(&self) -> usize {
        self.areas.len()
    }

    /// Reports whether the grid has no cells; never true for a built grid.
    pub fn is_empty(&self) -> bool {
        self.areas.is_empty()
    }

    /// Returns the dense index of cell `(u, v)` on `face`.
    pub fn index(&self, face: Face, u: u32, v: u32) -> usize {
        let n = self.n as usize;
        face as usize * n * n + v as usize * n + u as usize
    }

    /// Returns the face holding cell `i`.
    pub fn face_of(&self, i: usize) -> Face {
        let per_face = self.n as usize * self.n as usize;
        Face::ALL[(i / per_face).min(5)]
    }

    /// Returns the `+u, -u, +v, -v` neighbours of cell `i` in its own face axes.
    pub fn neighbours(&self, i: usize) -> [u32; 4] {
        self.neighbours[i]
    }

    /// Returns the unit direction of the centre of cell `i`.
    pub fn centre(&self, i: usize) -> DVec3 {
        self.centres[i]
    }

    /// Returns the surface area of cell `i` in square metres.
    pub fn area_m2(&self, i: usize) -> f64 {
        self.areas[i]
    }

    /// Returns every cell area in square metres, by dense index.
    pub fn areas_m2(&self) -> &[f64] {
        &self.areas
    }

    /// Returns the summed area of every cell in square metres.
    pub fn total_area_m2(&self) -> f64 {
        self.areas.iter().sum()
    }
}

fn flat_index(n: u32, half: i32, at: FaceCell) -> u32 {
    let u = (at.cell.x + half) as u32;
    let v = (at.cell.z + half) as u32;
    at.face as u32 * n * n + v * n + u
}
