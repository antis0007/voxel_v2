//! Persistent face addresses in the legacy integer chunk grid.
use super::{Face, Projection, local_frame};
use glam::{DMat3, DVec3};

// Radius is HALF_M * 4 / pi, so a face edge spans a quarter circumference.
pub const HALF_M: f64 = 24_000.0;
pub const RADIUS_M: f64 = 30_558.0;
pub const PITCH_M: f64 = 65_536.0;

pub fn split(p: DVec3) -> (Face, DVec3) {
    let index = ((p.x + PITCH_M * 0.5) / PITCH_M).floor().clamp(0.0, 5.0) as usize;
    let face = Face::ALL[index];
    (face, DVec3::new(p.x - index as f64 * PITCH_M, p.y, p.z))
}

pub fn join(face: Face, p: DVec3) -> DVec3 {
    DVec3::new(face as u8 as f64 * PITCH_M + p.x, p.y, p.z)
}

pub fn to_body(p: DVec3) -> DVec3 {
    let (face, local) = split(p);
    Projection::Tangent.to_sphere(face, local.x / HALF_M, local.z / HALF_M)
        * (RADIUS_M + local.y)
}

pub fn from_body(p: DVec3) -> DVec3 {
    let (face, s, t) = Projection::Tangent.from_sphere(p);
    join(face, DVec3::new(s * HALF_M, p.length() - RADIUS_M, t * HALF_M))
}

pub fn frame(p: DVec3) -> DMat3 {
    let (face, local) = split(p);
    local_frame(Projection::Tangent, face, local.x / HALF_M, local.z / HALF_M)
}

#[cfg(test)]
#[path = "atlas_tests.rs"]
mod tests;
