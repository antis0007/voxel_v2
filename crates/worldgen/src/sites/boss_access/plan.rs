//! A boss complex as an ordered list of shapes (first match wins), bucketed
//! into a coarse column grid so a voxel tests only the few shapes over it.
use super::{B, Box3};
use crate::sites::modifiers::{SiteModifier, dressing};
use glam::Vec3;

/// Shrine-local bounds every complex fits in: `[u0, u1, y0, y1, w0, w1]`.
pub const BOUNDS: Box3 = [-16, 16, -6, 15, -10, 29];
const CELL: i32 = 4;
const GU: usize = ((BOUNDS[1] - BOUNDS[0]) / CELL) as usize;
const GW: usize = ((BOUNDS[5] - BOUNDS[4] + CELL - 1) / CELL) as usize;

pub(super) fn inside(b: Box3, l: Vec3) -> bool {
    l.x >= b[0] as f32
        && l.x < b[1] as f32
        && l.y >= b[2] as f32
        && l.y < b[3] as f32
        && l.z >= b[4] as f32
        && l.z < b[5] as f32
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Shape {
    Box(Box3),
    /// Annulus around `(u, w)`, radii `r0..r1` metres, heights `y0..y1`.
    Ring { u: f32, w: f32, r0: f32, r1: f32, y0: i32, y1: i32 },
}

impl Shape {
    fn hit(&self, l: Vec3) -> bool {
        match *self {
            Self::Box(b) => inside(b, l),
            Self::Ring { u, w, r0, r1, y0, y1 } => {
                let r = (l.x - u).hypot(l.z - w);
                l.y >= y0 as f32 && l.y < y1 as f32 && r >= r0 && r < r1
            }
        }
    }
    fn columns(&self) -> (i32, i32, i32, i32) {
        match *self {
            Self::Box(b) => (b[0], b[1], b[4], b[5]),
            Self::Ring { u, w, r1, .. } => (
                (u - r1).floor() as i32,
                (u + r1).ceil() as i32,
                (w - r1).floor() as i32,
                (w + r1).ceil() as i32,
            ),
        }
    }
    fn mirrored(self) -> Self {
        match self {
            Self::Box(b) => Self::Box(mirror(b)),
            Self::Ring { u, w, r0, r1, y0, y1 } => Self::Ring { u: -u, w, r0, r1, y0, y1 },
        }
    }
}

pub(super) fn mirror(b: Box3) -> Box3 {
    [-b[1], -b[0], b[2], b[3], b[4], b[5]]
}

/// Block chooser. Patterns read the local metre cell, so they look the same at
/// every voxel scale.
#[derive(Clone, Copy, Debug)]
pub(super) enum Fill {
    B(B),
    /// Book-stack wall: planks with log spines, a copper rail every 4 m.
    Books,
    /// Collapsed masonry: cobblestone and gravel.
    Rubble,
}

impl From<B> for Fill {
    fn from(b: B) -> Self {
        Self::B(b)
    }
}

fn cell_hash(l: Vec3) -> u32 {
    let c = l.floor().as_ivec3();
    engine_core::hash::hash_u32(
        (c.x as u32).wrapping_mul(73_856_093) ^ (c.y as u32).wrapping_mul(19_349_663) ^ (c.z as u32).wrapping_mul(83_492_791),
    )
}

impl Fill {
    fn block(self, l: Vec3) -> B {
        match self {
            Self::B(b) => b,
            Self::Books => {
                if (l.y.floor() as i32).rem_euclid(4) == 0 {
                    B::CopperBlock
                } else if cell_hash(l).is_multiple_of(5) {
                    B::Log
                } else {
                    B::Planks
                }
            }
            Self::Rubble => {
                if cell_hash(l).is_multiple_of(3) { B::Gravel } else { B::Cobblestone }
            }
        }
    }
}

pub(super) struct Builder {
    items: Vec<(Shape, Fill, bool)>,
    /// Flip `u` for everything pushed while set (the arena half of a complex).
    pub mirror: bool,
    /// Site modifiers may restyle shapes pushed while set; mechanisms, stairs
    /// and treasure boxes are pushed with it cleared.
    pub dress: bool,
    modifier: SiteModifier,
}

impl Builder {
    pub fn put(&mut self, b: Box3, f: impl Into<Fill>) {
        self.shape(Shape::Box(b), f);
    }
    pub fn ring(&mut self, u: f32, w: f32, r0: f32, r1: f32, y: (i32, i32), f: impl Into<Fill>) {
        self.shape(Shape::Ring { u, w, r0, r1, y0: y.0, y1: y.1 }, f);
    }
    pub fn shape(&mut self, s: Shape, f: impl Into<Fill>) {
        let s = if self.mirror { s.mirrored() } else { s };
        self.items.push((s, f.into(), self.dress));
    }
    pub fn new(modifier: SiteModifier) -> Self {
        Self { items: Vec::new(), mirror: false, dress: false, modifier }
    }
    pub fn finish(self) -> Plan {
        let mut grid = vec![Vec::new(); GU * GW];
        for (i, (s, _, _)) in self.items.iter().enumerate() {
            let (u0, u1, w0, w1) = s.columns();
            let gu = |u: i32| ((u - BOUNDS[0]).div_euclid(CELL)).clamp(0, GU as i32 - 1) as usize;
            let gw = |w: i32| ((w - BOUNDS[4]).div_euclid(CELL)).clamp(0, GW as i32 - 1) as usize;
            for x in gu(u0)..=gu(u1 - 1) {
                for z in gw(w0)..=gw(w1 - 1) {
                    grid[z * GU + x].push(i as u16);
                }
            }
        }
        Plan { items: self.items, grid, modifier: self.modifier }
    }
}

pub struct Plan {
    items: Vec<(Shape, Fill, bool)>,
    grid: Vec<Vec<u16>>,
    modifier: SiteModifier,
}

impl Plan {
    /// Block at a shrine-local point, `None` where the complex leaves the
    /// natural world alone.
    pub fn sample(&self, l: Vec3) -> Option<B> {
        if !inside(BOUNDS, l) {
            return None;
        }
        let gx = ((l.x.floor() as i32 - BOUNDS[0]) / CELL) as usize;
        let gz = ((l.z.floor() as i32 - BOUNDS[4]) / CELL) as usize;
        self.grid[gz.min(GW - 1) * GU + gx.min(GU - 1)]
            .iter()
            .map(|&i| &self.items[i as usize])
            .find(|(s, _, _)| s.hit(l))
            .map(|(_, f, dress)| {
                let b = f.block(l);
                if *dress && self.modifier != SiteModifier::None {
                    dressing(self.modifier, l.floor().as_ivec3(), b).unwrap_or(b)
                } else {
                    b
                }
            })
    }
}
