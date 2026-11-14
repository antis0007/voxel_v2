//! Opt-in dune relief. Historical generator 300 remains in carve.
//! Position-only body-space sampling shares the coastline/sand availability
//! mask and the footprint filter with the authoritative planetary column.
use glam::DVec3;
use std::f64::consts::TAU;
use super::carve::{stripes, wind};

pub const VERSION: u32 = 303;

/// A long windward climb, short lee descent, then a quiet interdune floor.
/// Cubic easing joins the floor and crest continuously.
/// Unlike cos(phi - k*sin(phi)), this is not even in phi.
fn profile(phase: f64) -> f64 {
    let t = phase.rem_euclid(1.0);
    let ease = |v: f64| v * v * (3.0 - 2.0 * v);
    if t < 0.70 { ease(t / 0.70) }
    else if t < 0.94 { 1.0 - ease((t - 0.70) / 0.24) }
    else { 0.0 }
}

pub(super) fn height(seed: u32, p: DVec3, footprint: f64, up: DVec3, erg: f64) -> f64 {
    let downwind = wind(seed, up);
    if downwind == DVec3::ZERO { return 0.0; }
    let mut height = 0.0;
    // Broad carriers only in sand seas; coastal belts keep human-scale
    // ridges. Ripples resolve close up rather than aliasing at distance.
    for (wavelength, crest_m, salt) in [
        (180.0, 11.0 * erg.clamp(0.0, 1.0), 303_227_u32),
        (46.0, 3.2, 303_211), (12.0, 0.50, 303_223),
        (3.2, 0.07, 303_229),
    ] {
        let resolved = 1.0 - super::smooth(wavelength * 0.125, wavelength * 0.5, footprint);
        if crest_m <= 0.0 || resolved <= 0.0 { continue; }
        let (cos, sin) = stripes(seed.wrapping_add(salt), p, downwind, wavelength);
        let coherence_sq = cos * cos + sin * sin;
        // Amplitude squared makes phase cancellation produce quiet crest
        // ends, not discontinuities or isolated needles.
        height += crest_m * resolved * coherence_sq
            * profile(sin.atan2(cos) / TAU);
    }
    height
}
/// A second opt-in candidate; generator303 remains reproducible.
pub const COHERENT_DUNES_GENERATOR: u32 = 305;

/// Dune crests persist for several wavelengths before forking. The gully
/// kernel's 1.25-wave cells made isolated hummocks rather than long ridges.
/// Reuse its positional corner hashes and partition-of-unity weights, but
/// with a wider support and much less corner-to-corner spacing jitter.
fn coherent_stripes(seed: u32, p: DVec3, downwind: DVec3, wavelength: f64) -> (f64, f64) {
    const SUPPORT: f64 = 4.0;
    let (f, around) = super::carve::corners(p, wavelength * SUPPORT);
    let (mut c, mut s) = (0.0, 0.0);
    for (at, offset, weight) in around {
        let (jitter, spacing) = super::carve::corner(seed, at[0], at[1], at[2]);
        let phase = TAU * SUPPORT * (1.0 + 0.16 * spacing)
            * (f - offset - jitter * 0.3).dot(downwind);
        let (sin, cos) = phase.sin_cos();
        c += weight * cos;
        s += weight * sin;
    }
    let scale = 1.0 / ((c * c + s * s).sqrt() + 0.10);
    (c * scale, s * scale)
}

pub(super) fn coherent_height(seed: u32, p: DVec3, footprint: f64, up: DVec3, erg: f64) -> f64 {
    let downwind = wind(seed, up);
    if downwind == DVec3::ZERO { return 0.0; }
    let erg = erg.clamp(0.0, 1.0);
    let mut height = 0.0;
    // Large dunes sit under coherent secondary ridges. The smallest band is
    // merely a surface ripple, not another landform competing in silhouette.
    for (wavelength, crest_m, salt) in [
        (210.0, 17.0 * erg, 305_227_u32),
        (58.0, 5.0 + 1.4 * erg, 305_211),
        (14.0, 0.65, 305_223),
        (3.2, 0.08, 305_229),
    ] {
        let resolved = 1.0 - super::smooth(wavelength * 0.125, wavelength * 0.5, footprint);
        if crest_m <= 0.0 || resolved <= 0.0 { continue; }
        let (cos, sin) = coherent_stripes(seed.wrapping_add(salt), p, downwind, wavelength);
        let coherence_sq = cos * cos + sin * sin;
        height += crest_m * resolved * coherence_sq * profile(sin.atan2(cos) / TAU);
    }
    height
}
/// Deposit after erosion and retain its physical thickness.
pub const DEPOSIT_GENERATOR: u32 = 306;

/// Smooth sand availability in body space, without smoothing dune crests.
pub const ENVELOPE_GENERATOR: u32 = 309;
pub(super) const ENVELOPE_CELL_M: f64 = 64.0;
#[derive(Clone, Copy, Default)]
pub(super) struct Envelope { pub sand: f64, pub erg: f64, pub calm: f64 }
type EnvelopeKey = (u64, [i64; 3]);
thread_local! { static ENVELOPES: std::cell::RefCell<Vec<Option<(EnvelopeKey, Envelope)>>> = const { std::cell::RefCell::new(Vec::new()) }; }
/// Advisory direct-mapped cache; computation is position-only and deterministic.
pub(super) fn envelope(world: u64, p: DVec3, mut sample: impl FnMut(DVec3) -> Envelope) -> Envelope {
    let (_, corners) = super::carve::corners(p, ENVELOPE_CELL_M);
    let mut result = Envelope::default();
    for (at, _, weight) in corners {
        if weight <= 0.0 { continue; }
        let key = (world, at);
        let slot = ((at[0].wrapping_mul(73856093) ^ at[1].wrapping_mul(19349663) ^ at[2].wrapping_mul(83492791)) as u64 ^ world) as usize % 2048;
        let hit = ENVELOPES.with(|v| v.borrow().get(slot).and_then(|v| *v).filter(|v| v.0 == key).map(|v| v.1));
        let value = hit.unwrap_or_else(|| {
            let dir = (DVec3::new(at[0] as f64, at[1] as f64, at[2] as f64) * ENVELOPE_CELL_M).normalize_or_zero();
            let value = sample(dir);
            ENVELOPES.with(|v| { let mut v = v.borrow_mut(); if v.is_empty() { v.resize(2048, None); } v[slot] = Some((key, value)); });
            value
        });
        result.sand += weight * value.sand;
        result.erg += weight * value.erg;
    }
    result
}
