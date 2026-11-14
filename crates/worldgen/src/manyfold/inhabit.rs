//! Ruins, plants and lamps on the close maze. Versions through 112 stay plaster.
use glam::{IVec3, Vec3};

use super::maze;
use super::shell::{self, Class, Sample};
use crate::{hash_u32, BlockId};

const AROUND: i32 = 216;
const CELL_Z: f32 = 5.0;
const WALL_M: f32 = 1.0;
const REACH: f32 = 2.6;
const LOW_LO: f32 = -6.2;
const LOW_HI: f32 = -3.4;
const TALL_LO: f32 = -10.0;
const TALL_HI: f32 = -7.2;
const SPAWN_REACH: f32 = 8.0;
const ROAD_AROUND: i32 = 18;
const ROAD_ALONG: i32 = 24;
const ROAD_WIDTH: i32 = 3;
const HALL_SPAN: i32 = 5;
const HALL_WALL: f32 = 2.0;
const REFUGE: f32 = 16.0;

pub(super) fn lining(version: u32, seed: u32, s: &Sample) -> Option<BlockId> {
    if version < maze::ROOF {
        return ruins(version, seed, s);
    }
    if matches!(s.class, Class::Wall) {
        return Some(BlockId::StoneBricks);
    }
    let (around, along, frac_around, frac_along, radius) = maze::grid_at(version, s, AROUND, CELL_Z);
    let refuge = version >= maze::ARRIVAL && maze::open_disk(version, s, REFUGE);
    let tall = tall_cell(version, around, along) || refuge;
    let grass = grass_cell(version, around, along) || refuge;
    if matches!(s.class, Class::Crust) {
        return Some(if grass { BlockId::Grass } else { BlockId::Limestone });
    }
    let (low_lo, low_hi) = low_band(version);
    let inner = if tall { TALL_LO } else { low_lo };
    if !matches!(s.class, Class::Hollow) || !(inner..-0.02).contains(&s.above) {
        return None;
    }
    if tall && (TALL_LO..TALL_HI).contains(&s.above) {
        return Some(ceiling(frac_around, frac_along));
    }
    if !tall && (low_lo..low_hi).contains(&s.above) {
        return Some(ceiling(frac_around, frac_along));
    }
    if beacon(version, around, along, frac_around, frac_along, s.above) {
        return Some(BlockId::AmberCap);
    }
    let clear = maze::open_disk(version, s, SPAWN_REACH) || tall || grass;
    if !clear && wall(version, seed, radius, around, along, frac_around, frac_along) {
        return Some(BlockId::StoneBricks);
    }
    if grass {
        if let Some(grown) = plant(version, seed, around, along, frac_around, frac_along, s.above) {
            return Some(grown);
        }
    }
    Some(BlockId::Air)
}

fn tall_cell(version: u32, around: i32, along: i32) -> bool {
    if version >= maze::ROUTE {
        return road(around, along) || yard(around, along);
    }
    version >= maze::STOREY && chamber(around, along)
}

fn grass_cell(version: u32, around: i32, along: i32) -> bool {
    if version >= maze::ROUTE {
        return road(around, along) || yard(around, along);
    }
    chamber(around, along)
}

fn wall(
    version: u32,
    seed: u32,
    radius: f32,
    around: i32,
    along: i32,
    frac_around: f32,
    frac_along: f32,
) -> bool {
    if version >= maze::HALLS {
        return partition(radius, around, along, frac_around, frac_along);
    }
    maze::blocked_at(
        seed, radius, around, along, frac_around, frac_along, AROUND, CELL_Z, WALL_M,
    )
}

fn beacon(
    version: u32,
    around: i32,
    along: i32,
    frac_around: f32,
    frac_along: f32,
    above: f32,
) -> bool {
    version >= maze::ARRIVAL
        && around.rem_euclid(ROAD_AROUND) < ROAD_WIDTH
        && along.rem_euclid(ROAD_ALONG) < ROAD_WIDTH
        && (0.25..0.75).contains(&frac_around)
        && (0.25..0.75).contains(&frac_along)
        && (-6.5..-1.0).contains(&above)
}

fn road(around: i32, along: i32) -> bool {
    around.rem_euclid(ROAD_AROUND) < ROAD_WIDTH || along.rem_euclid(ROAD_ALONG) < ROAD_WIDTH
}

fn yard(around: i32, along: i32) -> bool {
    let a = around.rem_euclid(ROAD_AROUND);
    let z = along.rem_euclid(ROAD_ALONG);
    (8..10).contains(&a) && (8..10).contains(&z)
}

fn partition(radius: f32, around: i32, along: i32, frac_around: f32, frac_along: f32) -> bool {
    if road(around, along) || yard(around, along) {
        return false;
    }
    let a = around.rem_euclid(ROAD_AROUND);
    let z = along.rem_euclid(ROAD_ALONG);
    let cell = std::f32::consts::TAU / AROUND as f32;
    let west_strip = (HALL_WALL / radius.max(16.0)) / cell;
    let south_strip = HALL_WALL / CELL_Z;
    let west = a >= ROAD_WIDTH
        && (a - ROAD_WIDTH) % HALL_SPAN == 0
        && frac_around < west_strip
        && !(0.35..0.65).contains(&frac_along);
    let south = z >= ROAD_WIDTH
        && (z - ROAD_WIDTH) % HALL_SPAN == 0
        && frac_along < south_strip
        && !(0.35..0.65).contains(&frac_around);
    west || south
}

fn ceiling(frac_around: f32, frac_along: f32) -> BlockId {
    if (0.42..0.58).contains(&frac_around) && (0.42..0.58).contains(&frac_along) {
        BlockId::AmberCap
    } else {
        BlockId::StoneBricks
    }
}

fn ruins(version: u32, seed: u32, s: &Sample) -> Option<BlockId> {
    if matches!(s.class, Class::Wall) {
        return Some(BlockId::StoneBricks);
    }
    let (ia, iz, frac_a, frac_z, radius) = maze::grid_at(version, s, AROUND, CELL_Z);
    if matches!(s.class, Class::Crust) {
        return Some(if turf(seed, ia, iz) {
            BlockId::Grass
        } else {
            BlockId::Limestone
        });
    }
    if !matches!(s.class, Class::Hollow) || !(-maze::WALL_H..-0.02).contains(&s.above) {
        return None;
    }
    if maze::open_disk(version, s, REACH) {
        return Some(BlockId::Air);
    }
    if lamp(ia, iz) && (0.46..0.54).contains(&frac_a) && (0.46..0.54).contains(&frac_z) {
        if (-2.15..-0.15).contains(&s.above) {
            return Some(BlockId::StoneBricks);
        }
        if (-3.15..-2.15).contains(&s.above) {
            return Some(BlockId::AmberCap);
        }
    }
    if maze::blocked_at(
        seed, radius, ia, iz, frac_a, frac_z, AROUND, CELL_Z, WALL_M,
    ) && s.above > -wall_top(seed, ia, iz)
    {
        return Some(if s.above > -1.15 && rubble(seed, ia, iz) {
            BlockId::Cobblestone
        } else {
            BlockId::StoneBricks
        });
    }
    Some(plant(version, seed, ia, iz, frac_a, frac_z, s.above).unwrap_or(BlockId::Air))
}

fn chamber(ia: i32, iz: i32) -> bool {
    ia.rem_euclid(6) == 3 && iz.rem_euclid(6) == 3
}

fn turf(seed: u32, ia: i32, iz: i32) -> bool {
    hash_u32(seed ^ cell(ia, iz) ^ 0x51ED_270B) % 3 == 0
}

fn lamp(ia: i32, iz: i32) -> bool {
    ia.rem_euclid(2) == 0 && iz.rem_euclid(2) == 0
}

fn rubble(seed: u32, ia: i32, iz: i32) -> bool {
    hash_u32(seed ^ cell(ia, iz) ^ 0xA5A5_1357) % 2 == 0
}

fn wall_top(seed: u32, ia: i32, iz: i32) -> f32 {
    match hash_u32(seed ^ cell(ia, iz) ^ 0x1B87_3593) % 5 {
        0 => 4.0,
        1 => 7.0,
        _ => 11.0,
    }
}

fn low_band(version: u32) -> (f32, f32) {
    if version >= maze::ROOM {
        (-8.4, -5.6)
    } else {
        (LOW_LO, LOW_HI)
    }
}

fn plant(
    version: u32,
    seed: u32,
    ia: i32,
    iz: i32,
    frac_a: f32,
    frac_z: f32,
    above: f32,
) -> Option<BlockId> {
    let pick = hash_u32(seed ^ cell(ia, iz) ^ 0x6C62_272E);
    if version >= maze::ROOM {
        let on_wall = frac_a < 0.08 || frac_a > 0.92;
        if on_wall && (-1.15..-0.35).contains(&above) && pick % 5 == 0 {
            return Some(BlockId::Vines);
        }
        if (0.18..0.30).contains(&frac_a)
            && (0.22..0.38).contains(&frac_z)
            && (-1.05..-0.2).contains(&above)
            && pick % 5 == 1
        {
            return Some(BlockId::Leaves);
        }
        return None;
    }
    if (0.18..0.30).contains(&frac_a)
        && (0.22..0.38).contains(&frac_z)
        && (-1.05..-0.2).contains(&above)
        && pick % 5 == 1
    {
        return Some(BlockId::Leaves);
    }
    if (0.16..0.28).contains(&frac_a) && (-5.5..-1.2).contains(&above) && pick % 4 == 0 {
        return Some(BlockId::Vines);
    }
    None
}

fn cell(ia: i32, iz: i32) -> u32 {
    (ia as u32).wrapping_mul(0x9E37_79B9) ^ (iz as u32).wrapping_mul(0x85EB_CA6B)
}

pub fn life(cfg: &crate::WorldSampleConfig, player: Vec3) -> Vec<(IVec3, Vec3, u32)> {
    if cfg.generator_version < maze::CLOSE {
        return Vec::new();
    }
    let version = cfg.generator_version;
    let center = shell::center_of(version);
    let p = player - center;
    let radial = Vec3::new(p.x, p.y, 0.0);
    if radial.length() < 1.0 {
        return Vec::new();
    }
    let raw = radial.x.atan2(-radial.y).rem_euclid(std::f32::consts::TAU);
    let angle = if version >= shell::GRAND {
        (raw + maze::twist(version, p.z)).rem_euclid(std::f32::consts::TAU)
    } else {
        raw
    };
    if version >= maze::TWISTED {
        let around = super::bend::AROUND;
        let arc = std::f32::consts::TAU / around as f32;
        let cell_z = super::bend::CELL_Z;
        let ia0 = (angle / arc).floor() as i32;
        let iz0 = (p.z / cell_z).floor() as i32;
        let mut out = Vec::new();
        for iz in iz0 - 6..=iz0 + 6 {
            let bays = super::bend::bays(version, iz);
            let z = (iz as f32 + 0.5) * cell_z;
            let radius = shell::shell_radius(version, z) - 1.6;
            for unwrapped in ia0 - 8..=ia0 + 8 {
                let ia = unwrapped.rem_euclid(around);
                for sub in 0..bays {
                    let Some(kind) = super::bend::encounter(cfg.seed, ia, iz, sub) else { continue };
                    let turn = (ia as f32 + (sub as f32 + 0.55) / bays as f32) * arc - maze::twist(version, z);
                    let stand = center + Vec3::new(turn.sin() * radius, -turn.cos() * radius, z);
                    if (stand - player).length_squared() >= 42.0 * 42.0 { continue; }
                    let cell = (stand / cfg.voxel_meters()).floor().as_ivec3();
                    let sample = shell::sample(cfg, cell);
                    if maze::open_disk(version, &sample, 12.0) || !shell::block(cfg, cell).is_empty() { continue; }
                    out.push((IVec3::new(ia * 4 + sub, iz, 7), stand, kind));
                }
            }
        }
        return out;
    }
    let step = std::f32::consts::TAU / AROUND as f32;
    let ia0 = (angle / step).floor() as i32;
    let iz0 = (p.z / CELL_Z).floor() as i32;
    let mut out = Vec::new();
    for diz in -8..=8 {
        for dia in -8..=8 {
            let ia = ia0 + dia;
            let iz = iz0 + diz;
            if lamp(ia, iz) || hash_u32(cfg.seed ^ cell(ia, iz) ^ 0xC2B2_AE35) % 11 != 0 {
                continue;
            }
            let stand = stand(version, ia, iz);
            let away = (stand - player).length();
            // Stay put as the player arrives. An inner cutoff removed the relic
            // under their feet and the next one, farther out, took its place.
            if away >= 42.0 {
                continue;
            }
            let kind = hash_u32(cfg.seed ^ cell(ia, iz)) % 6;
            out.push((IVec3::new(ia.rem_euclid(AROUND), iz, 7), stand, kind));
        }
    }
    out
}

fn stand(version: u32, ia: i32, iz: i32) -> Vec3 {
    let step = std::f32::consts::TAU / AROUND as f32;
    let z = (iz as f32 + 0.5) * CELL_Z;
    let mut angle = (ia as f32 + 0.5) * step;
    if version >= shell::GRAND {
        angle = (angle - super::grand::twist(z)).rem_euclid(std::f32::consts::TAU);
    }
    let radius = shell::shell_radius(version, z) - 1.6;
    let center = shell::center_of(version);
    center + Vec3::new(angle.sin() * radius, -angle.cos() * radius, z)
}
