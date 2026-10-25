use super::*;
use crate::data::{BodyKind, SystemDef};
use crate::fixtures::{body, orbit, star, system};
use crate::kepler::Elements;
use crate::system::BodyId;
use std::f64::consts::TAU;

const TICK: f64 = 1.0 / 120.0;

/// Star `s` with planet `p` on an inclined e = 0.9 orbit.
fn eccentric() -> SystemDef {
    let mut o = orbit(86_400.0);
    o.e = 0.9;
    o.i_deg = 33.0;
    o.node_deg = 120.0;
    o.peri_deg = 250.0;
    let mut s = body("s", star(), None, None);
    s.mass_kg = 2.0e30;
    system(vec![s, body("p", BodyKind::Planet, Some("s"), Some(o))])
}

/// Smallest distance from `p` to the orbit between eccentric anomalies `lo` and `hi`.
fn distance_to_arc(el: &Elements, p: DVec3, lo: f64, hi: f64) -> f64 {
    let d = |ea: f64| (el.position_at_eccentric(ea) - p).length();
    let (mut lo, mut hi) = (lo, hi);
    // Golden-section search; the distance is unimodal over one short arc.
    let r = 0.5 * (5f64.sqrt() - 1.0);
    for _ in 0..200 {
        let (x1, x2) = (hi - r * (hi - lo), lo + r * (hi - lo));
        if d(x1) < d(x2) {
            hi = x2;
        } else {
            lo = x1;
        }
    }
    d(0.5 * (lo + hi))
}

fn distance_to_segment(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    (a + ab * t - p).length()
}

#[test]
fn orbit_path_stays_within_chord_error() {
    let system = System::compile(&eccentric(), TICK).unwrap();
    let p = system.id("p").unwrap();
    let el = system.elements(p).unwrap();
    let bound = 1e-3 * el.a;
    let mut out = vec![DVec3::splat(9.0); 3];
    orbit_path(&system, p, bound, &mut out);
    assert!(out.len() > 8 && out.len() < 8192, "{} points", out.len());
    assert_eq!(out.first(), out.last(), "the polyline closes");
    let segments = out.len() - 1;
    let step = TAU / segments as f64;
    let mut worst: f64 = 0.0;
    for (k, pair) in out.windows(2).enumerate() {
        let mid = 0.5 * (pair[0] + pair[1]);
        let ea = k as f64 * step;
        worst = worst.max(distance_to_arc(el, mid, ea - step, ea + 2.0 * step));
    }
    assert!(worst <= 1.05 * bound, "worst {worst} vs bound {bound}");
    // The count is the smallest that meets the bound, so the worst segment is close to it.
    assert!(
        worst >= 0.5 * bound,
        "worst {worst} vs bound {bound}: too many segments"
    );

    // The path is in the same frame as the rails: the body stays on it.
    let period = 86_400 * 120;
    for k in 0..32u64 {
        let (pos, _) = system.relative_state(p, k * period / 32 + 11);
        let near = out
            .windows(2)
            .map(|s| distance_to_segment(pos, s[0], s[1]))
            .fold(f64::INFINITY, f64::min);
        assert!(near <= 1.05 * bound, "tick {k}: {near}");
    }
}

#[test]
fn orbit_path_caps_its_points_and_skips_the_root() {
    let system = System::compile(&eccentric(), TICK).unwrap();
    let mut out = Vec::new();
    orbit_path(&system, system.id("p").unwrap(), 1e-9, &mut out);
    assert_eq!(out.len(), 8192);
    for error in [0.0, -1.0, f64::NAN] {
        orbit_path(&system, system.id("p").unwrap(), error, &mut out);
        assert_eq!(out.len(), 8192, "error {error}");
    }
    // A huge tolerance still draws a closed polygon.
    orbit_path(&system, system.id("p").unwrap(), 1e30, &mut out);
    assert_eq!(out.len(), 4);
    orbit_path(&system, BodyId(0), 1.0, &mut out);
    assert!(out.is_empty());
}
