//! Hanging precinct in metres: approach, three routes, throne.
use super::plan::Plan;
use crate::BlockId as B;
use glam::Vec3;

pub const CRUST: f32 = 8.0;
pub const COURT_Y: f32 = 48.0;
pub const CHOIR_Y: f32 = 64.0;
pub const CISTERN_Y: f32 = 32.0;
pub const THRONE_Y: f32 = 72.0;

#[derive(Clone, Copy)]
pub struct Shape {
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
    pub floor: f32,
    pub wall: f32,
    pub roof: f32,
    pub part: u8,
}

pub const APPROACH: u8 = 0;
pub const COURT: u8 = 1;
pub const BRIDGE: u8 = 2;
pub const FOUNDRY: u8 = 3;
pub const CHOIR_STAIR: u8 = 4;
pub const CHOIR: u8 = 5;
pub const THRONE_STAIR: u8 = 6;
pub const THRONE: u8 = 7;
pub const CISTERN_STAIR: u8 = 8;
pub const CISTERN: u8 = 9;

fn shape(x0: f32, x1: f32, z0: f32, z1: f32, floor: f32, wall: f32, roof: f32, part: u8) -> Shape {
    Shape { x0, x1, z0, z1, floor, wall, roof, part }
}

pub fn shape_at(x: f32, z: f32) -> Option<Shape> {
    let rise = |z0: f32, z1: f32, y0: f32, y1: f32| { ((z - z0) / (z1 - z0)).clamp(0.0, 1.0) * (y1 - y0) + y0 };
    if x.abs() <= 3.2 && (42.0..58.0).contains(&z) {
        return Some(shape(-3.2, 3.2, 42.0, 58.0, rise(42.0, 58.0, COURT_Y, CHOIR_Y), 4.0, 0.0, CHOIR_STAIR));
    }
    if x.abs() <= 3.2 && (78.0..86.0).contains(&z) {
        return Some(shape(-3.2, 3.2, 78.0, 86.0, rise(78.0, 86.0, CHOIR_Y, THRONE_Y), 4.0, 0.0, THRONE_STAIR));
    }
    if (14.0..26.0).contains(&x) && (26.0..34.0).contains(&z) {
        let y = COURT_Y + ((x - 14.0) / 12.0).clamp(0.0, 1.0) * (CISTERN_Y - COURT_Y);
        return Some(shape(14.0, 26.0, 26.0, 34.0, y, 4.0, 0.0, CISTERN_STAIR));
    }
    if (-52.0..-28.0).contains(&x) && (18.0..42.0).contains(&z) {
        return Some(shape(-52.0, -28.0, 18.0, 42.0, COURT_Y, 7.0, 7.0, FOUNDRY));
    }
    if (-28.0..-14.0).contains(&x) && (26.0..34.0).contains(&z) {
        return Some(shape(-28.0, -14.0, 26.0, 34.0, COURT_Y, 5.0, 5.0, BRIDGE));
    }
    if (26.0..48.0).contains(&x) && (18.0..42.0).contains(&z) {
        return Some(shape(26.0, 48.0, 18.0, 42.0, CISTERN_Y, 6.0, 6.0, CISTERN));
    }
    if x.abs() <= 10.0 && (58.0..78.0).contains(&z) {
        return Some(shape(-10.0, 10.0, 58.0, 78.0, CHOIR_Y, 8.0, 8.0, CHOIR));
    }
    if x.abs() <= 8.0 && (86.0..102.0).contains(&z) {
        return Some(shape(-8.0, 8.0, 86.0, 102.0, THRONE_Y, 10.0, 10.0, THRONE));
    }
    if x.abs() <= 14.0 && (16.0..42.0).contains(&z) {
        return Some(shape(-14.0, 14.0, 16.0, 42.0, COURT_Y, 6.0, 0.0, COURT));
    }
    if x.abs() <= 3.5 && (0.0..16.0).contains(&z) {
        return Some(shape(-3.5, 3.5, 0.0, 16.0, COURT_Y, 5.0, 0.0, APPROACH));
    }
    None
}

pub fn floor_top(x: f32, z: f32) -> Option<f32> {
    shape_at(x, z).map(|s| s.floor)
}

fn edge(s: &Shape, x: f32, z: f32) -> bool {
    x - s.x0 < 1.0 || s.x1 - x < 1.0 || z - s.z0 < 1.0 || s.z1 - z < 1.0
}

fn door(x: f32, z: f32, y: f32, floor: f32) -> bool {
    if !(floor..floor + 3.2).contains(&y) {
        return false;
    }
    ((z - 16.0).abs() < 1.2 && x.abs() < 2.2)
        || ((z - 42.0).abs() < 1.2 && x.abs() < 2.2)
        || ((z - 58.0).abs() < 1.2 && x.abs() < 2.2)
        || ((z - 78.0).abs() < 1.2 && x.abs() < 2.2)
        || ((z - 86.0).abs() < 1.2 && x.abs() < 2.2)
        || (z < 1.2 && x.abs() < 2.2)
        || ((x + 14.0).abs() < 1.2 && (z - 30.0).abs() < 2.6)
        || ((x + 28.0).abs() < 1.2 && (z - 30.0).abs() < 2.6)
        || ((x - 14.0).abs() < 1.2 && (z - 30.0).abs() < 2.6)
        || ((x - 26.0).abs() < 1.2 && (z - 30.0).abs() < 2.6)
}

fn dist_seg(x: f32, z: f32, ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let dx = bx - ax;
    let dz = bz - az;
    let t = if dx == 0.0 && dz == 0.0 {
        0.0
    } else {
        ((x - ax) * dx + (z - az) * dz) / (dx * dx + dz * dz)
    };
    let t = t.clamp(0.0, 1.0);
    let px = ax + dx * t - x;
    let pz = az + dz * t - z;
    px.hypot(pz)
}

pub fn on_chart(x: f32, z: f32) -> bool {
    dist_seg(x, z, 0.0, 4.0, 0.0, 90.0) < 0.65
        || dist_seg(x, z, 0.0, 30.0, -40.0, 30.0) < 0.65
        || dist_seg(x, z, 0.0, 30.0, 30.0, 30.0) < 0.65
}

fn pier(x: f32, z: f32) -> bool {
    [(0.0, 2.0), (-12.0, 20.0), (12.0, 20.0), (-12.0, 38.0), (12.0, 38.0)]
        .into_iter()
        .any(|(px, pz)| (x - px).hypot(z - pz) < 1.35)
}

fn shelter(x: f32, z: f32, y: f32, floor: f32) -> Option<B> {
    if !(-11.0..-7.0).contains(&x) || !(20.0..24.0).contains(&z) || !(floor..floor + 2.2).contains(&y) {
        return None;
    }
    let rim = x + 11.0 < 1.0 || -7.0 - x < 1.0 || z - 20.0 < 1.0;
    if rim {
        return Some(B::Planks);
    }
    if (x + 9.0).abs() <= 0.55 && (z - 22.0).abs() <= 0.55 && y < floor + 1.0 {
        return Some(B::Workbench);
    }
    if (x + 8.0).abs() < 0.5 && (z - 21.0).abs() < 0.5 && y < floor + 1.0 {
        return Some(B::Bookcase);
    }
    None
}

fn slab(x: f32, y: f32, z: f32, voxel: f32) -> Option<B> {
    let half = voxel * 0.49;
    let mut lo = f32::MAX;
    let mut hi = f32::MIN;
    for (dx, dz) in [(0.0, 0.0), (-half, 0.0), (half, 0.0), (0.0, -half), (0.0, half)] {
        if let Some(top) = floor_top(x + dx, z + dz) {
            lo = lo.min(top - 1.0);
            hi = hi.max(top);
        }
    }
    (lo..hi).contains(&y).then_some(if on_chart(x, z) { B::Crystal } else { B::Limestone })
}

pub fn block_at(x: f32, y: f32, z: f32, voxel: f32, plan: &Plan) -> B {
    if y <= 1.0 {
        return B::Bedrock;
    }
    if y <= CRUST {
        return B::VoidStone;
    }
    if pier(x, z) && y < COURT_Y {
        return B::Basalt;
    }
    let Some(s) = shape_at(x, z) else {
        return B::Air;
    };
    if let Some(floor) = slab(x, y, z, voxel) {
        return floor;
    }
    if y < s.floor {
        return B::Air;
    }
    if s.roof > 0.0 && (s.floor + s.roof..s.floor + s.roof + 1.0).contains(&y) {
        let vent = s.part == FOUNDRY && ((x + 40.0) / 6.0).fract() < 0.18 && ((z - 30.0) / 6.0).fract() < 0.18;
        return if vent { B::Air } else { B::Limestone };
    }
    if edge(&s, x, z) && y < s.floor + s.wall && !door(x, z, y, s.floor) {
        return if s.part == THRONE { B::CopperBlock } else { B::Limestone };
    }
    if y < s.floor + 1.05 {
        if s.part == FOUNDRY && (z < plan.lava_south || z > plan.lava_north) {
            return B::Lava;
        }
        if s.part == FOUNDRY && x < -32.0 && x > -48.0 && ((plan.water_a0..plan.water_a1).contains(&z) || (plan.water_b0..plan.water_b1).contains(&z)) {
            return B::Water;
        }
        if s.part == CISTERN && x > plan.cistern {
            return B::Water;
        }
    }
    if s.part == CHOIR {
        let ring = (x.hypot(z - 68.0) - plan.ring).abs() < 0.7 && (s.floor + 3.0..s.floor + 4.0).contains(&y);
        if ring {
            return B::StarGlass;
        }
        let chain = plan.pillars.into_iter().any(|(cx, cz)| (x - cx).hypot(z - cz) < 0.55);
        if chain && y < s.floor + s.wall {
            return B::Basalt;
        }
    }
    if s.part == THRONE && x.abs() < 1.3 && (93.0..97.0).contains(&z) && y < s.floor + 3.4 {
        return B::CopperBlock;
    }
    if s.part == COURT && let Some(b) = shelter(x, z, y, s.floor) {
        return b;
    }
    B::Air
}

pub fn route_samples() -> Vec<Vec3> {
    let mut out = Vec::new();
    let seg = |out: &mut Vec<Vec3>, a: (f32, f32), b: (f32, f32)| {
        let steps = ((a.0 - b.0).hypot(a.1 - b.1) / 1.0).ceil() as i32;
        let steps = steps.max(1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = a.0 + (b.0 - a.0) * t;
            let z = a.1 + (b.1 - a.1) * t;
            let y = floor_top(x, z).unwrap_or(COURT_Y);
            out.push(Vec3::new(x, y, z));
        }
    };
    seg(&mut out, (0.0, 4.0), (0.0, 90.0));
    seg(&mut out, (0.0, 30.0), (-40.0, 30.0));
    seg(&mut out, (0.0, 30.0), (30.0, 30.0));
    out
}
