//! Integer mixing for seeded, position-independent randomness.

pub fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// `hash_u32` mapped to `[0, 1]`.
pub fn hash01(x: u32) -> f32 {
    hash_u32(x) as f32 / u32::MAX as f32
}
