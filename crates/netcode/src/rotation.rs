//! Body rotation in 32 bits: the largest quaternion component is dropped and
//! rebuilt from unit length, and the other three take 10 bits each.
//!
//! The three kept components lie in +-1/sqrt(2), so 10 bits give steps of about
//! 0.0014 and an angular error near 0.1 degree.
use glam::Quat;
use std::f32::consts::FRAC_1_SQRT_2;

const BITS: u32 = 10;
/// Even, so zero is a step and an upright body packs exactly.
const MAX: f32 = ((1 << BITS) - 2) as f32;

pub fn pack(q: Quat) -> u32 {
    let q = q.normalize();
    let c = q.to_array();
    let largest = (0..4).fold(0, |best, i| if c[i].abs() > c[best].abs() { i } else { best });
    // q and -q are the same rotation; flip so the dropped component is positive.
    let sign = if c[largest] < 0.0 { -1.0 } else { 1.0 };
    let mut bits = largest as u32;
    for v in c.iter().enumerate().filter(|(i, _)| *i != largest).map(|(_, v)| v) {
        let unit = (v * sign / FRAC_1_SQRT_2 * 0.5 + 0.5).clamp(0.0, 1.0);
        bits = (bits << BITS) | (unit * MAX).round() as u32;
    }
    bits
}

pub fn unpack(bits: u32) -> Quat {
    let largest = (bits >> (3 * BITS)) as usize;
    let mut kept = [0.0f32; 3];
    for (n, k) in kept.iter_mut().enumerate() {
        let raw = (bits >> (BITS * (2 - n as u32))) & ((1 << BITS) - 1);
        *k = (raw as f32 / MAX - 0.5) * 2.0 * FRAC_1_SQRT_2;
    }
    let rest = (1.0 - kept.iter().map(|k| k * k).sum::<f32>()).max(0.0).sqrt();
    let mut c = [0.0f32; 4];
    let mut next = kept.iter();
    for (i, slot) in c.iter_mut().enumerate() {
        *slot = if i == largest { rest } else { next.next().copied().unwrap_or(0.0) };
    }
    Quat::from_array(c).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn angle(a: Quat, b: Quat) -> f32 {
        2.0 * a.dot(b).abs().min(1.0).acos()
    }

    #[test]
    fn rotations_round_trip_within_a_fifth_of_a_degree() {
        let mut worst = 0.0f32;
        for i in 0..2_000u32 {
            let h = |s: u32| engine_core::hash::hash_u32(i.wrapping_mul(0x9e37_79b9) ^ s) as f32 / u32::MAX as f32;
            let axis = Vec3::new(h(1) - 0.5, h(2) - 0.5, h(3) - 0.5).normalize_or(Vec3::Y);
            let q = Quat::from_axis_angle(axis, h(4) * 12.0 - 6.0);
            worst = worst.max(angle(q, unpack(pack(q))));
        }
        assert!(worst < 0.2f32.to_radians(), "{} degrees", worst.to_degrees());
    }

    #[test]
    fn identity_and_its_negation_pack_the_same() {
        assert_eq!(pack(Quat::IDENTITY), pack(-Quat::IDENTITY));
        assert!(angle(unpack(pack(Quat::IDENTITY)), Quat::IDENTITY) < 1e-3);
    }
}
