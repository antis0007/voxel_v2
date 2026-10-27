//! Server check of client-owned movement (design section 4).
//!
//! A client reports its own pose each step. The server accepts it when it fits
//! two refilling budgets and the swept body stays out of solid cells:
//!
//! - horizontal distance refills at the speed cap, so dashes and slides spend a
//!   burst allowance and a sustained speed hack runs dry;
//! - ascent refills only while the body is supported, so jumps pass and flight
//!   without permission runs dry.
//!
//! A rejected pose returns a numbered correction. The client snaps to it and the
//! server ignores its poses until one echoes that number, so packets already in
//! flight cannot re-trigger it.
use glam::{IVec3, Vec3};

/// Solid-cell query on the server's published world.
pub trait Solid {
    fn solid(&self, cell: IVec3) -> bool;
}

/// Limits in voxels and seconds, supplied from the game's movement constants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// Largest sustained horizontal speed.
    pub speed: f32,
    /// Horizontal distance that may be spent above `speed` at once (dash, slide).
    pub burst: f32,
    /// Height gained from support with every jump the body can chain.
    pub ascent: f32,
    /// Body half-width and height; feet are at the pose position.
    pub half_width: f32,
    pub height: f32,
    /// Overlap allowed against solid cells, for replicas a tick behind an edit.
    pub skin: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    Accept,
    /// Pose was ignored while a correction is outstanding.
    Ignored,
    Correct { id: u16, position: Vec3 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reason {
    Speed,
    Ascent,
    Solid,
}

pub struct Validator {
    limits: Limits,
    accepted: Vec3,
    horizontal: f32,
    vertical: f32,
    correction: u16,
    awaiting: bool,
    pub flying_allowed: bool,
    pub last_reason: Option<Reason>,
}

impl Validator {
    pub fn new(limits: Limits, position: Vec3) -> Self {
        Self {
            limits,
            accepted: position,
            horizontal: limits.burst,
            vertical: limits.ascent,
            correction: 0,
            awaiting: false,
            flying_allowed: false,
            last_reason: None,
        }
    }

    pub fn accepted(&self) -> Vec3 {
        self.accepted
    }

    /// Server-caused motion (knockback, conveyor): extra horizontal and vertical allowance.
    pub fn impulse(&mut self, horizontal: f32, vertical: f32) {
        self.horizontal += horizontal.max(0.0);
        self.vertical += vertical.max(0.0);
    }

    /// Server-placed body (teleport, respawn, mount): always a correction.
    pub fn place(&mut self, position: Vec3) -> Verdict {
        self.accepted = position;
        self.horizontal = self.limits.burst;
        self.vertical = self.limits.ascent;
        self.correct()
    }

    fn correct(&mut self) -> Verdict {
        self.correction = self.correction.wrapping_add(1);
        self.awaiting = true;
        Verdict::Correct { id: self.correction, position: self.accepted }
    }

    fn body_hits(&self, world: &impl Solid, feet: Vec3) -> bool {
        let l = self.limits;
        let lo = (feet + Vec3::new(-l.half_width + l.skin, l.skin, -l.half_width + l.skin)).floor().as_ivec3();
        let hi = (feet + Vec3::new(l.half_width - l.skin, l.height - l.skin, l.half_width - l.skin)).floor().as_ivec3();
        (lo.x..=hi.x).any(|x| (lo.y..=hi.y).any(|y| (lo.z..=hi.z).any(|z| world.solid(IVec3::new(x, y, z)))))
    }

    fn supported(&self, world: &impl Solid, feet: Vec3) -> bool {
        self.body_hits(world, feet - Vec3::Y * (self.limits.skin + 0.05))
    }

    /// Checks one reported step of `dt` seconds. `ack` is the newest correction
    /// id the client has applied.
    pub fn step(&mut self, world: &impl Solid, position: Vec3, dt: f32, ack: u16) -> Verdict {
        let l = self.limits;
        let cap = l.speed * dt + l.burst;
        self.horizontal = if self.horizontal < cap {
            (self.horizontal + l.speed * dt).min(cap)
        } else {
            // Impulse surplus fades with a ~0.3 s half-life instead of vanishing.
            cap + (self.horizontal - cap) * 0.5f32.powf(dt / 0.3)
        };
        if self.awaiting {
            if ack != self.correction {
                return Verdict::Ignored;
            }
            self.awaiting = false;
        }
        if !position.is_finite() {
            self.last_reason = Some(Reason::Speed);
            return self.correct();
        }
        let delta = position - self.accepted;
        let run = Vec3::new(delta.x, 0.0, delta.z).length();
        // Sweep in steps under half a voxel so a fast move cannot pass a wall.
        let steps = (delta.length() / 0.4).ceil().max(1.0) as u32;
        let through_wall = (1..=steps).any(|i| self.body_hits(world, self.accepted + delta * (i as f32 / steps as f32)));
        let rising = delta.y.max(0.0);
        let reason = if through_wall {
            Some(Reason::Solid)
        } else if run > self.horizontal {
            Some(Reason::Speed)
        } else if !self.flying_allowed && rising > self.vertical {
            Some(Reason::Ascent)
        } else {
            None
        };
        if let Some(reason) = reason {
            self.last_reason = Some(reason);
            return self.correct();
        }
        self.horizontal -= run;
        if self.flying_allowed || self.supported(world, position) {
            self.vertical = l.ascent;
        } else {
            self.vertical -= rising;
        }
        self.accepted = position;
        Verdict::Accept
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat floor at y < 0 and a wall at x = 10.
    struct Room;
    impl Solid for Room {
        fn solid(&self, c: IVec3) -> bool {
            c.y < 0 || c.x == 10
        }
    }

    const LIMITS: Limits = Limits { speed: 6.0, burst: 3.0, ascent: 2.6, half_width: 0.3, height: 1.8, skin: 0.02 };
    const DT: f32 = 1.0 / 120.0;

    /// Feeds a path sampled at 120 Hz; returns the corrections and the first reason.
    fn walk(v: &mut Validator, path: impl Fn(f32) -> Vec3, seconds: f32) -> (usize, Option<Reason>) {
        let (mut ack, mut corrections) = (0, 0);
        for i in 1..=(seconds / DT) as u32 {
            match v.step(&Room, path(i as f32 * DT), DT, ack) {
                Verdict::Correct { id, .. } => {
                    corrections += 1;
                    ack = id;
                }
                Verdict::Accept | Verdict::Ignored => {}
            }
        }
        (corrections, v.last_reason)
    }

    #[test]
    fn honest_sprint_dash_and_jumps_are_accepted() {
        let mut v = Validator::new(LIMITS, Vec3::new(0.0, 0.0, 0.0));
        // 6 voxels/s along z, a 16 voxels/s dash for 0.16 s, hops of 1.2 voxels.
        let path = |t: f32| {
            let dash = (t - 1.0).clamp(0.0, 0.16) * 10.0;
            let hop = (t * std::f32::consts::PI).sin().max(0.0) * 1.2;
            Vec3::new(0.0, hop, t * 6.0 + dash)
        };
        assert_eq!(walk(&mut v, path, 4.0), (0, None));
    }

    #[test]
    fn a_sustained_speed_hack_is_corrected() {
        let mut v = Validator::new(LIMITS, Vec3::ZERO);
        let (n, reason) = walk(&mut v, |t| Vec3::new(0.0, 0.0, -t * 9.0), 3.0);
        assert!(n > 0);
        assert_eq!(reason, Some(Reason::Speed));
        assert!(v.accepted().z > -3.0 * 6.0 - LIMITS.burst - 0.1, "{}", v.accepted());
    }

    #[test]
    fn walking_through_a_wall_is_corrected_even_in_one_step() {
        let mut v = Validator::new(LIMITS, Vec3::new(9.0, 0.0, 0.0));
        assert!(matches!(v.step(&Room, Vec3::new(11.5, 0.0, 0.0), DT, 0), Verdict::Correct { .. }));
        assert_eq!(v.last_reason, Some(Reason::Solid));
        assert_eq!(v.accepted(), Vec3::new(9.0, 0.0, 0.0));
    }

    #[test]
    fn unpermitted_flight_is_corrected_and_permitted_flight_is_not() {
        let rise = |t: f32| Vec3::new(0.0, t * 3.0, 0.0);
        let mut v = Validator::new(LIMITS, Vec3::ZERO);
        assert_eq!(walk(&mut v, rise, 3.0).1, Some(Reason::Ascent));
        let mut v = Validator::new(LIMITS, Vec3::ZERO);
        v.flying_allowed = true;
        assert_eq!(walk(&mut v, rise, 3.0), (0, None));
    }

    #[test]
    fn poses_in_flight_are_ignored_until_the_correction_is_acknowledged() {
        let mut v = Validator::new(LIMITS, Vec3::ZERO);
        let Verdict::Correct { id, .. } = v.step(&Room, Vec3::new(50.0, 0.0, 0.0), DT, 0) else { panic!() };
        assert_eq!(v.step(&Room, Vec3::new(50.1, 0.0, 0.0), DT, 0), Verdict::Ignored);
        assert_eq!(v.step(&Room, Vec3::new(0.0, 0.0, 0.04), DT, id), Verdict::Accept);
    }

    #[test]
    fn server_placement_and_knockback_do_not_count_against_the_client() {
        let mut v = Validator::new(LIMITS, Vec3::ZERO);
        let Verdict::Correct { id, position } = v.place(Vec3::new(-40.0, 0.0, 5.0)) else { panic!() };
        assert_eq!(v.step(&Room, position, DT, id), Verdict::Accept);
        v.impulse(8.0, 0.0);
        assert_eq!(v.step(&Room, position + Vec3::X * 8.0, DT, id), Verdict::Accept);
    }
}
