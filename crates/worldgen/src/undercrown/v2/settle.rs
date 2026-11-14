//! Generator 52: fits the grid-placed mushrooms, lanterns and ice pipes to
//! the finished layout. Before 52 they were drawn wherever their grid fell,
//! over everything else in their box: caps cut through columns, altars and
//! the vault, stems stood in cave walls, lantern chains ran through caps.
//! Now each candidate is probed against the room at a quarter metre in both
//! mechanism extremes; it must stand in air, clear of every other feature,
//! the props and the passages, or it shrinks, moves, or is left out.
use super::feature::Kind;
use super::furnish::Fit;
use super::grove::{Grove, Lamp, Pipe, Shelf, Shroom};
use super::shape::At;
use super::{Layout, Mech};
use crate::{BlockId as B, hash2, hash3};
use glam::Vec3;

/// First generator that fits set dressing to the room.
pub const VERSION: u32 = crate::gates::GIANTS_AND_RUINS_GENERATOR;
/// Scale the room is probed at: the finest, where mouldings and lights
/// stand out furthest.
const PV: f32 = 0.25;
/// Spacing between probe points, metres.
const STEP: f32 = 0.5;
/// Air kept round each piece, metres.
const GAP: f32 = 0.4;
/// Mushroom spacing the grid aims for, metres.
const CELL: f32 = 7.0;

/// Every point of a disc, ring by ring, until `f` refuses one.
fn disc(x: f32, z: f32, r: f32, mut f: impl FnMut(f32, f32) -> bool) -> bool {
    if !f(x, z) {
        return false;
    }
    let rings = (r / STEP).ceil() as i32;
    (1..=rings).all(|k| {
        let rho = (k as f32 * STEP).min(r);
        let n = (std::f32::consts::TAU * rho / STEP).ceil() as i32;
        (0..n).all(|j| {
            let a = j as f32 / n as f32 * std::f32::consts::TAU;
            f(x + rho * a.cos(), z + rho * a.sin())
        })
    })
}

/// Heights from `lo` to `hi` at the probe spacing, `hi` included.
fn heights(lo: f32, hi: f32) -> impl Iterator<Item = f32> {
    let n = ((hi - lo) / STEP).ceil().max(0.0) as i32;
    (0..=n).map(move |k| (lo + k as f32 * STEP).min(hi))
}

impl Layout {
    /// Air in both mechanism states, with no feature but `skip` there.
    fn open(&self, skip: usize, p: Vec3) -> bool {
        self.open_but(&[skip], p)
    }

    fn open_but(&self, skip: &[usize], p: Vec3) -> bool {
        let cell = self.index.at(p);
        self.room_air(p) && [Mech::default(), Mech::ALL].into_iter().all(|mech| cell.features.iter().all(|&fi| skip.contains(&(fi as usize)) || self.features[fi as usize].sample(p, PV, mech).is_none_or(|b| b == B::Air)))
    }

    /// Air in the room itself, whatever furniture stands there.
    fn room_air(&self, p: Vec3) -> bool {
        let cell = self.index.at(p);
        self.contains(p) && [Mech::default(), Mech::ALL].into_iter().all(|mech| self.ground(cell, p, PV, mech, true) == Some(B::Air))
    }

    /// Rock or a solid floor with no feature on it; natural terrain counts.
    fn rock(&self, skip: usize, p: Vec3) -> bool {
        let cell = self.index.at(p);
        let mech = Mech::default();
        self.ground(cell, p, PV, mech, true).is_none_or(|b| b.is_solid()) && cell.features.iter().all(|&fi| fi as usize == skip || self.features[fi as usize].sample(p, PV, mech).is_none())
    }

    fn passage(&self, p: Vec3) -> bool {
        self.index.at(p).tunnels.iter().any(|&ti| self.tunnels[ti as usize].at(p) == At::Void)
    }

    fn near_prop(&self, x: f32, z: f32, reach: f32, lo: f32, hi: f32) -> bool {
        self.clear.iter().any(|p| (p.x - x).hypot(p.z - z) < reach && p.y > lo - 1.0 && p.y < hi + 1.0)
    }

    /// The ground under (`x`, `z`) within 1.5 m of `floor`.
    fn foot(&self, skip: usize, x: f32, z: f32, floor: f32) -> Option<f32> {
        let mut y = floor + 1.5 - PV * 0.5;
        while y > floor - 1.5 {
            let p = Vec3::new(x, y, z);
            if !self.open(skip, p) {
                return self.rock(skip, p).then_some(y + PV * 0.5);
            }
            y -= PV;
        }
        None
    }

    /// Where a hanging thing meets the roof above `from`: its whole width in
    /// air up to there, and rock (not a feature) where it stops, below
    /// `ceiling`.
    fn roof(&self, skip: usize, x: f32, z: f32, r: f32, from: f32, ceiling: f32) -> Option<f32> {
        let mut y = from;
        while y < ceiling {
            let mut hit = Vec3::ZERO;
            let clear = disc(x, z, r, |px, pz| {
                hit = Vec3::new(px, y, pz);
                self.open(skip, hit)
            });
            if !clear {
                return self.rock(skip, hit).then_some(y);
            }
            y += PV;
        }
        None
    }

    fn shroom_fits(&self, skip: usize, (lo, hi): (Vec3, Vec3), placed: &[Shroom], s: &Shroom) -> bool {
        let reach = s.r + GAP;
        if s.x - reach < lo.x || s.x + reach >= hi.x || s.z - reach < lo.z || s.z + reach >= hi.z || s.top() + GAP >= hi.y {
            return false;
        }
        if self.near_prop(s.x, s.z, s.r + 1.0, s.base, s.top()) || placed.iter().any(|o| (o.x - s.x).hypot(o.z - s.z) < o.r + s.r + GAP) {
            return false;
        }
        // Stem first, the cheapest refusal; it keeps out of passage mouths.
        let stem = heights(s.base + 0.25, s.cap).all(|y| disc(s.x, s.z, 1.15, |x, z| self.open(skip, Vec3::new(x, y, z))));
        let mouth = || disc(s.x, s.z, 2.5, |x, z| !self.passage(Vec3::new(x, s.base + 1.0, z)));
        // The cap, and two metres under its rim clear of the floor, though
        // pews and tables may stand there.
        let cap = || heights(s.rim() - GAP, s.top() + GAP).all(|y| disc(s.x, s.z, reach, |x, z| self.open(skip, Vec3::new(x, y, z))));
        let head = || heights(s.rim() - 2.0, s.rim() - GAP).all(|y| disc(s.x, s.z, s.r, |x, z| self.room_air(Vec3::new(x, y, z))));
        stem && mouth() && cap() && head()
    }

    /// Mushrooms on a grid spread evenly over the box, each moved round its
    /// cell, shrunk or left out to fit; shelf fungi on the columns.
    fn grove(&self, skip: usize, floor: f32, seed: u32) -> Grove {
        let (lo, hi) = (self.features[skip].lo, self.features[skip].hi);
        let mut shrooms = Vec::new();
        let (nx, nz) = (((hi.x - lo.x) / CELL).round().max(1.0) as i32, ((hi.z - lo.z) / CELL).round().max(1.0) as i32);
        let (wx, wz) = ((hi.x - lo.x) / nx as f32, (hi.z - lo.z) / nz as f32);
        for gz in 0..nz {
            for gx in 0..nx {
                let x = hash2(gx, gz, seed);
                if x < 0.3 {
                    continue;
                }
                let size = hash2(gx, gz, seed ^ 0x5f52);
                let (cx, cz) = (lo.x + (gx as f32 + 0.5) * wx, lo.z + (gz as f32 + 0.5) * wz);
                let r0 = 2.2 + 1.3 * size;
                // The cell's middle, then rings of spots round it out to 3 m.
                let spots = (0..25).map(|k| {
                    if k == 0 {
                        return (0.0, 0.0);
                    }
                    let (ring, a) = ((k - 1) / 8 + 1, ((k - 1) % 8) as f32 * std::f32::consts::FRAC_PI_4);
                    (ring as f32 * a.cos(), ring as f32 * a.sin())
                });
                'fit: for (ox, oz) in spots {
                    let Some(base) = self.foot(skip, cx + ox, cz + oz, floor) else { continue };
                    for r in [r0, r0 * 0.8, r0 * 0.62, 1.2] {
                        let low = base + 2.4 + 0.22 * r;
                        for cap in [low + (x - 0.3) * 4.0, low] {
                            let s = Shroom { x: cx + ox, z: cz + oz, base, cap, r, glow: size > 0.55 };
                            if self.shroom_fits(skip, (lo, hi), &shrooms, &s) {
                                shrooms.push(s);
                                break 'fit;
                            }
                        }
                    }
                }
            }
        }
        let shelves = self.shelves(skip, &shrooms, seed);
        Grove { shrooms, shelves }
    }

    /// Shelf fungi in tiers above head height on the faces of the columns
    /// standing in the box.
    fn shelves(&self, skip: usize, shrooms: &[Shroom], seed: u32) -> Vec<Shelf> {
        let (lo, hi) = (self.features[skip].lo, self.features[skip].hi);
        let mut out = Vec::new();
        for (ci, f) in self.features.iter().enumerate() {
            let Kind::Fit(Fit::Column { floor, top, .. }) = f.kind else { continue };
            let c = (f.lo + f.hi) * 0.5;
            if c.cmplt(lo).any() || c.cmpge(hi).any() {
                continue;
            }
            for (k, (nx, nz)) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)].into_iter().enumerate() {
                let roll = hash3(ci as i32, k as i32, 0, seed);
                if roll < 0.5 {
                    continue;
                }
                let shift = (hash3(ci as i32, k as i32, 1, seed) - 0.5) * 0.5;
                let glow = hash3(ci as i32, k as i32, 2, seed) < 0.3;
                let mut y = floor + 2.3 + (roll - 0.5) * 4.0;
                for tier in 0..1 + (roll > 0.8) as i32 {
                    let s = 0.7 - tier as f32 * 0.2;
                    let (x, z) = (c.x + nx * 0.6 - nz * shift, c.z + nz * 0.6 + nx * shift);
                    let shelf = Shelf { x, z, nx, nz, y, s, glow };
                    // The shelf's own volume and a gap round it, in air and
                    // clear of this room's mushrooms.
                    let open = |p: Vec3| self.open_but(&[skip, ci], p) && shrooms.iter().all(|m| m.sample(p, PV).is_none());
                    let fits = y + 0.6 < top - 0.6
                        && heights(y - 0.25, y + 0.6).all(|h| (1..=4).all(|o| (-3..=3).all(|a| open(Vec3::new(x + nx * o as f32 * 0.25 - nz * a as f32 * 0.3 * s, h, z + nz * o as f32 * 0.25 + nx * a as f32 * 0.3 * s)))));
                    if fits {
                        out.push(shelf);
                    }
                    y += 0.9;
                }
            }
        }
        out
    }

    /// The old lantern grid, keeping lanterns whose body hangs in air and
    /// whose chain reaches the roof without touching anything.
    fn lamps(&self, skip: usize, low: f32, seed: u32) -> Vec<Lamp> {
        let (lo, hi) = (self.features[skip].lo, self.features[skip].hi);
        let (nx, nz) = (((hi.x - lo.x) / 4.0).ceil() as i32, ((hi.z - lo.z) / 4.0).ceil() as i32);
        let mut out = Vec::new();
        for gz in 0..nz {
            for gx in 0..nx {
                let roll = hash2(gx, gz, seed);
                if roll < 0.45 {
                    continue;
                }
                let (x, z) = (lo.x + gx as f32 * 4.0 + 2.0, lo.z + gz as f32 * 4.0 + 2.0);
                let y = low + (roll - 0.45) * 4.0;
                let body = heights(y - GAP, y + 0.8 + GAP).all(|h| disc(x, z, 0.75 + GAP, |px, pz| self.open(skip, Vec3::new(px, h, pz))));
                if !body || self.near_prop(x, z, 1.5, y, y + 1.0) {
                    continue;
                }
                if let Some(top) = self.roof(skip, x, z, 0.6, y + 0.8, hi.y) {
                    out.push(Lamp { x, z, y, top: top + 0.5 });
                }
            }
        }
        out
    }

    /// The old ice pipe grid, each pipe tried at a few spots round its cell:
    /// standing pipes need ground under them, hanging ones a roof over them
    /// (and keep their old length below wherever that roof is), and both
    /// need air the whole way.
    fn pipes(&self, skip: usize, floor: f32, top: f32, seed: u32) -> Vec<Pipe> {
        let (lo, hi) = (self.features[skip].lo, self.features[skip].hi);
        let (nx, nz) = (((hi.x - lo.x) / 3.0).ceil() as i32, ((hi.z - lo.z) / 3.0).ceil() as i32);
        let reach = Pipe::R + GAP;
        let column = |x: f32, z: f32, y0: f32, y1: f32| heights(y0, y1).all(|y| disc(x, z, reach, |px, pz| self.open(skip, Vec3::new(px, y, pz))));
        let mut out = Vec::new();
        for gz in 0..nz {
            for gx in 0..nx {
                let x = hash2(gx, gz, seed);
                if (0.3..=0.75).contains(&x) {
                    continue;
                }
                let (cx, cz) = (lo.x + gx as f32 * 3.0 + 1.5, lo.z + gz as f32 * 3.0 + 1.5);
                let pipe = [(0.0, 0.0), (0.75, 0.0), (-0.75, 0.0), (0.0, 0.75), (0.0, -0.75)].into_iter().find_map(|(ox, oz)| {
                    let (px, pz) = (cx + ox, cz + oz);
                    if px - reach < lo.x || px + reach >= hi.x || pz - reach < lo.z || pz + reach >= hi.z || self.near_prop(px, pz, 1.5, floor, top) {
                        return None;
                    }
                    if x > 0.75 {
                        let base = self.foot(skip, px, pz, floor)?;
                        let y1 = base + 2.0 + x * 6.0;
                        let mouth = disc(px, pz, 2.0, |qx, qz| !self.passage(Vec3::new(qx, base + 1.0, qz)));
                        (mouth && y1 + GAP < hi.y && column(px, pz, base + 0.25, y1 + GAP)).then_some(Pipe { x: px, z: pz, y0: base - 0.5, y1 })
                    } else {
                        // The roof straight above; the pipe's last metre may
                        // meet it where it slopes.
                        let roof = self.roof(skip, px, pz, 0.0, floor + 2.5, hi.y)?;
                        let y0 = (roof - 1.0 - x * 8.0).max(floor + 2.5);
                        (roof - y0 >= 1.5 && column(px, pz, y0 - GAP, roof - 1.0)).then_some(Pipe { x: px, z: pz, y0, y1: roof + 0.5 })
                    }
                });
                out.extend(pipe);
            }
        }
        out
    }
}

/// Swaps each grid-placed piece of dressing for the pieces that fit:
/// mushrooms first, then the lanterns among them, then the ice pipes.
pub(super) fn settle(l: &mut Layout) {
    for pass in 0..3 {
        for i in 0..l.features.len() {
            let kind = match (pass, &l.features[i].kind) {
                (0, &Kind::Mushrooms { floor, seed }) => Kind::Grove(l.grove(i, floor, seed)),
                (1, &Kind::Fit(Fit::Lanterns { low, seed })) => Kind::Lamps(l.lamps(i, low, seed)),
                (2, &Kind::IcePipes { floor, top, seed }) => Kind::Pipes(l.pipes(i, floor, top, seed)),
                _ => continue,
            };
            l.features[i].kind = kind;
        }
    }
}
