//! One vertex per physical location, including cube edges and corners.
use engine_core::cube_sphere::{Face, Projection, atlas};
use glam::{DVec2, DVec3, IVec3};
use std::collections::HashMap;

pub const SIDE: usize = 128;

pub struct Mesh {
    side: usize,
    pub directions: Vec<DVec3>,
    pub neighbours: Vec<Vec<(usize, f64)>>,
    pub area: Vec<f64>,
    faces: Vec<usize>,
    offsets: Option<Vec<DVec2>>,
}

impl Mesh {
    pub fn new() -> Self {
        Self::with_side(SIDE)
    }

    pub(super) fn with_side(side: usize) -> Self {
        Self::build(side, false)
    }

    pub(super) fn irregular(side: usize) -> Self {
        Self::build(side, true)
    }

    fn build(side: usize, irregular: bool) -> Self {
        let mut ids = HashMap::<IVec3, usize>::new();
        let mut directions = Vec::new();
        let mut faces = Vec::with_capacity(6 * (side + 1).pow(2));
        let mut offsets = irregular.then(|| Vec::with_capacity(6 * side * side + 2));
        for face in Face::ALL {
            let b = face.basis();
            for z in 0..=side {
                for x in 0..=side {
                    let key = b.n * side as i32 + b.u * (2 * x as i32 - side as i32)
                        + b.v * (2 * z as i32 - side as i32);
                    let id = *ids.entry(key).or_insert_with(|| {
                        let id = directions.len();
                        let offset = if let Some(offsets) = &mut offsets {
                            let offset = if x == 0 || z == 0 || x == side || z == side {
                                DVec2::ZERO
                            } else {
                                node_offset(key)
                            };
                            offsets.push(offset);
                            offset
                        } else { DVec2::ZERO };
                        directions.push(Projection::Tangent.to_sphere(face,
                            2.0 * (x as f64 + offset.x) / side as f64 - 1.0,
                            2.0 * (z as f64 + offset.y) / side as f64 - 1.0));
                        id
                    });
                    faces.push(id);
                }
            }
        }
        let mut mesh = Self { side, neighbours: vec![Vec::with_capacity(8); directions.len()],
            area: vec![0.0; directions.len()], directions, faces, offsets };
        for face in Face::ALL {
            for z in 0..side {
                for x in 0..side {
                    let quad = [mesh.at(face, x, z), mesh.at(face, x + 1, z),
                        mesh.at(face, x + 1, z + 1), mesh.at(face, x, z + 1)];
                    let [a, b, c, d] = quad.map(|i| mesh.directions[i]);
                    let area = (triangle_area(a, b, c) + triangle_area(a, c, d)) * atlas::RADIUS_M.powi(2);
                    for &i in &quad {
                        mesh.area[i] += area * 0.25;
                        for &j in &quad {
                            if i != j && !mesh.neighbours[i].iter().any(|&(n, _)| n == j) {
                                let distance = (mesh.directions[i] - mesh.directions[j]).length() * atlas::RADIUS_M;
                                mesh.neighbours[i].push((j, distance));
                            }
                        }
                    }
                }
            }
        }
        // Stable tie order for receiver selection and floating-point accumulation.
        for neighbours in &mut mesh.neighbours { neighbours.sort_unstable_by_key(|e| e.0); }
        mesh
    }

    fn at(&self, face: Face, x: usize, z: usize) -> usize {
        let stride = self.side + 1;
        self.faces[(face as usize * stride + z) * stride + x]
    }

    pub(super) fn face_vertices(&self) -> &[usize] {
        &self.faces
    }

    pub(super) fn is_irregular(&self) -> bool {
        self.offsets.is_some()
    }

    pub fn stencil(&self, dir: DVec3) -> ([usize; 4], [f64; 4]) {
        let (face, s, t) = Projection::Tangent.from_sphere(dir);
        let side = self.side;
        let u = ((s + 1.0) * 0.5 * side as f64).clamp(0.0, side as f64);
        let v = ((t + 1.0) * 0.5 * side as f64).clamp(0.0, side as f64);
        let (x, z) = ((u.floor() as usize).min(side - 1), (v.floor() as usize).min(side - 1));
        let (nodes, p) = if let Some(offsets) = &self.offsets {
            self.locate_irregular(face, DVec2::new(u, v), x, z, offsets)
        } else {
            (self.quad(face, x, z), DVec2::new(u - x as f64, v - z as f64))
        };
        let (a, b) = (p.x, p.y);
        (nodes,
            [(1.0 - a) * (1.0 - b), a * (1.0 - b), (1.0 - a) * b, a * b])
    }

    fn quad(&self, face: Face, x: usize, z: usize) -> [usize; 4] {
        [self.at(face, x, z), self.at(face, x + 1, z),
            self.at(face, x, z + 1), self.at(face, x + 1, z + 1)]
    }

    /// Invert the bounded bilinear warp so terrain and runoff use the
    /// same physical vertices. The cube perimeter is left coincident.
    fn locate_irregular(&self, face: Face, point: DVec2, mut x: usize, mut z: usize,
        offsets: &[DVec2]) -> ([usize; 4], DVec2)
    {
        let inverse = |x: usize, z: usize| {
            let nodes = self.quad(face, x, z);
            let [a, b, c, d] = nodes.map(|i| offsets[i]);
            let target = point - DVec2::new(x as f64, z as f64);
            let (u, v, twist) = (DVec2::X + b - a, DVec2::Y + c - a, a - b - c + d);
            let mut local = target;
            for _ in 0..5 {
                let error = target - (a + u * local.x + v * local.y + twist * (local.x * local.y));
                if error.length_squared() < 1e-24 { break; }
                let dx = u + twist * local.y;
                let dz = v + twist * local.x;
                let determinant = dx.perp_dot(dz);
                if determinant.abs() < 1e-6 { break; }
                local += DVec2::new(error.perp_dot(dz), dx.perp_dot(error)) / determinant;
            }
            (nodes, local)
        };
        let origin = (x, z);
        let mut best = (f64::INFINITY, self.quad(face, x, z), DVec2::ZERO);
        let mut accept = |nodes, local: DVec2| {
            let inside = local.clamp(DVec2::ZERO, DVec2::ONE);
            let gap = local.distance_squared(inside);
            if gap < best.0 { best = (gap, nodes, inside); }
            (gap <= 1e-18).then_some((nodes, inside))
        };
        for _ in 0..3 {
            let (nodes, local) = inverse(x, z);
            if let Some(result) = accept(nodes, local) { return result; }
            let adjacent = |cell: usize, t: f64| {
                if t < -1e-9 { cell.saturating_sub(1) }
                else if t > 1.0 + 1e-9 { (cell + 1).min(self.side - 1) }
                else { cell }
            };
            let next = (adjacent(x, local.x), adjacent(z, local.y));
            if next == (x, z) { break; }
            (x, z) = next;
        }
        // A point near a skewed corner may walk into the diagonal cell
        // first. Every displaced cell lies within one of its original address.
        for z in origin.1.saturating_sub(1)..=(origin.1 + 1).min(self.side - 1) {
            for x in origin.0.saturating_sub(1)..=(origin.0 + 1).min(self.side - 1) {
                let (nodes, local) = inverse(x, z);
                if let Some(result) = accept(nodes, local) { return result; }
            }
        }
        (best.1, best.2)
    }

    pub fn sample(&self, field: &[f64], dir: DVec3) -> f64 {
        let (ids, weights) = self.stencil(dir);
        (0..4).map(|i| field[ids[i]] * weights[i]).sum()
    }

    pub fn reconstruct_height(&self, field: &[f64], gradients: &[DVec3], dir: DVec3) -> f64 {
        self.reconstruct_height_from_stencil(field, gradients, dir, self.stencil(dir))
    }

    // The stencil must come from this mesh and exactly this direction.
    pub(super) fn reconstruct_height_from_stencil(&self, field: &[f64], gradients: &[DVec3],
        dir: DVec3, (ids, w): ([usize; 4], [f64; 4])) -> f64
    {
        let fade = |t: f64| t * t * (3.0 - 2.0 * t);
        let (u, v) = (fade(w[1] + w[3]), fade(w[2] + w[3]));
        // Blend tangent planes, not just vertex heights. The weight of a
        // departing vertex and its derivative both vanish at the cell edge.
        let weights = [(1.0 - u) * (1.0 - v), u * (1.0 - v), (1.0 - u) * v, u * v];
        ids.into_iter().zip(weights).map(|(i, w)| {
            let offset = (dir - self.directions[i]) * atlas::RADIUS_M;
            (field[i] + gradients[i].dot(offset)) * w
        }).sum()
    }
}

fn node_offset(p: IVec3) -> DVec2 {
    let mut bits = (p.x as u32).wrapping_mul(0x9e3779b1)
        ^ (p.y as u32).wrapping_mul(0x85ebca77) ^ (p.z as u32).wrapping_mul(0xc2b2ae3d);
    bits = (bits ^ (bits >> 16)).wrapping_mul(0x7feb352d);
    bits = (bits ^ (bits >> 15)).wrapping_mul(0x846ca68b);
    bits ^= bits >> 16;
    DVec2::new(f64::from(bits & 0xffff), f64::from(bits >> 16)) * (0.48 / 65535.0)
        - DVec2::splat(0.24)
}

fn triangle_area(a: DVec3, b: DVec3, c: DVec3) -> f64 {
    2.0 * a.dot(b.cross(c)).abs().atan2(1.0 + a.dot(b) + b.dot(c) + c.dot(a))
}
