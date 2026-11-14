//! Built things inside the Undercrown's spaces: ramps, rock pillars, plank
//! bridges and huts, the counterweight, the bell tower, the waterfall, the
//! lava channel, shelves, mushrooms, ice pipes and pedestals, and the set
//! dressing in `furnish`.
//! Each one owns a box; outside it, it says nothing.
use super::furnish::{Fit, Here};
use super::grove::{Grove, Lamp, Pipe};
use super::{Mech, palette};
use crate::undercrown::plan::{PropKind, Style};
use crate::{BlockId as B, hash2, value_noise3};
use glam::Vec3;

#[derive(Clone, Debug)]
pub enum Kind {
    /// Surface apron and paired lamps, leaving the stair mouth open.
    Wellhead { floor: f32 },
    /// Solid under a plane rising from `y0` at the low end to `y1` at the
    /// high end of the box, along x or z. With `needs`, only once restored.
    Ramp { along_x: bool, y0: f32, y1: f32, style: Style, needs: Option<Mech> },
    /// A rock pillar or shelf: solid from the box floor to `top`.
    Pillar { top: f32, style: Style },
    /// Plank deck with rails along its long sides, top at `y`.
    Bridge { y: f32, needs: Option<Mech> },
    /// Plank houses on a platform at `y`: walls, a door gap, a roof.
    Huts { y: f32, seed: u32 },
    /// A counterweight hanging on a chain, and a static diagonal jib.
    Counterweight { floor: f32, top: f32 },
    /// Round tower: wall ring, spiral to a chamber floor, a bell above.
    /// `doors` are (angle, floor height) gaps in the wall.
    Tower { x: f32, z: f32, r: f32, y0: f32, chamber: f32, top: f32, doors: [(f32, f32); 3] },
    /// A water column into a plunge pool.
    Waterfall { pool: f32 },
    /// Lava in a trench; crusts to basalt once cooled.
    Lava { y: f32, needs: Mech },
    /// A raised water trough with stone sides, water to `y`.
    Channel { y: f32 },
    /// Shelf rows.
    Shelves { floor: f32, block: B },
    /// Giant mushrooms on a loose grid.
    Mushrooms { floor: f32, seed: u32 },
    /// Ice pillars hanging and standing.
    IcePipes { floor: f32, top: f32, seed: u32 },
    /// A stone pedestal under a prop.
    Pedestal { style: Style },
    /// Generator 292 prop model standing on `floor`; see `props`.
    Prop { kind: PropKind, style: Style, floor: Vec3 },
    /// A reliquary: dais, cache, offerings.
    Reliquary { floor: f32 },
    /// A rounded rock mass: an isle or a stack. Its footprint narrows from
    /// the box at the base to `flat` of it at `top`, which is level.
    Crag { top: f32, flat: f32, style: Style },
    /// Standing crystal on a loose grid.
    Shards { floor: f32, seed: u32 },
    /// An open plank hull along the box, its bottom at `y`, with a mast stump.
    Wreck { y: f32 },
    /// Furniture, fixtures and houses; see `furnish`.
    Fit(Fit),
    /// Generator 52 mushrooms, lanterns and ice pipes, each fitted to the
    /// room; see `grove` and `settle`.
    Grove(Grove),
    Lamps(Vec<Lamp>),
    Pipes(Vec<Pipe>),
}

#[derive(Clone, Debug)]
pub struct Feature {
    pub lo: Vec3,
    pub hi: Vec3,
    pub kind: Kind,
}

impl Kind {
    /// Dressing placed by `settle`, which gives way to the room.
    pub fn fitted(&self) -> bool {
        matches!(self, Kind::Grove(_) | Kind::Lamps(_) | Kind::Pipes(_))
    }
}

impl Feature {
    pub fn new(lo: Vec3, hi: Vec3, kind: Kind) -> Self {
        Self { lo, hi, kind }
    }

    pub fn bounds(&self) -> (Vec3, Vec3) {
        (self.lo, self.hi)
    }

    pub fn mech(&self) -> Option<Mech> {
        match self.kind {
            Kind::Ramp { needs, .. } | Kind::Bridge { needs, .. } => needs,
            Kind::Lava { needs, .. } => Some(needs),
            Kind::Fit(ref fit) => fit.mech(),
            _ => None,
        }
    }

    pub fn sample(&self, m: Vec3, v: f32, mech: Mech) -> Option<B> {
        if m.cmplt(self.lo).any() || m.cmpge(self.hi).any() {
            return None;
        }
        let (l, size) = (m - self.lo, self.hi - self.lo);
        match self.kind {
            Kind::Wellhead { floor } => {
                let dx = m.x - (self.lo.x + self.hi.x) * 0.5;
                let dz = m.z - (self.lo.z + self.hi.z) * 0.5;
                let r = dx.hypot(dz);
                if r > 6.5 { return None; }
                let post = (dx.abs() - 5.0).abs() < v.max(0.5) && dz.abs() < v.max(0.5);
                if post && m.y >= floor && m.y < floor + 4.0 {
                    return Some(if m.y >= floor + 3.0 { B::Crystal } else { B::StoneBricks });
                }
                if m.y >= floor { return Some(B::Air); }
                if r >= 3.5 { return Some(B::StoneBricks); }
                None
            }
            Kind::Ramp { along_x, y0, y1, style, needs } => {
                if needs.is_some_and(|n| !mech.has(n)) {
                    return None;
                }
                let t = if along_x { l.x / size.x } else { l.z / size.z };
                let top = y0 + (y1 - y0) * t;
                (m.y < ((top - y0.min(y1)) / v).ceil() * v + y0.min(y1)).then(|| palette(style)[2])
            }
            Kind::Pillar { top, style } => (m.y < top).then(|| {
                if m.y >= top - 1.0 { palette(style)[3] } else { super::stone(style, m, v) }
            }),
            Kind::Bridge { y, needs } => {
                if needs.is_some_and(|n| !mech.has(n)) {
                    return None;
                }
                let long_x = size.x >= size.z;
                let edge = if long_x { l.z.min(size.z - l.z) } else { l.x.min(size.x - l.x) };
                if m.y >= y - v && m.y < y {
                    return Some(B::Planks);
                }
                let along = if long_x { l.x } else { l.z };
                let post = (along.rem_euclid(3.0)) < v;
                (edge < v && m.y >= y && m.y < y + if post { 1.6 } else { 1.0 }).then_some(if post { B::Log } else { B::Planks })
            }
            Kind::Huts { y, seed } => {
                // Four-by-four huts on a 6 m grid across the platform.
                let (gx, gz) = ((l.x / 6.0).floor() as i32, (l.z / 6.0).floor() as i32);
                if hash2(gx, gz, seed) < 0.35 {
                    return None;
                }
                let (hx, hz) = (l.x - gx as f32 * 6.0 - 1.0, l.z - gz as f32 * 6.0 - 1.0);
                if !(0.0..4.0).contains(&hx) || !(0.0..4.0).contains(&hz) || m.y < y || m.y >= y + 4.0 {
                    return None;
                }
                let wall = hx < v || hx >= 4.0 - v || hz < v || hz >= 4.0 - v;
                let door = (hx - 2.0).abs() < 0.7 && hz < v && m.y < y + 2.2;
                let window = (hz - 2.0).abs() < 0.6 && (m.y - y - 1.6).abs() < 0.5;
                if m.y >= y + 3.0 {
                    return Some(if (m.y - y - 3.0) < (2.0 - (hx - 2.0).abs()).max(0.0) * 0.5 + v { B::Planks } else { return None });
                }
                (wall && !door && !window).then_some(if hx < v || hx >= 4.0 - v { B::Log } else { B::Planks })
            }
            Kind::Counterweight { floor, top } => {
                let c = (self.lo + self.hi) * 0.5;
                let (dx, dz) = (m.x - c.x, m.z - c.z);
                // The weight: a basalt block banded with copper.
                let (w0, w1) = (floor + 7.0, floor + 14.0);
                if dx.abs() < 2.5 && dz.abs() < 2.5 && m.y >= w0 && m.y < w1 {
                    return Some(if (m.y - w0).rem_euclid(3.5) < v { B::CopperBlock } else { B::Basalt });
                }
                // The chain to the ceiling.
                if dx.abs() < v.max(0.5) && dz.abs() < v.max(0.5) && m.y >= w1 && m.y < top {
                    return Some(if ((m.y / v) as i32).rem_euclid(2) == 0 { B::SilverBlock } else { B::CopperBlock });
                }
                // The jib: a static diagonal beam from a floor corner to the ceiling.
                let t = (l.x / size.x).clamp(0.0, 1.0);
                let jy = floor + 1.0 + t * (top - floor - 3.0);
                let jz = self.lo.z + 2.0;
                ((m.y - jy).abs() < 0.9 && (m.z - jz).abs() < 0.9).then_some(B::Basalt)
            }
            Kind::Tower { x, z, r, y0, chamber, top, doors } => {
                let (dx, dz) = (m.x - x, m.z - z);
                let d = dx.hypot(dz);
                if d >= r + 1.0 || m.y < y0 || m.y >= top + 1.0 {
                    return None;
                }
                let angle = dz.atan2(dx);
                let gap = doors.iter().any(|&(a, fy)| {
                    let da = (angle - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
                    (da * r).abs() < 1.2 && m.y >= fy && m.y < fy + 2.8
                });
                let arch = m.y >= chamber + 1.5 && m.y < chamber + 4.5 && ((angle * 4.0 / std::f32::consts::PI).rem_euclid(2.0) < 1.0);
                if d >= r {
                    return (!gap && !arch).then_some(if m.y >= top { B::StoneBricks } else { B::Limestone });
                }
                // Chamber floor, open over the arriving treads; the stair below.
                let stair = super::Spiral::new(y0, r - 3.0, r, v);
                if stair.landing(dx, dz, m.y, chamber) || (m.y < chamber && stair.tread(dx, dz, m.y)) {
                    return Some(B::StoneBricks);
                }
                // The bell, hung under the roof.
                let bell_top = top - 0.5;
                let bell = d < 2.4 * ((bell_top - m.y) / 3.0).clamp(0.0, 1.0).sqrt() && m.y >= bell_top - 3.0 && m.y < bell_top;
                if bell && m.y >= chamber + 2.5 {
                    return Some(if d > 1.6 || m.y >= bell_top - v { B::CopperBlock } else { B::Air });
                }
                (m.y >= top - v).then_some(B::StoneBricks).or(Some(B::Air))
            }
            Kind::Waterfall { pool } => Some(if m.y < pool + 1.0 || (l.x - size.x * 0.5).abs() < 1.0 { B::Water } else { return None }),
            Kind::Lava { y, needs } => (m.y < y).then_some(if mech.has(needs) {
                if m.y >= y - v { B::Basalt } else { B::Lava }
            } else {
                B::Lava
            }),
            Kind::Channel { y } => {
                let edge = l.z < v || l.z >= size.z - v;
                Some(if edge || m.y < y - 1.0 { palette(Style::Canal)[2] } else { B::Water })
            }
            Kind::Shelves { floor, block } => {
                let long_x = size.x >= size.z;
                let (along, across) = if long_x { (l.x, l.z) } else { (l.z, l.x) };
                let row = (across.rem_euclid(3.0)) < v.max(0.6) && across > 1.5 && across < (if long_x { size.z } else { size.x }) - 1.5;
                let aisle = along < 1.5 || along > (if long_x { size.x } else { size.z }) - 1.5 || (along.rem_euclid(8.0)) < 1.4;
                (row && !aisle && m.y >= floor && m.y < floor + 3.5).then_some(if (m.y - floor).rem_euclid(1.2) < v { B::Planks } else { block })
            }
            Kind::Mushrooms { floor, seed } => {
                let (gx, gz) = ((l.x / 7.0).floor() as i32, (l.z / 7.0).floor() as i32);
                let x = hash2(gx, gz, seed);
                if x < 0.4 {
                    return None;
                }
                let (cx, cz) = (gx as f32 * 7.0 + 3.5, gz as f32 * 7.0 + 3.5);
                let d = (l.x - cx).hypot(l.z - cz);
                let cap = floor + 3.0 + x * 4.0;
                if d < 0.8 && m.y >= floor && m.y < cap {
                    return Some(B::MushroomStem);
                }
                let dome = cap + 1.2 - (d / 3.0).powi(2) * 1.2;
                (d < 3.0 && m.y >= cap && m.y < dome).then_some(if m.y < cap + v { B::GlowCap } else { B::RedCap })
            }
            Kind::IcePipes { floor, top, seed } => {
                let (gx, gz) = ((l.x / 3.0).floor() as i32, (l.z / 3.0).floor() as i32);
                let x = hash2(gx, gz, seed);
                let (cx, cz) = (gx as f32 * 3.0 + 1.5, gz as f32 * 3.0 + 1.5);
                let d = (l.x - cx).hypot(l.z - cz);
                let stand = x > 0.75 && m.y < floor + 2.0 + x * 6.0;
                let hang = x < 0.3 && m.y > top - 1.0 - x * 8.0;
                (d < 0.7 && (stand || hang)).then_some(B::Ice)
            }
            Kind::Pedestal { style } => Some(palette(style)[2]),
            Kind::Prop { kind, style, floor } => super::props::sample(kind, style, floor, m, v),
            Kind::Reliquary { floor } => {
                let c = (self.lo + self.hi) * 0.5;
                let d = (m.x - c.x).hypot(m.z - c.z);
                let fy = m.y - floor;
                if d < 3.0 && fy < 0.5 {
                    return Some(B::SilverBlock);
                }
                if d < 0.8 && fy < 0.5 + v {
                    return Some(B::SurveyCache);
                }
                let ring = (d - 2.2).abs() < 0.5 && (m.z - c.z).atan2(m.x - c.x).rem_euclid(std::f32::consts::FRAC_PI_2) < 0.4;
                (ring && fy < 0.5 + 2.0 * v).then_some(if fy < 0.5 + v { B::StarGlass } else { B::GoldBlock })
            }
            Kind::Crag { top, flat, style } => {
                if m.y >= top {
                    return None;
                }
                let r = (l.x / size.x * 2.0 - 1.0).hypot(l.z / size.z * 2.0 - 1.0);
                let fy = ((m.y - self.lo.y) / (top - self.lo.y)).clamp(0.0, 1.0);
                let n = (value_noise3(m.x * 0.21, m.y * 0.13, m.z * 0.21, 7321) - 0.5) * 0.3;
                (r + n < (flat + (1.0 - flat) * (1.0 - fy)) * 0.85).then(|| if m.y >= top - 1.0 { palette(style)[3] } else { super::wilds::rock(style, m) })
            }
            Kind::Shards { floor, seed } => {
                let (gx, gz) = ((l.x / 4.0).floor() as i32, (l.z / 4.0).floor() as i32);
                let x = hash2(gx, gz, seed);
                if x < 0.45 {
                    return None;
                }
                let cx = gx as f32 * 4.0 + 1.2 + hash2(gx, gz, seed ^ 0x55) * 1.6;
                let cz = gz as f32 * 4.0 + 1.2 + hash2(gz, gx, seed ^ 0xaa) * 1.6;
                let tall = 1.5 + (x - 0.45) * 9.0;
                let fy = m.y - floor;
                let d = (l.x - cx).hypot(l.z - cz);
                (fy >= 0.0 && fy < tall && d < (0.9 * (1.0 - fy / tall)).max(v * 0.71)).then_some(B::Crystal)
            }
            Kind::Wreck { y } => {
                let long_x = size.x >= size.z;
                let (along, across, len, beam) = if long_x { (l.x, l.z - size.z * 0.5, size.x, size.z * 0.5) } else { (l.z, l.x - size.x * 0.5, size.z, size.x * 0.5) };
                let w = beam * (1.0 - (2.0 * along / len - 1.0).powi(2)).max(0.0).sqrt();
                let fy = m.y - y;
                if (along - len * 0.45).abs() < v.max(0.5) * 0.5 && across.abs() < v.max(0.5) * 0.5 && (0.0..5.0).contains(&fy) {
                    return Some(B::Log);
                }
                if across.abs() >= w || fy < 0.0 || fy >= 2.0 {
                    return None;
                }
                if fy < v {
                    return Some(B::Planks);
                }
                // The sides, with stretches stove in.
                let side = across.abs() >= w - v.max(0.6);
                (side && hash2((along / 2.0).floor() as i32, 3, 7331) > 0.25).then_some(if fy >= 2.0 - v { B::Log } else { B::Planks })
            }
            Kind::Fit(ref fit) => fit.sample(&Here { lo: self.lo, hi: self.hi, m, v }, mech),
            Kind::Grove(ref grove) => grove.sample(m, v),
            Kind::Lamps(ref all) => all.iter().find_map(|s| s.sample(m, v)),
            Kind::Pipes(ref all) => all.iter().find_map(|s| s.sample(m, v)),
        }
    }
}
