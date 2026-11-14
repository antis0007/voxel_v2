//! Unbounded white maze. The same rule applies at every distance in x and z.
use crate::{BlockId, ColumnSample, WorldSampleConfig, hash_u32};
use glam::{IVec3, Vec3};

const PITCH_MM: i64 = 6_000;
const WALL_MM: i64 = 1_000;
const FLOOR_MM: i64 = 48_000;
const FLOOR_THICK_MM: i64 = 1_000;
const WALL_H_MM: i64 = 10_000;

fn scale_mm(cfg: &WorldSampleConfig) -> i64 {
    (cfg.voxel_meters() * 1000.0).round() as i64
}

fn floor_div(a: i64, b: i64) -> i64 {
    let q = a / b;
    let r = a % b;
    if r < 0 { q - 1 } else { q }
}

fn centre_mm(block: i32, scale: i64) -> i64 {
    block as i64 * scale + scale / 2
}

fn opens_east(seed: u32, i: i32, j: i32) -> bool {
    hash_u32(
        seed ^ (i as u32).wrapping_mul(0x9E37_79B9) ^ (j as u32).wrapping_mul(0x85EB_CA6B),
    ) & 1
        == 0
}

/// Extra doors. They only remove walls, so the binary-tree passages stay connected.
fn extra(seed: u32, i: i32, j: i32, which: u32) -> bool {
    hash_u32(
        seed ^ (i as u32).wrapping_mul(0x27D4_EB2D)
            ^ (j as u32).wrapping_mul(0x1656_67B1)
            ^ which.wrapping_mul(0xC2B2_AE35),
    ) % 5
        == 0
}

fn west_wall(seed: u32, i: i32, j: i32) -> bool {
    !opens_east(seed, i - 1, j) && !extra(seed, i - 1, j, 1)
}

fn south_wall(seed: u32, i: i32, j: i32) -> bool {
    opens_east(seed, i, j - 1) && !extra(seed, i, j - 1, 2)
}

pub(super) fn block(cfg: &WorldSampleConfig, cell: IVec3) -> BlockId {
    let scale = scale_mm(cfg);
    let y = centre_mm(cell.y, scale);
    let floor_top = FLOOR_MM + FLOOR_THICK_MM;
    let wall_top = floor_top + WALL_H_MM;
    if (FLOOR_MM..floor_top).contains(&y) {
        return BlockId::WhitePlaster;
    }
    if !(floor_top..wall_top).contains(&y) {
        return BlockId::Air;
    }
    let x = centre_mm(cell.x, scale);
    let z = centre_mm(cell.z, scale);
    let i = floor_div(x, PITCH_MM) as i32;
    let j = floor_div(z, PITCH_MM) as i32;
    let lx = x - floor_div(x, PITCH_MM) * PITCH_MM;
    let lz = z - floor_div(z, PITCH_MM) * PITCH_MM;
    if (lx < WALL_MM && west_wall(cfg.seed, i, j)) || (lz < WALL_MM && south_wall(cfg.seed, i, j)) {
        BlockId::WhitePlaster
    } else {
        BlockId::Air
    }
}

pub(super) fn column(cfg: &WorldSampleConfig, _x: i32, _z: i32) -> ColumnSample {
    let height = ((FLOOR_MM + FLOOR_THICK_MM) as f32 / 1000.0 / cfg.voxel_meters() - 0.5).floor() as i32;
    ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone,
        height,
        surface: BlockId::WhitePlaster,
        filler: BlockId::WhitePlaster,
        vegetation_density: 0.0,
        cave: Default::default(),
    }
}

pub(super) fn spawn(cfg: &WorldSampleConfig) -> Vec3 {
    let scale = cfg.voxel_meters();
    let floor = ((FLOOR_MM + FLOOR_THICK_MM) as f32 / 1000.0 / scale - 0.5).floor();
    Vec3::new(3.0 / scale, floor + 1.0, 3.0 / scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorldType;
    use engine_core::VoxelScalePreset;

    fn world(scale: VoxelScalePreset, version: u32) -> WorldSampleConfig {
        WorldSampleConfig {
            planet: None,
            generator_version: version,
            seed: 424242,
            world_type: WorldType::Manyfold,
            voxel_scale: scale,
        }
    }

    fn at(cfg: &WorldSampleConfig, x_m: f32, y_m: f32, z_m: f32) -> BlockId {
        let m = cfg.voxel_meters();
        let cell = IVec3::new(
            (x_m / m - 0.5).floor() as i32,
            (y_m / m - 0.5).floor() as i32,
            (z_m / m - 0.5).floor() as i32,
        );
        super::super::block(cfg, cell)
    }

    #[test]
    fn the_maze_continues_without_a_boundary() {
        let cfg = world(VoxelScalePreset::OneMeter, super::super::INFINITE_MAZE);
        for (x, z) in [(0.0_f32, 0.0), (12_000.0, -18_000.0), (-9_000.0, 6_000.0), (1_000_002.0, -1_000_008.0)] {
            assert_eq!(at(&cfg, x + 3.0, 48.5, z + 3.0), BlockId::WhitePlaster, "floor at {x},{z}");
            assert_eq!(at(&cfg, x + 3.0, 40.0, z + 3.0), BlockId::Air, "below the floor at {x},{z}");
            assert_eq!(at(&cfg, x + 3.0, 70.0, z + 3.0), BlockId::Air, "above the walls at {x},{z}");
            let mut plaster = false;
            let mut air = false;
            let mut tall = false;
            for sx in 0..12 {
                for sz in 0..12 {
                    let standing = at(&cfg, x + 0.5 + sx as f32, 50.5, z + 0.5 + sz as f32);
                    let high = at(&cfg, x + 0.5 + sx as f32, 58.5, z + 0.5 + sz as f32);
                    plaster |= standing == BlockId::WhitePlaster;
                    air |= standing == BlockId::Air;
                    tall |= standing == BlockId::WhitePlaster && high == BlockId::WhitePlaster;
                }
            }
            assert!(plaster && air, "cell at {x},{z} is not a corridor and a wall");
            assert!(tall, "walls at {x},{z} are not 10 m");
        }
    }

    #[test]
    fn every_cell_keeps_a_binary_tree_exit() {
        let cfg = world(VoxelScalePreset::HalfMeter, super::super::INFINITE_MAZE);
        for (i, j) in [(-40, 12), (0, 0), (7, -3), (200, -200)] {
            let x = i as f32 * 6.0;
            let z = j as f32 * 6.0;
            let centre = at(&cfg, x + 3.0, 50.5, z + 3.0);
            assert_eq!(centre, BlockId::Air, "corridor blocked at {i},{j}");
            let east = at(&cfg, x + 6.5, 50.5, z + 3.0);
            let north = at(&cfg, x + 3.0, 50.5, z + 6.5);
            assert!(
                east == BlockId::Air || north == BlockId::Air,
                "no exit at {i},{j}"
            );
        }
    }

    #[test]
    fn spawn_stands_in_a_corridor() {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::HalfMeter, VoxelScalePreset::QuarterMeter] {
            let cfg = world(scale, super::super::INFINITE_MAZE);
            let feet = spawn(&cfg).floor().as_ivec3();
            assert!(!super::super::block(&cfg, feet).is_solid(), "spawn {feet} is inside a wall");
            assert!(super::super::block(&cfg, feet - IVec3::Y).is_solid(), "spawn has no floor");
        }
    }

    #[test]
    fn the_tube_stays_finite_below_this_gate() {
        let cfg = world(VoxelScalePreset::OneMeter, super::super::TALL_WHITE_WALLS);
        assert_eq!(at(&cfg, 12_000.0, 50.5, -18_000.0), BlockId::Air);
    }
}
