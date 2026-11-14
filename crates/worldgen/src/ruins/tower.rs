//! Gutted office towers: slab floors, a column grid, curtain walls with lost
//! panes, a switchback stair core, a broken crown and vines down the facades.
use super::shapes::{grows_moss, solid_half};
use crate::{BlockId as B, hash2, hash3, value_noise2};
use glam::{Vec2, Vec3};

pub(super) const FLOOR: f32 = 4.0;
const BAY: f32 = 6.0;
/// Stair core: a 6 m x 3.2 m shaft with two flights per storey.
const SHAFT: Vec2 = Vec2::new(6.0, 3.2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Facade {
    Glass,
    Plate,
    Punched,
}

#[derive(Clone, Debug)]
pub(super) struct Tower {
    /// Base centre on the deck.
    pub c: Vec2,
    pub half: Vec2,
    pub height: f32,
    /// Horizontal drift per metre of height.
    pub lean: Vec2,
    /// 0 is intact, 1 is a broken stump.
    pub ruin: f32,
    pub facade: Facade,
    /// Ground-floor doorway faces: +x, -x, +z, -z.
    pub doors: [bool; 4],
    pub seed: u32,
    pub rebuilt: bool,
}

impl Tower {
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let drift = self.lean * self.height;
        let lo = self.c + drift.min(Vec2::ZERO) - self.half - Vec2::ONE;
        let hi = self.c + drift.max(Vec2::ZERO) + self.half + Vec2::ONE;
        (Vec3::new(lo.x, 0.0, lo.y), Vec3::new(hi.x, self.height + 1.0, hi.y))
    }

    pub(super) fn top(&self, d: Vec2) -> f32 {
        if self.rebuilt {
            let direction = if self.seed & 1 == 0 { 1.0 } else { -1.0 };
            let collapse = ((direction * d.x + 0.55 * d.y) / self.half.x).clamp(0.0, 1.0);
            let broken = value_noise2(d.x / 3.5, d.y / 3.5, self.seed);
            return (self.height - (collapse * self.height * 0.36 + broken * 3.0) * self.ruin).max(FLOOR * 2.0);
        }
        if self.ruin < 0.05 {
            return self.height;
        }
        let broad = value_noise2(d.x / 7.0, d.y / 7.0, self.seed);
        let jag = hash2((d.x / 1.5).floor() as i32, (d.y / 1.5).floor() as i32, self.seed ^ 7);
        (self.height - self.ruin * (self.height * 0.45 * broad + 3.0 * jag)).max(FLOOR * 1.5)
    }

    fn shaft(&self, d: Vec2) -> Option<Vec2> {
        let x = if self.rebuilt { -SHAFT.x * 0.5 } else { -self.half.x * 0.5 };
        let origin = Vec2::new(x, -SHAFT.y * 0.5);
        let s = d - origin;
        (s.x >= 0.0 && s.y >= 0.0 && s.x < SHAFT.x && s.y < SHAFT.y).then_some(s)
    }

    fn hole(&self, level: i32, d: Vec2) -> bool {
        if self.rebuilt {
            return level > 1 && d.y.abs() > 2.4 && d.x.abs() > 4.3
                && value_noise2(d.x / 4.0 + level as f32 * 0.25, d.y / 4.0, self.seed ^ 0x401e)
                    > 0.82 - self.ruin * 0.15;
        }
        level > 0 && hash3(level, (d.x / 3.0).floor() as i32, (d.y / 3.0).floor() as i32, self.seed ^ 0x401e)
            < 0.05 + self.ruin * 0.25
    }

    pub fn sample(&self, q: Vec3, v: f32) -> Option<B> {
        let d = Vec2::new(q.x, q.z) - (self.c + self.lean * q.y);
        let a = d.abs();
        if q.y < 0.0 || a.x > self.half.x + v || a.y > self.half.y + v {
            return None;
        }
        let top = self.top(d);
        let half = if self.rebuilt { self.storey_half(q.y) } else { self.half };
        if a.x > half.x || a.y > half.y {
            let vine_top = if self.rebuilt { top.min(FLOOR * 2.0) } else { top };
            return if half == self.half { self.vine(d, q.y, vine_top) } else { None };
        }
        if q.y > top {
            return None;
        }
        let slab = (2.0 * solid_half(0.25, v)).max(v);
        let level = (q.y / FLOOR).floor() as i32;
        let fy = q.y - level as f32 * FLOOR;
        let wall = v.max(0.45);
        let edge = (half - a).min_element();
        if edge < wall {
            if self.rebuilt {
                return Some(self.street_facade(d, half, level, fy, v));
            }
            return Some(self.facade(d, a, level, fy, v));
        }
        if let Some(s) = self.shaft(d) {
            return self.stair(s, fy, slab, v);
        }
        if fy < slab {
            if self.hole(level, d) {
                return Some(B::Air);
            }
            let exposed = top < (level + 1) as f32 * FLOOR + slab;
            if exposed && fy > slab - v && grows_moss(q, self.seed) {
                return Some(B::GroveMoss);
            }
            return Some(if self.rebuilt && level == 0 && d.x.abs() < 2.5 { B::DressedStone } else { B::Concrete });
        }
        let col = solid_half(0.4, v);
        let near = |x: f32| (x - (x / BAY).round() * BAY).abs() < col;
        if near(d.x) && near(d.y) {
            return Some(B::Concrete);
        }
        if self.rebuilt && d.x.abs() > 4.0 && d.y.abs() > 2.2 && fy < slab + v.max(0.6) {
            let desk = (d.x.abs() - 4.5).abs() < 0.85 && d.y.rem_euclid(3.0) < 1.6;
            if desk && hash3(level, (d.y / 3.0).floor() as i32, d.x.signum() as i32, self.seed) > 0.4 {
                return Some(if self.facade == Facade::Plate { B::RustedSteel } else { B::Planks });
            }
        }
        Some(B::Air)
    }

    fn storey_half(&self, y: f32) -> Vec2 {
        let setback = match self.facade {
            Facade::Glass if y >= FLOOR * 2.0 => Vec2::new(1.8, 1.3),
            Facade::Punched if y >= FLOOR * 3.0 => Vec2::new(1.1, 1.6),
            Facade::Plate if y >= FLOOR * 4.0 => Vec2::new(2.0, 0.0),
            _ => Vec2::ZERO,
        };
        let crown = if y >= self.height - 2.0 * FLOOR && self.height >= 12.0 * FLOOR {
            Vec2::new(1.5, 0.35)
        } else {
            Vec2::ZERO
        };
        self.half - setback - crown
    }

    fn street_facade(&self, d: Vec2, half: Vec2, level: i32, fy: f32, v: f32) -> B {
        let inset = half - d.abs();
        let on_x = inset.x < inset.y;
        let (along, face) = if on_x { (d.y, usize::from(d.x < 0.0)) } else { (d.x, 2 + usize::from(d.y < 0.0)) };
        let masonry = match self.facade {
            Facade::Glass => B::Concrete,
            Facade::Punched => B::WeatheredBricks,
            Facade::Plate => B::RustedSteel,
        };
        if level == 0 && self.doors[face] && along.abs() < 2.1 && fy < 3.15 {
            return B::Air;
        }
        let bay = (along + BAY * 0.5).rem_euclid(BAY) - BAY * 0.5;
        let window = if self.facade == Facade::Punched { 1.45 } else { 2.3 };
        if inset.max_element() < 1.3 || bay.abs() > window {
            return if self.facade == Facade::Glass && level > 1 { B::SteelPlate } else { masonry };
        }
        if fy < v.max(0.85) || fy >= FLOOR - v.max(0.55) {
            return if level == 0 || level % 4 == 0 { B::DressedStone } else { masonry };
        }
        let bay_id = (along / BAY).round() as i32;
        let damage = hash3(level / 2, bay_id, face as i32, self.seed ^ 0x91ae);
        if damage < 0.22 + self.ruin * 0.50
            || self.facade == Facade::Plate && fy < 2.0
        {
            return if self.facade == Facade::Plate && fy < 2.0 { masonry } else { B::Air };
        }
        B::WindowGlass
    }

    fn facade(&self, d: Vec2, a: Vec2, level: i32, fy: f32, v: f32) -> B {
        let on_x = self.half.x - a.x < self.half.y - a.y;
        let along = if on_x { d.y } else { d.x };
        let face: i32 = match (on_x, if on_x { d.x } else { d.y } > 0.0) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (false, false) => 3,
        };
        let col = solid_half(0.45, v);
        let corner = (self.half - a).max_element() < col * 2.0;
        let bay = along - (along / BAY).round() * BAY;
        if corner || bay.abs() < col {
            return B::Concrete;
        }
        if level == 0 && self.doors[face as usize] && along.abs() < 2.6 && fy < 3.2 {
            return B::Air;
        }
        if fy < 1.1 {
            return if self.facade == Facade::Plate { B::SteelPlate } else { B::Concrete };
        }
        let pane = hash3(level, (along / 1.2).floor() as i32, face, self.seed ^ 0x9a1e);
        match self.facade {
            Facade::Punched if bay.abs() > 1.9 || fy > 3.3 => B::Concrete,
            Facade::Punched => B::Air,
            _ if pane < 0.22 + self.ruin * 0.45 => B::Air,
            _ => B::WindowGlass,
        }
    }

    /// Two flights per storey: up along +x on the near half, back on the far half.
    fn stair(&self, s: Vec2, fy: f32, slab: f32, v: f32) -> Option<B> {
        let step = v.max(0.25);
        let rise = |run: f32| ((run / SHAFT.x) * FLOOR * 0.5 / step).floor() * step;
        let surface = if s.y < SHAFT.y * 0.5 { rise(s.x) } else { FLOOR * 0.5 + rise(SHAFT.x - s.x) };
        // The half landing spans the far end of both flights.
        let surface = if s.x > SHAFT.x - 1.2 { FLOOR * 0.5 } else { surface };
        let under = surface - slab.max(0.8);
        if fy <= surface && fy >= under {
            return Some(B::Concrete);
        }
        // The storey slab rings the shaft except over the arrival of the next flight.
        if fy < slab && s.x < 1.2 && s.y >= SHAFT.y * 0.5 {
            return Some(B::Concrete);
        }
        Some(B::Air)
    }

    fn vine(&self, d: Vec2, y: f32, top: f32) -> Option<B> {
        let along = if d.x.abs() > self.half.x { d.y } else { d.x };
        let strand = (along / 1.2).floor() as i32;
        let side = (d.x.signum() as i32) * 2 + d.y.signum() as i32;
        let pick = hash2(strand, side, self.seed ^ 0x51e);
        if pick > 0.10 + self.ruin * 0.14 {
            return None;
        }
        let len = 2.0 + hash2(strand, side, self.seed ^ 0x52e) * 9.0;
        (y <= top && y >= top - len).then_some(B::Leaves)
    }

    /// Floor points (local metres) where loot and sentries can stand.
    pub fn floor_spots(&self, v: f32) -> Vec<Vec3> {
        let slab = (2.0 * solid_half(0.25, v)).max(v);
        let levels = (self.height / FLOOR) as i32;
        let mut spots = Vec::new();
        for level in 1..levels {
            let roll = hash2(level, 3, self.seed ^ 0xca5e);
            if roll > 0.45 {
                continue;
            }
            let y = level as f32 * FLOOR;
            let half = if self.rebuilt { self.storey_half(y + v) } else { self.half };
            let x = half.x * if self.rebuilt { 0.65 } else { 0.45 };
            let off = Vec2::new(x, (roll - 0.2) * half.y * 1.4);
            let d = off;
            if self.hole(level, d) || self.shaft(d).is_some() || self.top(d) < y + FLOOR {
                continue;
            }
            let p = self.c + self.lean * y + d;
            spots.push(Vec3::new(p.x, y + slab + 0.5 * v, p.y));
        }
        spots
    }
}
