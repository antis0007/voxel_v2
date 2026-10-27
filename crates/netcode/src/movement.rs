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
//! A body off the ground must also fall: after a short hang it has to drop at
//! least as far as gravity takes it, so repeating one pose in the air (hover) or
//! descending slowly fails. The server measures each fall from its own support
//! test and reports it through `take_landing`, so a client cannot skip fall damage.
//!
//! A rejected pose returns a numbered correction. The client snaps to it and the
//! server ignores its poses until one echoes that number, so packets already in
//! flight cannot re-trigger it.
use glam::{IVec3, Vec3};

mod air;

/// Solid-cell query on the server's published world.
pub trait Solid {
    fn solid(&self, cell: IVec3) -> bool;
    /// Cells that hold the body up and end a fall: solid ground, plus fluids and
    /// climbables. A fall that ends in a fluid still reports its height.
    fn supports(&self, cell: IVec3) -> bool {
        self.solid(cell)
    }
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
    /// Weakest downward acceleration an unsupported body has, and its fall speed cap.
    pub gravity: f32,
    pub terminal: f32,
    /// Seconds from leaving support before the body must fall: the apex time of the
    /// highest jump chain plus any gravity-free move such as an air dash.
    pub hang: f32,
    /// Body half-width and height; feet are at the pose position.
    pub half_width: f32,
    pub height: f32,
    /// Overlap allowed against solid cells, for replicas a tick behind an edit.
    pub skin: f32,
    /// Largest coordinate magnitude on any axis; beyond it `f32` loses sub-voxel precision.
    pub bound: f32,
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
    /// Stayed in the air or fell slower than gravity allows.
    Float,
    Solid,
    /// Non-finite or out of bounds.
    Bound,
}

pub struct Validator {
    limits: Limits,
    accepted: Vec3,
    horizontal: f32,
    vertical: f32,
    correction: u16,
    awaiting: bool,
    air: air::Air,
    /// Set while the game grants flight or lift (creative, thrusters); skips both air checks.
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
            air: air::Air::new(position.y),
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
        if vertical > 0.0 {
            self.air.restart(self.accepted.y);
        }
    }

    /// Height of the last fall the server saw end on support, once.
    pub fn take_landing(&mut self) -> Option<f32> {
        self.air.take_landing()
    }

    /// Server-placed body (teleport, respawn, mount): always a correction.
    pub fn place(&mut self, position: Vec3) -> Verdict {
        self.accepted = position;
        self.horizontal = self.limits.burst;
        self.vertical = self.limits.ascent;
        self.air = air::Air::new(position.y);
        self.correct()
    }

    fn correct(&mut self) -> Verdict {
        self.correction = self.correction.wrapping_add(1);
        self.awaiting = true;
        Verdict::Correct { id: self.correction, position: self.accepted }
    }

    /// Whether the body at `feet`, widened by `grow` on each side, overlaps a cell passing `test`.
    fn overlaps(&self, feet: Vec3, grow: f32, test: impl Fn(IVec3) -> bool) -> bool {
        let l = self.limits;
        let side = l.half_width - l.skin + grow;
        let lo = (feet + Vec3::new(-side, l.skin, -side)).floor().as_ivec3();
        let hi = (feet + Vec3::new(side, l.height - l.skin, side)).floor().as_ivec3();
        (lo.x..=hi.x).any(|x| (lo.y..=hi.y).any(|y| (lo.z..=hi.z).any(|z| test(IVec3::new(x, y, z)))))
    }

    fn body_hits(&self, world: &impl Solid, feet: Vec3) -> bool {
        self.overlaps(feet, 0.0, |c| world.solid(c))
    }

    fn grounded(&self, world: &impl Solid, feet: Vec3) -> bool {
        self.overlaps(feet - Vec3::Y * (self.limits.skin + 0.05), 0.0, |c| world.supports(c))
    }

    /// Side contact, which wall runs and climbs need; it restarts the hover clock only.
    fn touching(&self, world: &impl Solid, feet: Vec3) -> bool {
        self.overlaps(feet, self.limits.skin + 0.05, |c| world.solid(c))
    }

    /// Moves the accepted body down to `floor` or onto whatever stops it first.
    fn drop_toward(&mut self, world: &impl Solid, floor: f32) {
        let start = self.accepted;
        let floor = floor.max(start.y - air::MAX_DROP).max(-self.limits.bound);
        let steps =((start.y - floor) / 0.4).ceil().max(1.0) as u32;
        for i in 1..=steps {
            let next = Vec3::new(start.x, start.y + (floor - start.y) * (i as f32 / steps as f32), start.z);
            if self.body_hits(world, next) {
                break;
            }
            self.accepted = next;
        }
        // The arc keeps its clock: restarting it would grant a fresh hang after every
        // correction, a slow fall. Later corrections keep pulling along the curve.
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
        self.air.advance(dt);
        if self.awaiting {
            if ack != self.correction {
                return Verdict::Ignored;
            }
            self.awaiting = false;
        }
        if !position.is_finite() || position.abs().max_element() > l.bound {
            self.last_reason = Some(Reason::Bound);
            return self.correct();
        }
        let delta = position - self.accepted;
        let run = Vec3::new(delta.x, 0.0, delta.z).length();
        // Sweep in steps under half a voxel so a fast move cannot pass a wall.
        let steps = (delta.length() / 0.4).ceil().max(1.0) as u32;
        let through_wall = (1..=steps).any(|i| self.body_hits(world, self.accepted + delta * (i as f32 / steps as f32)));
        let rising = delta.y.max(0.0);
        let grounded = self.grounded(world, position);
        let touching = self.touching(world, position);
        let ceiling = self.air.ceiling(l.gravity, l.terminal, l.hang);
        let reason = if through_wall {
            Some(Reason::Solid)
        } else if run > self.horizontal {
            Some(Reason::Speed)
        } else if !self.flying_allowed && rising > self.vertical {
            Some(Reason::Ascent)
        } else if !self.flying_allowed && !grounded && !touching && position.y > ceiling {
            Some(Reason::Float)
        } else {
            None
        };
        if let Some(reason) = reason {
            self.last_reason = Some(reason);
            // Any refusal of an airborne body applies gravity to the accepted pose;
            // otherwise a client refused for rising would keep its server body aloft.
            let held = self.grounded(world, self.accepted) || self.touching(world, self.accepted);
            if !self.flying_allowed && !held && (reason == Reason::Float || self.accepted.y > ceiling) {
                self.drop_toward(world, ceiling.min(self.accepted.y) - air::SLACK);
            }
            return self.correct();
        }
        self.horizontal -= run;
        if self.flying_allowed || grounded {
            self.vertical = l.ascent;
        } else {
            self.vertical -= rising;
        }
        self.accepted = position;
        self.air.accept(position.y, grounded || self.flying_allowed, touching);
        Verdict::Accept
    }
}

#[cfg(test)]
mod tests;
