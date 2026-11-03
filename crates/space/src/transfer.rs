//! Patched-conic transfers between bodies on the rails.
//!
//! Each leg is a two-body conic about one attractor, joined at sphere of
//! influence boundaries where only the hyperbolic excess velocity is carried
//! across. Transfer orbits are Hohmann ellipses between the bodies' semi-major
//! axes; the rails' eccentricities (0.017 and 0.02 at home) shift the real
//! optimum by well under the error of the patching itself.
use crate::kepler::{Elements, G};
use crate::system::{BodyId, System};
use glam::DVec3;
use std::f64::consts::{PI, TAU};

/// Laplace sphere of influence radius, `a (m / M)^(2/5)`.
pub fn soi_radius(a: f64, m: f64, parent_mass: f64) -> f64 {
    a * (m / parent_mass).powf(0.4)
}

/// Hill sphere radius at periapsis, `a (1 - e) (m / 3M)^(1/3)`.
pub fn hill_radius(a: f64, e: f64, m: f64, parent_mass: f64) -> f64 {
    a * (1.0 - e) * (m / (3.0 * parent_mass)).cbrt()
}

/// Circular orbit speed at radius `r`, m/s.
pub fn circular_speed(mu: f64, r: f64) -> f64 {
    (mu / r).sqrt()
}

/// Escape speed at radius `r`, m/s.
pub fn escape_speed(mu: f64, r: f64) -> f64 {
    (2.0 * mu / r).sqrt()
}

/// Burn from a circular orbit at `r` onto a hyperbola with excess `v_inf`.
///
/// Also the capture burn into a circular orbit at `r`, by time symmetry.
pub fn hyperbolic_burn(mu: f64, r: f64, v_inf: f64) -> f64 {
    (v_inf * v_inf + 2.0 * mu / r).sqrt() - circular_speed(mu, r)
}

/// Period with which two orbits return to the same relative phase, s.
///
/// Infinite for equal periods.
pub fn synodic_period(t1: f64, t2: f64) -> f64 {
    1.0 / (1.0 / t1 - 1.0 / t2).abs()
}

/// Two-impulse Hohmann transfer between coplanar circular orbits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hohmann {
    /// Speed change at the inner or outer starting orbit, m/s; may be negative.
    pub depart: f64,
    /// Speed change to circularise at the destination radius, m/s.
    pub arrive: f64,
    /// Half the transfer ellipse's period, s.
    pub time_s: f64,
    pub transfer_a: f64,
}

impl Hohmann {
    /// Transfer from radius `r1` to `r2` about an attractor of parameter `mu`.
    pub fn new(mu: f64, r1: f64, r2: f64) -> Hohmann {
        let a = 0.5 * (r1 + r2);
        let vis_viva = |r: f64| (mu * (2.0 / r - 1.0 / a)).sqrt();
        Hohmann {
            depart: vis_viva(r1) - circular_speed(mu, r1),
            arrive: circular_speed(mu, r2) - vis_viva(r2),
            time_s: PI * (a * a * a / mu).sqrt(),
            transfer_a: a,
        }
    }

    pub fn total(&self) -> f64 {
        self.depart.abs() + self.arrive.abs()
    }
}

/// Angle the target must lead the departure body by at the burn, radians.
///
/// The target sweeps `n·t` during the half-ellipse, which itself sweeps π.
/// Wrapped into `[0, 2π)`.
pub fn lead_angle(target_mean_motion: f64, time_s: f64) -> f64 {
    (PI - target_mean_motion * time_s).rem_euclid(TAU)
}

/// A complete patched-conic plan from a low orbit to a low orbit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    /// Leg about the shared attractor.
    pub cruise: Hohmann,
    /// Burn leaving the circular parking orbit, m/s.
    pub depart: f64,
    /// Burn into the circular capture orbit, including any plane change, m/s.
    pub capture: f64,
    /// Angle between the two orbit planes, radians.
    pub plane_change: f64,
    /// Sphere of influence of the destination, m.
    pub target_soi: f64,
}

impl Plan {
    pub fn total(&self) -> f64 {
        self.depart + self.capture
    }
}

/// Plane normal of an orbit, from its elements.
fn normal(el: &Elements) -> DVec3 {
    let (si, ci) = el.i.sin_cos();
    let (sn, cn) = el.node.sin_cos();
    DVec3::new(si * sn, ci, si * cn)
}

/// Polar angle about +Y, counted from +X toward -Z like the rails.
fn longitude(v: DVec3) -> f64 {
    (-v.z).atan2(v.x)
}

impl System {
    /// Patched-conic plan between circular orbits `park_alt` and `capture_alt`
    /// metres above `from` and `to`.
    ///
    /// The two bodies must either share a parent or have `to` orbit `from`;
    /// `None` otherwise.
    pub fn transfer_plan(
        &self,
        from: BodyId,
        to: BodyId,
        park_alt: f64,
        capture_alt: f64,
    ) -> Option<Plan> {
        let to_el = self.elements(to)?;
        let mu_to = G * self.mass(to);
        let r_cap = self.radius(to) + capture_alt;
        let r_park = self.radius(from) + park_alt;
        let target_soi = soi_radius(to_el.a, self.mass(to), self.mass(self.parent(to)?));
        if self.parent(to) == Some(from) {
            // Out of the parking orbit itself, so the cruise leg starts there.
            let cruise = Hohmann::new(G * self.mass(from), r_park, to_el.a);
            // Arriving at apoapsis, the target overtakes the craft.
            let v_inf = cruise.arrive.abs();
            let plane_change = to_el.i;
            Some(Plan {
                cruise,
                depart: cruise.depart,
                capture: capture_with_turn(mu_to, r_cap, v_inf, plane_change),
                plane_change,
                target_soi,
            })
        } else {
            let from_el = self.elements(from)?;
            if self.parent(from) != self.parent(to) {
                return None;
            }
            let cruise = Hohmann::new(from_el.mu, from_el.a, to_el.a);
            let plane_change = normal(from_el).angle_between(normal(to_el));
            let mu_from = G * self.mass(from);
            Some(Plan {
                cruise,
                depart: hyperbolic_burn(mu_from, r_park, cruise.depart.abs()),
                capture: capture_with_turn(mu_to, r_cap, cruise.arrive.abs(), plane_change),
                plane_change,
                target_soi,
            })
        }
    }

    /// First tick at or after `tick` when `to` leads `from` by the Hohmann
    /// lead angle, about their shared parent.
    ///
    /// Scans in steps of `step` ticks, then bisects to the exact tick. Gives
    /// `None` if the bodies do not share a parent or no window opens within
    /// `horizon` ticks.
    pub fn next_window(
        &self,
        from: BodyId,
        to: BodyId,
        tick: u64,
        step: u64,
        horizon: u64,
    ) -> Option<u64> {
        let parent = self.parent(from)?;
        if self.parent(to) != Some(parent) {
            return None;
        }
        let (from_el, to_el) = (self.elements(from)?, self.elements(to)?);
        let cruise = Hohmann::new(from_el.mu, from_el.a, to_el.a);
        let lead = lead_angle(to_el.mean_motion(), cruise.time_s);
        // Signed error in (-π, π]; a window is a rising zero crossing, since
        // the inner body's phase gains on the outer one's.
        let error = |t: u64| {
            let phase = longitude(self.relative_state(to, t).0)
                - longitude(self.relative_state(from, t).0);
            let d = (phase - lead).rem_euclid(TAU);
            if d > PI { d - TAU } else { d }
        };
        let inward = from_el.a > to_el.a;
        let step = step.max(1);
        let mut lo = tick;
        let mut e_lo = error(lo);
        while lo - tick < horizon {
            let hi = lo + step;
            let e_hi = error(hi);
            // Phase falls when the departure body is inner, rises when outer.
            let crossed = if inward { e_lo <= 0.0 && e_hi > 0.0 } else { e_lo >= 0.0 && e_hi < 0.0 };
            if crossed && (e_hi - e_lo).abs() < PI {
                return Some(refine(lo, hi, |t| (error(t) > 0.0) == inward));
            }
            if e_lo == 0.0 {
                return Some(lo);
            }
            lo = hi;
            e_lo = e_hi;
        }
        None
    }
}

/// Capture burn that also turns the arrival plane by `turn` radians.
///
/// The turn is folded into the periapsis burn, so the cost is the vector
/// difference between hyperbolic periapsis speed and circular speed.
fn capture_with_turn(mu: f64, r: f64, v_inf: f64, turn: f64) -> f64 {
    let v_peri = (v_inf * v_inf + 2.0 * mu / r).sqrt();
    let v_circ = circular_speed(mu, r);
    (v_peri * v_peri + v_circ * v_circ - 2.0 * v_peri * v_circ * turn.cos()).sqrt()
}

/// Smallest tick in `(lo, hi]` where `past` holds, given it holds at `hi`.
fn refine(mut lo: u64, mut hi: u64, past: impl Fn(u64) -> bool) -> u64 {
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if past(mid) { hi = mid } else { lo = mid }
    }
    hi
}

#[cfg(test)]
mod tests;
