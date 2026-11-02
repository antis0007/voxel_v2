//! Mirrors the uncommitted `engine_core::cube_sphere::atlas` and collapses onto it when that lands.

use engine_core::cube_sphere::{Face, Projection, local_frame};
use glam::{DMat3, DVec3};

pub const HALF_M: f64 = 16_000.0;
pub const RADIUS_M: f64 = 20_371.0;
pub const PITCH_M: f64 = 65_536.0;

/// Face and face-local position of a chart point.
pub fn split(p: DVec3) -> (Face, DVec3) {
    let index = ((p.x + PITCH_M * 0.5) / PITCH_M).floor().clamp(0.0, 5.0);
    (Face::ALL[index as usize], DVec3::new(p.x - index * PITCH_M, p.y, p.z))
}

/// Inverse of `split`: face-local position back to chart space.
pub fn join(face: Face, p: DVec3) -> DVec3 {
    DVec3::new(face as u8 as f64 * PITCH_M + p.x, p.y, p.z)
}

/// Chart point to body-centred position on the sphere.
pub fn to_body(p: DVec3) -> DVec3 {
    let (face, local) = split(p);
    Projection::Tangent.to_sphere(face, local.x / HALF_M, local.z / HALF_M) * (RADIUS_M + local.y)
}

/// Body-centred position back to chart space.
pub fn from_body(p: DVec3) -> DVec3 {
    let (face, s, t) = Projection::Tangent.from_sphere(p);
    join(face, DVec3::new(s * HALF_M, p.length() - RADIUS_M, t * HALF_M))
}

/// Local east/up/north frame at a chart point.
pub fn frame(p: DVec3) -> DMat3 {
    let (face, local) = split(p);
    local_frame(Projection::Tangent, face, local.x / HALF_M, local.z / HALF_M)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_round_trips_through_the_body_frame() {
        let p = DVec3::new(3.0 * PITCH_M + 1234.5, 250.0, -8000.25);
        assert!((from_body(to_body(p)) - p).length() < 1e-6);
        assert!((to_body(p).length() - (RADIUS_M + 250.0)).abs() < 1e-6);
    }
}
