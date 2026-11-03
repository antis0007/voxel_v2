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
const FRAME: u8 = 32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BodyState {
    pub position: [i64; 3],
    pub rotation: u32,
    pub velocity: [i32; 3],
    pub angular: [i32; 3],
    /// Game-defined bits: animation, health bucket, flags.
    pub state: u32,
    /// Assembly whose local grid `position`, `velocity` and `angular` are in; 0 is the world.
    pub frame: u32,
}

impl BodyState {
    /// Quantises a world-frame body in voxel units; velocities saturate far past any game speed.
    pub fn quantise(position: Vec3, rotation: Quat, velocity: Vec3, angular: Vec3, state: u32) -> Self {
        let fixed = |v: Vec3, steps: f32| v.to_array().map(|c| (f64::from(c) * f64::from(steps)).round() as i64);
        let small = |v: Vec3, steps: f32| fixed(v, steps).map(|c| c.clamp(-(1 << 20), 1 << 20) as i32);
        Self {
            position: fixed(position, POSITION_STEPS),
            rotation: rotation::pack(rotation),
            velocity: small(velocity, VELOCITY_STEPS),
            angular: small(angular, ANGULAR_STEPS),
            state,
            frame: 0,
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
            (self.frame != base.frame, FRAME),
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
        if mask & FRAME != 0 {
            put_u64(out, u64::from(self.frame));
        }
    }

    pub fn read_delta(base: &Self, r: &mut Reader) -> Result<Self, WireError> {
        let mask = r.u8()?;
        if mask & !(POSITION | ROTATION | VELOCITY | ANGULAR | STATE | FRAME) != 0 {
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
        if mask & FRAME != 0 {
            s.frame = r.u32()?;
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests;
