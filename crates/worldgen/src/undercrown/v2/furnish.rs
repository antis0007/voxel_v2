//! Set dressing for the rooms: kitchen tables and a hearth, chapel pews and
//! an altar, the courthouse dais and rail, sunken baths, seed cabinets. The
//! fixtures (columns, lamps, braziers, furnaces, lava and ice) are in
//! `fixtures`, the borough houses in `houses`. Heights snap to whole voxels
//! so every scale gets the same furniture, only finer.
use super::{Mech, palette};
use crate::undercrown::plan::Style;
use crate::{BlockId as B, hash3};
use glam::Vec3;

/// Which side of a feature's box is its back (against the wall).
#[derive(Clone, Copy, Debug)]
pub enum Back {
    LoX,
    HiX,
    LoZ,
    HiZ,
}

#[derive(Clone, Debug)]
pub enum Fit {
    /// A long table with benches and table lamps, along the long axis.
    Tables { floor: f32 },
    /// A brick hearth with a fire in it, its back to a wall.
    Hearth { floor: f32, back: Back },
    /// Pew rows along x with a central aisle.
    Pews { floor: f32 },
    /// A stepped dais with an altar and two candles.
    Altar { floor: f32, style: Style },
    /// A raised judges' dais, its bench at the back, a rail along its front.
    Dais { floor: f32, style: Style, back: Back },
    /// A sunken pool with a stone rim, `depth` below the floor.
    Bath { floor: f32, depth: f32 },
    /// Wall cabinets of jars, along the long axis.
    Cabinets { floor: f32, block: B },
    /// See `fixtures`.
    Column { floor: f32, top: f32, style: Style },
    Lamp { floor: f32, h: f32 },
    Brazier { floor: f32, block: B },
    Furnaces { floor: f32, back: Back },
    Lavafall { needs: Mech },
    Lanterns { low: f32, seed: u32 },
    Organ { floor: f32, seed: u32 },
    IceSheet { back: Back },
    /// See `houses`.
    Houses { floor: f32, style: Style, seed: u32, passages: [f32; 2] },
}

/// A point inside a feature's box.
pub(super) struct Here {
    pub lo: Vec3,
    pub hi: Vec3,
    pub m: Vec3,
    pub v: f32,
}

impl Here {
    pub fn l(&self) -> Vec3 {
        self.m - self.lo
    }
    pub fn size(&self) -> Vec3 {
        self.hi - self.lo
    }
    /// (along, across, along length, across length) on the box's long axis.
    pub fn long(&self) -> (f32, f32, f32, f32) {
        let (l, s) = (self.l(), self.size());
        if s.x >= s.z { (l.x, l.z, s.x, s.z) } else { (l.z, l.x, s.z, s.x) }
    }
    /// (depth from the back, along the back, depth, width).
    pub fn rel_to_back(&self, back: Back) -> (f32, f32, f32, f32) {
        let (l, s) = (self.l(), self.size());
        match back {
            Back::LoX => (l.x, l.z, s.x, s.z),
            Back::HiX => (s.x - l.x, l.z, s.x, s.z),
            Back::LoZ => (l.z, l.x, s.z, s.x),
            Back::HiZ => (s.z - l.z, l.x, s.z, s.x),
        }
    }
    /// True in the one voxel column holding the xz point `(x, z)`.
    pub fn column(&self, x: f32, z: f32) -> bool {
        (self.m.x / self.v).floor() == (x / self.v).floor() && (self.m.z / self.v).floor() == (z / self.v).floor()
    }
    /// True in the one voxel column at the box's centre.
    pub fn centre(&self) -> bool {
        let c = (self.lo + self.hi) * 0.5;
        self.column(c.x, c.z)
    }
    /// A height in whole voxels, at least one.
    pub fn snap(&self, h: f32) -> f32 {
        (h / self.v).round().max(1.0) * self.v
    }
}

impl Fit {
    pub fn mech(&self) -> Option<Mech> {
        match *self {
            Fit::Lavafall { needs } => Some(needs),
            _ => None,
        }
    }

    pub(super) fn sample(&self, at: &Here, mech: Mech) -> Option<B> {
        match *self {
            Fit::Tables { floor } => tables(at, floor),
            Fit::Hearth { floor, back } => hearth(at, floor, back),
            Fit::Pews { floor } => pews(at, floor),
            Fit::Altar { floor, style } => altar(at, floor, style),
            Fit::Dais { floor, style, back } => dais(at, floor, style, back),
            Fit::Bath { floor, depth } => bath(at, floor, depth),
            Fit::Cabinets { floor, block } => cabinets(at, floor, block),
            Fit::Column { floor, top, style } => super::fixtures::column(at, floor, top, style),
            Fit::Lamp { floor, h } => super::fixtures::lamp(at, floor, h),
            Fit::Brazier { floor, block } => super::fixtures::brazier(at, floor, block),
            Fit::Furnaces { floor, back } => super::fixtures::furnaces(at, floor, back),
            Fit::Lavafall { needs } => super::fixtures::lavafall(at, mech.has(needs)),
            Fit::Lanterns { low, seed } => super::fixtures::lanterns(at, low, seed),
            Fit::Organ { floor, seed } => super::fixtures::organ(at, floor, seed),
            Fit::IceSheet { back } => super::fixtures::ice_sheet(at, back),
            Fit::Houses { floor, style, seed, passages } => super::houses::houses(at, floor, style, seed, passages),
        }
    }
}

fn tables(at: &Here, floor: f32) -> Option<B> {
    let (along, across, la, lc) = at.long();
    let fy = at.m.y - floor;
    let off = (across - lc * 0.5).abs();
    if along < 0.5 || along > la - 0.5 {
        return None;
    }
    let top = at.snap(0.9);
    if off < 0.6 {
        let leg = along < 0.5 + at.v.max(0.3) || along > la - 0.5 - at.v.max(0.3);
        if fy < top && (fy >= top - at.v.max(0.2) || leg) {
            return Some(if fy >= top - at.v.max(0.2) { B::Planks } else { B::Log });
        }
        // A lamp every three metres down the middle of the table.
        let c = (at.lo + at.hi) * 0.5;
        let lamp_at = |a: f32| if at.size().x >= at.size().z { at.column(at.lo.x + a, c.z) } else { at.column(c.x, at.lo.z + a) };
        let n = ((la - 1.0) / 3.0).floor().max(1.0) as i32;
        let lit = (0..n).any(|i| lamp_at(0.5 + (la - 1.0) * (i as f32 + 0.5) / n as f32));
        return (lit && fy >= top && fy < top + at.v).then_some(B::Torch);
    }
    ((1.0..1.6).contains(&off) && fy < at.snap(0.5)).then_some(B::Planks)
}

fn hearth(at: &Here, floor: f32, back: Back) -> Option<B> {
    let (dep, along, dl, wl) = at.rel_to_back(back);
    let fy = at.m.y - floor;
    let off = (along - wl * 0.5).abs();
    let firebox = off < 0.8 && dep >= 0.5 && fy < at.snap(1.5);
    if firebox {
        // The fire: one torch on the hearth floor, mid-box.
        let lit = fy < at.v && off < at.v * 0.5 + 0.01 && (dep - 0.5) < at.v;
        return lit.then_some(B::Torch);
    }
    let mantle = at.snap(2.0);
    if fy < mantle {
        return Some(if fy >= mantle - at.v.max(0.3) { B::StoneBricks } else { B::CokeOvenBrick });
    }
    // The hood narrows to a chimney into the rock.
    (dep < dl * 0.6 && off < 0.9 + (1.0 - (fy - mantle) / 1.5).max(0.0) * (wl * 0.5 - 0.9)).then_some(B::CokeOvenBrick)
}

fn pews(at: &Here, floor: f32) -> Option<B> {
    let (l, s) = (at.l(), at.size());
    if (l.x - s.x * 0.5).abs() < 1.0 || l.x < 0.3 || l.x > s.x - 0.3 {
        return None;
    }
    let fy = at.m.y - floor;
    let r = l.z.rem_euclid(3.0);
    if r < 0.6 && fy < at.snap(0.5) {
        return Some(B::Planks);
    }
    (r >= 0.6 && r < 0.6 + at.v.max(0.25) && fy < at.snap(1.1)).then_some(B::Log)
}

fn altar(at: &Here, floor: f32, style: Style) -> Option<B> {
    let (l, s) = (at.l(), at.size());
    let [wall, _, trim, _] = palette(style);
    let fy = at.m.y - floor;
    let step = at.snap(0.4);
    let inset = l.x.min(s.x - l.x).min(l.z).min(s.z - l.z);
    let dais = if inset < 0.8 { step } else { step * 2.0 };
    if fy < dais {
        return Some(trim);
    }
    let c = (at.lo + at.hi) * 0.5;
    let (dx, dz) = ((at.m.x - c.x).abs(), (at.m.z - c.z).abs());
    let top = dais + at.snap(1.0);
    // Wide enough that a candle snapped to the voxel grid always stands on it.
    if dx < 1.4 && dz < 0.6 && fy < top {
        return Some(if fy >= top - at.v.max(0.2) { B::GoldBlock } else { wall });
    }
    // Candles at the altar's two ends.
    let candle = at.column(c.x - 0.8, c.z) || at.column(c.x + 0.8, c.z);
    (candle && fy >= top && fy < top + at.v).then_some(B::Torch)
}

fn dais(at: &Here, floor: f32, style: Style, back: Back) -> Option<B> {
    let (dep, along, dl, wl) = at.rel_to_back(back);
    let [wall, _, trim, _] = palette(style);
    let fy = at.m.y - floor;
    let h = at.snap(1.0);
    let gap = (along - wl * 0.5).abs() < 1.0;
    if fy < h {
        // A step up at the gap in the rail.
        if gap && dep > dl - 1.0 && fy >= at.snap(0.5) {
            return None;
        }
        return Some(if fy >= h - at.v.max(0.25) { trim } else { wall });
    }
    let fy = fy - h;
    // The bench along the back, the high seat in the middle.
    if dep < 1.2 && (fy < at.snap(1.0) || ((along - wl * 0.5).abs() < 0.8 && dep < 0.6 && fy < at.snap(2.2))) {
        return Some(if dep < 0.6 { B::Log } else { B::Planks });
    }
    let rail = dep >= dl - at.v.max(0.3) && !gap && fy < at.snap(1.0);
    rail.then_some(if along.rem_euclid(1.5) < at.v.max(0.3) { B::Log } else { B::Planks })
}

fn bath(at: &Here, floor: f32, depth: f32) -> Option<B> {
    let (l, s) = (at.l(), at.size());
    let edge = l.x.min(s.x - l.x).min(l.z).min(s.z - l.z);
    let fy = at.m.y - floor;
    if fy < -depth {
        return None;
    }
    let rim = at.snap(0.5);
    if edge < at.v.max(0.5) {
        return (fy < rim).then_some(palette(Style::Canal)[2]);
    }
    // Steps down at one end.
    if l.x < 1.5 && fy < -depth + (1.5 - l.x) / 1.5 * depth - at.v * 0.5 {
        return Some(B::Limestone);
    }
    (fy < 0.0).then_some(B::Water)
}

fn cabinets(at: &Here, floor: f32, block: B) -> Option<B> {
    let (along, ..) = at.long();
    let fy = at.m.y - floor;
    let top = at.snap(2.4);
    if fy >= top {
        return None;
    }
    if fy >= top - at.v.max(0.2) || along.rem_euclid(1.6) < at.v.min(0.2) || (fy.rem_euclid(0.8)) < at.v.min(0.2) {
        return Some(B::Planks);
    }
    let p = (at.m / at.v).floor().as_ivec3();
    Some(if hash3(p.x, p.y, p.z, 4901) > 0.45 { block } else { B::Planks })
}
