//! Face coordinates to unit sphere directions and back.
//!
//! Face coordinates `(s, t)` lie in `[-1, 1]`, `s` along the face `u` axis and
//! `t` along `v`. Each map is a function of the cube point `n + s'u + t'v`, so
//! points on a shared edge land on the same direction from either face.
use super::{Face, FaceGrid};
use glam::{DMat3, DVec3, IVec3};
use std::f64::consts::FRAC_PI_4;

/// Maps between face coordinates and unit sphere directions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Projection {
    /// Normalizes the cube point; inverse is a ratio.
    Gnomonic,
    /// Pre-warps each coordinate by `tan(pi s / 4)`: equal angles along the axes.
    Tangent,
    /// Analytic cube-to-sphere map per axis; inverse by Gauss-Newton.
    Spherified,
}

/// Newton stops once a step is shorter than this.
const NEWTON_TOLERANCE: f64 = 1e-15;
const NEWTON_MAX_ITERATIONS: usize = 32;

impl Projection {
    /// Returns the unit direction of face coordinates `(s, t)`.
    pub fn to_sphere(self, face: Face, s: f64, t: f64) -> DVec3 {
        let (u, n, v) = axes(face);
        match self {
            Projection::Gnomonic => (n + s * u + t * v).normalize(),
            Projection::Tangent => (n + tan_warp(s) * u + tan_warp(t) * v).normalize(),
            // Analytically unit length already; normalizing absorbs rounding.
            Projection::Spherified => spherify(n + s * u + t * v).normalize(),
        }
    }

    /// Returns the face and clamped face coordinates of direction `dir`.
    ///
    /// A zero or non-finite `dir` is a caller error and returns `(face, 0, 0)`.
    pub fn from_sphere(self, dir: DVec3) -> (Face, f64, f64) {
        let face = pick_face(dir);
        let (u, n, v) = axes(face);
        let dn = dir.dot(n);
        if !(dn > 0.0 && dn.is_finite()) {
            return (face, 0.0, 0.0);
        }
        // `n` is the largest axis of `dir`, so both ratios lie in [-1, 1].
        let (a, b) = (dir.dot(u) / dn, dir.dot(v) / dn);
        let (s, t) = match self {
            Projection::Gnomonic => (a, b),
            Projection::Tangent => (a.atan() / FRAC_PI_4, b.atan() / FRAC_PI_4),
            // Scaling by `dn` first keeps the length finite for tiny and huge `dir`.
            Projection::Spherified => spherified_inverse(face, (dir / dn).normalize(), a, b),
        };
        (face, s.clamp(-1.0, 1.0), t.clamp(-1.0, 1.0))
    }

    /// Returns `(d dir / ds, d dir / dt)`, computed analytically.
    pub fn tangents(self, face: Face, s: f64, t: f64) -> (DVec3, DVec3) {
        let (u, n, v) = axes(face);
        match self {
            Projection::Gnomonic => {
                let c = n + s * u + t * v;
                (unit_derivative(c, u), unit_derivative(c, v))
            }
            Projection::Tangent => {
                let (sw, tw) = (tan_warp(s), tan_warp(t));
                let c = n + sw * u + tw * v;
                // d/ds tan(pi s / 4) = (pi / 4) (1 + tan^2).
                let (ks, kt) = (FRAC_PI_4 * (1.0 + sw * sw), FRAC_PI_4 * (1.0 + tw * tw));
                (unit_derivative(c, u) * ks, unit_derivative(c, v) * kt)
            }
            Projection::Spherified => {
                let j = spherify_jacobian(n + s * u + t * v);
                (j * u, j * v)
            }
        }
    }
}

/// Picks the face of the largest `|component|`; ties go x, then y, then z.
pub fn pick_face(dir: DVec3) -> Face {
    let a = dir.abs();
    if a.x >= a.y && a.x >= a.z {
        if dir.x >= 0.0 { Face::PosX } else { Face::NegX }
    } else if a.y >= a.z {
        if dir.y >= 0.0 { Face::PosY } else { Face::NegY }
    } else if dir.z >= 0.0 {
        Face::PosZ
    } else {
        Face::NegZ
    }
}

/// Returns the surface frame with columns `(east, up, north)`.
///
/// `up` equals `to_sphere` bit for bit and `east` is the unit vector along
/// `d dir / ds`. `north` completes a right-handed frame (determinant +1, like
/// the face basis `(u, n, v)`) on the `+t` side of `east`. It is not along
/// `d dir / dt`: the two differ by up to about 30 degrees near cube corners,
/// where the tangents are 60 or 120 degrees apart.
pub fn local_frame(p: Projection, face: Face, s: f64, t: f64) -> DMat3 {
    let up = p.to_sphere(face, s, t);
    let (ds, _) = p.tangents(face, s, t);
    // `ds` is already perpendicular to `up`; the projection absorbs rounding.
    let east = (ds - up * up.dot(ds)).normalize();
    // det(east, up, east x up) = |east|^2 = 1, the same handedness as u x n == v.
    let north = east.cross(up);
    DMat3::from_cols(east, up, north)
}

impl FaceGrid {
    /// Returns the face coordinates of the centre of `cell`.
    pub fn cell_face_coords(self, cell: IVec3) -> (f64, f64) {
        let h = f64::from(self.half());
        ((f64::from(cell.x) + 0.5) / h, (f64::from(cell.z) + 0.5) / h)
    }
}

fn axes(face: Face) -> (DVec3, DVec3, DVec3) {
    let b = face.basis();
    (b.u.as_dvec3(), b.n.as_dvec3(), b.v.as_dvec3())
}

fn tan_warp(x: f64) -> f64 {
    (FRAC_PI_4 * x).tan()
}

/// Derivative of `c / |c|` as `c` moves along `w`.
fn unit_derivative(c: DVec3, w: DVec3) -> DVec3 {
    let r = c.length();
    let d = c / r;
    (w - d * d.dot(w)) / r
}

/// Square-root factors `sqrt(1 - c_j^2/2 - c_k^2/2 + c_j^2 c_k^2/3)` per axis.
///
/// On the cube surface each factor is at least `sqrt(1/3)`, so never zero.
fn spherify_roots(c: DVec3) -> DVec3 {
    let q = c * c;
    let f = |j: f64, k: f64| (1.0 - 0.5 * j - 0.5 * k + j * k / 3.0).sqrt();
    DVec3::new(f(q.y, q.z), f(q.z, q.x), f(q.x, q.y))
}

fn spherify(c: DVec3) -> DVec3 {
    c * spherify_roots(c)
}

/// Jacobian of `spherify`; column `j` is the derivative along axis `j`.
///
/// `d g_i / d c_i = r_i` and `d g_i / d c_j = c_i c_j (c_k^2/3 - 1/2) / r_i`.
fn spherify_jacobian(c: DVec3) -> DMat3 {
    let r = spherify_roots(c);
    let w = c * c / 3.0 - 0.5;
    let off = |ci: f64, cj: f64, wk: f64, ri: f64| ci * cj * wk / ri;
    DMat3::from_cols(
        DVec3::new(r.x, off(c.y, c.x, w.z, r.y), off(c.z, c.x, w.y, r.z)),
        DVec3::new(off(c.x, c.y, w.z, r.x), r.y, off(c.z, c.y, w.x, r.z)),
        DVec3::new(off(c.x, c.z, w.y, r.x), off(c.y, c.z, w.x, r.y), r.z),
    )
}

/// Solves `spherify(n + s u + t v) == dir` by Gauss-Newton from `(s, t)`.
fn spherified_inverse(face: Face, dir: DVec3, mut s: f64, mut t: f64) -> (f64, f64) {
    let (u, n, v) = axes(face);
    for _ in 0..NEWTON_MAX_ITERATIONS {
        let c = n + s * u + t * v;
        let r = dir - spherify(c);
        let j = spherify_jacobian(c);
        let (ds, dt) = (j * u, j * v);
        // Three equations, two unknowns: solve the 2x2 normal equations.
        let (a11, a12, a22) = (ds.dot(ds), ds.dot(dt), dt.dot(dt));
        let (b1, b2) = (ds.dot(r), dt.dot(r));
        let det = a11 * a22 - a12 * a12;
        if det.is_nan() || det <= 0.0 {
            break;
        }
        let (step_s, step_t) = ((a22 * b1 - a12 * b2) / det, (a11 * b2 - a12 * b1) / det);
        s += step_s;
        t += step_t;
        if step_s.hypot(step_t) < NEWTON_TOLERANCE {
            break;
        }
    }
    (s, t)
}

#[cfg(test)]
#[path = "projection_tests.rs"]
mod tests;
