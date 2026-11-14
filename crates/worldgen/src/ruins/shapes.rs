//! Volume primitives for abandoned districts, in local metres (y = 0 is the deck).
use super::hull::Hull;
use super::tower::Tower;
use crate::{BlockId as B, hash2, value_noise2};
use glam::Vec3;

/// Half-thickness that still covers a voxel centre at this scale; thinner
/// members would sample as dotted lines on coarse worlds.
pub(super) fn solid_half(r: f32, v: f32) -> f32 {
    r.max(0.55 * v)
}

pub(super) enum Shape {
    /// Axis-aligned box; `moss` grows a patchy layer on its top face.
    Slab { c: Vec3, half: Vec3, mat: B, moss: bool },
    /// Square-section member from `a` to `b` with half-width `r`.
    Beam { a: Vec3, b: Vec3, r: f32, mat: B, moss: bool },
    /// Thin plate from `a` to `b`, `half_width` across the run, `thick` deep.
    Sheet { a: Vec3, b: Vec3, half_width: f32, thick: f32, mat: B },
    /// Upper half-ring of a portal pylon; spans X when `along_x`, else Z.
    Arch { c: Vec3, radius: f32, r: f32, depth: f32, along_x: bool, mat: B },
    /// Lumpy ellipsoid: rubble heaps, tree crowns.
    Blob { c: Vec3, half: Vec3, mat: B, seed: u32 },
    PlanterTree { base: Vec3 },
    /// A hanging strand of leaves, one voxel wide.
    Vine { x: f32, z: f32, top: f32, len: f32 },
    /// Raised landing pad with painted ring and cross.
    Pad { c: Vec3, r: f32, h: f32 },
    ServiceStair { c: Vec3, height: f32 },
    Container { c: Vec3, turn: u32, mat: B, open: bool },
    Tower(Box<Tower>),
    CityWing(Box<super::city::CityWing>),
    Hull(Box<Hull>),
}

pub(super) struct Part {
    pub lo: Vec3,
    pub hi: Vec3,
    pub shape: Shape,
    pub elevation: f32,
}

impl Part {
    pub fn new(shape: Shape) -> Self {
        let (lo, hi) = bounds(&shape);
        Self { lo, hi, shape, elevation: 0.0 }
    }

    pub fn lift(mut self, elevation: f32) -> Self {
        self.raise(elevation);
        self
    }

    pub fn raise(&mut self, elevation: f32) {
        self.elevation += elevation;
        self.lo.y += elevation;
        self.hi.y += elevation;
    }

    pub fn contains_xz(&self, x: f32, z: f32, pad: f32) -> bool {
        x >= self.lo.x - pad && x <= self.hi.x + pad && z >= self.lo.z - pad && z <= self.hi.z + pad
    }

    pub fn fit_voxels(&mut self, v: f32) {
        if let Shape::Slab { c, half, .. } = &self.shape {
            let half = half.max(Vec3::splat(solid_half(0.0, v)));
            let centre = *c + Vec3::Y * self.elevation;
            self.lo = centre - half;
            self.hi = centre + half;
        }
    }
}

fn bounds(shape: &Shape) -> (Vec3, Vec3) {
    match shape {
        Shape::Slab { c, half, .. } | Shape::Blob { c, half, .. } => (*c - *half, *c + *half),
        Shape::PlanterTree { base } => (
            *base - Vec3::new(3.2, 0.0, 3.2),
            *base + Vec3::new(3.2, 8.0, 3.2),
        ),
        Shape::Beam { a, b, r, .. } => (a.min(*b) - Vec3::splat(*r + 1.0), a.max(*b) + Vec3::splat(*r + 1.0)),
        Shape::Sheet { a, b, half_width, .. } => {
            let pad = Vec3::splat(half_width + 1.0);
            (a.min(*b) - pad, a.max(*b) + pad)
        }
        Shape::Arch { c, radius, r, depth, along_x, .. } => {
            let span = radius + r;
            let ext = if *along_x { Vec3::new(span, span, *depth) } else { Vec3::new(*depth, span, span) };
            (*c - Vec3::new(ext.x, 0.0, ext.z), *c + ext)
        }
        Shape::Vine { x, z, top, len } => (Vec3::new(x - 1.0, top - len, z - 1.0), Vec3::new(x + 1.0, *top, z + 1.0)),
        Shape::Pad { c, r, h } => (*c - Vec3::new(*r, 0.0, *r), *c + Vec3::new(*r, *h, *r)),
        Shape::ServiceStair { c, height } => (*c - Vec3::new(7.0, 0.0, 3.6), *c + Vec3::new(7.0, *height, 3.6)),
        Shape::Container { c, turn, .. } => {
            let h = super::cargo::HALF;
            let xz = super::rotate(*turn, h.x, h.z).abs();
            (*c - Vec3::new(xz.x, 0.0, xz.y), *c + Vec3::new(xz.x, super::cargo::HEIGHT, xz.y))
        }
        Shape::Tower(t) => t.bounds(),
        Shape::CityWing(w) => w.bounds(),
        Shape::Hull(h) => h.bounds(),
    }
}

/// Some(Air) means the shape claims the cell as open space (rooms, hull interiors).
pub(super) fn sample(shape: &Shape, q: Vec3, v: f32, seed: u32) -> Option<B> {
    match shape {
        Shape::Slab { c, half, mat, moss } => {
            let half = half.max(Vec3::splat(solid_half(0.0, v)));
            let d = (q - *c).abs();
            if d.x > half.x || d.y > half.y || d.z > half.z {
                return None;
            }
            if *moss && q.y > c.y + half.y - v && grows_moss(q, seed) {
                return Some(B::GroveMoss);
            }
            Some(*mat)
        }
        Shape::Beam { a, b, r, mat, moss } => {
            let r = solid_half(*r, v);
            beam(q, *a, *b, (r, r), *mat, *moss, v, seed)
        }
        Shape::Sheet { a, b, half_width, thick, mat } => {
            beam(q, *a, *b, (*half_width, solid_half(*thick, v)), *mat, false, v, seed)
        }
        Shape::Arch { c, radius, r, depth, along_x, mat } => {
            let d = q - *c;
            let (span, through) = if *along_x { (d.x, d.z) } else { (d.z, d.x) };
            if d.y < 0.0 || through.abs() > *depth {
                return None;
            }
            let ring = (span * span + d.y * d.y).sqrt() - radius;
            (ring.abs() <= solid_half(*r, v)).then_some(*mat)
        }
        Shape::Blob { c, half, mat, seed: s } => {
            let n = (q - *c) / *half;
            let lump = value_noise2(q.x * 0.45 + q.y * 0.3, q.z * 0.45 - q.y * 0.2, *s) - 0.5;
            (n.length() < 1.0 + lump * 0.5).then_some(*mat)
        }
        Shape::PlanterTree { base } => {
            let d = (q - *base).abs();
            let stem = solid_half(0.47, v);
            if d.x.max(d.z) <= stem && d.y <= 6.4 {
                return Some(B::Log);
            }
            let crown = ((d.x / 3.05).powi(2) + (d.z / 2.7).powi(2)
                + ((d.y - 6.0) / 2.0).powi(2)) <= 1.0;
            // Even the smallest top slice overlaps the centre column.
            (crown || (d.x.max(d.z) <= stem && d.y > 6.4)).then_some(B::Leaves)
        }
        Shape::Vine { x, z, top, len } => {
            let w = 0.5 * v.max(0.25);
            ((q.x - x).abs() <= w && (q.z - z).abs() <= w && q.y <= *top && q.y >= top - len).then_some(B::Leaves)
        }
        Shape::Pad { c, r, h } => pad(q - *c, *r, *h, v),
        Shape::ServiceStair { c, .. } => service_stair(q - *c, v),
        Shape::Container { c, turn, mat, open } => {
            let d = q - *c;
            let xz = super::rotate((4 - turn) % 4, d.x, d.z);
            super::cargo::container(Vec3::new(xz.x, d.y, xz.y), v, *mat, *open)
        }
        Shape::Tower(t) => t.sample(q, v),
        Shape::CityWing(w) => w.sample(q, v),
        Shape::Hull(h) => h.sample(q, v),
    }
}

fn service_stair(q: Vec3, v: f32) -> Option<B> {
    let rise = ((q.x + 4.5) / 9.0).clamp(0.0, 1.0) * 2.0;
    let top = if q.z < 0.0 { rise } else { 4.0 - rise };
    let top = (top / v).round() * v;
    let y = (q.y - top).rem_euclid(4.0);
    (y >= 4.0 - v.max(0.5)).then_some(B::SteelPlate)
}

/// Patchy cover: about a third of exposed tops, in metre-scale clumps.
pub(super) fn grows_moss(q: Vec3, seed: u32) -> bool {
    value_noise2(q.x / 3.5, q.z / 3.5, seed ^ 0x6d05) > 0.58
}

#[allow(clippy::too_many_arguments)]
fn beam(q: Vec3, a: Vec3, b: Vec3, (rs, ru): (f32, f32), mat: B, moss: bool, v: f32, seed: u32) -> Option<B> {
    let axis = b - a;
    let len = axis.length();
    if len < 1e-3 {
        return None;
    }
    let dir = axis / len;
    let rel = q - a;
    let along = rel.dot(dir);
    let cap = rs.min(ru);
    if along < -cap || along > len + cap {
        return None;
    }
    let perp = rel - dir * along.clamp(0.0, len);
    let side = if dir.y.abs() > 0.95 { Vec3::X } else { dir.cross(Vec3::Y).normalize() };
    let up = side.cross(dir);
    let (s, u) = (perp.dot(side), perp.dot(up));
    if s.abs() > rs || u.abs() > ru {
        return None;
    }
    // `up` faces the sky on any member flatter than about 60 degrees.
    if moss && up.y.abs() > 0.5 && u * up.y.signum() > ru - v && grows_moss(q, seed) {
        return Some(B::GroveMoss);
    }
    Some(mat)
}

fn pad(d: Vec3, r: f32, h: f32, v: f32) -> Option<B> {
    let radial = (d.x * d.x + d.z * d.z).sqrt();
    if radial > r || d.y < 0.0 || d.y > h {
        return None;
    }
    if d.y > h - v {
        let ring = radial > r - 1.8 && radial < r - 0.9;
        let cross = (d.x.abs() < 0.7 || d.z.abs() < 0.7) && radial < r * 0.4;
        if ring || cross {
            return Some(B::HazardPaint);
        }
    }
    Some(B::Concrete)
}

/// Scattered integer hash in [0, 1) for plan decisions.
pub(super) fn roll(seed: u32, i: i32, j: i32) -> f32 {
    hash2(i, j, seed)
}
