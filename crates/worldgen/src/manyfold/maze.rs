//! Block maze in the bay after the court, then a maze filling the shell.
//! Versions through 109 keep the grand drum. Versions through 111 keep the one bay.
//! Versions through 112 keep the plaster field. 113 fills that field with ruins.
use glam::Vec3;

use super::shell::{self, Class, Sample};
use crate::{hash_u32, BlockId};

pub const VERSION: u32 = 110;

/// Seeded maze around the whole shell. Versions through 111 keep the single bay.
pub const FIELD: u32 = 112;

/// Stone maze, plants, lamps and nearby creatures. Versions through 112 stay plaster.
pub const CLOSE: u32 = 113;

/// Roofed rooms. Versions through 113 keep the open drum.
pub const ROOF: u32 = 114;

/// Chambers rise above the corridor ceiling. Versions through 114 keep one storey.
pub const STOREY: u32 = 115;

/// Avenues and rings are cut before the maze walls. Versions through 115 keep the hash.
pub const ROUTE: u32 = 116;

/// Rooms with doors fill the blocks between the roads. Versions through 116 keep the hash.
pub const HALLS: u32 = 117;

/// Spawn court and a lamp at each crossing. Versions through 117 keep the plain halls.
pub const ARRIVAL: u32 = 118;

/// Taller corridors and vines only on the wall, one voxel deep.
/// Versions through 118 keep the 3.4 m cap and the vine slabs.
pub const ROOM: u32 = 119;

/// Waist-wide passages with closed edges, and a two-metre bedrock rind.
/// Versions through 119 keep the open hall grid and the half-metre rind.
pub const BEND: u32 = 120;

/// Twisted districts, vaulted crossings, and courtyards on the shell.
pub const TWISTED: u32 = 244;

pub(super) fn twist(version: u32, z: f32) -> f32 {
    let base = super::grand::twist(z);
    if version < TWISTED {
        return base;
    }
    base + 0.055 * (z * 0.028).sin() + 0.012 * (z * 0.081).sin()
}

const CELL_Z: f32 = 8.0;
const WALL_M: f32 = 1.2;
pub(super) const WALL_H: f32 = 11.0;
const AROUND: i32 = 96;

const CELL: f32 = 12.0;
const ACROSS: i32 = 5;
const ALONG: i32 = 4;

pub struct Mouth {
    pub center: Vec3,
    pub into_maze: Vec3,
    pub up: Vec3,
}

pub fn mouths(version: u32) -> Option<(Mouth, Mouth)> {
    if version < VERSION {
        return None;
    }
    let (mid, pitch) = bay(version);
    let z = door_z(mid);
    let above = -1.55;
    Some((
        mouth(version, 4, pitch * 0.90, z, above),
        mouth(version, 5, pitch * 0.08, z, above),
    ))
}

pub(super) fn dress(version: u32, seed: u32, s: &Sample) -> Option<BlockId> {
    if version >= BEND {
        return super::bend::dress(version, seed, s);
    }
    if version >= CLOSE {
        return super::inhabit::lining(version, seed, s);
    }
    if version >= FIELD {
        return field(version, seed, s);
    }
    if version < VERSION || s.row != 1 {
        return None;
    }
    let b = bay(version);
    rooms(b, s).or_else(|| throat(b, s))
}

fn field(version: u32, seed: u32, s: &Sample) -> Option<BlockId> {
    if matches!(s.class, Class::Crust) {
        return Some(BlockId::Limestone);
    }
    if !matches!(s.class, Class::Hollow) || !(-WALL_H..-0.02).contains(&s.above) {
        return None;
    }
    if reserved(version, s) {
        return Some(BlockId::Air);
    }
    let (ia, iz, frac_a, frac_z, radius) = grid(version, s);
    Some(if blocked(seed, radius, ia, iz, frac_a, frac_z) {
        BlockId::WhitePlaster
    } else {
        BlockId::Air
    })
}

fn grid(version: u32, s: &Sample) -> (i32, i32, f32, f32, f32) {
    grid_at(version, s, AROUND, CELL_Z)
}

pub(super) fn grid_at(
    version: u32,
    s: &Sample,
    around: i32,
    cell_z: f32,
) -> (i32, i32, f32, f32, f32) {
    let angle = unrolled(version, s);
    let cell = std::f32::consts::TAU / around as f32;
    let scaled = angle / cell;
    let ia = (scaled.floor() as i32).rem_euclid(around);
    let frac_a = scaled - scaled.floor();
    let scaled_z = s.z / cell_z;
    let iz = scaled_z.floor() as i32;
    let frac_z = scaled_z - scaled_z.floor();
    (ia, iz, frac_a, frac_z, shell::shell_radius(version, s.z))
}

pub(super) fn unrolled(version: u32, s: &Sample) -> f32 {
    let pitch = shell::pitch_for(version);
    (s.sector as f32 * pitch + s.local_a).rem_euclid(std::f32::consts::TAU)
}

fn reserved(version: u32, s: &Sample) -> bool {
    open_disk(version, s, 6.0)
}

pub(super) fn open_disk(version: u32, s: &Sample, reach: f32) -> bool {
    let (z, angle) = spawn_mark(version);
    if near(version, s, z, angle, reach) {
        return true;
    }
    mouth_marks(version).into_iter().any(|(z, angle)| near(version, s, z, angle, 3.4))
}

fn spawn_mark(version: u32) -> (f32, f32) {
    let z = shell::row_len(version) * 0.5;
    let angle = (shell::pitch_for(version) * 0.5 + twist(version, z)).rem_euclid(std::f32::consts::TAU);
    (z, angle)
}

pub(super) fn mouth_marks(version: u32) -> [(f32, f32); 2] {
    let row = shell::row_len(version);
    let z = row + row * 0.5 + 24.0;
    let pitch = shell::pitch_for(version);
    [
        (z, (pitch * 4.90).rem_euclid(std::f32::consts::TAU)),
        (z, (pitch * 5.08).rem_euclid(std::f32::consts::TAU)),
    ]
}

fn near(version: u32, s: &Sample, z: f32, angle: f32, reach: f32) -> bool {
    if (s.z - z).abs() > reach {
        return false;
    }
    let mut delta = (unrolled(version, s) - angle).abs();
    delta = delta.min(std::f32::consts::TAU - delta);
    delta * shell::shell_radius(version, s.z) < reach
}

fn blocked(seed: u32, radius: f32, ia: i32, iz: i32, frac_a: f32, frac_z: f32) -> bool {
    blocked_at(seed, radius, ia, iz, frac_a, frac_z, AROUND, CELL_Z, WALL_M)
}

pub(super) fn blocked_at(
    seed: u32,
    radius: f32,
    ia: i32,
    iz: i32,
    frac_a: f32,
    frac_z: f32,
    around: i32,
    cell_z: f32,
    wall_m: f32,
) -> bool {
    let cell = std::f32::consts::TAU / around as f32;
    let wall_a = (wall_m / radius.max(16.0)) / cell;
    let prev = (ia - 1).rem_euclid(around);
    let west = frac_a < wall_a && !opens_east(seed, prev, iz) && !extra(seed, prev, iz, 1);
    let south = frac_z < wall_m / cell_z
        && opens_east(seed, ia, iz.wrapping_sub(1))
        && !extra(seed, ia, iz.wrapping_sub(1), 2);
    west || south
}

fn opens_east(seed: u32, i: i32, j: i32) -> bool {
    hash_u32(seed ^ (i as u32).wrapping_mul(0x9E37_79B9) ^ (j as u32).wrapping_mul(0x85EB_CA6B)) & 1 == 0
}

fn extra(seed: u32, i: i32, j: i32, which: u32) -> bool {
    hash_u32(
        seed ^ (i as u32).wrapping_mul(0x27D4_EB2D)
            ^ (j as u32).wrapping_mul(0x1656_67B1)
            ^ which.wrapping_mul(0xC2B2_AE35),
    ) % 6
        == 0
}

fn bay(version: u32) -> (f32, f32) {
    let row = shell::row_len(version);
    (row * 0.5, shell::pitch_for(version))
}

fn door_z(mid: f32) -> f32 {
    mid + 24.0
}

fn origin_a(pitch: f32) -> f32 {
    pitch * 0.20
}

fn mouth(version: u32, sector: i32, local_a: f32, local_z: f32, above: f32) -> Mouth {
    let row = shell::row_len(version);
    let z = row + local_z;
    let raw = sector as f32 * shell::pitch_for(version) + local_a - shell_twist(version, z);
    let dist = shell::shell_radius(version, z) + above;
    let center = shell::center_of(version);
    Mouth {
        center: center + Vec3::new(raw.sin() * dist, -raw.cos() * dist, z),
        into_maze: Vec3::new(raw.cos(), raw.sin(), 0.0),
        up: Vec3::new(-raw.sin(), raw.cos(), 0.0),
    }
}

fn shell_twist(version: u32, z: f32) -> f32 {
    if version >= shell::GRAND {
        twist(version, z)
    } else {
        0.0
    }
}

fn rooms(b: (f32, f32), s: &Sample) -> Option<BlockId> {
    let (u, v) = cell(b, s)?;
    if !(-6.5..-0.4).contains(&s.above) {
        return None;
    }
    Some(if open(u, v) {
        BlockId::Air
    } else {
        BlockId::Limestone
    })
}

fn throat(b: (f32, f32), s: &Sample) -> Option<BlockId> {
    if !matches!(s.class, Class::Wall) || !(-8.0..-0.4).contains(&s.above) {
        return None;
    }
    let (mid, pitch) = b;
    if (s.local_z - door_z(mid)).abs() > 5.0 {
        return None;
    }
    let approach = s.sector == 4 && (pitch * 0.68..pitch).contains(&s.local_a);
    let entry = s.sector == 5 && (0.0..origin_a(pitch)).contains(&s.local_a);
    (approach || entry).then_some(BlockId::Air)
}

fn cell(b: (f32, f32), s: &Sample) -> Option<(i32, i32)> {
    if s.sector != 5 {
        return None;
    }
    let (mid, pitch) = b;
    let u = ((s.local_z - (mid + 18.0)) / CELL).floor() as i32;
    let v = ((s.local_a - origin_a(pitch)) / (pitch * 0.14)).floor() as i32;
    ((0..ACROSS).contains(&u) && (0..ALONG).contains(&v)).then_some((u, v))
}

fn open(u: i32, v: i32) -> bool {
    matches!(
        (u, v),
        (0, 0)
            | (0, 1)
            | (0, 2)
            | (1, 2)
            | (2, 2)
            | (2, 1)
            | (2, 0)
            | (3, 0)
            | (4, 0)
            | (4, 1)
            | (4, 2)
            | (4, 3)
            | (3, 3)
            | (2, 3)
            | (1, 3)
    )
}
