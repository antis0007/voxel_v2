//! Physically coherent satellites from a few knobs, and an audit of authored ones.
//!
//! Home is a scaled world: real surface gravity on a 20 km radius makes it
//! about 1.7e6 kg/m³. A satellite at a rocky 3e3 kg/m³ would sit far inside
//! its fluid Roche limit (about 400 km), so moons default to the parent's
//! density, which keeps the scale model self-similar.
use crate::kepler::G;
use crate::system::{BodyId, System};
use crate::transfer::{hill_radius, soi_radius};
use std::f64::consts::PI;

/// Prograde orbits stay bound below this share of the Hill radius.
pub const STABLE_HILL_FRACTION: f64 = 0.5;
/// Fluid-body Roche coefficient, `d = k R_p (rho_p / rho_m)^(1/3)`.
const ROCHE_FLUID: f64 = 2.44;
/// Smallest useful sphere of influence, in body radii, for a low capture orbit.
pub const MIN_SOI_RADII: f64 = 2.5;

/// Inputs for a moon; every field has a default.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoonDesign {
    pub radius_m: f64,
    /// `None` takes the parent's mean density.
    pub density_kg_m3: Option<f64>,
    /// Orbit radius as a share of the parent's Hill radius.
    pub hill_fraction: f64,
}

impl Default for MoonDesign {
    fn default() -> MoonDesign {
        MoonDesign {
            radius_m: 4_000.0,
            density_kg_m3: None,
            hill_fraction: 0.45,
        }
    }
}

/// Derived values ready for a `BodyDef` and its `OrbitDef`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moon {
    pub mass_kg: f64,
    pub a_m: f64,
    pub period_s: f64,
    pub surface_gravity: f64,
    pub soi_m: f64,
}

/// Mean density of a sphere, kg/m³.
pub fn density(mass: f64, radius: f64) -> f64 {
    mass / (4.0 / 3.0 * PI * radius.powi(3))
}

/// Fluid Roche limit of a satellite of density `rho_m` about a parent.
pub fn roche_limit(parent_radius: f64, parent_density: f64, rho_m: f64) -> f64 {
    ROCHE_FLUID * parent_radius * (parent_density / rho_m).cbrt()
}

/// Ways a satellite can be physically incoherent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Issue {
    /// Orbit periapsis inside the fluid Roche limit, metres.
    InsideRoche { periapsis: f64, limit: f64 },
    /// Apoapsis beyond the stable share of the parent's Hill radius.
    BeyondHill { apoapsis: f64, limit: f64 },
    /// Sphere of influence too small for a low capture orbit, in body radii.
    TinySoi { radii: f64 },
    /// The parent does not orbit anything, so it has no Hill sphere.
    RootParent,
}

/// Parent of `parent`'s own orbit: mass and Hill radius, if it orbits.
fn parent_hill(system: &System, parent: BodyId) -> Option<f64> {
    let el = system.elements(parent)?;
    let grand = system.parent(parent)?;
    Some(hill_radius(el.a, el.e, system.mass(parent), system.mass(grand)))
}

/// Designs a moon of `parent`; the result passes `audit` by construction
/// unless the inputs themselves are incoherent, which `Err` reports.
pub fn design_moon(system: &System, parent: BodyId, d: &MoonDesign) -> Result<Moon, Issue> {
    let hill = parent_hill(system, parent).ok_or(Issue::RootParent)?;
    let (pm, pr) = (system.mass(parent), system.radius(parent));
    let rho = d.density_kg_m3.unwrap_or_else(|| density(pm, pr));
    let mass_kg = rho * 4.0 / 3.0 * PI * d.radius_m.powi(3);
    let a_m = d.hill_fraction * hill;
    let mu = G * (pm + mass_kg);
    let moon = Moon {
        mass_kg,
        a_m,
        period_s: 2.0 * PI * (a_m.powi(3) / mu).sqrt(),
        surface_gravity: G * mass_kg / (d.radius_m * d.radius_m),
        soi_m: soi_radius(a_m, mass_kg, pm),
    };
    let issue = check(a_m, 0.0, d.radius_m, rho, moon.soi_m, pr, density(pm, pr), hill);
    issue.map_or(Ok(moon), Err)
}

#[allow(clippy::too_many_arguments)]
fn check(a: f64, e: f64, r: f64, rho: f64, soi: f64, pr: f64, prho: f64, hill: f64) -> Option<Issue> {
    let limit = roche_limit(pr, prho, rho);
    if a * (1.0 - e) < limit {
        return Some(Issue::InsideRoche { periapsis: a * (1.0 - e), limit });
    }
    let stable = STABLE_HILL_FRACTION * hill;
    if a * (1.0 + e) > stable {
        return Some(Issue::BeyondHill { apoapsis: a * (1.0 + e), limit: stable });
    }
    (soi < MIN_SOI_RADII * r).then_some(Issue::TinySoi { radii: soi / r })
}

/// Every satellite of an orbiting body that breaks a coherence rule.
pub fn audit(system: &System) -> Vec<(BodyId, Issue)> {
    let mut found = Vec::new();
    for i in 0..system.len() {
        let b = BodyId(i as u16);
        let (Some(el), Some(parent)) = (system.elements(b), system.parent(b)) else { continue };
        let Some(hill) = parent_hill(system, parent) else { continue };
        let (m, r) = (system.mass(b), system.radius(b));
        if r <= 0.0 {
            continue;
        }
        let (pm, pr) = (system.mass(parent), system.radius(parent));
        let soi = soi_radius(el.a, m, pm);
        if let Some(issue) = check(el.a, el.e, r, density(m, r), soi, pr, density(pm, pr), hill) {
            found.push((b, issue));
        }
    }
    found
}

#[cfg(test)]
mod tests;
