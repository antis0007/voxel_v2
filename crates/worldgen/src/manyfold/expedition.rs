//! Balcony, wall street, floor route, high route, and court.
//! Placed from the bay length, so the vast shell and the grand drum both carry it.
//! Versions through 107 keep the dream sampler.
use crate::BlockId;
use glam::Vec3;

use super::shell::{self, Class, Sample};

pub const VERSION: u32 = 108;

struct Bay {
    row: f32,
    mid: f32,
    pitch: f32,
}

fn bay(version: u32) -> Bay {
    let row = shell::row_len(version);
    Bay {
        row,
        mid: row * 0.5,
        pitch: shell::pitch_for(version),
    }
}

pub fn dress(cfg: &crate::WorldSampleConfig, s: &Sample) -> BlockId {
    if let Some(block) = built(bay(cfg.generator_version), s) {
        return block;
    }
    super::dream::dress(cfg, s)
}

/// Metres. On the balcony floor, one stride off the spawn cell.
pub fn cradle(cfg: &crate::WorldSampleConfig) -> Vec3 {
    let version = cfg.generator_version;
    let b = bay(version);
    let z = b.mid + 18.0;
    let mut angle = b.pitch * 0.5;
    if version >= shell::GRAND {
        angle = (angle - super::grand::twist(z)).rem_euclid(std::f32::consts::TAU);
    }
    let dist = shell::shell_radius(version, z) - 1.2;
    let center = shell::center_of(version);
    center + Vec3::new(angle.sin() * dist, -angle.cos() * dist, z)
}

fn built(b: Bay, s: &Sample) -> Option<BlockId> {
    if plug(&b, s) {
        return None;
    }
    street(&b, s)
        .or_else(|| stair(&b, s))
        .or_else(|| ledge(&b, s))
        .or_else(|| court(&b, s))
        .or_else(|| routes(&b, s))
}

fn plug(b: &Bay, s: &Sample) -> bool {
    s.sector == 4 && s.row == 1 && (s.local_z - b.mid).abs() < 1.2
}

fn street(b: &Bay, s: &Sample) -> Option<BlockId> {
    let (z0, z1) = (b.mid * 0.25, b.mid * 0.55);
    let a0 = b.pitch * 0.08;
    let a1 = b.pitch * 0.34;
    if s.sector != 2
        || s.row != 0
        || !(z0..z1).contains(&s.local_z)
        || !(a0..a1).contains(&s.local_a)
    {
        return None;
    }
    let door = (z1 - 2.5..z1).contains(&s.local_z)
        && (b.pitch * 0.14..b.pitch * 0.24).contains(&s.local_a)
        && (-5.0..-0.4).contains(&s.above);
    if door {
        return Some(BlockId::Air);
    }
    let skin = s.local_z < z0 + 2.4
        || s.local_z > z1 - 2.6
        || s.local_a < a0 + b.pitch * 0.04
        || s.local_a > a1 - b.pitch * 0.04;
    if skin && (-14.0..0.0).contains(&s.above) {
        return Some(BlockId::Limestone);
    }
    if (z0 + 3.5..z0 + 6.5).contains(&s.local_z)
        && (b.pitch * 0.16..b.pitch * 0.22).contains(&s.local_a)
        && (-2.6..-1.3).contains(&s.above)
    {
        return Some(BlockId::Bookcase);
    }
    if (-1.2..-0.3).contains(&s.above) {
        return Some(BlockId::Planks);
    }
    if (-8.0..-1.2).contains(&s.above) {
        return Some(BlockId::Air);
    }
    None
}

fn stair(b: &Bay, s: &Sample) -> Option<BlockId> {
    let up = s.sector == 0
        && s.row == 0
        && high(b, s)
        && (b.mid * 0.55..b.mid - 2.0).contains(&s.local_z);
    let down = s.sector == 4
        && s.row == 0
        && high(b, s)
        && (b.mid + 8.0..b.mid + 32.0).contains(&s.local_z);
    let (z0, z1, rising) = if up {
        (b.mid * 0.55, b.mid - 2.0, true)
    } else if down {
        (b.mid + 8.0, b.mid + 32.0, false)
    } else {
        return None;
    };
    let t = (s.local_z - z0) / (z1 - z0);
    let surface = if rising {
        -1.2 - 20.8 * t
    } else {
        -22.0 + 20.8 * t
    };
    if (surface - 1.0..surface).contains(&s.above) {
        return Some(BlockId::Limestone);
    }
    if matches!(s.class, Class::Wall) && (surface..surface + 3.5).contains(&s.above) {
        return Some(BlockId::Air);
    }
    None
}

fn ledge(b: &Bay, s: &Sample) -> Option<BlockId> {
    if s.row != 0
        || !(0..=4).contains(&s.sector)
        || !high(b, s)
        || !(b.mid - 10.0..b.mid + 10.0).contains(&s.local_z)
    {
        return None;
    }
    if (-23.2..-20.0).contains(&s.above) {
        return Some(BlockId::Limestone);
    }
    if matches!(s.class, Class::Wall) && (-20.8..-16.5).contains(&s.above) {
        return Some(BlockId::Air);
    }
    None
}

fn court(b: &Bay, s: &Sample) -> Option<BlockId> {
    if s.sector != 4 || s.row != 1 {
        return None;
    }
    if (b.mid - 40.0..b.mid - 28.0).contains(&s.local_z)
        && (b.pitch * 0.15..b.pitch * 0.35).contains(&s.local_a)
        && (-0.6..2.0).contains(&s.above)
    {
        return Some(BlockId::Water);
    }
    if (b.mid - 24.0..b.mid - 20.0).contains(&s.local_z)
        && (b.pitch * 0.12..b.pitch * 0.20).contains(&s.local_a)
        && (-2.4..-0.2).contains(&s.above)
    {
        return Some(BlockId::Planks);
    }
    if (b.mid + 12.0..b.mid + 18.0).contains(&s.local_z)
        && (b.pitch * 0.40..b.pitch * 0.55).contains(&s.local_a)
        && (-3.4..-2.2).contains(&s.above)
    {
        return Some(BlockId::Crystal);
    }
    None
}

fn routes(b: &Bay, s: &Sample) -> Option<BlockId> {
    if !matches!(s.class, Class::Wall) {
        return None;
    }
    let room = s.sector == 4
        && s.row == 1
        && (b.mid + 8.0..b.mid + 40.0).contains(&s.local_z)
        && (b.pitch * 0.10..b.pitch * 0.70).contains(&s.local_a);
    if room && s.above > -16.0 {
        return Some(BlockId::Air);
    }
    if s.above <= -8.0 {
        return None;
    }
    let low = s.row == 0
        && (0..=4).contains(&s.sector)
        && (b.mid - 8.0..b.mid + 8.0).contains(&s.local_z);
    let gallery = s.sector == 4
        && s.row == 0
        && (b.mid + 8.0..b.row).contains(&s.local_z)
        && (b.pitch * 0.08..b.pitch * 0.28).contains(&s.local_a);
    let into_court = s.sector == 4
        && s.row == 1
        && (0.0..b.mid - 12.0).contains(&s.local_z)
        && (b.pitch * 0.08..b.pitch * 0.28).contains(&s.local_a);
    let arrival =
        s.sector == 4 && s.row == 1 && (0.0..b.mid - 12.0).contains(&s.local_z) && high(b, s);
    (low || gallery || into_court || arrival).then_some(BlockId::Air)
}

fn high(b: &Bay, s: &Sample) -> bool {
    (b.pitch * 0.55..b.pitch * 0.75).contains(&s.local_a)
}
