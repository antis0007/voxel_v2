//! Wire form of a body pose: chunk plus a fixed-point offset, and 16-bit angles.
//!
//! Positions are voxel units, as `Player::pos` is. The chunk carries magnitude,
//! so precision is 1/256 voxel everywhere, including far from the origin where an
//! `f32` coordinate alone has less.
use engine_core::CHUNK_SIZE;
use glam::{IVec3, Vec3};
use std::f32::consts::{FRAC_PI_2, TAU};

/// Fixed-point steps per voxel.
pub const STEPS: f32 = 256.0;
/// Bytes in `PackedPose::encode`.
pub const ENCODED_LEN: usize = 22;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackedPose {
    pub chunk: IVec3,
    /// Offset from the chunk's minimum corner in `1/STEPS` voxel, 0..8192.
    pub offset: [i16; 3],
    pub yaw: u16,
    pub pitch: u16,
}

impl PackedPose {
    /// Packs a position in voxels, yaw in radians (any turn) and pitch clamped to +-90 degrees.
    pub fn pack(position: Vec3, yaw: f32, pitch: f32) -> Self {
        // Round the absolute position first so a value just below a chunk edge
        // carries into the next chunk instead of clamping.
        let span = i64::from(CHUNK_SIZE) * STEPS as i64;
        let fixed = position.to_array().map(|v| (f64::from(v) * f64::from(STEPS)).round() as i64);
        let chunk = fixed.map(|f| f.div_euclid(span) as i32);
        Self {
            chunk: IVec3::from_array(chunk),
            offset: fixed.map(|f| f.rem_euclid(span) as i16),
            yaw: ((yaw.rem_euclid(TAU) / TAU * 65_536.0).round() as u32 & 0xffff) as u16,
            pitch: (((pitch.clamp(-FRAC_PI_2, FRAC_PI_2) / FRAC_PI_2 + 1.0) * 32_767.5).round()) as u16,
        }
    }

    pub fn position(&self) -> Vec3 {
        let o = Vec3::new(f32::from(self.offset[0]), f32::from(self.offset[1]), f32::from(self.offset[2]));
        (self.chunk * CHUNK_SIZE).as_vec3() + o / STEPS
    }

    pub fn yaw(&self) -> f32 {
        f32::from(self.yaw) / 65_536.0 * TAU
    }

    pub fn pitch(&self) -> f32 {
        (f32::from(self.pitch) / 32_767.5 - 1.0) * FRAC_PI_2
    }

    pub fn encode(&self, out: &mut Vec<u8>) {
        for c in self.chunk.to_array() {
            out.extend(c.to_le_bytes());
        }
        for o in self.offset {
            out.extend(o.to_le_bytes());
        }
        out.extend(self.yaw.to_le_bytes());
        out.extend(self.pitch.to_le_bytes());
    }

    /// Reads one pose; `None` on short input or an out-of-range offset.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let bytes = bytes.get(..ENCODED_LEN)?;
        let i32_at = |i: usize| i32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
        let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        let offset = [u16_at(12) as i16, u16_at(14) as i16, u16_at(16) as i16];
        let limit = (CHUNK_SIZE as f32 * STEPS) as i16;
        if offset.iter().any(|o| !(0..limit).contains(o)) {
            return None;
        }
        Some(Self { chunk: IVec3::new(i32_at(0), i32_at(4), i32_at(8)), offset, yaw: u16_at(18), pitch: u16_at(20) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(p: Vec3) -> Vec3 {
        let mut bytes = Vec::new();
        PackedPose::pack(p, 0.0, 0.0).encode(&mut bytes);
        assert_eq!(bytes.len(), ENCODED_LEN);
        PackedPose::decode(&bytes).unwrap().position()
    }

    #[test]
    fn positions_round_trip_within_half_a_step_across_chunk_edges() {
        for p in [
            Vec3::ZERO,
            Vec3::new(31.999, -0.001, 32.0),
            Vec3::new(-0.3, -32.0, -31.9),
            Vec3::new(-1_000.25, 64.5, 2_000.75),
            Vec3::new(-65_537.5, -300.125, 65_535.875),
        ] {
            let back = round_trip(p);
            assert!((back - p).abs().max_element() <= 0.5 / STEPS + 1e-6, "{p} -> {back}");
        }
    }

    #[test]
    fn large_coordinates_keep_the_chunk_and_floor_division() {
        let p = PackedPose::pack(Vec3::new(-33.0, 1_000_000.0, -1.0), 0.0, 0.0);
        assert_eq!(p.chunk, IVec3::new(-2, 31_250, -1));
        assert_eq!(p.offset, [31 * 256, 0, 31 * 256]);
    }

    #[test]
    fn angles_wrap_yaw_and_clamp_pitch() {
        for yaw in [0.0f32, 1.0, -1.0, 3.0 * TAU + 0.5] {
            let back = PackedPose::pack(Vec3::ZERO, yaw, 0.0).yaw();
            let d = (back - yaw.rem_euclid(TAU)).rem_euclid(TAU);
            assert!(d.min(TAU - d) < 1e-4, "{yaw} -> {back}");
        }
        assert!((PackedPose::pack(Vec3::ZERO, 0.0, 4.0).pitch() - FRAC_PI_2).abs() < 1e-4);
        assert!((PackedPose::pack(Vec3::ZERO, 0.0, -0.3).pitch() + 0.3).abs() < 1e-4);
    }

    #[test]
    fn decode_rejects_short_and_out_of_range_input() {
        let mut bytes = Vec::new();
        PackedPose::pack(Vec3::ONE, 0.0, 0.0).encode(&mut bytes);
        assert!(PackedPose::decode(&bytes[..ENCODED_LEN - 1]).is_none());
        bytes[12..14].copy_from_slice(&(-1i16).to_le_bytes());
        assert!(PackedPose::decode(&bytes).is_none());
    }
}
