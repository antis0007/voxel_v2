//! Generator 52 set dressing placed one piece at a time: giant mushrooms,
//! shelf fungi on columns, hanging lanterns and ice pipes, each fitted to
//! the room by `settle` so it stands in open air, clear of every other
//! feature, rather than on a grid that ignores the walls.
use crate::BlockId as B;
use glam::Vec3;

/// A giant mushroom: a flared stem with a skirt, a domed cap whose rim
/// droops, glowing gills under it.
#[derive(Clone, Copy, Debug)]
pub struct Shroom {
    pub x: f32,
    pub z: f32,
    /// Foot of the stem, on the ground.
    pub base: f32,
    /// Underside of the cap at the stem.
    pub cap: f32,
    pub r: f32,
    /// Luminous all over, else red with glowing gills.
    pub glow: bool,
}

/// How far the cap's rim drops below its middle, per metre of radius.
const DROOP: f32 = 0.22;
/// Dome height per metre of radius.
const DOME: f32 = 0.42;

impl Shroom {
    pub fn rim(&self) -> f32 {
        self.cap - DROOP * self.r
    }

    pub fn top(&self) -> f32 {
        self.cap + DOME * self.r
    }

    pub fn sample(&self, m: Vec3, v: f32) -> Option<B> {
        let d = (m.x - self.x).hypot(m.z - self.z);
        let fy = m.y - self.base;
        if fy < 0.0 || d >= self.r {
            return None;
        }
        // The stem widens over its last metre into the ground and stays at
        // least one voxel wide. A skirt hangs two thirds of the way up where
        // voxels are fine enough to draw it as a ring, not a crossbar.
        let stem = 0.55 + 0.45 * (1.0 - fy).max(0.0);
        let skirt = v <= 0.25 && (fy - (self.cap - self.base) * 0.66).abs() < 0.2 && d < 0.95;
        if m.y < self.cap + 0.3 && (d < stem.max(v * 0.75) || skirt) {
            return Some(B::MushroomStem);
        }
        let t = d / self.r;
        let under = self.cap - DROOP * self.r * t * t;
        let over = (self.cap + DOME * self.r * (1.0 - t * t).sqrt()).max(under + v);
        if m.y < under || m.y >= over {
            return None;
        }
        let gills = m.y < under + v.max(0.25) && t < 0.92;
        Some(if self.glow || gills { B::GlowCap } else { B::RedCap })
    }
}

/// A bracket fungus: a half-round shelf on one face of a square column,
/// thickest where it meets the stone. Too small to show at a metre.
#[derive(Clone, Copy, Debug)]
pub struct Shelf {
    /// The middle of the shelf's root, on the column face.
    pub x: f32,
    pub z: f32,
    /// Outward face normal, one of the four axis directions.
    pub nx: f32,
    pub nz: f32,
    /// Underside height.
    pub y: f32,
    /// Reach out from the face.
    pub s: f32,
    /// Luminous all through, else pale with gills that glow where the shelf
    /// is two voxels thick.
    pub glow: bool,
}

impl Shelf {
    pub fn sample(&self, m: Vec3, v: f32) -> Option<B> {
        if v > 0.5 {
            return None;
        }
        let (dx, dz) = (m.x - self.x, m.z - self.z);
        let (out, along) = (dx * self.nx + dz * self.nz, dz * self.nx - dx * self.nz);
        let t = out / self.s;
        if out < 0.0 || t * t + (along / (1.3 * self.s)).powi(2) >= 1.0 {
            return None;
        }
        let thick = (0.15 + 0.25 * (1.0 - t * t)).max(v);
        let gills = m.y < self.y + v && thick >= 2.0 * v;
        (m.y >= self.y && m.y < self.y + thick).then_some(if self.glow || gills { B::GlowCap } else { B::MushroomStem })
    }
}

/// The mushrooms and shelf fungi of one room.
#[derive(Clone, Debug, Default)]
pub struct Grove {
    pub shrooms: Vec<Shroom>,
    pub shelves: Vec<Shelf>,
}

impl Grove {
    pub fn sample(&self, m: Vec3, v: f32) -> Option<B> {
        self.shrooms.iter().find_map(|s| s.sample(m, v)).or_else(|| self.shelves.iter().find_map(|s| s.sample(m, v)))
    }
}

/// A glowing lantern hung on a chain that runs up into the rock.
#[derive(Clone, Copy, Debug)]
pub struct Lamp {
    pub x: f32,
    pub z: f32,
    pub y: f32,
    /// Where the chain ends, half a metre into the roof.
    pub top: f32,
}

impl Lamp {
    pub fn sample(&self, m: Vec3, v: f32) -> Option<B> {
        let column = (m.x / v).floor() == (self.x / v).floor() && (m.z / v).floor() == (self.z / v).floor();
        let size = (0.8 / v).round().max(1.0) * v;
        let body = (m.x - self.x).abs() < 0.5 && (m.z - self.z).abs() < 0.5 || column;
        if body && m.y >= self.y && m.y < self.y + size {
            return Some(B::GlowCap);
        }
        (column && m.y >= self.y + size && m.y < self.top).then_some(B::CopperBlock)
    }
}

/// An ice pipe between two heights, one end set in the floor or the roof.
#[derive(Clone, Copy, Debug)]
pub struct Pipe {
    pub x: f32,
    pub z: f32,
    pub y0: f32,
    pub y1: f32,
}

impl Pipe {
    pub const R: f32 = 0.7;

    pub fn sample(&self, m: Vec3, v: f32) -> Option<B> {
        let d = (m.x - self.x).hypot(m.z - self.z);
        (d < Self::R.max(v * 0.75) && m.y >= self.y0 && m.y < self.y1).then_some(B::Ice)
    }
}
