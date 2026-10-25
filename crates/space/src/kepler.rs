//! Kepler's equation and orbital elements to state vectors.
use glam::{DMat3, DVec3};
use std::f64::consts::{PI, TAU};

/// Newtonian gravitational constant in m³ kg⁻¹ s⁻².
pub const G: f64 = 6.674_30e-11;

/// Highest supported eccentricity, matching the bound in the data validator.
const MAX_ECCENTRICITY: f64 = 0.999;
const NEWTON_TOLERANCE: f64 = 1e-14;
const NEWTON_ITERATIONS: usize = 16;
/// Halving a bracket of width at most 2 reaches the f64 spacing well before this.
const BISECTION_STEPS: usize = 80;

/// Solves `E - e·sin E = M` for the eccentric anomaly `E`.
///
/// `mean_anomaly` may be any real. A non-finite input gives NaN.
pub fn solve_kepler(mean_anomaly: f64, e: f64) -> f64 {
    debug_assert!(
        (0.0..=MAX_ECCENTRICITY).contains(&e),
        "eccentricity {e} outside 0 to {MAX_ECCENTRICITY}"
    );
    let mut m = mean_anomaly.rem_euclid(TAU);
    if !m.is_finite() {
        return f64::NAN;
    }
    // A tiny negative input rounds up to exactly TAU.
    if m >= TAU {
        m = 0.0;
    }
    // Solve on [0, π] and mirror, since E(2π - M) = 2π - E(M). Near 2π the f64
    // spacing of M is 8.9e-16; divided by 1 - e·cos E (about 1e-3 at
    // e = 0.999) that exceeds the 1e-14 tolerance, so Newton can stall there
    // (8 of 6000 probes did). Near 0 the rounding is proportionally smaller.
    if m > PI {
        TAU - solve_lower_half(TAU - m, e)
    } else {
        solve_lower_half(m, e)
    }
}

/// Eccentric anomaly for `m` in `[0, π]`, Newton first and bisection if it stalls.
fn solve_lower_half(m: f64, e: f64) -> f64 {
    // The root lies in [0, π]; the final Newton step can land a rounding
    // error outside it, notably a tiny negative value at m = 0.
    newton(m, e).unwrap_or_else(|| bisect(m, e)).clamp(0.0, PI)
}

/// Newton iteration for `m` in `[0, π]`; `None` if it misses the cap.
///
/// Above e = 0.8 it starts at π, where `f` is non-negative and convex on
/// `[0, π]`, so the iterates fall monotonically onto the root.
fn newton(m: f64, e: f64) -> Option<f64> {
    let mut ea = if e > 0.8 { PI } else { m + e * m.sin() };
    for _ in 0..NEWTON_ITERATIONS {
        let step = (ea - e * ea.sin() - m) / (1.0 - e * ea.cos());
        ea -= step;
        if step.abs() < NEWTON_TOLERANCE {
            return ea.is_finite().then_some(ea);
        }
    }
    None
}

/// Bisection on `[m - e, m + e]`, which brackets the root because `E - M = e·sin E`.
fn bisect(m: f64, e: f64) -> f64 {
    let (mut lo, mut hi) = (m - e, m + e);
    for _ in 0..BISECTION_STEPS {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if mid - e * mid.sin() - m < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Classical orbital elements about a parent, in radians and metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elements {
    /// Semi-major axis in metres.
    pub a: f64,
    /// Eccentricity, 0 to 0.999.
    pub e: f64,
    /// Inclination of the orbit plane to the XZ plane.
    pub i: f64,
    /// Longitude of the ascending node.
    pub node: f64,
    /// Argument of periapsis, measured from the ascending node: `peri - node`.
    pub arg_peri: f64,
    /// `G·(M_parent + m)` in m³/s².
    pub mu: f64,
}

impl Elements {
    /// Builds elements whose period is `period_s`, from Kepler's third law.
    pub fn from_period(
        period_s: f64,
        mu: f64,
        e: f64,
        i: f64,
        node: f64,
        arg_peri: f64,
    ) -> Elements {
        let a = (mu * period_s * period_s / (4.0 * PI * PI)).cbrt();
        Elements {
            a,
            e,
            i,
            node,
            arg_peri,
            mu,
        }
    }

    /// Mean anomaly advance per second, `sqrt(mu / a³)`.
    pub fn mean_motion(&self) -> f64 {
        (self.mu / (self.a * self.a * self.a)).sqrt()
    }

    /// Rotation from the flat in-plane frame to the system frame.
    ///
    /// The flat frame lies in the XZ plane with angles counted from +X
    /// toward -Z, the sense of a positive turn about +Y. Applied right to
    /// left: turn by `arg_peri` about +Y, tilt by `i` about +X (the line of
    /// nodes, so the body rises through the plane at argument 0), then carry
    /// the line of nodes to `node` with a turn about +Y. At `i = 0` the
    /// angular momentum stays on +Y; in general it points along
    /// `(sin i·sin node, cos i, sin i·cos node)`.
    fn plane_rotation(&self) -> DMat3 {
        DMat3::from_rotation_y(self.node)
            * DMat3::from_rotation_x(self.i)
            * DMat3::from_rotation_y(self.arg_peri)
    }

    /// Position (m) and velocity (m/s) relative to the parent, system frame.
    pub fn state_at(&self, mean_anomaly: f64) -> (DVec3, DVec3) {
        let ea = solve_kepler(mean_anomaly, self.e);
        let (sin_e, cos_e) = ea.sin_cos();
        let minor = (1.0 - self.e * self.e).sqrt();
        let rate = self.a * self.mean_motion() / (1.0 - self.e * cos_e);
        // In-plane (x toward periapsis, y a quarter turn ahead) is laid out as
        // (x, 0, -y) so that motion turns the same way as a positive turn about +Y.
        let position = DVec3::new(self.a * (cos_e - self.e), 0.0, -self.a * minor * sin_e);
        let velocity = DVec3::new(-rate * sin_e, 0.0, -rate * minor * cos_e);
        let rotation = self.plane_rotation();
        (rotation * position, rotation * velocity)
    }
}

#[cfg(test)]
mod tests;
