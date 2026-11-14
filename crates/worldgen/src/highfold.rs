//! The Highfold: islands hanging over a shallow sea. Each 88 m cell may hold
//! one island: a grassed top, a stone core and a long tapering keel that
//! trails roots. Everything is in world metres, the same at every voxel scale.
//!
//! No historical world is a Highfold realm, so this first layout needs no
//! generator gate. Any later change to it must gate on `generator_version`.
use crate::{hash3, value_noise2, value_noise3, BlockId, ColumnSample, WorldSampleConfig};
use glam::{IVec3, Vec2, Vec3};

/// Sea surface and floor.
pub const SEA_M: f32 = 8.0;
const SEABED_M: f32 = -18.0;
/// Island cell edge.
const CELL_M: f32 = 88.0;
/// The arrival island over the origin.
const HOME_TOP_M: f32 = 120.0;
const HOME_R: f32 = 30.0;

const ISLE: u32 = 0x4849_4746;
const TREE: u32 = 0x5452_4545;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Island {
    pub centre: Vec2,
    pub top: f32,
    pub radius: f32,
    /// Keel depth below the top.
    pub keel: f32,
}

/// The island of a cell, or None for open sky. The origin cell always holds
/// the arrival island.
pub fn island(seed: u32, cell: (i32, i32)) -> Option<Island> {
    if cell == (0, 0) {
        return Some(Island { centre: Vec2::ZERO, top: HOME_TOP_M, radius: HOME_R, keel: HOME_R * 1.3 });
    }
    // Keep the arrival island clear of neighbours that would crowd it.
    if cell.0.abs() <= 1 && cell.1.abs() <= 1 && hash3(cell.0, 1, cell.1, seed ^ ISLE) < 0.5 {
        return None;
    }
    if hash3(cell.0, 0, cell.1, seed ^ ISLE) < 0.38 {
        return None;
    }
    let j = |k: i32| hash3(cell.0, k, cell.1, seed ^ ISLE);
    let radius = 10.0 + 24.0 * j(2) * j(2);
    Some(Island {
        centre: Vec2::new((cell.0 as f32 + 0.25 + 0.5 * j(3)) * CELL_M, (cell.1 as f32 + 0.25 + 0.5 * j(4)) * CELL_M),
        top: 60.0 + 140.0 * j(5),
        radius,
        keel: radius * (0.8 + 0.9 * j(6)),
    })
}

fn cell_of(x: f32, z: f32) -> (i32, i32) {
    ((x / CELL_M).floor() as i32, (z / CELL_M).floor() as i32)
}

/// Height of the grass top at `x, z`, with `d` the warped radial distance.
fn surface(seed: u32, isle: &Island, x: f32, z: f32, d: f32) -> f32 {
    isle.top + (1.0 - d * d) * 3.5 + (value_noise2(x / 9.0, z / 9.0, seed ^ ISLE) - 0.5) * 2.0
}

/// Top of the arrival island over the origin metre.
fn home_top(seed: u32) -> f32 {
    let Some(isle) = island(seed, (0, 0)) else { return HOME_TOP_M };
    let warp = 0.78 + 0.44 * value_noise2(0.5 / 17.0, 0.5 / 17.0, seed ^ ISLE.rotate_left(3));
    let d = Vec2::new(0.5, 0.5).length() / (isle.radius * warp);
    surface(seed, &isle, 0.5, 0.5, d)
}

/// Solid island material at `m`, or None for air. `d` is the horizontal
/// distance to the island centre over its radius, warped by noise so no
/// island is a disc.
fn island_block(seed: u32, isle: &Island, m: Vec3) -> Option<BlockId> {
    let flat = Vec2::new(m.x, m.z) - isle.centre;
    let warp = 0.78 + 0.44 * value_noise2(m.x / 17.0, m.z / 17.0, seed ^ ISLE.rotate_left(3));
    let d = flat.length() / (isle.radius * warp);
    if d >= 1.0 {
        return None;
    }
    // Top: a low dome with hummocks; keel: a cone with a ragged underside.
    let top = surface(seed, isle, m.x, m.z, d);
    let rag = value_noise3(m.x / 6.0, m.y / 6.0, m.z / 6.0, seed ^ ISLE.rotate_left(9));
    let bottom = isle.top - isle.keel * (1.0 - d).powf(0.7) * (0.75 + 0.5 * rag);
    if m.y > top || m.y < bottom {
        return None;
    }
    let depth = top - m.y;
    Some(if depth < 1.0 {
        BlockId::Grass
    } else if depth < 3.5 {
        BlockId::Dirt
    } else if rag > 0.78 && depth > 6.0 {
        BlockId::Crystal
    } else {
        BlockId::Limestone
    })
}

/// Roots trailing under a keel, and a sparse tree on top.
fn growth(seed: u32, isle: &Island, m: Vec3, vm: f32) -> Option<BlockId> {
    let flat = Vec2::new(m.x, m.z) - isle.centre;
    let d = flat.length() / isle.radius;
    if d >= 1.0 {
        return None;
    }
    // Trees: one per 9 m cell on the island top, trunk then a crown.
    let tc = (Vec2::new(m.x, m.z) / 9.0).floor();
    let (tx, tz) = (tc.x as i32, tc.y as i32);
    if hash3(tx, 0, tz, seed ^ TREE) > 0.55 {
        let trunk = (tc + Vec2::new(0.3 + 0.4 * hash3(tx, 1, tz, seed ^ TREE), 0.3 + 0.4 * hash3(tx, 2, tz, seed ^ TREE))) * 9.0;
        let td = (trunk - isle.centre).length() / isle.radius;
        if td < 0.75 {
            let ground = isle.top + (1.0 - td * td) * 3.5 + 0.5;
            let height = 5.0 + 4.0 * hash3(tx, 3, tz, seed ^ TREE);
            let h = Vec2::new(m.x, m.z).distance(trunk);
            if h < 0.5 && m.y > ground - 1.0 && m.y < ground + height {
                return Some(BlockId::Log);
            }
            let crown = Vec3::new(trunk.x, ground + height, trunk.y);
            if (m - crown).length() < 2.8 + hash3(tx, 4, tz, seed ^ TREE) && m.y > ground + 2.0 {
                return Some(BlockId::Leaves);
            }
        }
    }
    // Roots: thin vertical strands hanging below the keel rim.
    let rc = (Vec2::new(m.x, m.z) / 2.0).floor();
    let (rx, rz) = (rc.x as i32, rc.y as i32);
    if hash3(rx, 5, rz, seed ^ TREE) > 0.93 && d < 0.85 {
        let strand = (rc + Vec2::splat(0.5)) * 2.0;
        if Vec2::new(m.x, m.z).distance(strand) < vm.max(0.3) {
            let length = 4.0 + 10.0 * hash3(rx, 6, rz, seed ^ TREE);
            let under = isle.top - isle.keel * (1.0 - d).powf(0.7) * 0.75;
            if m.y < under && m.y > under - length {
                return Some(BlockId::Vines);
            }
        }
    }
    None
}

pub fn column(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    let vm = cfg.voxel_meters();
    let (mx, mz) = ((x as f32 + 0.5) * vm, (z as f32 + 0.5) * vm);
    let top = island(cfg.seed, cell_of(mx, mz))
        .filter(|i| (Vec2::new(mx, mz) - i.centre).length() < i.radius)
        .map_or(SEA_M, |i| i.top + 4.0);
    ColumnSample {
        deposit_m: 0.0,
        deposit_bed_surface: BlockId::Sand,
        height: cfg.blocks_for_height(top),
        surface: if top > SEA_M { BlockId::Grass } else { BlockId::Water },
        filler: BlockId::Limestone,
        vegetation_density: 0.0,
        cave: Default::default(),
    }
}

pub fn block(cfg: &WorldSampleConfig, p: IVec3) -> BlockId {
    let vm = cfg.voxel_meters();
    let m = (p.as_vec3() + Vec3::splat(0.5)) * vm;
    if m.y < SEABED_M {
        return BlockId::Sand;
    }
    if m.y < SEA_M {
        return BlockId::Water;
    }
    if shaft_at(cfg.seed, m) {
        return BlockId::LightlessShaft;
    }
    // An island may overhang into a neighbouring cell; check the 3x3 block.
    let (cx, cz) = cell_of(m.x, m.z);
    for dz in -1..=1 {
        for dx in -1..=1 {
            if let Some(isle) = island(cfg.seed, (cx + dx, cz + dz)) {
                if let Some(b) = island_block(cfg.seed, &isle, m).or_else(|| growth(cfg.seed, &isle, m, vm)) {
                    return b;
                }
            }
        }
    }
    BlockId::Air
}

/// The return shaft: a 1 m block set into the arrival island's top.
pub fn shaft_at(seed: u32, m: Vec3) -> bool {
    if !(0.0..1.0).contains(&m.x) || !(0.0..1.0).contains(&m.z) || (m.y - HOME_TOP_M).abs() > 8.0 {
        return false;
    }
    let top = home_top(seed);
    (top - 1.0..top).contains(&m.y)
}

/// Feet position in voxels: on the arrival island beside the shaft.
pub fn spawn(cfg: &WorldSampleConfig) -> Vec3 {
    let vm = cfg.voxel_meters();
    let at = Vec3::new(3.5, 0.0, 0.5);
    let top = (0..80)
        .map(|i| HOME_TOP_M + 8.0 - i as f32 * 0.25)
        .find(|&y| block(cfg, (Vec3::new(at.x, y - 0.5 * vm, at.z) / vm).floor().as_ivec3()).is_solid())
        .unwrap_or(HOME_TOP_M + 3.5);
    Vec3::new(at.x, top + 0.02, at.z) / vm
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(scale: crate::VoxelScalePreset) -> WorldSampleConfig {
        WorldSampleConfig {
            generator_version: crate::CURRENT_GENERATOR_VERSION,
            seed: 7,
            world_type: crate::WorldType::Realm(crate::RealmKind::Highfold),
            voxel_scale: scale,
            planet: None,
        }
    }

    /// Arrival stands on grass with clear headroom, beside the return shaft.
    #[test]
    fn arrival_is_on_the_home_island() {
        for scale in [crate::VoxelScalePreset::OneMeter, crate::VoxelScalePreset::QuarterMeter] {
            let c = cfg(scale);
            let feet = spawn(&c);
            let cell = feet.floor().as_ivec3();
            for dy in 0..=(2.0 / c.voxel_meters()).ceil() as i32 {
                assert!(!block(&c, cell + IVec3::Y * dy).is_solid(), "headroom at {scale:?}");
            }
            assert!(block(&c, cell - IVec3::Y).is_solid(), "footing at {scale:?}");
            let shaft = (Vec3::new(0.5, home_top(c.seed) - 0.5, 0.5) / c.voxel_meters()).floor().as_ivec3();
            assert_eq!(block(&c, shaft), BlockId::LightlessShaft);
        }
    }

    /// Islands are common enough to hop between and rare enough to fall past.
    #[test]
    fn islands_fill_part_of_the_sky() {
        let mut n = 0;
        for z in -10..10 {
            for x in -10..10 {
                n += usize::from(island(7, (x, z)).is_some());
            }
        }
        assert!((150..320).contains(&n), "{n} of 400 cells hold an island");
    }
}
