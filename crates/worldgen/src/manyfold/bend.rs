//! Closed passages on the shell. Versions through 119 keep the open halls.
//! Cell size stays walkable at the 75 m waist, then splits again where the shell widens.
use super::maze;
use super::shell::{self, Class, Sample};
use crate::{hash_u32, BlockId};

pub(super) const AROUND: i32 = 80;
pub(super) const CELL_Z: f32 = 7.0;

const CLEAR: f32 = 2.6;
const REGION: i32 = 10;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Tight,
    Hall,
    Court,
    Alley,
}

pub(super) fn dress(version: u32, seed: u32, s: &Sample) -> Option<BlockId> {
    if matches!(s.class, Class::Wall) {
        return Some(BlockId::StoneBricks);
    }
    let (ia, iz, frac_a, frac_z, radius) = maze::grid_at(version, s, AROUND, CELL_Z);
    let kind = kind(seed, ia, iz);
    let twisted = version >= maze::TWISTED;
    if twisted && let Some(block) = gate_gallery(version, s, radius) {
        return Some(block);
    }
    let landmark = twisted && mix(seed, ia, iz, 0, 12) % 5 == 0;
    if matches!(s.class, Class::Crust) {
        if twisted && (0.38..0.62).contains(&frac_a) && matches!(kind, Kind::Hall | Kind::Court) {
            return Some(if landmark { BlockId::GlazedTiles } else { BlockId::CarvedStone });
        }
        return Some(if twisted && matches!(kind, Kind::Hall | Kind::Tight) { BlockId::DressedStone } else { floor(kind) });
    }
    if !matches!(s.class, Class::Hollow) {
        return None;
    }
    let (ceil_lo, ceil_hi) = ceiling(kind);
    if !(ceil_lo..-0.02).contains(&s.above) {
        return None;
    }
    if (ceil_lo..ceil_hi).contains(&s.above) {
        if landmark && kind == Kind::Court && (0.16..0.84).contains(&frac_a) && (0.16..0.84).contains(&frac_z) {
            return Some(BlockId::Air);
        }
        let cap = roof(kind, frac_a, frac_z);
        return Some(if twisted && cap != BlockId::AmberCap { BlockId::LimePlaster } else { cap });
    }
    if maze::open_disk(version, s, 8.0) {
        return Some(BlockId::Air);
    }
    let bays = bays(version, iz);
    let sub_f = frac_a * bays as f32;
    let sub = sub_f.floor() as i32;
    let sub_frac = sub_f - sub as f32;
    let bay_m = radius * std::f32::consts::TAU / AROUND as f32 / bays as f32;
    let wall_a = wall_m(kind, bay_m);
    let wall_z = wall_m(kind, CELL_Z);
    if barrier(
        seed, version, ia, iz, sub, bays, sub_frac, frac_z, bay_m, wall_a, wall_z,
    ) {
        if twisted {
            // Put an arch through occasional partitions. The shoulders remain
            // solid, and the opening narrows into a lintel above head height.
            let across = if sub_frac < wall_a / bay_m.max(0.1) { frac_z } else { sub_frac };
            let from_center = (across - 0.54).abs();
            if landmark && kind != Kind::Alley && from_center < 0.26 && s.above > -(3.9 - 4.0 * from_center) {
                return Some(BlockId::Air);
            }
            if (landmark || kind == Kind::Hall) && from_center < 0.10 && (-2.6..-1.6).contains(&s.above) {
                return Some(BlockId::AmberCap);
            }
            if s.above > -0.95 || (-3.5..-2.9).contains(&s.above) {
                return Some(if kind == Kind::Alley { BlockId::Basalt } else { BlockId::CarvedStone });
            }
        }
        return Some(if twisted && matches!(kind, Kind::Tight | Kind::Court) { BlockId::DressedStone } else { wall(kind) });
    }
    if (!twisted || kind == Kind::Tight && landmark) && vine(
        kind, seed, ia, iz, sub, sub_frac, frac_z, s.above, bay_m, wall_a, wall_z,
    ) {
        return Some(BlockId::Vines);
    }
    Some(BlockId::Air)
}

/// A pair of high vaulted galleries gives each mouth a readable landmark and
/// a distinct route into the surrounding maze. Both share the maze's twist.
fn gate_gallery(version: u32, s: &Sample, radius: f32) -> Option<BlockId> {
    let [(z, first), (_, second)] = maze::mouth_marks(version);
    let along = (s.z - z).abs();
    if along > 12.0 || !matches!(s.class, Class::Hollow | Class::Crust) {
        return None;
    }
    let angle = maze::unrolled(version, s);
    for (axis, tile) in [(first, BlockId::GlazedTiles), (second, BlockId::CarvedStone)] {
        let delta = (angle - axis).abs();
        let across = delta.min(std::f32::consts::TAU - delta) * radius;
        if across > 5.8 {
            continue;
        }
        let rib = (along - 5.0).abs() < 0.65 || (along - 10.0).abs() < 0.65;
        if matches!(s.class, Class::Crust) {
            return Some(if across < 1.25 { tile } else { BlockId::DressedStone });
        }
        let vault = 4.0 + 4.6 * (1.0 - (across / 5.8).powi(2));
        let vault_base = vault - if rib { 0.6 } else { 0.0 };
        let height = -s.above;
        if (vault_base..vault + 1.2).contains(&height) {
            return Some(if rib { BlockId::DressedStone } else { BlockId::LimePlaster });
        }
        if (0.0..vault).contains(&height) {
            if rib && across > 4.7 {
                return Some(if (2.2..3.3).contains(&height) { BlockId::AmberCap } else { BlockId::DressedStone });
            }
            return Some(BlockId::Air);
        }
    }
    None
}

pub(super) fn bays(version: u32, iz: i32) -> i32 {
    let z = (iz as f32 + 0.5) * CELL_Z;
    let arc = shell::shell_radius(version, z) * std::f32::consts::TAU / AROUND as f32;
    (arc / 7.0).round().clamp(1.0, 4.0) as i32
}

pub(super) fn east_open(seed: u32, ia: i32, iz: i32, sub: i32) -> bool {
    let pick = mix(seed, ia, iz, sub, 1);
    (pick & 1 == 0) || extra(kind(seed, ia, iz), pick)
}

pub(super) fn north_open(seed: u32, ia: i32, iz: i32, sub: i32) -> bool {
    let pick = mix(seed, ia, iz, sub, 1);
    (pick & 1 == 1) || extra(kind(seed, ia, iz), pick)
}

fn barrier(
    seed: u32,
    version: u32,
    ia: i32,
    iz: i32,
    sub: i32,
    bays: i32,
    sub_frac: f32,
    frac_z: f32,
    bay_m: f32,
    wall_a: f32,
    wall_z: f32,
) -> bool {
    let west =
        wall_a > 0.0 && sub_frac < wall_a / bay_m.max(0.1) && west_closed(seed, ia, iz, sub, bays);
    let south =
        wall_z > 0.0 && frac_z < wall_z / CELL_Z && south_closed(seed, version, ia, iz, sub);
    west || south
}

fn west_closed(seed: u32, ia: i32, iz: i32, sub: i32, bays: i32) -> bool {
    if sub == 0 {
        let prev = (ia - 1).rem_euclid(AROUND);
        !east_open(seed, prev, iz, bays - 1)
    } else {
        !east_open(seed, ia, iz, sub - 1)
    }
}

fn south_closed(seed: u32, version: u32, ia: i32, iz: i32, sub: i32) -> bool {
    if bays(version, iz) != bays(version, iz - 1) {
        return false;
    }
    !north_open(seed, ia, iz - 1, sub)
}

fn wall_m(kind: Kind, span: f32) -> f32 {
    let want: f32 = match kind {
        Kind::Alley => 1.6,
        Kind::Tight => 1.25,
        Kind::Hall => 1.05,
        Kind::Court => 1.0,
    };
    let cap = span - CLEAR;
    if cap < 1.0 {
        0.0
    } else {
        want.min(cap)
    }
}

pub(super) fn encounter(seed: u32, ia: i32, iz: i32, sub: i32) -> Option<u32> {
    let pick = mix(seed, ia, iz, sub, 14);
    let (rarity, roster) = match kind(seed, ia, iz) {
        Kind::Tight => (13, [0, 1, 3]),
        Kind::Hall => (18, [2, 4, 5]),
        Kind::Court => (24, [0, 2, 5]),
        Kind::Alley => (9, [1, 3, 4]),
    };
    (pick % rarity == 0).then(|| roster[(pick / rarity % 3) as usize])
}

fn kind(seed: u32, ia: i32, iz: i32) -> Kind {
    match mix(seed, ia.div_euclid(REGION), iz.div_euclid(REGION), 0, 4) % 4 {
        0 => Kind::Tight,
        1 => Kind::Hall,
        2 => Kind::Court,
        _ => Kind::Alley,
    }
}

fn extra(kind: Kind, pick: u32) -> bool {
    match kind {
        Kind::Alley => false,
        Kind::Tight => pick % 6 == 0,
        Kind::Hall => pick % 4 == 0,
        Kind::Court => pick % 3 != 0,
    }
}

fn ceiling(kind: Kind) -> (f32, f32) {
    match kind {
        Kind::Alley => (-6.4, -4.0),
        Kind::Tight => (-7.6, -4.6),
        Kind::Hall => (-10.4, -6.8),
        Kind::Court => (-12.6, -8.2),
    }
}

fn floor(kind: Kind) -> BlockId {
    match kind {
        Kind::Tight => BlockId::Limestone,
        Kind::Hall => BlockId::StoneBricks,
        Kind::Court => BlockId::Grass,
        Kind::Alley => BlockId::Cobblestone,
    }
}

fn wall(kind: Kind) -> BlockId {
    match kind {
        Kind::Tight => BlockId::StoneBricks,
        Kind::Hall => BlockId::WhitePlaster,
        Kind::Court => BlockId::StoneBricks,
        Kind::Alley => BlockId::Cobblestone,
    }
}

fn roof(kind: Kind, frac_a: f32, frac_z: f32) -> BlockId {
    let lamp = matches!(kind, Kind::Hall | Kind::Court)
        && (0.40..0.60).contains(&frac_a)
        && (0.40..0.60).contains(&frac_z);
    if lamp {
        BlockId::AmberCap
    } else {
        wall(kind)
    }
}

fn vine(
    kind: Kind,
    seed: u32,
    ia: i32,
    iz: i32,
    sub: i32,
    sub_frac: f32,
    frac_z: f32,
    above: f32,
    bay_m: f32,
    wall_a: f32,
    wall_z: f32,
) -> bool {
    if !matches!(kind, Kind::Tight | Kind::Alley) || !(-2.4..-0.45).contains(&above) {
        return false;
    }
    let beside = sub_frac < wall_a / bay_m.max(0.1) + 0.08 || frac_z < wall_z / CELL_Z + 0.08;
    beside && mix(seed, ia, iz, sub, 9) % 5 == 0
}

fn mix(seed: u32, ia: i32, iz: i32, sub: i32, salt: u32) -> u32 {
    hash_u32(
        seed ^ (ia as u32).wrapping_mul(0x9E37_79B9)
            ^ (iz as u32).wrapping_mul(0x85EB_CA6B)
            ^ (sub as u32).wrapping_mul(0xC2B2_AE35)
            ^ salt.wrapping_mul(0x27D4_EB2D),
    )
}
