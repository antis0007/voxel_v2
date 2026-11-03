//! Assembly poses for passengers: frame conversion, point velocity and a bounded
//! per-assembly history the server samples at a client's reported tick.
//!
//! World positions are `f64` so a hull far from the origin converts to and from
//! its local grid without the sub-voxel loss `f32` has past a few hundred thousand voxels.
use crate::interp::Interpolate;
use glam::{DVec3, Quat, Vec3};
use std::collections::VecDeque;

/// Rigid pose: local point `p` is at `position + rotation * p` in the parent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: DVec3,
    pub rotation: Quat,
}

impl Pose {
    pub const IDENTITY: Self = Self { position: DVec3::ZERO, rotation: Quat::IDENTITY };

    pub fn to_world(&self, local: Vec3) -> DVec3 {
        self.position + (self.rotation * local).as_dvec3()
    }

    pub fn to_local(&self, world: DVec3) -> Vec3 {
        self.rotation.inverse() * (world - self.position).as_vec3()
    }

    /// Pose of a child given in this pose's local grid.
    pub fn compose(&self, child: &Pose) -> Pose {
        Pose { position: self.to_world(child.position.as_vec3()), rotation: (self.rotation * child.rotation).normalize() }
    }
}

impl Interpolate for Pose {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        Self { position: a.position + (b.position - a.position) * f64::from(t), rotation: a.rotation.slerp(b.rotation, t) }
    }
}

/// Carrier state at one tick: pose plus the motion a body inherits when it leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Carrier {
    pub pose: Pose,
    /// Centre-of-mass velocity in world voxels per second.
    pub velocity: Vec3,
    /// World angular velocity in radians per second, about `pose.position`.
    pub angular: Vec3,
}

impl Carrier {
    /// World velocity of the carrier material at `world`: `v + omega x (x - c)`.
    pub fn point_velocity(&self, world: DVec3) -> Vec3 {
        self.velocity + self.angular.cross((world - self.pose.position).as_vec3())
    }

    /// World velocity of a body leaving at local `local` with local velocity `local_velocity`.
    pub fn leave_velocity(&self, local: Vec3, local_velocity: Vec3) -> Vec3 {
        self.pose.rotation * local_velocity + self.point_velocity(self.pose.to_world(local))
    }

    /// World up in the local grid; gravity never follows a banked hull.
    pub fn local_up(&self) -> Vec3 {
        (self.pose.rotation.inverse() * Vec3::Y).normalize_or(Vec3::Y)
    }
}

/// Recent poses of one assembly by simulation tick, oldest dropped past `capacity`.
pub struct CarrierHistory {
    seconds_per_tick: f64,
    capacity: usize,
    entries: VecDeque<(u64, Pose)>,
}

impl CarrierHistory {
    pub fn new(seconds_per_tick: f64, capacity: usize) -> Self {
        Self { seconds_per_tick, capacity: capacity.max(2), entries: VecDeque::new() }
    }

    /// Records the pose at `tick`; ticks at or before the newest are ignored.
    pub fn push(&mut self, tick: u64, pose: Pose) {
        if self.entries.back().is_some_and(|(t, _)| *t >= tick) {
            return;
        }
        self.entries.push_back((tick, pose));
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }

    /// Pose at `tick`, interpolated between recorded ticks; `None` outside the window.
    pub fn pose(&self, tick: u64) -> Option<Pose> {
        let right = self.entries.partition_point(|(t, _)| *t < tick);
        let (bt, b) = self.entries.get(right)?;
        if *bt == tick {
            return Some(*b);
        }
        let (at, a) = self.entries.get(right.checked_sub(1)?)?;
        Some(Pose::lerp(a, b, ((tick - at) as f64 / (bt - at) as f64) as f32))
    }

    /// Pose and motion at `tick`, from the recorded neighbours on either side.
    pub fn carrier(&self, tick: u64) -> Option<Carrier> {
        let pose = self.pose(tick)?;
        let (first, last) = (self.entries.front()?.0, self.entries.back()?.0);
        let (lo, hi) = (tick.saturating_sub(1).max(first), (tick + 1).min(last));
        if hi <= lo {
            return Some(Carrier { pose, velocity: Vec3::ZERO, angular: Vec3::ZERO });
        }
        let (a, b) = (self.pose(lo)?, self.pose(hi)?);
        let span = (hi - lo) as f64 * self.seconds_per_tick;
        let velocity = ((b.position - a.position) / span).as_vec3();
        let turn = (b.rotation * a.rotation.inverse()).normalize();
        let turn = if turn.w < 0.0 { -turn } else { turn };
        let angular = turn.to_scaled_axis() / span as f32;
        Some(Carrier { pose, velocity, angular })
    }
}
