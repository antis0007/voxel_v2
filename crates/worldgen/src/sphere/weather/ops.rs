//! Differential operators on the cube-sphere cell graph.
//!
//! Gradient and divergence use a per-cell weighted least-squares fit over the
//! four neighbours in the tangent plane, so they stay consistent across face
//! edges and at the eight three-valent corners without a lat-lon pole.

use glam::DVec3;

use super::super::SphereGrid;

#[derive(Clone)]
pub struct Operators {
    pub nb: Vec<[u32; 4]>,
    pub centre: Vec<DVec3>,
    /// Tangent vector per neighbour: `grad s = sum coef[k] * (s[nb k] - s[i])`.
    coef: Vec<[DVec3; 4]>,
}

fn tangent_basis(n: DVec3) -> (DVec3, DVec3) {
    let helper = if n.x.abs() < 0.6 { DVec3::X } else { DVec3::Y };
    let e1 = n.cross(helper).normalize();
    (e1, n.cross(e1))
}

impl Operators {
    pub fn new(grid: &SphereGrid) -> Self {
        let len = grid.len();
        let mut nb = Vec::with_capacity(len);
        let mut centre = Vec::with_capacity(len);
        let mut coef = Vec::with_capacity(len);
        for i in 0..len {
            let n = grid.centre(i);
            let (e1, e2) = tangent_basis(n);
            let links = grid.neighbours(i);
            let mut d = [(0.0f64, 0.0f64, 0.0f64); 4];
            let (mut axx, mut axy, mut ayy) = (0.0, 0.0, 0.0);
            for (slot, &j) in d.iter_mut().zip(&links) {
                let delta = grid.centre(j as usize) - n;
                let flat = delta - n * n.dot(delta);
                let (x, y) = (flat.dot(e1), flat.dot(e2));
                let l2 = x * x + y * y;
                if j as usize == i || l2 < 1e-18 {
                    continue;
                }
                let w = 1.0 / l2;
                *slot = (x, y, w);
                axx += w * x * x;
                axy += w * x * y;
                ayy += w * y * y;
            }
            let det = (axx * ayy - axy * axy).max(1e-30);
            let (ixx, ixy, iyy) = (ayy / det, -axy / det, axx / det);
            let mut c = [DVec3::ZERO; 4];
            for (out, &(x, y, w)) in c.iter_mut().zip(&d) {
                let (cx, cy) = (w * (ixx * x + ixy * y), w * (ixy * x + iyy * y));
                *out = e1 * cx + e2 * cy;
            }
            nb.push(links);
            centre.push(n);
            coef.push(c);
        }
        Self { nb, centre, coef }
    }

    pub fn len(&self) -> usize {
        self.centre.len()
    }

    pub fn gradient(&self, s: &[f64], out: &mut [DVec3]) {
        for (i, g) in out.iter_mut().enumerate() {
            let mut sum = DVec3::ZERO;
            for (c, &j) in self.coef[i].iter().zip(&self.nb[i]) {
                sum += *c * (s[j as usize] - s[i]);
            }
            *g = sum;
        }
    }

    /// Surface divergence of a tangent vector field.
    pub fn divergence(&self, v: &[DVec3], out: &mut [f64]) {
        for (i, d) in out.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (c, &j) in self.coef[i].iter().zip(&self.nb[i]) {
                sum += c.dot(v[j as usize] - v[i]);
            }
            *d = sum;
        }
    }

    /// Relaxes each cell toward its neighbour mean by `k`; damps the 2-dx noise of the collocated grid.
    pub fn smooth(&self, s: &mut [f64], scratch: &mut [f64], k: f64) {
        for (i, o) in scratch.iter_mut().enumerate() {
            let mean = self.nb[i].iter().map(|&j| s[j as usize]).sum::<f64>() * 0.25;
            *o = s[i] + k * (mean - s[i]);
        }
        s.copy_from_slice(scratch);
    }

    pub fn smooth_vec(&self, v: &mut [DVec3], scratch: &mut [DVec3], k: f64) {
        for (i, o) in scratch.iter_mut().enumerate() {
            let mean = self.nb[i].iter().map(|&j| v[j as usize]).sum::<DVec3>() * 0.25;
            *o = v[i] + (mean - v[i]) * k;
        }
        v.copy_from_slice(scratch);
    }

    /// Nearest cell to `p`, found by walking neighbours from `from`.
    pub fn locate(&self, from: usize, p: DVec3) -> usize {
        let mut at = from;
        let mut best = self.centre[at].dot(p);
        for _ in 0..256 {
            let mut next = at;
            for &j in &self.nb[at] {
                let d = self.centre[j as usize].dot(p);
                if d > best {
                    best = d;
                    next = j as usize;
                }
            }
            if next == at {
                break;
            }
            at = next;
        }
        if best < 0.995 {
            // A walk can stall on a face seam; a scan is rare and still exact.
            at = (0..self.centre.len()).max_by(|&a, &b| self.centre[a].dot(p).total_cmp(&self.centre[b].dot(p))).unwrap_or(at);
        }
        at
    }

    pub fn coef_at(&self, i: usize, k: usize) -> DVec3 {
        self.coef[i][k]
    }

    /// Gradient of `s` at cell `i`.
    pub fn gradient_at(&self, s: &[f64], i: usize) -> DVec3 {
        let mut sum = DVec3::ZERO;
        for (c, &j) in self.coef[i].iter().zip(&self.nb[i]) {
            sum += *c * (s[j as usize] - s[i]);
        }
        sum
    }
}
