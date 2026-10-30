//! Planet classes and the base level each one drains to.

use super::grid::SphereGrid;

/// Kind of planet, which decides where water ends up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlanetClass {
    /// Oceans take the runoff; only deep closed basins stay terminal.
    Wet,
    /// Frozen oceans as on `Wet`, but shallow closed basins also stay terminal.
    Icy,
    /// No ocean; every closed depression is a terminal basin.
    Airless,
}

impl PlanetClass {
    /// Reports whether connected ground below sea level forms an ocean.
    pub fn has_ocean(self) -> bool {
        matches!(self, PlanetClass::Wet | PlanetClass::Icy)
    }

    /// Returns the basin depth, as a share of relief, that keeps a basin terminal.
    ///
    /// `None` keeps every closed depression without filling it.
    pub fn retain_share(self) -> Option<f64> {
        match self {
            PlanetClass::Wet => Some(0.06),
            PlanetClass::Icy => Some(0.015),
            PlanetClass::Airless => None,
        }
    }
}

/// Inputs of one planet solve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanetParams {
    pub class: PlanetClass,
    pub seed: u32,
    /// Ocean surface height in metres; unused when the class has no ocean.
    pub sea_level_m: f64,
    /// Height scale of the noise in metres; heights lie within `+-relief_m`.
    pub relief_m: f64,
}

/// Smallest share of planet area a region below sea level needs to be ocean.
///
/// Smaller low regions are ordinary depressions under the class retention rule.
pub const OCEAN_MIN_SHARE: f64 = 0.002;

/// Marks cells of connected below-sea regions large enough to be ocean.
pub(crate) fn ocean_mask(grid: &SphereGrid, h: &[f64], params: &PlanetParams) -> Vec<bool> {
    let len = grid.len();
    let mut ocean = vec![false; len];
    if !params.class.has_ocean() {
        return ocean;
    }
    let min_area = OCEAN_MIN_SHARE * grid.total_area_m2();
    let low = |i: usize| h[i] < params.sea_level_m;
    let mut seen = vec![false; len];
    let mut stack = Vec::new();
    let mut region = Vec::new();
    for start in 0..len {
        if seen[start] || !low(start) {
            continue;
        }
        seen[start] = true;
        stack.push(start as u32);
        region.clear();
        let mut area = 0.0;
        while let Some(i) = stack.pop() {
            let i = i as usize;
            region.push(i);
            area += grid.area_m2(i);
            for j in grid.neighbours(i) {
                let j = j as usize;
                if !seen[j] && low(j) {
                    seen[j] = true;
                    stack.push(j as u32);
                }
            }
        }
        if area >= min_area {
            for &i in &region {
                ocean[i] = true;
            }
        }
    }
    ocean
}
