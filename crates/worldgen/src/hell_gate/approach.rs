//! Gate 89: a ruined processional road to the buried gate's tower, a taller
//! broken tower and a pilastered sanctuary. Versions 83-88 keep the bare stair
//! tower and room. All distances are metres from the gate centre.
use crate::{BlockId as B, ColumnSample, WATER_LEVEL_METERS, WorldSampleConfig, hash2, hash3, value_noise2};
use glam::{IVec3, Vec2, Vec3};

pub const VERSION: u32 = 89;
/// The road runs out from the tower doorway along one cardinal axis.
const ROAD_START: f32 = 14.5;
const ROAD_END: f32 = 58.0;
const ROAD_HALF: f32 = 2.4;
const COLUMN_SIDE: f32 = 4.2;
/// The first arch stands whole; the second has lost one side.
const ARCHES: [f32; 2] = [30.0, 46.0];
/// A failed crystal ring marks where the road begins.
const RING_AT: f32 = 55.0;
const TOLL_AT: f32 = 38.0;
const TOLL_OFFSET: f32 = 10.5;
const ENCLOSURE: f32 = 21.5;
/// Nothing is placed beyond this radius.
pub const REACH: f32 = 62.0;

pub fn enabled(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= VERSION
}

struct Ctx<'a> {
    cfg: &'a WorldSampleConfig,
    c: Vec3,
    p: IVec3,
    m: Vec3,
    vm: f32,
    seed: u32,
    axis: Vec2,
    across: Vec2,
    toll_side: f32,
    /// Along the road and across it (positive toward `across`).
    u: f32,
    w: f32,
    /// Height of the cell centre above this column's ground.
    h: f32,
    radius: f32,
}

impl Ctx<'_> {
    /// Ground top under the road centre at `u`, so an arch or ring has one datum.
    fn datum(&self, u: f32) -> f32 {
        let q = Vec2::new(self.c.x, self.c.z) + self.axis * u;
        let x = (q.x / self.vm).floor() as i32;
        let z = (q.y / self.vm).floor() as i32;
        (crate::sample_column(self.cfg, x, z).height + 1) as f32 * self.vm
    }
    /// True for the one cell containing the point `u`, `w`, `y` (absolute).
    fn is_cell(&self, u: f32, w: f32, y: f32) -> bool {
        let q = Vec2::new(self.c.x, self.c.z) + self.axis * u + self.across * w;
        (Vec3::new(q.x, y, q.y) / self.vm).floor().as_ivec3() == self.p
    }
    fn on_ground(&self, height: f32) -> bool {
        self.h > -self.vm && self.h < height
    }
    fn world(&self, along: f32, across: f32) -> IVec3 {
        let v = self.axis * along + self.across * across;
        IVec3::new(v.x.round() as i32, 0, v.y.round() as i32)
    }
}

/// Wall torch hung on the face whose support lies at `offset`.
pub(super) fn wall_torch(offset: IVec3) -> B {
    match (offset.x, offset.z) {
        (0, 1) => B::TorchWallNorth,
        (0, -1) => B::TorchWallSouth,
        (1, 0) => B::TorchWallWest,
        _ => B::TorchWallEast,
    }
}

/// Vine on the face toward a support centred at `centre`, or none if that
/// neighbour cell is not part of the support.
fn vine_toward(m: Vec3, centre: Vec2, vm: f32, solid: impl Fn(Vec2) -> bool) -> Option<B> {
    let to = centre - Vec2::new(m.x, m.z);
    let (offset, vine) = if to.x.abs() > to.y.abs() {
        if to.x > 0.0 { (Vec2::X, B::VinesWest) } else { (Vec2::NEG_X, B::VinesEast) }
    } else if to.y > 0.0 { (Vec2::Y, B::Vines) } else { (Vec2::NEG_Y, B::VinesSouth) };
    solid(Vec2::new(m.x, m.z) + offset * vm).then_some(vine)
}

fn site_seed(cfg: &WorldSampleConfig, c: Vec3) -> u32 {
    cfg.seed ^ (c.x as i32 as u32).wrapping_mul(0x27d4_eb2f) ^ (c.z as i32 as u32).wrapping_mul(0x1656_67b1)
}

/// Outward direction of the road in (x, z), one of the four axes.
pub fn road_axis(cfg: &WorldSampleConfig, c: Vec3) -> Vec2 {
    [Vec2::NEG_Y, Vec2::X, Vec2::Y, Vec2::NEG_X][(hash2(c.x as i32, c.z as i32, site_seed(cfg, c)) * 3.999) as usize]
}

/// Surface ruins outside the tower: `d` is the cell centre minus the gate centre.
pub fn outer(cfg: &WorldSampleConfig, c: Vec3, p: IVec3, d: Vec3, column: ColumnSample) -> Option<B> {
    let radius = d.x.hypot(d.z);
    if radius > REACH {
        return None;
    }
    let vm = cfg.voxel_meters();
    let top = (column.height + 1) as f32 * vm;
    if top < WATER_LEVEL_METERS + 0.5 {
        return None;
    }
    let m = c + d;
    let h = m.y - top;
    if !(-2.0..10.0).contains(&h) {
        return None;
    }
    let seed = site_seed(cfg, c);
    let axis = road_axis(cfg, c);
    let across = Vec2::new(-axis.y, axis.x);
    let flat = Vec2::new(d.x, d.z);
    let x = Ctx {
        cfg, c, p, m, vm, seed, axis, across,
        toll_side: if hash2(1, 7, seed) < 0.5 { -1.0 } else { 1.0 },
        u: flat.dot(axis), w: flat.dot(across), h, radius,
    };
    toll_house(&x)
        .or_else(|| quenched_ring(&x))
        .or_else(|| arches(&x))
        .or_else(|| colonnade(&x))
        .or_else(|| enclosure(&x))
        .or_else(|| road(&x))
}

fn road(x: &Ctx) -> Option<B> {
    if !(ROAD_START..ROAD_END).contains(&x.u) {
        return None;
    }
    let side = if x.w < 0.0 { -1.0 } else { 1.0 };
    let edge = ROAD_HALF - 0.7 * value_noise2(x.u * 0.4, side * 5.0, x.seed);
    let aw = x.w.abs();
    if aw < edge {
        if x.h < 0.0 && x.h > -x.vm {
            let tile = hash2((x.m.x * 2.0).floor() as i32, (x.m.z * 2.0).floor() as i32, x.seed ^ 0x51);
            // Paving is lost more often toward the outer end of the road.
            let lost = 0.06 + 0.22 * (x.u - ROAD_START) / (ROAD_END - ROAD_START);
            return (tile >= lost).then_some(if tile < 0.5 { B::WeatheredBricks } else { B::StoneBricks });
        }
        // Growth never stands on the road itself.
        return (x.h > 0.0 && x.h < 4.0).then_some(B::Air);
    }
    // Torches on plinths flank the tower doorway.
    let plinth = (x.u - 15.6).abs() < 0.45 && (aw - 3.3).abs() < 0.45;
    if plinth && x.on_ground(1.0) {
        return Some(B::DressedStone);
    }
    // The plinth's centre column: its own ground is the datum.
    if x.is_cell(15.6, side * 3.3, x.m.y) && (1.0..1.0 + x.vm).contains(&x.h) {
        return Some(B::Torch);
    }
    // Below half-metre cells a kerb would be a metre-high wall.
    let kerb = x.vm <= 0.5 && aw < edge + 0.5 && hash2((x.u / 1.5).floor() as i32, side as i32, x.seed ^ 0x77) > 0.3;
    (kerb && x.h > 0.0 && x.h < 0.45f32.max(x.vm)).then_some(B::DressedStone)
}

fn colonnade(x: &Ctx) -> Option<B> {
    let k = ((x.u - 19.0) / 7.0).round();
    if !(0.0..=5.0).contains(&k) {
        return None;
    }
    let at = 19.0 + k * 7.0;
    if ARCHES.iter().any(|a| (a - at).abs() < 3.0) || (at - RING_AT).abs() < 3.0 {
        return None;
    }
    let side = if x.w < 0.0 { -1.0 } else { 1.0 };
    let ws = side * COLUMN_SIDE;
    let (du, dw) = (x.u - at, x.w - ws);
    if du.abs() > 1.6 {
        return None;
    }
    let roll = hash2(k as i32, side as i32, x.seed ^ 0xc01);
    let shaft = 0.55f32.max(x.vm * 0.75);
    let near_toll = side == x.toll_side && (at - TOLL_AT).abs() < 6.0;
    let toppled = roll < 0.28 && !near_toll;
    if toppled {
        // Drums lie outward from the road where the column fell.
        let t = dw * side;
        let length = 3.0 + roll * 5.0;
        if (0.7..length).contains(&t) && t % 1.5 < 1.38 && du.hypot(x.h - 0.55) < shaft {
            return Some(B::Limestone);
        }
    }
    if du.abs().max(dw.abs()) < 0.85 && x.on_ground(0.5f32.max(x.vm)) {
        return Some(B::DressedStone);
    }
    let whole = roll > 0.72;
    let height = if toppled { 0.9 } else if whole { 5.6 } else { 1.4 + 3.6 * hash2(k as i32, side as i32, x.seed ^ 0x5a) };
    if whole && (5.6..6.2).contains(&x.h) && du.abs().max(dw.abs()) < 0.8 {
        return Some(B::CarvedStone);
    }
    let centre = Vec2::new(x.c.x, x.c.z) + x.axis * at + x.across * ws;
    let dist = Vec2::new(x.m.x, x.m.z).distance(centre);
    // Broken tops are ragged, a fifth of a metre either way per cell.
    let ragged = if whole { 0.0 } else { (hash3(x.p.x, 0, x.p.z, x.seed) - 0.5) * 0.4 };
    let top = height + ragged;
    if dist < shaft && x.on_ground(top) {
        let cap = !whole && x.h > top - x.vm && hash2(x.p.x, x.p.z, x.seed ^ 0x3055) < 0.6;
        return Some(if cap { B::GroveMoss } else { B::Limestone });
    }
    let climbing = value_noise2(x.m.y * 0.7, (x.m.x + x.m.z) * 0.9, x.seed ^ 0x71e5) > 0.52;
    if climbing && dist < shaft + x.vm && x.h > 0.0 && x.h < top * 0.8 {
        return vine_toward(x.m, centre, x.vm, |q| q.distance(centre) < shaft);
    }
    None
}

fn arches(x: &Ctx) -> Option<B> {
    for (i, &at) in ARCHES.iter().enumerate() {
        let du = x.u - at;
        if du.abs() > 1.6 || x.w.abs() > 7.5 {
            continue;
        }
        let y = x.m.y - x.datum(at);
        let aw = x.w.abs();
        let broken = i == 1;
        // The broken half falls away from the toll house.
        let fallen = broken && x.w * x.toll_side < 0.0;
        if (2.9..3.9).contains(&aw) && du.abs() < 0.6 && x.h > -x.vm {
            let ragged = (hash3(x.p.x, 0, x.p.z, x.seed ^ 0xa4c) - 0.5) * 0.6;
            let top = if fallen { 2.4 + ragged } else { 4.6 };
            if y < top {
                return Some(if y < 0.7 { B::DressedStone } else { B::Limestone });
            }
        }
        if du.abs() < 0.7 && (4.6..5.5).contains(&y) && aw < 4.1 && !(fallen && aw > 0.9) {
            return Some(B::DressedStone);
        }
        if !broken && du.abs() < 0.55 && (5.5..6.0).contains(&y) && aw < 0.55 {
            return Some(B::CarvedStone);
        }
        if fallen && (4.6..7.2).contains(&aw) && (0.3..1.5).contains(&du) && x.on_ground(0.9) {
            return Some(B::DressedStone);
        }
        if !broken {
            let side = if x.w < 0.0 { -1.0 } else { 1.0 };
            let datum = x.datum(at);
            if x.is_cell(at, side * (2.9 - x.vm * 0.5), datum + 2.6) {
                return Some(wall_torch(x.world(0.0, side)));
            }
        }
    }
    None
}

fn toll_house(x: &Ctx) -> Option<B> {
    let lu = x.u - TOLL_AT;
    // Negative toward the road.
    let lw = (x.w - x.toll_side * TOLL_OFFSET) * x.toll_side;
    if lu.abs() > 3.2 || lw.abs() > 2.6 {
        return None;
    }
    let inner = lu.abs() < 2.6 && lw.abs() < 2.0;
    if !inner {
        let door = lw < 0.0 && lu.abs() < 0.9;
        let window = lw > 0.0 && lu.abs() < 0.5 && (1.2..2.0).contains(&x.h);
        let corner = lu.abs() > 2.6 && lw.abs() > 2.0;
        let height = 0.5 + 3.4 * value_noise2(x.m.x * 0.45, x.m.z * 0.45, x.seed ^ 0x7011) + if corner { 1.2 } else { 0.0 };
        if (door && x.h > 0.0 && x.h < 2.4) || window {
            return Some(B::Air);
        }
        if x.on_ground(height) {
            return Some(if x.h < 0.9 { B::DressedStone } else { B::WeatheredBricks });
        }
        return None;
    }
    if x.h < 0.0 && x.h > -x.vm {
        return (hash2((x.m.x * 2.0).floor() as i32, (x.m.z * 2.0).floor() as i32, x.seed ^ 0x9e1) > 0.12)
            .then_some(B::Cobblestone);
    }
    // The toll counter faces the door.
    if lw.abs() < 0.35 && lu.abs() < 1.6 && x.h > 0.0 && x.h < 1.0 {
        return Some(B::DressedStone);
    }
    (x.h > 0.0 && x.h < 3.5).then_some(B::Air)
}

/// A crystal ring like a player's gate, failed and half fallen. The road
/// passes through it; its lower arc is buried under the paving.
fn quenched_ring(x: &Ctx) -> Option<B> {
    let du = x.u - RING_AT;
    if du.abs() > 3.0 || x.w.abs() > 8.0 {
        return None;
    }
    let ry = x.m.y - (x.datum(RING_AT) + 3.4);
    // At least a cell thick and diagonally connected at coarse scales.
    let (major, minor) = (3.7, 0.5f32.max(x.vm * 0.9));
    let around = x.w.hypot(ry);
    let angle = ry.atan2(x.w * x.toll_side);
    let missing = (0.25..1.35).contains(&angle) || (x.w.abs() < 2.6 && ry < -2.0);
    if !missing && (around - major).hypot(du) < minor && x.h > -x.vm {
        // Fewer shards where each one is a whole metre of crystal.
        let chance = if x.vm >= 1.0 { 0.15 } else { 0.35 };
        let shard = around < major - minor * 0.2 && hash3(x.p.x, x.p.y, x.p.z, x.seed ^ 0xc75) < chance;
        return Some(if shard { B::Crystal } else { B::Basalt });
    }
    let fw = x.w * x.toll_side;
    if (4.8..7.4).contains(&fw) && (-1.2..1.6).contains(&du) && x.on_ground(0.8)
        && value_noise2(x.m.x * 1.3, x.m.z * 1.3, x.seed ^ 0xfa11) > 0.35 {
        return Some(if hash3(x.p.x, x.p.y, x.p.z, x.seed ^ 0xc76) < 0.2 { B::Crystal } else { B::Basalt });
    }
    None
}

/// A low curtain wall around the tower, broken into runs, gated at the road.
fn enclosure(x: &Ctx) -> Option<B> {
    if (x.radius - ENCLOSURE).abs() > 0.45f32.max(x.vm * 0.5) || (x.u > 0.0 && x.w.abs() < 3.0) {
        return None;
    }
    let pier = x.u > 0.0 && x.w.abs() < 4.0;
    let height = if pier { 3.4 } else { 3.6 * value_noise2(x.m.x * 0.11, x.m.z * 0.11, x.seed ^ 0xe4c) - 0.9 };
    (height > 0.25 && x.on_ground(height)).then_some(if x.h < 1.0 { B::Cobblestone } else { B::WeatheredBricks })
}
