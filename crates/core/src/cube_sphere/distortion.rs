//! Distortion of a face-to-sphere projection, sampled at cell centres.
use super::{Face, Projection};
use glam::DVec3;

/// Area, angle and length spread of a projection over one face.
///
/// A map with no distortion reads 1, 0 and 1.
#[derive(Clone, Copy, Debug)]
pub struct Distortion {
    /// Largest over smallest `|d dir / ds x d dir / dt|`: solid angle per unit of face area.
    pub area_ratio: f64,
    /// Largest distance of the angle between the two tangents from 90 degrees.
    pub max_angle_deviation_deg: f64,
    /// Largest over smallest `|d dir / ds|`: arc length per unit of `s`.
    pub max_edge_length_ratio: f64,
}

/// Measures `p` on `samples_per_axis` squared cell centres of one face.
///
/// The six faces are rotations of one another, so one face gives the whole
/// answer. Cell centres never reach the face corner, so a ratio is approached
/// from inside and never exceeds its analytic limit. Zero samples count as one.
pub fn measure(p: Projection, samples_per_axis: u32) -> Distortion {
    measure_face(p, Face::PosY, samples_per_axis)
}

fn measure_face(p: Projection, face: Face, samples_per_axis: u32) -> Distortion {
    let n = samples_per_axis.max(1);
    let centre = |i: u32| -1.0 + (2.0 * f64::from(i) + 1.0) / f64::from(n);
    let mut area = Range::default();
    let mut length = Range::default();
    let mut deviation: f64 = 0.0;
    for i in 0..n {
        for j in 0..n {
            let (ds, dt) = p.tangents(face, centre(i), centre(j));
            let cross = ds.cross(dt).length();
            area.add(cross);
            length.add(ds.length());
            deviation = deviation.max((angle(ds, dt) - 90f64.to_radians()).abs().to_degrees());
        }
    }
    Distortion {
        area_ratio: area.ratio(),
        max_angle_deviation_deg: deviation,
        max_edge_length_ratio: length.ratio(),
    }
}

/// Angle between two vectors; `atan2` stays accurate near 0 and pi where `acos` does not.
fn angle(a: DVec3, b: DVec3) -> f64 {
    a.cross(b).length().atan2(a.dot(b))
}

#[derive(Clone, Copy)]
struct Range {
    min: f64,
    max: f64,
}

impl Default for Range {
    fn default() -> Self {
        Range { min: f64::INFINITY, max: f64::NEG_INFINITY }
    }
}

impl Range {
    fn add(&mut self, x: f64) {
        self.min = self.min.min(x);
        self.max = self.max.max(x);
    }

    fn ratio(self) -> f64 {
        self.max / self.min
    }
}

#[cfg(test)]
#[path = "distortion_tests.rs"]
mod tests;
