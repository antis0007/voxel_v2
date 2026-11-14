//! Seven districts on the swelling shell, and three illusion lessons.
use crate::BlockId;
use super::shell::{self, Class, Sample};

pub const VERSION: u32 = 105;

fn region(version: u32, z: f32) -> i32 {
    ((z / shell::district_span(version)).floor() as i32).rem_euclid(7)
}

fn mid_z(version: u32) -> f32 {
    shell::row_len(version) * 0.5
}

fn mid_a(version: u32) -> f32 {
    shell::pitch_for(version) * 0.5
}

pub fn dress(cfg: &crate::WorldSampleConfig, s: &Sample) -> BlockId {
    let version = cfg.generator_version;
    if let Some(block) = lane(version, s) {
        return block;
    }
    if let Some(block) = painted(version, s) {
        return block;
    }
    if let Some(block) = shaft(version, s) {
        return block;
    }
    if let Some(block) = court(version, s) {
        return block;
    }
    if let Some(block) = seam(version, s) {
        return block;
    }
    district(version, s)
}

/// The overhead wall is hollow. Its outer face is masonry you can walk through.
fn lane(version: u32, s: &Sample) -> Option<BlockId> {
    if region(version, s.z) != 0 || s.sector != 8 {
        return None;
    }
    let centre = (s.local_z - mid_z(version)).abs() < 0.45 && (0.05..0.09).contains(&s.local_a);
    if centre && (-1.6..-0.6).contains(&s.above) {
        return Some(BlockId::Bookcase);
    }
    if (0.04..0.11).contains(&s.local_a) && (-3.2..-0.45).contains(&s.above) {
        return Some(BlockId::Air);
    }
    if (0.04..0.11).contains(&s.local_a) && (-0.45..0.0).contains(&s.above) {
        return Some(BlockId::Planks);
    }
    if (0.11..0.16).contains(&s.local_a) && (-8.0..0.0).contains(&s.above) {
        return Some(BlockId::Limestone);
    }
    if s.local_a < 0.04 && (-8.0..0.0).contains(&s.above) {
        return Some(BlockId::MirageBrick);
    }
    None
}

/// A black floor one metre deep. The cell under the dark skin is stone.
fn painted(version: u32, s: &Sample) -> Option<BlockId> {
    if region(version, s.z) != 0 || s.sector != 3 || s.row.rem_euclid(10) != 2 {
        return None;
    }
    if (s.local_z - mid_z(version)).abs() > 1.1 || (s.local_a - mid_a(version)).abs() > 0.06 {
        return None;
    }
    if (-0.4..1.4).contains(&s.above) {
        return Some(BlockId::MirageVoid);
    }
    if (1.5..3.4).contains(&s.above) {
        return Some(BlockId::Limestone);
    }
    None
}

/// Stone-coloured skin over a short room. Steps climb back to the corridor.
fn shaft(version: u32, s: &Sample) -> Option<BlockId> {
    if s.sector != 0 || !(0.0..shell::district_span(version)).contains(&s.z) {
        return None;
    }
    if (s.local_a - mid_a(version)).abs() > 0.05 {
        return None;
    }
    let climb = s.z - 7.2;
    if (0.0..2.4).contains(&climb) {
        let tread = 4.2 - climb * 1.6;
        if (s.above - tread).abs() < 0.45 && (-0.2..4.6).contains(&s.above) {
            return Some(BlockId::Limestone);
        }
    }
    if (s.z - 6.3).abs() > 0.9 {
        return None;
    }
    if (-0.4..1.4).contains(&s.above) {
        return Some(BlockId::MirageBrick);
    }
    if (1.4..4.2).contains(&s.above) {
        return Some(BlockId::Air);
    }
    if (4.2..6.0).contains(&s.above) {
        return Some(BlockId::Limestone);
    }
    None
}

fn court(version: u32, s: &Sample) -> Option<BlockId> {
    if region(version, s.z) != 0 || s.sector != 0 || s.row.rem_euclid(10) != 0 {
        return None;
    }
    if (s.z - 6.3).abs() < 1.0 && (s.local_a - mid_a(version)).abs() < 0.06 {
        return None;
    }
    if matches!(s.class, Class::Wall) {
        return Some(BlockId::Air);
    }
    let mid = (s.local_a - mid_a(version)).abs() < 0.12;
    if mid && (s.local_z - mid_z(version)).abs() < 1.6 && (-0.2..1.8).contains(&s.above) {
        return Some(BlockId::Planks);
    }
    if mid && (s.local_z - 1.4).abs() < 1.6 {
        if (0.0..1.4).contains(&s.above) {
            return Some(BlockId::Water);
        }
        if (0.9..2.0).contains(&s.above) {
            return Some(BlockId::Limestone);
        }
    }
    if mid && (s.local_z - 2.2).abs() < 0.4 && (-2.4..-1.4).contains(&s.above) {
        return Some(BlockId::Crystal);
    }
    matches!(s.class, Class::Crust).then_some(BlockId::Grass)
}

/// A plank crossing where one district meets the next. The sides drop a metre.
fn seam(version: u32, s: &Sample) -> Option<BlockId> {
    let span = shell::district_span(version);
    let into = s.z - (s.z / span).floor() * span;
    if !(0.4..2.2).contains(&into) {
        return None;
    }
    let mid = (s.local_a - mid_a(version)).abs() < 0.04;
    if mid && (-0.3..0.5).contains(&s.above) {
        return Some(BlockId::Planks);
    }
    if !mid && (0.2..1.3).contains(&s.above) && matches!(s.class, Class::Crust) {
        return Some(BlockId::Air);
    }
    None
}

fn district(version: u32, s: &Sample) -> BlockId {
    let r = region(version, s.z);
    if r == 0 && matches!(s.class, Class::Wall) && (-1.7..-1.0).contains(&s.above) {
        return BlockId::GlazedTiles;
    }
    if r == 3 && matches!(s.class, Class::Hollow) {
        if let Some(step) = stair(version, s) {
            return step;
        }
    }
    if r == 5 {
        if let Some(block) = guest(version, s) {
            return block;
        }
    }
    if r == 2 && matches!(s.class, Class::Crust) && basin(version, s) {
        return if (0.0..1.4).contains(&s.above) { BlockId::Water } else { BlockId::Clay };
    }
    let cap = if version >= shell::GRAND {
        match r {
            5 => 72.0,
            6 => 40.0,
            _ => 120.0,
        }
    } else if version >= shell::VAST {
        match r {
            5 => 32.0,
            6 => 24.0,
            _ => 64.0,
        }
    } else {
        match r {
            5 => 4.0,
            6 => 3.0,
            _ => 10.0,
        }
    };
    if matches!(s.class, Class::Wall) && s.above < -cap {
        return BlockId::Air;
    }
    match s.class {
        Class::Bedrock => BlockId::Bedrock,
        Class::Hollow => {
            if r == 1 && lamp(s) {
                BlockId::Crystal
            } else if r == 6 && thicket(version, s) {
                BlockId::Leaves
            } else {
                BlockId::Air
            }
        }
        Class::Crust => floor(r),
        Class::Wall => wall(r, s.row),
    }
}

fn floor(r: i32) -> BlockId {
    match r {
        0 => BlockId::Limestone,
        1 => BlockId::WeatheredBricks,
        2 => BlockId::Clay,
        3 => BlockId::CarvedStone,
        4 => BlockId::LimePlaster,
        6 => BlockId::Grass,
        _ => BlockId::Grass,
    }
}

fn wall(r: i32, row: i32) -> BlockId {
    match r {
        0 => BlockId::Limestone,
        1 => BlockId::BronzeBlock,
        2 => BlockId::LimePlaster,
        3 => BlockId::StoneBricks,
        4 if row.rem_euclid(2) == 0 => BlockId::WhitePlaster,
        4 => BlockId::LimePlaster,
        5 => BlockId::WhitePlaster,
        _ => BlockId::DressedStone,
    }
}

fn stair(version: u32, s: &Sample) -> Option<BlockId> {
    if s.local_a < shell::pitch_for(version) * 0.7 {
        return None;
    }
    let n = (s.local_z / 1.2).floor();
    if !(0.0..5.0).contains(&n) {
        return None;
    }
    let tread = -0.4 - n * 0.5;
    ((s.above - tread).abs() < 0.45).then_some(BlockId::StoneBricks)
}

fn guest(version: u32, s: &Sample) -> Option<BlockId> {
    if !matches!(s.class, Class::Hollow) {
        return None;
    }
    let mid = (s.local_a - mid_a(version)).abs() < 0.08 && (s.local_z - mid_z(version)).abs() < 2.0;
    if mid && (-6.2..-5.2).contains(&s.above) {
        return Some(BlockId::Leaves);
    }
    if s.sector == 4 && (2.0..5.0).contains(&s.local_z) && (0.15..0.35).contains(&s.local_a)
        && (-2.0..0.0).contains(&s.above)
    {
        return Some(BlockId::Log);
    }
    None
}

fn basin(version: u32, s: &Sample) -> bool {
    s.sector.rem_euclid(2) == 0
        && (s.local_a - mid_a(version)).abs() < 0.12
        && (s.local_z - mid_z(version)).abs() < 2.0
}

fn lamp(s: &Sample) -> bool {
    s.row.rem_euclid(2) == 0
        && (s.local_a - 0.02).abs() < 0.03
        && (s.local_z - 2.0).abs() < 0.4
        && (-3.2..-2.4).contains(&s.above)
}

fn thicket(version: u32, s: &Sample) -> bool {
    (s.local_a - mid_a(version)).abs() < 0.05
        && (s.local_z - 3.0).abs() < 0.5
        && (-1.4..-0.6).contains(&s.above)
}
