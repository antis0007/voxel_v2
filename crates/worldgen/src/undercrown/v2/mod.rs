//! Generator 32 Undercrown geometry. The same districts, rooms and props as
//! generator 19 (save identity is unchanged), authored as real spaces: a
//! well shaft, vaulted halls, a noise-carved chasm with ledges and a bridge
//! village on a rock pillar, a bell tower, an observatory, a flooded borough
//! street with house fronts, a foundry with a lava channel, and a sealed
//! Glass Archive.
//!
//! Mechanisms change the world. The sampler takes a `Mech` set; the world is
//! generated with none, and `edits` diffs two states over a mechanism's box,
//! so the app writes exactly the cells a ring, repair or gate changes.
use super::plan::Style;
use crate::{BlockId as B, WorldSampleConfig, hash3};
use dress::Owner;
use shape::At;
use glam::{IVec3, Vec3};

mod dress;
mod feature;
mod fine;
mod fixtures;
mod glow;
mod grove;
mod furnish;
mod houses;
mod index;
mod layout;
mod layout_chasm;
mod layout_deep;
mod layout_dress;
mod settle;
mod shape;
mod site;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_geometry;
#[cfg(test)]
mod tests_grid;
#[cfg(test)]
mod tests_reach;
#[cfg(test)]
mod tests_pin;
#[cfg(test)]
mod tests_clip;

pub use layout::author;

/// First generator with this geometry.
pub const VERSION: u32 = 32;

/// Mechanism states, one bit each. The world generates with none set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mech(pub u16);

impl Mech {
    /// Freight platform repaired: the broken stair is whole.
    pub const STAIR: Self = Self(1);
    /// Water lift repaired: a bridge from the village to the bell tower.
    pub const BRIDGE: Self = Self(2);
    /// Bell rung: the listening stair's missing flight is restored.
    pub const BELL: Self = Self(4);
    /// Chain lift wound: the portcullis at its foot rises.
    pub const WINCH: Self = Self(8);
    /// Bypass sluice open: the courthouse drains.
    pub const SLUICE: Self = Self(16);
    /// Cooling gate open: the lava channel crusts over.
    pub const COOLING: Self = Self(32);
    /// Observatory lens restored: the archive door opens.
    pub const LENS: Self = Self(64);
    /// Receiving platform repaired: a freight ramp to the foundry.
    pub const FREIGHT: Self = Self(128);
    pub const ALL: Self = Self(255);

    pub fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Clone, Debug)]
pub enum Shape {
    /// Vaulted hall; `rise` is the vault height over the walls.
    Hall { x0: f32, x1: f32, z0: f32, z1: f32, floor: f32, h: f32, rise: f32 },
    /// Round room with a dome.
    Round { x: f32, z: f32, r: f32, floor: f32, h: f32, dome: f32 },
    /// Natural cavern: a noisy ellipsoid over a terraced floor.
    Cave { c: Vec3, rad: Vec3, floor: f32 },
    /// Vertical shaft with a spiral stair. `gap` is a band of missing steps
    /// that a mechanism restores.
    /// `landings` are full floors (with a hole where the stair arrives) at
    /// the heights where passages leave the shaft.
    Shaft { x: f32, z: f32, r: f32, y0: f32, y1: f32, gap: Option<(f32, f32, Mech)>, landings: [Option<f32>; 2] },
}

#[derive(Clone, Debug)]
pub struct Space {
    /// Index into `Plan::rooms`, for naming.
    pub room: usize,
    pub shape: Shape,
    pub style: Style,
    /// Standing water below this height, unless the mechanism has drained it.
    pub water: Option<(f32, Option<Mech>)>,
    /// Tombstone shell: no digging in.
    pub sealed: bool,
}

/// Straight passage between two floor points, ramped, with an arched roof
/// and a width that varies along it; `gate` blocks its middle with a block
/// until a mechanism opens it.
#[derive(Clone, Debug)]
pub struct Tunnel {
    pub a: Vec3,
    pub b: Vec3,
    pub half: f32,
    pub h: f32,
    pub style: Style,
    pub gate: Option<(Mech, B)>,
    /// Tombstone shell, so nobody digs around the gate.
    pub sealed: bool,
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub anchor: Vec3,
    pub spaces: Vec<Space>,
    pub tunnels: Vec<Tunnel>,
    pub features: Vec<feature::Feature>,
    pub min: Vec3,
    pub max: Vec3,
    /// Points kept clear of cavern formations: the props.
    pub clear: Vec<Vec3>,
    /// Column grid of candidates, built once the layout is authored.
    pub index: index::Index,
    /// Generator version the layout was authored for.
    pub version: u32,
}

/// Wall, alternate, trim and floor stone for a style.
pub fn palette(style: Style) -> [B; 4] {
    match style {
        Style::Rootbound => [B::StoneBricks, B::WeatheredBricks, B::Cobblestone, B::Cobblestone],
        Style::Canal => [B::Limestone, B::StoneBricks, B::StoneBricks, B::Limestone],
        Style::Cliff => [B::Limestone, B::Basalt, B::StoneBricks, B::Limestone],
        Style::Fungal => [B::Limestone, B::Mud, B::MushroomStem, B::FungalSoil],
        Style::Foundry => [B::Basalt, B::BlastBrick, B::CokeOvenBrick, B::Basalt],
        Style::Archive => [B::StoneBricks, B::Limestone, B::SilverBlock, B::StoneBricks],
        Style::Ice => [B::Ice, B::Limestone, B::Limestone, B::Ice],
        Style::Sea => [B::Limestone, B::Sand, B::StoneBricks, B::Sand],
    }
}

fn stone(style: Style, m: Vec3, v: f32) -> B {
    let p = (m / v).floor().as_ivec3();
    let [wall, alt, ..] = palette(style);
    let course = ((m.y / 2.0).floor() as i32).rem_euclid(2) == 1;
    if course ^ (hash3(p.x, p.y, p.z, 41) > 0.85) { alt } else { wall }
}

impl Layout {
    pub fn contains(&self, m: Vec3) -> bool {
        m.cmpge(self.min).all() && m.cmple(self.max).all()
    }

    /// Block at a point for a mechanism state, or `None` for natural terrain.
    pub fn sample(&self, cfg: &WorldSampleConfig, m: Vec3, mech: Mech) -> Option<B> {
        if !self.contains(m) {
            return None;
        }
        self.sample_with(m, cfg.voxel_meters(), mech, true)
    }

    /// The sampler proper, over the candidates of one index cell. Without
    /// `deco`, sconces are left out: they ask this for the cell behind them,
    /// and must not ask each other.
    fn sample_with(&self, m: Vec3, v: f32, mech: Mech, deco: bool) -> Option<B> {
        let cell = self.index.at(m);
        for &fi in &cell.features {
            let f = &self.features[fi as usize];
            if let Some(b) = f.sample(m, v, mech) {
                // Fitted dressing was probed at a coarser spacing than a
                // torch is wide; where it still meets the room, the room wins.
                if f.kind.fitted() && self.ground(cell, m, v, mech, deco) != Some(B::Air) {
                    continue;
                }
                return Some(b);
            }
        }
        self.ground(cell, m, v, mech, deco)
    }

    /// The spaces, passages and their dressing at a point, with no features.
    fn ground(&self, cell: &index::Cell, m: Vec3, v: f32, mech: Mech, deco: bool) -> Option<B> {
        let mut shell: Option<(Style, bool, Owner)> = None;
        let mut floor: Option<Style> = None;
        // A passage keeps its ramp where it runs into a cavern, whose ragged
        // edge would otherwise eat the first metres of the ramp.
        let ramp = || cell.tunnels.iter().map(|&ti| &self.tunnels[ti as usize]).find(|t| t.at(m) == At::Floor);
        for &si in &cell.spaces {
            let s = &self.spaces[si as usize];
            match s.at(m) {
                At::Void if s.eroded(m) && self.claimed(cell, si as usize, m) => {
                    shell = Some(shell.map_or((s.style, s.sealed, Owner::Space(si as usize)), |(st, sealed, o)| (st, sealed || s.sealed, o)));
                }
                At::Void => {
                    if matches!(s.shape, Shape::Cave { .. })
                        && let Some(t) = ramp()
                    {
                        return Some(palette(t.style)[3]);
                    }
                    return Some(self.void(si as usize, cell, m, v, mech, deco));
                }
                At::Floor => floor = floor.or(Some(s.style)),
                At::Shell => shell = Some(shell.map_or((s.style, s.sealed, Owner::Space(si as usize)), |(st, sealed, o)| (st, sealed || s.sealed, o))),
                At::Out => {}
            }
        }
        for &ti in &cell.tunnels {
            let t = &self.tunnels[ti as usize];
            match t.at(m) {
                At::Void => {
                    if let Some((needs, block)) = t.gate
                        && !mech.has(needs)
                        && (t.frame(m).t - 0.5).abs() < 0.08
                    {
                        return Some(block);
                    }
                    let f = t.frame(m);
                    if let Some(kerb) = self.fine_kerb(t.style, f.lateral, f.half, m.y - f.floor, v) {
                        return Some(kerb);
                    }
                    let lit = if deco { self.passage_dress(t, m, v, mech) } else { None };
                    return Some(lit.unwrap_or(B::Air));
                }
                At::Floor => floor = floor.or(Some(t.style)),
                At::Shell => shell = Some(shell.map_or((t.style, t.sealed, Owner::Tunnel), |(st, sealed, o)| (st, sealed || t.sealed, o))),
                At::Out => {}
            }
        }
        if let Some(style) = floor {
            return Some(fine::flagstone(self.version, style, m, v).unwrap_or(palette(style)[3]));
        }
        shell.map(|(style, sealed, owner)| if sealed { B::TombStone } else { self.wall(cell, owner, style, m, v) })
    }

    /// Erosion gives way: another space's void or floor, or a passage's
    /// floor, already claims this point.
    fn claimed(&self, cell: &index::Cell, si: usize, m: Vec3) -> bool {
        cell.spaces.iter().any(|&o| o as usize != si && matches!(self.spaces[o as usize].at(m), At::Void | At::Floor))
            || cell.tunnels.iter().any(|&ti| self.tunnels[ti as usize].at(m) == At::Floor)
    }

    fn void(&self, si: usize, cell: &index::Cell, m: Vec3, v: f32, mech: Mech, deco: bool) -> B {
        let s = &self.spaces[si];
        if let Shape::Shaft { x, z, r, y0, gap, landings, .. } = s.shape {
            let (dx, dz) = (m.x - x, m.z - z);
            let stair = Spiral::new(y0, (r - 2.4).max(1.0), r, v);
            for h in landings.into_iter().flatten() {
                if stair.landing(dx, dz, m.y, h) {
                    return palette(s.style)[2];
                }
            }
            let missing = gap.is_some_and(|(g0, g1, needs)| m.y >= g0 && m.y < g1 && !mech.has(needs));
            if !missing && stair.tread(dx, dz, m.y) {
                return palette(s.style)[2];
            }
            // Wall torches at the four compass points every nine metres; not
            // solid, so they never block the stair, and only on rock (the
            // well opens into the landing).
            if deco && dx.hypot(dz) > r - v && (m.y - y0 - 2.0).rem_euclid(9.0) < v && m.y - y0 > 2.0 {
                let dir = if dz.abs() < 0.6 {
                    Some(Vec3::X * dx.signum())
                } else if dx.abs() < 0.6 {
                    Some(Vec3::Z * dz.signum())
                } else {
                    None
                };
                if let Some(torch) = dir.and_then(|dir| self.sconce(m, v, mech, dir)) {
                    return torch;
                }
            }
        }
        if let Some((level, drained_by)) = s.water
            && m.y < level
            && !drained_by.is_some_and(|d| mech.has(d))
        {
            return B::Water;
        }
        if deco {
            let dressed = match s.shape {
                Shape::Hall { .. } => self.fine_hall(s, m, v).or_else(|| self.hall_dress(s, cell, m, v, mech)),
                Shape::Round { x, z, r, floor, .. } => self.round_dress(x, z, r, floor, m, v, mech),
                Shape::Cave { .. } => {
                    let lamp = if self.version >= glow::VERSION { self.cave_lamp(s, m, v, mech) } else { None };
                    lamp.or_else(|| self.cave_dress(si, cell, m, v))
                }
                Shape::Shaft { .. } => None,
            };
            if let Some(b) = dressed {
                return b;
            }
        }
        B::Air
    }

    /// Cells a mechanism change writes: every voxel in the mechanism's boxes
    /// whose block differs between the two states.
    pub fn edits(&self, cfg: &WorldSampleConfig, from: Mech, to: Mech) -> Vec<(IVec3, B)> {
        let v = cfg.voxel_meters();
        let changed = Mech(from.0 ^ to.0);
        let mut out = Vec::new();
        for f in self.features.iter().filter(|f| f.mech().is_some_and(|m| changed.0 & m.0 != 0)) {
            scan(f.bounds(), v, |p, m| self.diff(cfg, p, m, from, to, &mut out));
        }
        for s in &self.spaces {
            let gated = matches!(s.shape, Shape::Shaft { gap: Some((_, _, m)), .. } if changed.0 & m.0 != 0)
                || s.water.is_some_and(|(_, d)| d.is_some_and(|d| changed.0 & d.0 != 0));
            if gated {
                let (lo, hi) = s.bounds();
                scan((lo, hi), v, |p, m| self.diff(cfg, p, m, from, to, &mut out));
            }
        }
        for t in self.tunnels.iter().filter(|t| t.gate.is_some_and(|(m, _)| changed.0 & m.0 != 0)) {
            let lo = t.a.min(t.b) - Vec3::new(t.half + 1.0, 1.0, t.half + 1.0);
            let hi = t.a.max(t.b) + Vec3::new(t.half + 1.0, t.h + 1.0, t.half + 1.0);
            scan((lo, hi), v, |p, m| self.diff(cfg, p, m, from, to, &mut out));
        }
        out.sort_by_key(|(p, _)| (p.x, p.y, p.z));
        out.dedup_by_key(|(p, _)| *p);
        out
    }

    fn diff(&self, cfg: &WorldSampleConfig, p: IVec3, m: Vec3, from: Mech, to: Mech, out: &mut Vec<(IVec3, B)>) {
        let (a, b) = (self.sample(cfg, m, from), self.sample(cfg, m, to));
        if a == b {
            return;
        }
        match b {
            Some(b) => out.push((p, b)),
            // The new state leaves this cell to nature. The world generator
            // gives nature here exactly when the generated (no mechanism)
            // state does too; otherwise leave the cell as it stands rather
            // than invent a block.
            None if self.sample(cfg, m, Mech::default()).is_none() => out.push((p, crate::sample_block(cfg, p))),
            None => {}
        }
    }
}

/// A spiral stair of discrete treads, one voxel of rise each, round a ring
/// from `r_in` to `r_out`. Each tread is an angular sector about 1.3 m wide
/// at mid radius, so a full turn climbs well over head height.
pub(super) struct Spiral {
    y0: f32,
    r_in: f32,
    r_out: f32,
    v: f32,
    n: i64,
    dtheta: f32,
}

impl Spiral {
    pub(super) fn new(y0: f32, r_in: f32, r_out: f32, v: f32) -> Self {
        let n = ((std::f32::consts::TAU * (r_in + r_out) * 0.5 / 1.3).floor() as i64).max(4);
        Self { y0, r_in, r_out, v, n, dtheta: std::f32::consts::TAU / n as f32 }
    }
    fn sector(&self, dx: f32, dz: f32) -> i64 {
        ((dz.atan2(dx).rem_euclid(std::f32::consts::TAU) / self.dtheta).floor() as i64).min(self.n - 1)
    }
    fn ring(&self, dx: f32, dz: f32) -> bool {
        let d = dx.hypot(dz);
        d >= self.r_in && d < self.r_out
    }
    /// Solid tread at this point: tread `t` fills rise layer `t` in sector `t mod n`.
    pub(super) fn tread(&self, dx: f32, dz: f32, y: f32) -> bool {
        let t = ((y - self.y0) / self.v).floor() as i64;
        t >= 0 && self.ring(dx, dz) && t.rem_euclid(self.n) == self.sector(dx, dz)
    }
    /// A floor at height `h` across the whole shaft, open over three metres
    /// of treads below the arriving one so the climber keeps head room.
    pub(super) fn landing(&self, dx: f32, dz: f32, y: f32, h: f32) -> bool {
        if y < h - self.v || y >= h {
            return false;
        }
        let top = ((h - self.y0) / self.v).round() as i64 - 1;
        let k = self.sector(dx, dz);
        // Three metres of treads, whatever the voxel size.
        let treads = ((3.0 / self.v).round() as i64).clamp(3, self.n - 1);
        let open = self.ring(dx, dz) && (1..=treads).any(|i| (top - i).rem_euclid(self.n) == k);
        !open
    }
}

fn scan((lo, hi): (Vec3, Vec3), v: f32, mut f: impl FnMut(IVec3, Vec3)) {
    let (a, b) = ((lo / v).floor().as_ivec3(), (hi / v).ceil().as_ivec3());
    for y in a.y..=b.y {
        for z in a.z..=b.z {
            for x in a.x..=b.x {
                let p = IVec3::new(x, y, z);
                f(p, (p.as_vec3() + Vec3::splat(0.5)) * v);
            }
        }
    }
}

/// Which prop flag drives which mechanism, by (district, room).
const TRIGGERS: [((u8, u16), Mech, u32); 8] = [
    ((0, 4), Mech::STAIR, super::state::flags::REPAIRED),
    ((1, 2), Mech::BRIDGE, super::state::flags::REPAIRED),
    ((1, 5), Mech::BELL, super::state::flags::RESTORED),
    ((2, 1), Mech::WINCH, super::state::flags::OPERATED),
    ((2, 4), Mech::SLUICE, super::state::flags::OPEN),
    ((4, 1), Mech::COOLING, super::state::flags::OPEN),
    ((1, 6), Mech::LENS, super::state::flags::RESTORED),
    ((1, 4), Mech::FREIGHT, super::state::flags::REPAIRED),
];

/// Mechanism state implied by the saved prop flags. Flags are history; the
/// voxels written when they changed are what the world actually holds.
pub fn current(state: &super::State) -> Mech {
    let mut mech = Mech::default();
    for ((district, room), m, flag) in TRIGGERS {
        if super::ObjectId::new(district, room, 1).is_some_and(|id| state.has(id, flag)) {
            mech = mech.with(m);
        }
    }
    mech
}
