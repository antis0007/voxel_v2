//! A downed carriage: a rounded-square hull tapering to the nose, the tail torn
//! open, breaches in the plating, internal ribs and a cargo deck.
use crate::{BlockId as B, value_noise2};
use glam::Vec3;

const RIB: f32 = 6.0;

#[derive(Clone, Debug)]
pub(super) struct Hull {
    /// Tail centre.
    pub a: Vec3,
    /// Nose centre.
    pub b: Vec3,
    pub radius: f32,
    /// Roll about the long axis, radians.
    pub roll: f32,
    pub seed: u32,
    pub routed: bool,
}

struct Frame {
    dir: Vec3,
    side: Vec3,
    up: Vec3,
    len: f32,
}

impl Hull {
    fn frame(&self) -> Frame {
        let axis = self.b - self.a;
        let len = axis.length().max(1.0);
        let dir = axis / len;
        let side0 = if dir.y.abs() > 0.95 {
            Vec3::X
        } else {
            dir.cross(Vec3::Y).normalize()
        };
        let up0 = side0.cross(dir);
        let (s, c) = self.roll.sin_cos();
        Frame {
            dir,
            side: side0 * c + up0 * s,
            up: up0 * c - side0 * s,
            len,
        }
    }

    pub fn bounds(&self) -> (Vec3, Vec3) {
        let r = Vec3::splat(self.radius + 1.0);
        (self.a.min(self.b) - r, self.a.max(self.b) + r)
    }

    fn profile(&self, t: f32) -> f32 {
        let nose = ((t - 0.70) / 0.30).clamp(0.0, 1.0);
        self.radius * ((1.0 - nose * nose).sqrt() * 0.82 + 0.18)
    }

    pub fn deck(&self) -> f32 {
        -0.3 * self.radius
    }

    pub fn sample(&self, q: Vec3, v: f32) -> Option<B> {
        let f = self.frame();
        let rel = q - self.a;
        let along = rel.dot(f.dir);
        let (u, w) = (rel.dot(f.side), rel.dot(f.up));
        let angle = w.atan2(u);
        // The tail ripped away along a ragged line.
        let tear = if self.routed {
            2.1 + 1.1 * (angle + 0.7).sin()
        } else {
            4.0 * value_noise2(angle * 2.0, 0.5, self.seed ^ 0x7a11)
        };
        if along < tear || along > f.len {
            return None;
        }
        let r = self.profile(along / f.len);
        let s = (u.abs().powi(4) + (w * 1.12).abs().powi(4)).powf(0.25);
        if s > r {
            return None;
        }
        let shell = v.max(0.7) * if self.routed && along < 6.0 { 1.6 } else { 1.0 };
        let deck = self.deck();
        if self.routed && s > r - shell * 2.5 && self.cargo_door(along, u, w, f.len) {
            return Some(B::Air);
        }
        if s > r - shell {
            return Some(self.plating(along, u, w, angle, r, f.len));
        }
        if w < deck && w > deck - shell {
            return Some(B::SteelPlate);
        }
        let rib = along.rem_euclid(RIB) < shell.max(0.5);
        if rib && s > r - shell * 2.5 && w > deck {
            return Some(B::SteelPlate);
        }
        Some(B::Air)
    }

    fn plating(&self, along: f32, u: f32, w: f32, angle: f32, r: f32, len: f32) -> B {
        if self.cargo_door(along, u, w, len) {
            return B::Air;
        }
        let vulnerable = !self.routed || (along > 10.0 && along.rem_euclid(RIB) > 1.6);
        let breach = value_noise2(along / 5.0, angle * r / 5.0, self.seed ^ 0xb4e) > 0.74;
        if vulnerable && breach && w > self.deck() {
            return B::Air;
        }
        let t = along / len;
        if (0.62..0.66).contains(&t) || (0.2..0.23).contains(&t) {
            return B::HazardPaint;
        }
        if along.rem_euclid(RIB) < 0.6 {
            return B::RustedSteel;
        }
        let corrosion = value_noise2(along / 3.0, angle * r / 3.0, self.seed ^ 0xc0) + w / r * 0.25;
        if corrosion > 0.62 {
            B::RustedSteel
        } else {
            B::SteelPlate
        }
    }

    fn cargo_door(&self, along: f32, u: f32, w: f32, len: f32) -> bool {
        u > 0.0
            && (0.0..6.0).contains(&(along - len * 0.4))
            && w > self.deck()
            && w < self.deck() + 4.0
    }

    /// Threshold and outward direction of the open cargo door.
    pub(super) fn cargo_exit(&self) -> (Vec3, Vec3) {
        let f = self.frame();
        let at = self.a
            + f.dir * (f.len * 0.4 + 3.0)
            + f.up * self.deck()
            + f.side * (self.radius - 0.6);
        (at, f.side)
    }

    /// Deck points (local metres) for cargo and sentries.
    pub fn deck_spots(&self, v: f32) -> Vec<Vec3> {
        let f = self.frame();
        let mut spots = Vec::new();
        let mut along = f.len * 0.18;
        let mut flip = 1.0;
        while along < f.len * 0.66 {
            spots.push(
                self.a
                    + f.dir * along
                    + f.up * (self.deck() + 0.5 * v)
                    + f.side * flip * self.radius * 0.35,
            );
            along += 7.0;
            flip = -flip;
        }
        spots
    }
}
