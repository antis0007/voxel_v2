//! Replicated entity state in integers, and its delta form against a baseline.
//!
//! The server quantises once and keeps the integers as its baseline, so the client
//! rebuilds exactly the value the server compares against. Only fields that differ
//! from the baseline are written, behind a one-byte mask; position goes as a
//! difference, which costs one or two bytes per axis for a body that moved a little.
use crate::rotation;
use crate::wire::{Reader, WireError, put_i64, put_u64};
use glam::{Quat, Vec3};

/// Position steps per voxel.
pub const POSITION_STEPS: f32 = 256.0;
/// Velocity steps per voxel per second.
pub const VELOCITY_STEPS: f32 = 64.0;
/// Angular velocity steps per radian per second.
pub const ANGULAR_STEPS: f32 = 1024.0;

const POSITION: u8 = 1;
const ROTATION: u8 = 2;
const VELOCITY: u8 = 4;
const ANGULAR: u8 = 8;
const STATE: u8 = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BodyState {
    pub position: [i64; 3],
    pub rotation: u32,
    pub velocity: [i32; 3],
    pub angular: [i32; 3],
    /// Game-defined bits: animation, health bucket, flags.
    pub state: u32,
}

impl BodyState {
    /// Quantises a body in voxel units; velocities saturate far past any game speed.
    pub fn quantise(position: Vec3, rotation: Quat, velocity: Vec3, angular: Vec3, state: u32) -> Self {
        let fixed = |v: Vec3, steps: f32| v.to_array().map(|c| (f64::from(c) * f64::from(steps)).round() as i64);
        let small = |v: Vec3, steps: f32| fixed(v, steps).map(|c| c.clamp(-(1 << 20), 1 << 20) as i32);
        Self {
            position: fixed(position, POSITION_STEPS),
            rotation: rotation::pack(rotation),
            velocity: small(velocity, VELOCITY_STEPS),
            angular: small(angular, ANGULAR_STEPS),
            state,
        }
    }

    pub fn position(&self) -> Vec3 {
        Vec3::from_array(self.position.map(|c| (c as f64 / f64::from(POSITION_STEPS)) as f32))
    }

    pub fn rotation(&self) -> Quat {
        rotation::unpack(self.rotation)
    }

    pub fn velocity(&self) -> Vec3 {
        Vec3::from_array(self.velocity.map(|c| c as f32 / VELOCITY_STEPS))
    }

    pub fn angular(&self) -> Vec3 {
        Vec3::from_array(self.angular.map(|c| c as f32 / ANGULAR_STEPS))
    }

    /// Dead-reckoned pose `seconds` after this state, for bodies updated less often
    /// than every snapshot.
    pub fn extrapolate(&self, seconds: f32) -> (Vec3, Quat) {
        let turn = Quat::from_scaled_axis(self.angular() * seconds);
        (self.position() + self.velocity() * seconds, (turn * self.rotation()).normalize())
    }

    /// Writes the fields that differ from `base`.
    pub fn write_delta(&self, base: &Self, out: &mut Vec<u8>) {
        let mask = [
            (self.position != base.position, POSITION),
            (self.rotation != base.rotation, ROTATION),
            (self.velocity != base.velocity, VELOCITY),
            (self.angular != base.angular, ANGULAR),
            (self.state != base.state, STATE),
        ]
        .iter()
        .filter(|(changed, _)| *changed)
        .fold(0, |m, (_, bit)| m | bit);
        out.push(mask);
        if mask & POSITION != 0 {
            (0..3).for_each(|i| put_i64(out, self.position[i].wrapping_sub(base.position[i])));
        }
        if mask & ROTATION != 0 {
            out.extend(self.rotation.to_le_bytes());
        }
        if mask & VELOCITY != 0 {
            self.velocity.iter().for_each(|&v| put_i64(out, i64::from(v)));
        }
        if mask & ANGULAR != 0 {
            self.angular.iter().for_each(|&v| put_i64(out, i64::from(v)));
        }
        if mask & STATE != 0 {
            put_u64(out, u64::from(self.state));
        }
    }

    pub fn read_delta(base: &Self, r: &mut Reader) -> Result<Self, WireError> {
        let mask = r.u8()?;
        if mask & !(POSITION | ROTATION | VELOCITY | ANGULAR | STATE) != 0 {
            return Err(WireError("unknown body field"));
        }
        let mut s = *base;
        if mask & POSITION != 0 {
            for i in 0..3 {
                s.position[i] = base.position[i].wrapping_add(r.i64()?);
            }
        }
        if mask & ROTATION != 0 {
            s.rotation = u32::from_le_bytes([r.u8()?, r.u8()?, r.u8()?, r.u8()?]);
        }
        if mask & VELOCITY != 0 {
            s.velocity = [r.i32()?, r.i32()?, r.i32()?];
        }
        if mask & ANGULAR != 0 {
            s.angular = [r.i32()?, r.i32()?, r.i32()?];
        }
        if mask & STATE != 0 {
            s.state = r.u32()?;
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tumbling(t: f32) -> BodyState {
        let position = Vec3::new(-70_000.25 + t * 3.0, 40.0 - t * t * 4.0, 12_345.5);
        BodyState::quantise(position, Quat::from_rotation_y(t), Vec3::new(3.0, -8.0 * t, 0.0), Vec3::Y, 7)
    }

    #[test]
    fn deltas_rebuild_the_exact_integers() {
        for (a, b) in [(BodyState::default(), tumbling(0.5)), (tumbling(0.5), tumbling(0.53)), (tumbling(1.0), tumbling(1.0))] {
            let mut out = Vec::new();
            b.write_delta(&a, &mut out);
            let mut r = Reader::new(&out);
            assert_eq!(BodyState::read_delta(&a, &mut r), Ok(b));
            assert!(r.done());
        }
    }

    #[test]
    fn a_resting_body_costs_one_byte_and_a_moving_one_stays_small() {
        let mut out = Vec::new();
        tumbling(1.0).write_delta(&tumbling(1.0), &mut out);
        assert_eq!(out.len(), 1);
        out.clear();
        // One 30 Hz snapshot of a falling, turning body.
        tumbling(1.0 + 1.0 / 30.0).write_delta(&tumbling(1.0), &mut out);
        assert!(out.len() <= 18, "{} bytes", out.len());
    }

    #[test]
    fn quantisation_keeps_far_positions_to_half_a_step() {
        let p = Vec3::new(-1_000_000.5, 300.25, 999_999.75);
        let back = BodyState::quantise(p, Quat::IDENTITY, Vec3::ZERO, Vec3::ZERO, 0).position();
        assert!((back - p).abs().max_element() <= 0.0625, "{back}");
    }

    #[test]
    fn dead_reckoning_follows_velocity_and_spin() {
        let s = BodyState::quantise(Vec3::ZERO, Quat::IDENTITY, Vec3::new(2.0, 0.0, -1.0), Vec3::Y * 1.5, 0);
        let (p, q) = s.extrapolate(0.5);
        assert!((p - Vec3::new(1.0, 0.0, -0.5)).length() < 1e-3);
        assert!(q.angle_between(Quat::from_rotation_y(0.75)) < 1e-2);
    }

    #[test]
    fn unknown_mask_bits_and_short_input_are_refused() {
        assert!(BodyState::read_delta(&BodyState::default(), &mut Reader::new(&[0x80])).is_err());
        assert!(BodyState::read_delta(&BodyState::default(), &mut Reader::new(&[POSITION, 2])).is_err());
    }
}
