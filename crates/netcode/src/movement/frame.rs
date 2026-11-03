//! Movement check for bodies standing in an assembly's local grid (ADR 0040).
//!
//! A passenger reports poses in its carrier's grid, so validating there removes
//! the carrier's motion between two reported ticks before any budget runs. Only
//! the direction of gravity follows the carrier's pose: heights are measured
//! along world up projected into the grid. The body box stays on grid axes.
use super::{Reason, Solid, Validator, Verdict};
use crate::carrier::{Carrier, Pose};
use glam::{DVec3, Vec3};

/// Seconds of inherited carrier speed granted as burst when a drifting body lands.
const LANDING_SLIDE: f32 = 0.5;
/// Seconds of support that halve inherited drift.
const DRIFT_HALF_LIFE: f32 = 0.15;

impl Validator {
    /// Assembly the accepted pose is in; corrections are in this frame.
    pub fn frame(&self) -> u32 {
        self.frame
    }

    /// Checks one step reported in `frame`. `solid` is that frame's grid (the
    /// world for 0) and `carrier` gives each assembly's state at the reported
    /// tick. A frame change converts the accepted pose without a correction.
    pub fn step_in_frame(
        &mut self,
        solid: &impl Solid,
        carrier: impl Fn(u32) -> Option<Carrier>,
        frame: u32,
        position: Vec3,
        dt: f32,
        ack: u16,
    ) -> Verdict {
        if self.awaiting && ack != self.correction {
            return self.step(solid, position, dt, ack);
        }
        if frame != self.frame {
            if self.frame != 0 {
                let Some(from) = carrier(self.frame) else { return self.refuse_frame(dt) };
                self.leave(&from, dt);
            }
            if frame != 0 {
                let Some(to) = carrier(frame) else { return self.refuse_frame(dt) };
                self.board(frame, &to, dt);
            }
        }
        if self.frame != 0 {
            let Some(here) = carrier(self.frame) else { return self.refuse_frame(dt) };
            self.reorient(here.local_up());
        }
        self.step(solid, position, dt, ack)
    }

    /// Server-placed body at `position` in `frame`, with `up` as world up there.
    pub fn place_in_frame(&mut self, frame: u32, up: Vec3, position: Vec3) -> Verdict {
        self.frame = frame;
        self.up = up;
        self.place(position)
    }

    fn refuse_frame(&mut self, dt: f32) -> Verdict {
        self.air.advance(dt);
        self.last_reason = Some(Reason::Frame);
        self.correct()
    }

    // Carrier states are sampled at the new report's tick while the accepted pose is
    // from the previous one, so each conversion steps the carrier back or forward by `dt`.

    /// Converts to world once and keeps the carrier point velocity as drift.
    fn leave(&mut self, from: &Carrier, dt: f32) {
        let here = from.pose.to_world(self.accepted);
        let inherited = from.point_velocity(here);
        let world = here - (inherited * dt).as_dvec3();
        self.rebase(world, Pose::IDENTITY, Vec3::Y);
        self.frame = 0;
        self.drift = Vec3::new(inherited.x, 0.0, inherited.z);
        if inherited.y > 0.0 {
            // Rise the carrier throws the body, as a free-fall apex height.
            let rise = inherited.y * inherited.y / (2.0 * self.limits.gravity.max(f32::EPSILON));
            self.impulse(0.0, rise);
        }
    }

    /// Converts into `to`'s grid; the body adopts the carrier's motion on contact.
    fn board(&mut self, frame: u32, to: &Carrier, dt: f32) {
        let then = self.accepted.as_dvec3();
        let world = then + (to.point_velocity(then) * dt).as_dvec3();
        self.rebase(world, to.pose, to.local_up());
        self.frame = frame;
        self.drift = Vec3::ZERO;
    }

    fn rebase(&mut self, world: DVec3, into: Pose, up: Vec3) {
        let before = self.height(self.accepted);
        self.accepted = into.to_local(world);
        self.up = up;
        self.air.shift(self.height(self.accepted) - before);
    }

    /// Follows a turning carrier's view of world up, keeping the body's height.
    fn reorient(&mut self, up: Vec3) {
        let before = self.height(self.accepted);
        self.up = up;
        self.air.shift(self.height(self.accepted) - before);
    }

    /// Support bleeds inherited drift into burst allowance instead of stopping it at
    /// once, so the take-off step of a jump keeps nearly all of it.
    pub(super) fn land_drift(&mut self, dt: f32) {
        let keep = 0.5f32.powf(dt / DRIFT_HALF_LIFE);
        let lost = self.drift * (1.0 - keep);
        self.horizontal += lost.length() * LANDING_SLIDE;
        self.drift = if self.drift.length() * keep < 0.05 { Vec3::ZERO } else { self.drift * keep };
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
