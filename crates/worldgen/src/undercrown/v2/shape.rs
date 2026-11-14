//! Where a point falls in a space or a passage: void, floor band, shell or
//! outside. Halls erode outward above their plinth band; passages carry an
//! arched roof and a width that breathes along their length. Both only ever
//! add air beyond the authored box, so the walkable floor never shrinks.
use super::{Shape, Space, Tunnel};
use crate::value_noise3;
use glam::{Vec2, Vec3};

/// How a point relates to a space: inside its void, in its floor band, in
/// its shell, or outside.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum At {
    Void,
    Floor,
    Shell,
    Out,
}

/// Deepest outward erosion of a hall wall, metres.
pub(super) const ERODE: f32 = 0.9;
/// Shell pad round a hall; wider than the erosion so rock always remains.
const PAD: f32 = 2.0;

pub(super) fn hall_ceiling(x0: f32, x1: f32, z0: f32, z1: f32, h: f32, rise: f32, m: Vec3) -> f32 {
    let (w, d) = (x1 - x0, z1 - z0);
    let t = if w <= d { ((m.x - x0) / w * 2.0 - 1.0).abs() } else { ((m.z - z0) / d * 2.0 - 1.0).abs() };
    h + rise * (1.0 - t * t)
}

impl Space {
    /// Height over the floor below which the walls stay straight: the plinth
    /// and sconce band, raised clear of any standing water.
    pub(super) fn plinth(&self, floor: f32) -> f32 {
        let wet = self.water.map_or(0.0, |(level, _)| level - floor + 0.6);
        2.6f32.max(wet + 0.6)
    }

    /// Outward erosion of a hall at a point: none in the plinth band, then a
    /// two-octave noise in metres, so every voxel scale shares one shape.
    fn erosion(&self, m: Vec3, floor: f32) -> f32 {
        let start = self.plinth(floor);
        let fy = m.y - floor;
        if self.sealed || fy < start {
            return 0.0;
        }
        let ramp = ((fy - start) / 1.5).min(1.0);
        let n = value_noise3(m.x * 0.31, m.y * 0.23, m.z * 0.31, 4703) * 0.7 + value_noise3(m.x * 0.9, m.y * 0.9, m.z * 0.9, 4707) * 0.3;
        ramp * (n * 1.25 - 0.15).clamp(0.0, 1.0) * ERODE
    }

    /// True where the void comes only from erosion, outside the authored box.
    pub(super) fn eroded(&self, m: Vec3) -> bool {
        matches!(self.shape, Shape::Hall { x0, x1, z0, z1, .. } if m.x < x0 || m.x >= x1 || m.z < z0 || m.z >= z1)
    }

    pub(super) fn at(&self, m: Vec3) -> At {
        match self.shape {
            Shape::Hall { x0, x1, z0, z1, floor, h, rise } => {
                if m.x < x0 - PAD || m.x >= x1 + PAD || m.z < z0 - PAD || m.z >= z1 + PAD || m.y < floor - PAD || m.y >= floor + h + rise + PAD {
                    return At::Out;
                }
                let e = self.erosion(m, floor);
                let inside = m.x >= x0 - e && m.x < x1 + e && m.z >= z0 - e && m.z < z1 + e;
                if inside && m.y >= floor && m.y < floor + hall_ceiling(x0, x1, z0, z1, h, rise, m) + e * 0.6 {
                    At::Void
                } else if m.x >= x0 && m.x < x1 && m.z >= z0 && m.z < z1 && m.y >= floor - 1.0 && m.y < floor {
                    At::Floor
                } else {
                    At::Shell
                }
            }
            Shape::Round { x, z, r, floor, h, dome } => {
                let d = (m.x - x).hypot(m.z - z);
                if d >= r + 1.5 || m.y < floor - 1.5 || m.y >= floor + h + dome + 1.5 {
                    return At::Out;
                }
                let top = floor + h + dome * (1.0 - (d / r).powi(2)).max(0.0);
                if d < r && m.y >= floor && m.y < top {
                    At::Void
                } else if d < r && m.y >= floor - 1.0 && m.y < floor {
                    At::Floor
                } else {
                    At::Shell
                }
            }
            Shape::Cave { c, rad, floor } => {
                let q = (m - c) / rad;
                let k = q.length_squared();
                if k > 1.6 {
                    return At::Out;
                }
                let wobble = (value_noise3(m.x * 0.08, m.y * 0.08, m.z * 0.08, 907) - 0.5) * 0.5;
                let terrace = (value_noise3(m.x * 0.05, 0.0, m.z * 0.05, 911) * 3.0).floor() * 0.5;
                if k + wobble < 1.0 && m.y >= floor + terrace {
                    At::Void
                } else if k + wobble < 1.35 {
                    At::Shell
                } else {
                    At::Out
                }
            }
            Shape::Shaft { x, z, r, y0, y1, .. } => {
                let d = (m.x - x).hypot(m.z - z);
                if d >= r + 1.5 || m.y < y0 - 1.0 || m.y >= y1 {
                    return At::Out;
                }
                if d < r && m.y >= y0 { At::Void } else if d < r { At::Floor } else { At::Shell }
            }
            Shape::Rift { a, b, half, top } => {
                let k = cut(a, b, half, top, m);
                if k.lat.abs() >= half * REACH + 2.5 || m.y < a.y - 2.0 || m.y >= top + 2.0 {
                    return At::Out;
                }
                if k.lat < k.pos && k.lat > -k.neg && m.y >= k.ground && m.y < k.roof { At::Void } else { At::Shell }
            }
        }
    }

    /// A box holding every point that is not `Out`.
    pub(super) fn bounds(&self) -> (Vec3, Vec3) {
        match self.shape {
            Shape::Hall { x0, x1, z0, z1, floor, h, rise } => (Vec3::new(x0, floor, z0) - PAD, Vec3::new(x1, floor + h + rise, z1) + PAD),
            Shape::Round { x, z, r, floor, h, dome } => (Vec3::new(x - r, floor, z - r) - 2.0, Vec3::new(x + r, floor + h + dome, z + r) + 2.0),
            Shape::Cave { c, rad, .. } => (c - rad * 1.3 - 1.0, c + rad * 1.3 + 1.0),
            Shape::Shaft { x, z, r, y0, y1, .. } => (Vec3::new(x - r, y0, z - r) - 2.0, Vec3::new(x + r, y1, z + r) + 2.0),
            Shape::Rift { a, b, half, top } => {
                let pad = Vec3::new(half * REACH + 2.5, 2.0, half * REACH + 2.5);
                (a.min(b) - pad, a.max(b).with_y(top) + pad)
            }
        }
    }
}

/// Furthest a rift wall stands from its centre line, in half widths.
pub(super) const REACH: f32 = 1.4;

/// A rift's roof line `t` of the way from `a` to `b`: level over the first
/// third, then sinking 30 m toward `b`, where the hill above has fallen away.
pub(super) fn rift_top(top: f32, t: f32) -> f32 {
    top - 30.0 * ((t - 0.3) / 0.7).clamp(0.0, 1.0)
}

/// Where a point sits across a rift.
pub(super) struct Cut {
    /// Signed distance from the centre line; round the ends, from the end.
    pub lat: f32,
    /// Wall distance on the positive and the negative side at this height.
    pub pos: f32,
    pub neg: f32,
    pub roof: f32,
    /// The bed in the middle, the banks beside it.
    pub ground: f32,
}

/// A rift's cross-section at a point. Each wall keeps between 0.75 and
/// `REACH` half widths below the taper, so the banks (outside 0.4) always
/// leave a path and authored crossings know where rock begins.
pub(super) fn cut(a: Vec3, b: Vec3, half: f32, top: f32, m: Vec3) -> Cut {
    let (pa, p) = (xz(a), xz(m));
    let ab = xz(b) - pa;
    let len = ab.length().max(1e-3);
    let t = ((p - pa).dot(ab) / (len * len)).clamp(0.0, 1.0);
    let off = p - (pa + ab * t);
    let lat = if ab.perp_dot(off) >= 0.0 { off.length() } else { -off.length() };
    let along = t * len;
    let top = rift_top(top, t);
    let u = ((m.y - (top - 14.0)) / 14.0).clamp(0.0, 1.0);
    let taper = 1.0 - 0.4 * u * u * (3.0 - 2.0 * u);
    let grain = (value_noise3(m.x * 0.3, m.y * 0.3, m.z * 0.3, 7305) - 0.5) * 0.1;
    let side = |seed: u32| half * (0.8 + 0.55 * value_noise3(along * 0.045, m.y * 0.05, 0.0, seed) + grain) * taper;
    let roof = top - 5.0 * value_noise3(along * 0.05, 0.0, 0.0, 7307) - 3.0 * (lat / half).powi(2);
    let ground = a.y + if lat.abs() > 0.4 * half { 2.0 } else { 0.0 };
    Cut { lat, pos: side(7301), neg: side(7303), roof, ground }
}

/// Where a point sits along a passage.
#[derive(Clone, Copy)]
pub(super) struct Frame {
    /// Distance from the centre line.
    pub lateral: f32,
    /// Floor height under the point.
    pub floor: f32,
    /// 0 at `a`, 1 at `b`.
    pub t: f32,
    /// Half width here.
    pub half: f32,
    /// Roof height over the floor here.
    pub roof: f32,
    /// Which side of the centre line: +1 left of a→b, -1 right.
    pub side: f32,
}

impl Tunnel {
    pub(super) fn length(&self) -> f32 {
        (xz(self.b) - xz(self.a)).length()
    }

    pub(super) fn frame(&self, m: Vec3) -> Frame {
        let (a, b) = (xz(self.a), xz(self.b));
        let p = xz(m);
        let ab = b - a;
        let len = ab.length();
        let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        let off = p - (a + ab * t);
        let lateral = off.length();
        // Width breathes along the passage, settling to the authored width
        // within a metre and a half of either end so mouths and gates match.
        let along = t * len;
        let settle = (along.min(len - along) / 1.5).clamp(0.0, 1.0);
        let breathe = if self.gate.is_some() || self.sealed { 0.0 } else { value_noise3(along * 0.22, self.a.x * 0.37, self.a.z * 0.37, 4721) * 0.8 * settle };
        let half = self.half + breathe;
        // An arched roof: the authored height at the quarter points, higher
        // at the crown, lower at the walls; never under 2.6 m.
        let u = (lateral / half).min(1.0);
        let roof = (self.h - 0.6 + 1.1 * (1.0 - u * u)).max(2.6);
        let side = if ab.perp_dot(off) >= 0.0 { 1.0 } else { -1.0 };
        Frame { lateral, floor: self.a.y + (self.b.y - self.a.y) * t, t, half, roof, side }
    }

    pub(super) fn at(&self, m: Vec3) -> At {
        let f = self.frame(m);
        if f.lateral >= f.half + 1.5 || m.y < f.floor - 1.5 || m.y >= f.floor + self.h + 2.0 {
            return At::Out;
        }
        if f.lateral < f.half && m.y >= f.floor && m.y < f.floor + f.roof {
            At::Void
        } else if f.lateral < f.half && m.y >= f.floor - 1.0 && m.y < f.floor {
            At::Floor
        } else {
            At::Shell
        }
    }

    /// A box holding every point that is not `Out`.
    pub(super) fn bounds(&self) -> (Vec3, Vec3) {
        let pad = Vec3::new(self.half + 2.5, 0.0, self.half + 2.5);
        (self.a.min(self.b) - pad - Vec3::Y * 1.5, self.a.max(self.b) + pad + Vec3::Y * (self.h + 2.0))
    }
}

pub(super) fn xz(v: Vec3) -> Vec2 {
    Vec2::new(v.x, v.z)
}
