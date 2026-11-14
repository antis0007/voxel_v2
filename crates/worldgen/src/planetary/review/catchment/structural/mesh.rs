//! Irregular triangular control volumes for the regional rock solve.
use crate::geo::grid::Grid;
use glam::DVec2;
use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

#[derive(Clone, Copy, Default)]
pub(in crate::planetary) struct Link {
    pub to: u32,
    pub distance: f32,
    pub share: f32,
}

#[derive(Clone, Default)]
pub(in crate::planetary) struct Links {
    edges: [Link; 8],
    count: usize,
}

impl Links {
    fn push(&mut self, link: Link) {
        self.edges[self.count] = link;
        self.count += 1;
    }

    pub fn as_slice(&self) -> &[Link] {
        &self.edges[..self.count]
    }
}

pub(in crate::planetary) struct Mesh {
    pub grid: Grid,
    pub points: Vec<DVec2>,
    pub area: Vec<f64>,
    pub links: Vec<Links>,
    diagonals: Vec<bool>,
}

impl Mesh {
    pub fn new(grid: Grid) -> Self {
        Self::with_alignment(grid, false)
    }

    pub fn with_alignment(grid: Grid, aligned: bool) -> Self {
        let points: Vec<_> = (0..grid.len())
            .map(|i| {
                let mut p = DVec2::from_array(grid.at(i));
                if !grid.on_border(i) {
                    let salt = (grid.cell_m as u32).wrapping_mul(0xa38d6a1d);
                    let index = if aligned {
                        let cell = (p / grid.cell_m).floor().as_ivec2();
                        (cell.x as u32).wrapping_mul(0x9e3779b1) ^ hash(cell.y as u32)
                    } else { i as u32 };
                    let v = hash(index ^ salt);
                    let a = f64::from(v & 0xffff) / 65535.0 - 0.5;
                    let b = f64::from(v >> 16) / 65535.0 - 0.5;
                    p += DVec2::new(a, b) * (0.56 * grid.cell_m);
                }
                p
            })
            .collect();
        let mut mesh = Self {
            grid,
            points,
            area: vec![0.0; grid.len()],
            links: vec![Links::default(); grid.len()],
            diagonals: vec![false; grid.len()],
        };
        for z in 0..grid.n - 1 {
            for x in 0..grid.n - 1 {
                let a = z * grid.n + x;
                let b = a + 1;
                let c = a + grid.n;
                let d = c + 1;
                mesh.diagonals[a] = mesh.points[a].distance_squared(mesh.points[d])
                    <= mesh.points[b].distance_squared(mesh.points[c]);
                for [i, j, k] in mesh.triangles(a) {
                    let twice = (mesh.points[j] - mesh.points[i])
                        .perp_dot(mesh.points[k] - mesh.points[i])
                        .abs();
                    for node in [i, j, k] {
                        mesh.area[node] += twice / 6.0;
                    }
                    for (l, r) in [(i, j), (j, k), (k, i)] {
                        if mesh.links[l].as_slice().iter().any(|e| e.to as usize == r) {
                            continue;
                        }
                        let distance = mesh.points[l].distance(mesh.points[r]) as f32;
                        mesh.links[l].push(Link {
                            to: r as u32,
                            distance,
                            share: 0.0,
                        });
                        mesh.links[r].push(Link {
                            to: l as u32,
                            distance,
                            share: 0.0,
                        });
                    }
                }
            }
        }
        mesh
    }

    fn triangles(&self, a: usize) -> [[usize; 3]; 2] {
        let b = a + 1;
        let c = a + self.grid.n;
        let d = c + 1;
        if self.diagonals[a] {
            [[a, b, d], [a, d, c]]
        } else {
            [[a, b, c], [b, d, c]]
        }
    }

    /// Resample a solved field from the triangular surface to review cells.
    pub fn regularize(&self, field: impl Fn(usize) -> f64) -> Vec<f32> {
        (0..self.grid.len())
            .map(|i| {
                let p = DVec2::from_array(self.grid.at(i));
                let (ids, weights) = self.stencil(p);
                ids.into_iter().zip(weights).map(|(j, w)| field(j) * w).sum::<f64>() as f32
            })
            .collect()
    }

    /// Interpolate directly from the irregular surface. Outside its outermost
    /// triangles, use the nearest boundary node.
    pub fn stencil(&self, p: DVec2) -> ([usize; 3], [f64; 3]) {
        let n = self.grid.n;
        let (x, z, tx, tz) = self.grid.cell(p.x, p.y);
        for v in z.saturating_sub(1)..=(z + 1).min(n - 2) {
            for u in x.saturating_sub(1)..=(x + 1).min(n - 2) {
                for ids @ [a, b, c] in self.triangles(v * n + u) {
                    let pa = self.points[a];
                    let ab = self.points[b] - pa;
                    let ac = self.points[c] - pa;
                    let det = ab.perp_dot(ac);
                    let q = p - pa;
                    let t = q.perp_dot(ac) / det;
                    let s = ab.perp_dot(q) / det;
                    if t >= -1e-8 && s >= -1e-8 && t + s <= 1.0 + 1e-8 {
                        return (ids, [1.0 - t - s, t, s]);
                    }
                }
            }
        }
        let nearest = (z + usize::from(tz >= 0.5)) * n + x + usize::from(tx >= 0.5);
        ([nearest; 3], [1.0, 0.0, 0.0])
    }

    pub fn route(&self, height: &[f64], fixed: &[bool], rain: &[f64]) -> Routing {
        self.route_flow(height, fixed, rain, None)
    }

    pub fn route_momentum(&self, height: &[f64], fixed: &[bool], rain: &[f64], inlets: &[DVec2]) -> Routing {
        self.route_flow(height, fixed, rain, Some(inlets))
    }

    fn route_flow(&self, height: &[f64], fixed: &[bool], rain: &[f64], inlets: Option<&[DVec2]>) -> Routing {
        let len = height.len();
        let moving = inlets.is_some();
        let mut order = Vec::with_capacity(len);
        let mut rank = vec![0_u32; len];
        let mut parents = vec![u32::MAX; len];
        let mut spill = height.to_vec();
        let mut heap = BinaryHeap::new();
        for i in 0..len {
            if fixed[i] {
                parents[i] = i as u32;
                heap.push(Reverse(Key(height[i], i as u32)));
            }
        }
        while let Some(Reverse(Key(level, node))) = heap.pop() {
            let i = node as usize;
            rank[i] = order.len() as u32;
            order.push(node);
            for edge in self.links[i].as_slice() {
                let j = edge.to as usize;
                if parents[j] == u32::MAX {
                    parents[j] = node;
                    spill[j] = spill[i].max(height[j]);
                    heap.push(Reverse(Key(height[j].max(level + 0.0001), edge.to)));
                }
            }
        }
        let mut q: Vec<_> = rain.iter().zip(&self.area).map(|(&r, &a)| r * a).collect();
        let mut links = vec![Links::default(); len];
        let mut grade = vec![0.0; len];
        let mut momentum = inlets.map(<[DVec2]>::to_vec);
        let mut speed = if moving { vec![0.0; len] } else { Vec::new() };
        for &i in order.iter().rev() {
            let i = i as usize;
            if fixed[i] {
                continue;
            }
            let routes = &mut links[i];
            let mut sum = 0.0;
            let confinement = (q[i] / 42_000.0).min(1.0);
            let velocity = momentum.as_ref().map_or(DVec2::ZERO, |p| p[i] / q[i].max(1e-12));
            for &edge in self.links[i].as_slice() {
                let j = edge.to as usize;
                let slope = (height[i] - height[j]) / f64::from(edge.distance);
                if rank[j] >= rank[i] || slope <= 1e-8 || spill[i] > height[i] + 0.08 {
                    continue;
                }
                let w = if moving {
                    let direction = (self.points[j] - self.points[i]) / f64::from(edge.distance);
                    let carry = velocity.dot(direction).max(0.0);
                    slope.powf(1.55) * (1.0 + 1.8 * carry / (1.0 + carry))
                } else {
                    slope.powi(2) * (1.0 - confinement + confinement * slope.powi(2))
                };
                routes.push(Link {
                    share: w as f32,
                    ..edge
                });
                grade[i] += slope * w;
                sum += w;
            }
            if sum == 0.0 {
                let to = parents[i];
                let distance = self.points[i].distance(self.points[to as usize]) as f32;
                routes.push(Link {
                    to,
                    distance,
                    share: 1.0,
                });
            } else {
                grade[i] /= sum;
                for e in &mut routes.edges[..routes.count] {
                    e.share = (f64::from(e.share) / sum) as f32;
                }
            }
            // Round only the stored weights. Normalize before distributing
            // water to keep the runoff budget stable on long routes.
            let normalized: f64 = routes.as_slice().iter().map(|e| f64::from(e.share)).sum();
            for &e in routes.as_slice() {
                let j = e.to as usize;
                let discharge = q[i] * f64::from(e.share) / normalized;
                q[j] += discharge;
                if let Some(momentum) = &mut momentum && grade[i] > 0.0 {
                    // Transport discharge and its velocity together. Gravity
                    // turns the flow while drag relaxes it over physical distance.
                    let distance = f64::from(e.distance);
                    let direction = (self.points[j] - self.points[i]) / distance;
                    let equilibrium = (38.0 * (height[i] - height[j]).max(0.0) / distance).sqrt();
                    let memory = (-distance / 19.0).exp();
                    let next = velocity * memory + direction * (equilibrium * (1.0 - memory));
                    momentum[j] += next * discharge;
                    speed[i] += next.length() * f64::from(e.share) / normalized;
                }
            }
        }
        Routing {
            order,
            q,
            links,
            grade,
            speed,
        }
    }
}

pub(in crate::planetary) struct Routing {
    pub order: Vec<u32>,
    pub q: Vec<f64>,
    pub links: Vec<Links>,
    pub grade: Vec<f64>,
    pub speed: Vec<f64>,
}

#[derive(Clone, Copy, PartialEq)]
struct Key(f64, u32);
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0).then(self.1.cmp(&other.1))
    }
}

fn hash(mut n: u32) -> u32 {
    n = (n ^ (n >> 16)).wrapping_mul(0x7feb352d);
    n = (n ^ (n >> 15)).wrapping_mul(0x846ca68b);
    n ^ (n >> 16)
}
