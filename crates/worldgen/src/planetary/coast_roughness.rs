//! Fractal shoreline planform for the spherical terrain (generator 288).
//!
//! The 224-cell solve resolves continents, shelves and drainage, but its
//! interpolated sea-level contour follows cell edges and plate fronts, so
//! coasts read as long straight segments. Real coasts are statistically
//! self-similar with a fractal dimension near 1.2-1.3 (Mandelbrot 1967).
//! Sapoval, Baldassarri and Gabrielli (2004) show wave erosion of
//! heterogeneous rock self-stabilises at about D = 4/3, while sediment-fed
//! coasts are straightened by longshore drift, and glacial troughs leave
//! the roughest coasts (fjords, skerries).
//!
//! Near the coast the solved surface is resampled at `p + w(p)`, a vector
//! fBm domain warp. Bays, headlands, drowned valleys and broken islands come
//! from the solved relief itself, displaced by the warp, rather than from
//! added height noise. The warp only acts in a band near the coast, so
//! ranges, drainage divides and the routed graph are unchanged. Octaves finer than the sample footprint are filtered, so the
//! globe, horizon and voxel columns read the same coastline at their scale.
//!
//! Generator 289 also stretches the warp along the rock grain. Where fold
//! belts, faults or rifts run near the sea, drowned ridges become chains of
//! long islands and inlets parallel to the structure (Dalmatian and ria
//! coasts), instead of an isotropic fractal.
use super::{SEA, noise, smooth};
use glam::DVec3;

/// Longest warp wavelength, metres. A cube face spans about 48 km.
const LONGEST_M: f64 = 4_000.0;
/// Shortest wavelength still worth sampling at voxel scale.
const SHORTEST_M: f64 = 12.0;
/// Lateral amplitude of the longest octave as a fraction of its wavelength.
const LEAD: f64 = 0.2;
/// Amplitude ratio between successive octaves: 2^-H with H = 2 - D, D ~ 1.3.
const PERSISTENCE: f64 = 0.616;
/// Height displacement bound; keeps cliff tops and trench walls in place.
const MAX_SHIFT_M: f64 = 90.0;

/// Rock and history at the sample point.
pub(super) struct Shore {
    pub collision: f64,
    pub rift: f64,
    pub former_ice: f64,
    /// Mobile cover thickness from the solve, metres.
    pub sediment_m: f64,
    /// Strike of the rock grain, scaled by how strongly it is expressed;
    /// zero before generator 289 and on unstructured coasts.
    pub grain: DVec3,
}

/// Longest stretch of warp features along the grain.
const GRAIN_STRETCH: f64 = 3.5;

/// Rock grain at `dir` from the structure-intensity gradient: the strike
/// runs across the gradient, with strength from how steeply it changes.
pub(super) fn grain(dir: DVec3, structure_gradient: DVec3) -> DVec3 {
    let across = structure_gradient - dir * structure_gradient.dot(dir);
    let strength = smooth(2.0e-5, 1.5e-4, across.length());
    dir.cross(across).normalize_or_zero() * strength
}

/// Roughness multiplier: resistant, faulted and glaciated coasts are
/// rough; thick sediment cover leaves smooth barrier and strand coasts.
fn roughness(s: &Shore) -> f64 {
    let rocky = 0.85 + 0.5 * s.collision.clamp(0.0, 1.0) + 0.35 * s.rift.clamp(0.0, 1.0)
        + 0.7 * s.former_ice.clamp(0.0, 1.0);
    (rocky * (1.0 - 0.4 * smooth(2.0, 24.0, s.sediment_m))).clamp(0.3, 1.8)
}

/// Total lateral reach of the warp for a roughness of one, metres.
fn reach() -> f64 {
    let mut sum = 0.0;
    let mut amp = LEAD * LONGEST_M;
    let mut wavelength = LONGEST_M;
    while wavelength >= SHORTEST_M {
        sum += amp;
        amp *= PERSISTENCE;
        wavelength *= 0.5;
    }
    sum
}

/// Weight of the coastal band around the solved shoreline for height `h`
/// and slope (m/m). Zero away from the coast, so ranges and drainage keep
/// their solved surface.
pub(super) fn band(h: f64, slope: f64, shore: &Shore) -> f64 {
    let rough = roughness(shore);
    // Estimated horizontal distance to the solved shoreline.
    let distance = (h - SEA).abs() / slope.max(1e-6);
    let band = reach() * rough;
    1.0 - smooth(0.7 * band, 1.25 * band, distance)
}

/// Tangent warp offset in metres at body point `p`. Octaves finer than the
/// footprint are filtered out.
pub(super) fn warp(seed: u32, p: DVec3, footprint: f64, shore: &Shore) -> DVec3 {
    let mut warp = DVec3::ZERO;
    let mut amp = LEAD * LONGEST_M * roughness(shore);
    let mut wavelength = LONGEST_M;
    let mut octave = 0u32;
    while wavelength >= SHORTEST_M {
        let filter = 1.0 - smooth(wavelength * 0.25, wavelength * 1.5, footprint);
        if filter <= 0.0 {
            break;
        }
        let q = stretched(p, shore.grain) / wavelength;
        let salt = 3301 + octave * 7;
        // Rotate each octave's lattice so cell edges never align across scales.
        let q = DVec3::new(q.x + 0.31 * q.z, q.y - 0.27 * q.x, q.z + 0.23 * q.y);
        let v = DVec3::new(
            noise(seed, q, salt) - 0.5,
            noise(seed, q, salt + 1) - 0.5,
            noise(seed, q, salt + 2) - 0.5,
        );
        warp += v * (2.0 * amp * filter);
        amp *= PERSISTENCE;
        wavelength *= 0.5;
        octave += 1;
    }
    let up = p.normalize_or_zero();
    warp - up * warp.dot(up)
}

/// Compresses noise coordinates along the grain so features run with it.
fn stretched(p: DVec3, grain: DVec3) -> DVec3 {
    let strength = grain.length();
    if strength <= 0.0 { return p; }
    let strike = grain / strength;
    let squeeze = 1.0 - (1.0 - 1.0 / GRAIN_STRETCH) * strength;
    p - strike * (strike.dot(p) * (1.0 - squeeze))
}

/// Bounded height change from resampling the solved surface at the warped
/// point (`moved`) instead of here (`here`).
pub(super) fn shift(moved: f64, here: f64, weight: f64) -> f64 {
    MAX_SHIFT_M * ((moved - here) / MAX_SHIFT_M).tanh() * weight
}
