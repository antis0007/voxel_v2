//! Seeded Hell bodies shared by flight, collision, saves and the sky.
use engine_core::{cube_sphere::atlas, hash::hash_u32};
use engine_space::BodyId;
use glam::DVec3;

const FIRST: u16 = 0xff00;
const COUNT_SALT: u32 = 0x4d4f_4f4e;

pub(super) fn reserved(id: BodyId) -> bool { (FIRST..FIRST + 3).contains(&id.0) }

pub(super) struct Moon {
    pub centre: DVec3,
    pub radius: f64,
}

impl Moon {
    pub fn influence(&self) -> f64 { self.radius + 1_400.0 }
}

pub(super) fn moon(seed: u32, body: BodyId, tick: u64) -> Option<Moon> {
    let i = u32::from(body.0.checked_sub(FIRST)?);
    if i > hash_u32(seed ^ COUNT_SALT) % 3 { return None; }
    let h = hash_u32(seed ^ (i + 1).wrapping_mul(0x9e37_79b9));
    let tau = std::f64::consts::TAU;
    let distance = 44_000.0 + f64::from(i) * 14_000.0;
    let phase = f64::from(h & 65_535) / 65_535.0 * tau
        + tick as f64 / 120.0 * (0.0014 + f64::from(i) * 0.0007);
    let node = f64::from((h >> 16) & 255) / 255.0 * tau;
    let tilt = 0.40 + f64::from(h >> 24) / 255.0 * 0.65;
    let (s, c) = phase.sin_cos();
    let (sn, cn) = node.sin_cos();
    let z = s * tilt.cos();
    let centre = DVec3::new(c * cn - z * sn, s * tilt.sin(), c * sn + z * cn) * distance;
    let angle = 0.021 + f64::from((h >> 8) & 255) / 255.0 * 0.032;
    Some(Moon { centre, radius: distance * angle.sin() })
}

pub(super) fn nearby(seed: u32, atlas_m: DVec3, tick: u64) -> Option<BodyId> {
    let point = atlas::to_body(atlas_m);
    (FIRST..FIRST + 3).map(BodyId).find(|&body| {
        moon(seed, body, tick).is_some_and(|m| point.distance_squared(m.centre) <= m.influence().powi(2))
    })
}

/// Eye-relative centres in the same axes as the sky ray. Radius is in metres.
pub(crate) fn sky(seed: u32, eye: DVec3, tick: u64) -> [[f32; 4]; 3] {
    let origin = atlas::to_body(eye);
    let axes = atlas::frame(eye).transpose();
    std::array::from_fn(|i| moon(seed, BodyId(FIRST + i as u16), tick)
        .map_or([0.0; 4], |m| (axes * (m.centre - origin)).as_vec3().extend(m.radius as f32).to_array()))
}
