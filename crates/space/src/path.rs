//! Map polylines of parent-relative orbits.
use crate::system::{BodyId, System};
use glam::DVec3;
use std::f64::consts::TAU;

/// Most points one path holds, closing point included.
const MAX_POINTS: usize = 8192;
/// Fewest segments, so a coarse tolerance still draws a polygon.
const MIN_SEGMENTS: usize = 3;

/// Writes a closed polyline of `b`'s parent-relative orbit into `out`.
///
/// Samples are uniform in eccentric anomaly, starting at periapsis, and the
/// last point repeats the first, so `out` draws as a line strip. Add the
/// parent's position for system coordinates. Uses the fewest segments whose
/// largest gap to the true ellipse is at most `max_chord_error_m`, capped at
/// `MAX_POINTS` points; a non-positive or NaN tolerance takes the cap. The
/// root has no orbit and leaves `out` empty.
pub fn orbit_path(system: &System, b: BodyId, max_chord_error_m: f64, out: &mut Vec<DVec3>) {
    out.clear();
    let Some(elements) = system.elements(b) else {
        return;
    };
    let segments = segment_count(elements.a, max_chord_error_m);
    out.reserve(segments + 1);
    let step = TAU / segments as f64;
    out.extend((0..segments).map(|k| elements.position_at_eccentric(k as f64 * step)));
    out.push(out[0]);
}

/// Fewest uniform eccentric-anomaly segments within `error` of an orbit of semi-major axis `a`.
///
/// In centred coordinates the ellipse is `(a·cos E, b·sin E)`, an affine image
/// of a circle, so a segment spanning `h` about centre `Ec` has its farthest
/// arc point at `Ec`, where the tangent parallels the chord. That gap is
/// `(1 - cos(h/2))·a·b / sqrt(a²·sin²Ec + b²·cos²Ec)`, largest at periapsis
/// and apoapsis where it is `a·(1 - cos(h/2)) = 2a·sin²(h/4)`. Bounding that by
/// `error` gives `h <= 4·asin(sqrt(error / 2a))`, which stays accurate for a
/// tiny ratio where `acos(1 - error/a)` would cancel.
fn segment_count(a: f64, error: f64) -> usize {
    let max_segments = MAX_POINTS - 1;
    // Checked before `min`, which would turn a NaN ratio into 1.
    let ratio = error / (2.0 * a);
    if ratio.is_nan() || ratio <= 0.0 {
        return max_segments;
    }
    let step = 4.0 * ratio.min(1.0).sqrt().asin();
    // `as` saturates, so a tiny step still lands on the cap.
    ((TAU / step).ceil() as usize).clamp(MIN_SEGMENTS, max_segments)
}

#[cfg(test)]
mod tests;
