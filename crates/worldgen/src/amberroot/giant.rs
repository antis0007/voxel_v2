//! One colossal Amberroot tree: a fluted, twisting trunk, buttress roots that
//! hug the ground, a few heavy limbs under broad crown masses with openings,
//! hanging strands, and amber shelf fungi. Shapes are metre-space distance
//! tests, so every voxel scale shares the silhouette.
use crate::{BlockId as B, hash2, value_noise3};
use glam::{Vec2, Vec3};
use std::f32::consts::{PI, TAU};

const ROOTS: usize = 7;

struct Crown {
    c: Vec3,
    rx: f32,
    up: f32,
    down: f32,
    /// Deep teal needle-leaf mass instead of the lighter broadleaf green.
    teal: bool,
}

pub(super) struct Giant {
    pub x: f32,
    pub z: f32,
    pub base: f32,
    pub radius: f32,
    height: f32,
    seed: u32,
    phase: f32,
    lean: Vec2,
    roots: [(f32, f32); ROOTS],
    limbs: Vec<(Vec3, Vec3, f32)>,
    crowns: Vec<Crown>,
    shelves: [(f32, f32, f32); 6],
    /// Door direction (world xz) of a hollow trunk.
    pub hollow: Option<Vec2>,
    /// Quadratic Bezier of a root that arches over the approach.
    arch: Option<[Vec3; 3]>,
    reach: f32,
    top: f32,
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

fn segment(p: Vec3, a: Vec3, b: Vec3) -> (f32, f32) {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-4)).clamp(0.0, 1.0);
    ((p - a - ab * t).length(), t)
}

fn bezier(a: &[Vec3; 3], t: f32) -> Vec3 {
    a[0] * (1.0 - t) * (1.0 - t) + a[1] * 2.0 * t * (1.0 - t) + a[2] * t * t
}

impl Giant {
    /// `teal` is the share of crown masses in deep teal (generator 43); 0 keeps
    /// the generator 35 all-broadleaf canopy.
    pub fn new(base: Vec3, radius: f32, height: f32, seed: u32, teal: f32) -> Self {
        let h = |salt: i32| hash2(salt, 3, seed);
        let phase = h(0) * TAU;
        let lean_a = h(1) * TAU;
        let mut g = Self {
            x: base.x,
            z: base.z,
            base: base.y,
            radius,
            height,
            seed,
            phase,
            lean: Vec2::new(lean_a.cos(), lean_a.sin()) * 0.05,
            roots: [(0.0, 0.0); ROOTS],
            limbs: Vec::new(),
            crowns: Vec::new(),
            shelves: [(0.0, 0.0, 0.0); 6],
            hollow: None,
            arch: None,
            reach: 0.0,
            top: 0.0,
        };
        for k in 0..ROOTS {
            let a = phase + k as f32 * TAU / ROOTS as f32 + (h(10 + k as i32) - 0.5) * 0.5;
            g.roots[k] = (a, radius * (2.4 + h(20 + k as i32) * 1.8));
        }
        for k in 0..4 {
            let y = height * (0.5 + 0.08 * k as f32 + (h(30 + k) - 0.5) * 0.06);
            let b = phase * 1.3 + k as f32 * TAU / 4.0 + (h(40 + k) - 0.5) * 0.9;
            let len = 7.0 + radius * 1.6 + h(50 + k) * 4.0;
            let rise = 4.0 + h(60 + k) * 4.0;
            let start = g.axis(y);
            let end = start + Vec3::new(b.cos() * len, rise, b.sin() * len);
            g.limbs.push((start, end, radius * 0.34));
            g.crowns.push(Crown {
                c: end + Vec3::Y * 1.5,
                rx: 7.0 + radius * 1.2 + h(70 + k) * 3.0,
                up: 4.0 + h(80 + k) * 2.0,
                down: 2.4 + h(90 + k),
                teal: h(95 + k) < teal,
            });
        }
        let crest = g.axis(height * 0.84);
        g.crowns.push(Crown { c: crest + Vec3::Y * 2.0, rx: 6.0 + radius, up: 5.0, down: 3.0, teal: h(99) < teal });
        // Bracket fungi in small tiers: three clumps, two brackets each.
        for k in 0..6 {
            let clump = (k / 2) as i32;
            let y = 2.0 + clump as f32 * 3.4 + h(100 + clump) * 1.2 + (k % 2) as f32 * 0.9;
            let a = h(110 + clump) * TAU + (k % 2) as f32 * 0.35;
            g.shelves[k] = (y, a, 0.7 + h(120 + k as i32) * 0.5);
        }
        let mut reach = radius * 2.0 + g.roots.iter().map(|r| r.1).fold(0.0, f32::max);
        let mut top = height;
        for c in &g.crowns {
            reach = reach.max((c.c.x - g.x).abs().max((c.c.z - g.z).abs()) + c.rx * 1.1);
            top = top.max(c.c.y + c.up);
        }
        g.reach = reach;
        g.top = top;
        g
    }

    /// Throws a root over the approach; widens the bounds to cover it.
    pub fn set_arch(&mut self, arch: [Vec3; 3]) {
        for p in arch {
            self.reach = self.reach.max((p.x - self.x).abs().max((p.z - self.z).abs()) + 2.0);
            self.top = self.top.max(p.y + 2.0);
        }
        self.arch = Some(arch);
    }

    /// World position of the trunk axis `y` metres above the base.
    fn axis(&self, y: f32) -> Vec3 {
        let w = self.wobble(y);
        Vec3::new(self.x + w.x, self.base + y, self.z + w.y)
    }

    fn wobble(&self, y: f32) -> Vec2 {
        let y = y.max(0.0);
        let amp = 2.4 * (y / self.height).min(1.0);
        self.lean * y + Vec2::new((y / 9.0 + self.phase).sin(), (y / 12.5 + self.phase * 1.7).cos()) * amp
    }

    fn trunk_radius(&self, y: f32, theta: f32) -> f32 {
        let t = (y / self.height).clamp(0.0, 1.0);
        let taper = self.radius * (1.0 - 0.58 * t);
        let flare = self.radius * 0.85 * (-y.max(0.0) / 2.8).exp();
        (taper + flare) * (1.0 + 0.13 * (5.0 * theta + y * 0.21 + self.phase).sin())
    }

    /// Is (x, z) under one of this tree's crown masses?
    pub fn shades(&self, x: f32, z: f32) -> bool {
        self.crowns.iter().any(|c| (c.c.x - x).hypot(c.c.z - z) < c.rx * 0.8)
    }

    /// Trunk radius at ground level, ignoring fluting.
    pub fn ground_radius(&self) -> f32 {
        self.radius * 1.85 * 1.13
    }

    /// Some(Air) marks a carved hollow; None means this tree has nothing here.
    pub fn block(&self, m: Vec3, ground: f32, vm: f32, clear: bool) -> Option<B> {
        let dx = m.x - self.x;
        let dz = m.z - self.z;
        if dx.abs() > self.reach || dz.abs() > self.reach || m.y > self.top || m.y < self.base - 4.0 {
            return None;
        }
        let y = m.y - self.base;
        if m.y >= ground - 1.0 && y < self.height * 0.86 {
            let w = self.wobble(y);
            let q = Vec2::new(dx - w.x, dz - w.y);
            let rho = q.length();
            let theta = q.y.atan2(q.x);
            let r = self.trunk_radius(y, theta);
            if rho < r {
                if let Some(door) = self.hollow {
                    let inner = r * 0.62;
                    if y < 11.0 && rho < inner {
                        return Some(B::Air);
                    }
                    // A pointed doorway, 4.4 m at the crown.
                    let along = q.dot(door);
                    let lateral = (q.x * door.y - q.y * door.x).abs();
                    let rise = m.y - ground;
                    if along > 0.0 && rise < 4.4 && lateral < 1.4 * (1.0 - (rise / 4.4).powi(3)).max(0.0).sqrt() {
                        return Some(B::Air);
                    }
                    // Amber brackets on the inner wall light the hollow from the door.
                    let facing = (q / rho.max(1e-3)).dot(-door);
                    if rho < inner + vm && (2.3..2.3 + vm).contains(&y) && (facing > 0.75 || facing.abs() < 0.2) {
                        return Some(B::AmberCap);
                    }
                }
                return Some(B::Log);
            }
            if !clear && let Some(b) = self.shelf(y, rho, theta, r, vm) {
                return Some(b);
            }
        }
        if !clear && let Some(b) = self.root(dx, dz, m.y - ground) {
            return Some(b);
        }
        if let Some(arch) = &self.arch
            && (0..=24).any(|i| (bezier(arch, i as f32 / 24.0) - m).length() < 1.5)
        {
            return Some(B::Log);
        }
        for &(a, b, r0) in &self.limbs {
            let (d, t) = segment(m, a, b);
            if d < r0 * (1.0 - 0.55 * t) {
                return Some(B::Log);
            }
        }
        self.leaves(m, vm)
    }

    fn shelf(&self, y: f32, rho: f32, theta: f32, r: f32, vm: f32) -> Option<B> {
        for &(sy, a, depth) in &self.shelves {
            let layer = ((y - sy) / vm).floor();
            if !(0.0..2.0).contains(&layer) {
                continue;
            }
            let delta = wrap(theta - a);
            let lateral = (rho * delta.sin()).abs() / (depth * 1.1);
            if delta.cos() <= 0.0 || lateral >= 1.0 {
                continue;
            }
            if rho < r + depth * (1.0 - lateral * lateral).sqrt() {
                // Amber gills under a bark-brown lip.
                return Some(if layer < 1.0 { B::AmberCap } else { B::Log });
            }
        }
        None
    }

    fn root(&self, dx: f32, dz: f32, yr: f32) -> Option<B> {
        if yr < -1.0 || yr > self.radius * 1.6 + 1.0 {
            return None;
        }
        let rho = dx.hypot(dz);
        let theta = dz.atan2(dx);
        for (k, &(a, len)) in self.roots.iter().enumerate() {
            let delta = wrap(theta - a - 0.22 * (rho / 5.0 + k as f32).sin());
            if delta.cos() <= 0.0 {
                continue;
            }
            let s = rho * delta.cos() - self.radius * 0.9;
            if s < 0.0 || s > len {
                continue;
            }
            let f = 1.0 - s / len;
            let top = self.radius * 1.5 * f.powf(1.7) + 0.7;
            if yr > top {
                continue;
            }
            let half = 0.6 + self.radius * 0.35 * f;
            let profile = (1.0 - (yr.max(0.0) / top).powi(2)).max(0.0).sqrt().max(0.35);
            if (rho * delta.sin()).abs() < half * profile {
                return Some(B::Log);
            }
        }
        None
    }

    fn leaves(&self, m: Vec3, vm: f32) -> Option<B> {
        for c in &self.crowns {
            let q = m - c.c;
            let horiz = (q.x * q.x + q.z * q.z) / (c.rx * c.rx);
            if horiz > 1.3 {
                continue;
            }
            let ry = if q.y > 0.0 { c.up } else { c.down };
            let e = horiz + (q.y / ry).powi(2);
            if e < 1.3 {
                let n = value_noise3(m.x / 3.6, m.y / 3.6, m.z / 3.6, self.seed);
                let hole = value_noise3(m.x / 8.5, m.y / 6.0, m.z / 8.5, self.seed.wrapping_add(1));
                if e < 0.72 + (n - 0.5) * 0.6 && !(e > 0.3 && hole < 0.26) {
                    return Some(if c.teal { B::PineLeaves } else { B::Leaves });
                }
            }
            if q.y < 0.0 && horiz < 0.66 {
                let cell = vm.max(0.5);
                let (cx, cz) = ((m.x / cell).floor() as i32, (m.z / cell).floor() as i32);
                if hash2(cx, cz, self.seed.wrapping_add(5)) > 0.965 {
                    let bottom = -c.down * (0.72 - horiz).sqrt();
                    let len = 1.0 + hash2(cx, cz, self.seed.wrapping_add(6)) * 4.0;
                    if q.y < bottom && q.y > bottom - len {
                        return Some(B::Leaves);
                    }
                }
            }
        }
        None
    }
}
