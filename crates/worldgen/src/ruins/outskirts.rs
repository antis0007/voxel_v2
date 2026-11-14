//! Generator 299: the ground a city gives way to. Past the kerb of an Anchor
//! city the streets carry on as lanes that fray into the grass, and lots thin
//! out along them: houses near the deck, then sheds, tanks and bare
//! foundations, then nothing. Occupancy and building size fall off
//! exponentially with distance from the deck, so a district reads as a centre
//! with sprawl instead of a rectangle dropped on a plain.
//!
//! Everything here sits on natural ground. A lane paves the top voxel of
//! whatever terrain it crosses; a lot is levelled at the height surveyed at
//! its centre the first time a voxel of it is sampled, and is dropped if the
//! ground under it is steep or wet.
use super::shapes::roll;
use super::urban::{self, Urban};
use super::CacheLoot;
use crate::{BlockId as B, WorldSampleConfig, sample_column, value_noise2};
use glam::{IVec3, Vec2, Vec3};
use std::cell::Cell;

pub const VERSION: u32 = engine_voxel::ANCHOR_OUTSKIRTS_GENERATOR;

/// Sprawl ends this far outside the deck, metres.
pub(super) const REACH: f32 = 230.0;
/// Distance over which lot occupancy falls by e, metres.
const FALLOFF: f32 = 75.0;
/// Lot pitch, metres.
const LOT: f32 = 16.0;
/// A lot this close to a lane fronts it and is as likely as the distance allows.
const FRONTAGE: f32 = 27.0;
/// Bare ground between lanes is this much less likely to be built on.
const BACKLAND: f32 = 0.3;
/// Most height difference across a lot that still gets built on, metres.
const MAX_RELIEF: f32 = 2.0;
/// Tallest outskirt structure above its lot, metres; chunk residency above
/// terrain is only guaranteed to tree height.
const MAX_HEIGHT: f32 = 6.8;
const BUCKET: f32 = 32.0;
const STOREY: f32 = 3.2;
const POLE_PITCH: f32 = 28.0;

#[derive(Clone, Copy)]
struct Lane {
    from: Vec2,
    dir: Vec2,
    len: f32,
    half: f32,
    id: u32,
}

impl Lane {
    /// Distance along and across the lane, if `q` is beside its paved length.
    fn offset(&self, q: Vec2) -> Option<(f32, f32)> {
        let delta = q - self.from;
        let along = delta.dot(self.dir);
        (0.0..=self.len).contains(&along).then(|| (along, delta.perp_dot(self.dir)))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    House,
    Foundation,
    Shed,
    Tank,
    Rubble,
}

#[derive(Clone, Copy)]
enum Ground {
    Unknown,
    Unfit,
    /// Floor level in district-local metres above the datum.
    Level(f32),
}

struct Lot {
    c: Vec2,
    half: Vec2,
    kind: Kind,
    height: f32,
    /// Wall the door is in: 0 -x, 1 +x, 2 -z, 3 +z; it faces the lane or the city.
    door: u8,
    wall: B,
    /// 0 intact .. 1 razed.
    ruin: f32,
    stash: Option<CacheLoot>,
    seed: u32,
    ground: Cell<Ground>,
}

pub(super) struct Outskirts {
    lanes: Vec<Lane>,
    lots: Vec<Lot>,
    lo: Vec2,
    width: i32,
    depth: i32,
    buckets: Vec<Vec<u16>>,
}

/// The frame a district hands down so lots can be surveyed lazily.
#[derive(Clone, Copy)]
pub(super) struct Site<'a> {
    pub cfg: &'a WorldSampleConfig,
    pub centre: Vec2,
    pub turn: u32,
    pub datum: f32,
}

impl Site<'_> {
    /// Ground top above the datum at a district-local point.
    fn ground(&self, q: Vec2) -> (f32, bool) {
        let w = self.centre + super::rotate(self.turn, q.x, q.y);
        let column = sample_column(self.cfg, self.cfg.blocks_for_height(w.x), self.cfg.blocks_for_height(w.y));
        (self.cfg.world_y(column.height + 1) - self.datum, column.height <= self.cfg.water_level())
    }

    fn cell(&self, q: Vec3) -> IVec3 {
        let d = super::rotate(self.turn, q.x, q.z);
        let w = Vec3::new(self.centre.x + d.x, self.datum + q.y, self.centre.y + d.y);
        (w / self.cfg.voxel_meters()).floor().as_ivec3()
    }
}

impl Outskirts {
    /// Lanes and candidate lots around a city deck. `ramps` are the deck's
    /// surveyed approaches as (edge point, outward direction, run).
    /// `built` reports district structures over a local rectangle; lots keep off them.
    pub fn plan(urban: Urban, seed: u32, ramps: &[(Vec2, Vec2, f32)], built: impl Fn(Vec2, Vec2) -> bool) -> Self {
        let half = urban.half;
        let mut lanes = Vec::new();
        let mut lane = |from: Vec2, dir: Vec2, main: bool, id: u32| {
            // A surveyed approach already carries the avenue to the ground; the lane starts at its foot.
            let run = ramps.iter().find(|(edge, d, _)| edge.distance(from) < 1.0 && d.dot(dir) > 0.9).map(|r| r.2 + 2.0);
            if main && run.is_none() {
                return;
            }
            let r = roll(seed, id as i32, 931);
            // Most side streets stop within a block; a few run out to the edge of the sprawl.
            let len = if main { 150.0 + r * (REACH - 150.0) } else { 26.0 + r * r * (REACH - 40.0) };
            let start = run.unwrap_or(0.0);
            if len > start + 12.0 {
                lanes.push(Lane { from: from + dir * start, dir, len: len - start, half: if main { 3.2 } else { 2.2 }, id });
            }
        };
        for (i, &x) in urban::CITY_X.iter().enumerate() {
            for side in [-1.0f32, 1.0] {
                let at = Vec2::new(urban.street_x(i, side * half.y), side * half.y);
                lane(at, Vec2::Y * side, x == 0.0, (i as u32) * 2 + u32::from(side > 0.0));
            }
        }
        for (i, &z) in urban::CITY_Z.iter().enumerate() {
            for side in [-1.0f32, 1.0] {
                lane(Vec2::new(side * half.x, z), Vec2::X * side, z == urban::CROSS_AVENUE,
                    32 + (i as u32) * 2 + u32::from(side > 0.0));
            }
        }

        // A long lane grows a side road or two, parallel to the deck: the start of a block.
        for i in 0..lanes.len() {
            let trunk = lanes[i];
            if trunk.len < 80.0 {
                continue;
            }
            for k in 0..2u32 {
                let r = |salt: i32| roll(seed ^ trunk.id.wrapping_mul(0x9e37), k as i32 + salt, 933);
                if r(0) > 0.7 {
                    continue;
                }
                let along = 38.0 + r(10) * (trunk.len - 60.0);
                let side = if (k == 0) == (r(20) < 0.5) { 1.0 } else { -1.0 };
                let dir = trunk.dir.perp() * side;
                lanes.push(Lane {
                    from: trunk.from + trunk.dir * along + dir * trunk.half,
                    dir,
                    len: 28.0 + r(30) * 62.0,
                    half: 1.8,
                    id: 64 + trunk.id * 2 + k,
                });
            }
        }

        let span = half + Vec2::splat(REACH);
        let cells = (span / LOT).ceil().as_ivec2();
        let mut lots: Vec<Lot> = Vec::new();
        for gz in -cells.y..=cells.y {
            for gx in -cells.x..=cells.x {
                let key = seed ^ (gx as u32).wrapping_mul(0x9e37_79b9) ^ (gz as u32).wrapping_mul(0x85eb_ca6b) ^ 0x0a75;
                let r = |salt: i32| roll(key, salt, 937);
                let c = Vec2::new(gx as f32, gz as f32) * LOT + (Vec2::new(r(1), r(2)) - 0.5) * 7.0;
                let out = -urban.edge(c);
                if !(11.0..REACH).contains(&out) {
                    continue;
                }
                // Nearest lane: frontage decides both the odds and which way the door faces.
                let front = lanes.iter().filter_map(|l| l.offset(c).map(|(_, across)| (across, l)))
                    .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()));
                let fronted = front.filter(|(across, _)| across.abs() < FRONTAGE);
                let odds = 0.92 * (-out / FALLOFF).exp() * if fronted.is_some() { 1.0 } else { BACKLAND };
                if r(3) > odds {
                    continue;
                }
                let near = (-out / 60.0).exp();
                let kind = match r(4) + 0.25 * (1.0 - near) {
                    k if k < 0.46 => Kind::House,
                    k if k < 0.60 => Kind::Shed,
                    k if k < 0.70 => Kind::Tank,
                    k if k < 0.92 => Kind::Foundation,
                    _ => Kind::Rubble,
                };
                let lot_half = match kind {
                    Kind::House => Vec2::new(3.6 + r(5) * 2.2, 3.0 + r(6) * 1.8),
                    Kind::Shed => Vec2::new(3.0 + r(5) * 2.6, 2.6 + r(6) * 1.4),
                    Kind::Tank => Vec2::splat(2.4 + r(5) * 1.1),
                    Kind::Foundation => Vec2::new(3.2 + r(5) * 3.0, 2.8 + r(6) * 2.2),
                    Kind::Rubble => Vec2::new(2.4 + r(5) * 1.4, 2.2 + r(6) * 1.2),
                };
                let lot_half = if r(7) < 0.5 { lot_half } else { Vec2::new(lot_half.y, lot_half.x) };
                let (lo, hi) = (c - lot_half, c + lot_half);
                // Keep the carriageways, the ramps and the neighbours clear.
                let on_lane = lanes.iter().any(|l| l.offset(c).is_some_and(|(_, across)| {
                    across.abs() < l.half + 2.5 + if l.dir.x != 0.0 { lot_half.y } else { lot_half.x }
                }));
                let on_ramp = ramps.iter().any(|(edge, dir, run)| {
                    let d = c - *edge;
                    let along = d.dot(*dir);
                    along > -4.0 && along < run + 6.0 && d.perp_dot(*dir).abs() < urban::AVENUE + 4.0 + lot_half.max_element()
                });
                let crowded = lots.iter().any(|o| lo.x < o.c.x + o.half.x + 2.0 && hi.x > o.c.x - o.half.x - 2.0
                    && lo.y < o.c.y + o.half.y + 2.0 && hi.y > o.c.y - o.half.y - 2.0);
                if on_lane || on_ramp || crowded || lots.len() >= u16::MAX as usize
                    || built(lo - Vec2::splat(3.0), hi + Vec2::splat(3.0))
                {
                    continue;
                }
                let storeys = if kind == Kind::House && near > 0.42 && r(8) < near { 2.0 } else { 1.0 };
                let height = match kind {
                    Kind::House => STOREY * storeys,
                    Kind::Shed => 3.0 + r(9),
                    Kind::Tank => 4.6 + r(9) * 2.0,
                    Kind::Foundation => 0.5 + r(9) * 0.7,
                    Kind::Rubble => 3.8 + r(9) * 1.6,
                }.min(MAX_HEIGHT);
                let door = match fronted {
                    Some((across, l)) if l.dir.x != 0.0 => if across > 0.0 { 2 } else { 3 },
                    Some((across, _)) => if across > 0.0 { 1 } else { 0 },
                    // No lane: face the city.
                    None if c.x.abs() * half.y > c.y.abs() * half.x => if c.x > 0.0 { 0 } else { 1 },
                    None => if c.y > 0.0 { 2 } else { 3 },
                };
                let wall = match (r(10) * 3.0) as u32 { 0 => B::StoneBricks, 1 => B::Concrete, _ => B::WeatheredBricks };
                let stash = (kind == Kind::House && r(11) < 0.3).then_some(CacheLoot::Household)
                    .or((kind == Kind::Shed && r(11) < 0.35).then_some(CacheLoot::Workshop));
                // Farther out has stood empty longer.
                let ruin = (0.25 + 0.55 * (1.0 - near) + 0.35 * (r(12) - 0.5)).clamp(0.05, 1.0);
                lots.push(Lot { c, half: lot_half, kind, height, door, wall, ruin, stash, seed: key, ground: Cell::new(Ground::Unknown) });
            }
        }

        let lo = -span - Vec2::splat(BUCKET);
        let size = ((span + Vec2::splat(BUCKET)) * 2.0 / BUCKET).ceil().as_ivec2();
        let mut buckets = vec![Vec::new(); (size.x * size.y) as usize];
        for (i, lot) in lots.iter().enumerate() {
            let g0 = ((lot.c - lot.half - lo) / BUCKET).floor().as_ivec2();
            let g1 = ((lot.c + lot.half - lo) / BUCKET).floor().as_ivec2();
            for z in g0.y.max(0)..=g1.y.min(size.y - 1) {
                for x in g0.x.max(0)..=g1.x.min(size.x - 1) {
                    buckets[(z * size.x + x) as usize].push(i as u16);
                }
            }
        }
        Self { lanes, lots, lo, width: size.x, depth: size.y, buckets }
    }

    fn lot_at(&self, q: Vec2, pad: f32) -> Option<&Lot> {
        let g = ((q - self.lo) / BUCKET).floor().as_ivec2();
        if g.x < 0 || g.y < 0 || g.x >= self.width || g.y >= self.depth {
            return None;
        }
        self.buckets[(g.y * self.width + g.x) as usize].iter().map(|&i| &self.lots[i as usize])
            .find(|lot| (q.x - lot.c.x).abs() <= lot.half.x + pad && (q.y - lot.c.y).abs() <= lot.half.y + pad)
    }

    /// Whether plants stay off this point: a lane or a lot's floor.
    pub fn clears(&self, q: Vec2, reach: f32) -> bool {
        self.lot_at(q, reach + 0.5).is_some()
            || self.lanes.iter().any(|l| l.offset(q).is_some_and(|(_, across)| across.abs() < l.half + reach))
    }

    pub fn loot(&self, site: Site, p: IVec3, q: Vec2) -> Option<CacheLoot> {
        let lot = self.lot_at(q, 0.0)?;
        (lot.stash_cell(site)? == p).then_some(lot.stash).flatten()
    }

    /// The block at district-local `q`, where `ground` is the natural terrain top above the datum.
    pub fn sample(&self, site: Site, p: IVec3, q: Vec3, ground: f32, seed: u32) -> Option<B> {
        let v = site.cfg.voxel_meters();
        let xz = Vec2::new(q.x, q.z);
        if let Some(lot) = self.lot_at(xz, 0.0)
            && let Some(floor) = lot.level(site)
        {
            if lot.stash_cell(site) == Some(p) {
                return lot.stash.map(|loot| loot.block());
            }
            return lot.sample(q, floor, ground, v);
        }
        for lane in &self.lanes {
            let Some((along, across)) = lane.offset(xz) else { continue };
            let side = across.abs();
            // A timber pole every pitch on one verge; some have fallen.
            let slot = (along / POLE_PITCH).round();
            if (side - lane.half - 1.6).abs() <= 0.5 * v.max(0.6) && across > 0.0
                && (along - slot * POLE_PITCH).abs() <= 0.5 * v.max(0.6)
                && slot > 0.0 && roll(seed ^ lane.id, slot as i32, 941) < 0.62
            {
                let top = ground + 4.6 + 2.0 * roll(seed ^ lane.id, slot as i32, 942);
                if q.y >= ground && q.y < top.min(ground + MAX_HEIGHT) {
                    return Some(B::Log);
                }
            }
            if side > lane.half || q.y >= ground || q.y < ground - v {
                continue;
            }
            // The surface breaks up toward the far end and at the verges first.
            let worn = along / lane.len + 0.35 * (side / lane.half).powi(2);
            let patch = value_noise2(q.x / 5.0, q.z / 5.0, seed ^ 0x1a9e);
            if patch < worn * 0.95 - 0.12 {
                continue;
            }
            return Some(if patch < worn * 0.95 + 0.06 || value_noise2(q.x * 0.31, q.z * 0.31, seed) > 0.74 {
                B::Gravel
            } else {
                B::Basalt
            });
        }
        None
    }
}

impl Lot {
    /// Floor level above the datum, surveying the lot on first use.
    fn level(&self, site: Site) -> Option<f32> {
        if let Ground::Unknown = self.ground.get() {
            let v = site.cfg.voxel_meters();
            let (centre, wet) = site.ground(self.c);
            let corners = [self.c - self.half, self.c + self.half];
            let relief = corners.into_iter().map(|c| site.ground(c)).fold((0.0f32, wet), |(worst, wet), (g, w)| {
                (worst.max((g - centre).abs()), wet || w)
            });
            self.ground.set(if relief.1 || relief.0 > MAX_RELIEF {
                Ground::Unfit
            } else {
                Ground::Level((centre / v).round() * v)
            });
        }
        match self.ground.get() {
            Ground::Level(floor) => Some(floor),
            _ => None,
        }
    }

    fn stash_cell(&self, site: Site) -> Option<IVec3> {
        self.stash?;
        let floor = self.level(site)?;
        let v = site.cfg.voxel_meters();
        // Against the back wall, opposite the door.
        let inset = (self.half - Vec2::splat(1.0 + v)).max(Vec2::ZERO);
        let back = match self.door {
            0 => Vec2::new(inset.x, 0.0),
            1 => Vec2::new(-inset.x, 0.0),
            2 => Vec2::new(0.0, inset.y),
            _ => Vec2::new(0.0, -inset.y),
        };
        let at = self.c + back;
        Some(site.cell(Vec3::new(at.x, floor + 0.5 * v, at.y)))
    }

    fn sample(&self, q: Vec3, floor: f32, ground: f32, v: f32) -> Option<B> {
        let d = Vec2::new(q.x - self.c.x, q.z - self.c.y);
        let y = q.y - floor;
        let from_wall = (self.half - d.abs()).min_element();
        let thick = v.max(0.5);
        let moss = |salt: u32| value_noise2(q.x / 3.0, q.z / 3.0, self.seed ^ salt);
        // Footing down to the natural ground, and a cleared room above the floor.
        if y < -v {
            return (q.y >= ground).then_some(B::WeatheredBricks);
        }
        if y < 0.0 {
            return Some(match self.kind {
                Kind::Rubble => if moss(1) > 0.5 { B::Gravel } else { B::Grass },
                Kind::Foundation if from_wall > thick && moss(2) > 0.42 => B::Grass,
                _ if moss(3) > 0.66 => B::GroveMoss,
                Kind::Shed | Kind::Tank => B::Concrete,
                _ => B::DressedStone,
            });
        }
        if y >= self.height {
            return (q.y < ground).then_some(B::Air);
        }
        let open = Some(B::Air);
        // Position along the perimeter, for the broken top edge and the openings.
        let (along, wall_side) = if self.half.x - d.x.abs() < self.half.y - d.y.abs() {
            (d.y, if d.x < 0.0 { 0 } else { 1 })
        } else {
            (d.x, if d.y < 0.0 { 2 } else { 3 })
        };
        let run = along + f32::from(wall_side) * 37.0;
        match self.kind {
            Kind::Tank => {
                let radial = d.length();
                let r = self.half.x;
                if radial > r {
                    return (q.y < ground).then_some(B::Air);
                }
                let shell = radial > r - thick;
                let lid = y >= self.height - thick;
                // A torn panel lets the tank be entered.
                let torn = wall_side == self.door && along.abs() < 1.1 && y < 2.4 + self.ruin * 2.0;
                if (shell && !torn) || (lid && moss(4) > 0.25 + 0.4 * self.ruin) {
                    return Some(B::RustedSteel);
                }
                open
            }
            Kind::Rubble => {
                // A heap with one standing chimney stack.
                let stack = d.x.abs() <= 0.5 * v.max(1.0) && d.y.abs() <= 0.5 * v.max(1.0);
                if stack {
                    return Some(B::WeatheredBricks);
                }
                let heap = 1.5 * (1.0 - (d / self.half).length()) + 0.9 * (moss(5) - 0.5);
                if y < heap {
                    return Some(if moss(6) > 0.55 { B::Gravel } else { self.wall });
                }
                (q.y < ground).then_some(B::Air)
            }
            Kind::Foundation => {
                let stub = self.height * (0.35 + 0.65 * value_noise2(run / 2.6, 0.5, self.seed ^ 7));
                if from_wall < thick && y < stub {
                    return Some(if y >= stub - v && moss(8) > 0.5 { B::GroveMoss } else { self.wall });
                }
                (q.y < ground).then_some(B::Air)
            }
            Kind::Shed => {
                let front = wall_side == self.door;
                let roof = y >= self.height - thick;
                if roof {
                    return (moss(9) > 0.18 + 0.45 * self.ruin).then_some(B::RustedSteel).or(open);
                }
                if from_wall < thick && !(front && along.abs() < self.half.min_element() * 0.7) {
                    let panel = value_noise2(run / 2.2, y / 2.2, self.seed ^ 10);
                    return (panel > 0.3 * self.ruin).then_some(B::SteelPlate).or(open);
                }
                open
            }
            Kind::House => {
                // The wall top is highest at the corners and bitten down between them.
                let bite = value_noise2(run / 3.4, 0.5, self.seed ^ 11);
                let standing = self.height * (1.0 - self.ruin * (0.25 + 0.75 * bite)).max(0.18);
                if from_wall < thick {
                    if y >= standing {
                        return open;
                    }
                    let storey = y.rem_euclid(STOREY);
                    let door = wall_side == self.door && along.abs() < 0.7_f32.max(v) && y < 2.2;
                    let window = (1.0..2.2).contains(&storey) && along.rem_euclid(3.2) > 1.0 && along.rem_euclid(3.2) < 2.2;
                    return if door || window { open } else { Some(self.wall) };
                }
                // Upper floor and roof survive in patches, more of them near the walls.
                let cover = moss(12) + 0.25 * (1.0 - from_wall / self.half.min_element());
                let upper = self.height > STOREY + 0.1 && (STOREY - thick..STOREY).contains(&y);
                if upper && cover > 0.35 + 0.5 * self.ruin {
                    return Some(B::Log);
                }
                let roof = y >= self.height - thick && y < standing;
                if roof && cover > 0.45 + 0.5 * self.ruin {
                    return Some(B::WeatheredBricks);
                }
                // Fallen masonry along the inside of the walls.
                if y < v && from_wall < thick + 1.6 && moss(13) > 0.62 - 0.2 * self.ruin {
                    return Some(B::Gravel);
                }
                open
            }
        }
    }
}
