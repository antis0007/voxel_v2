//! Bakes 65x65 globe tiles from worldgen columns under a time budget.

use super::chart;
use super::tile::{TILE_SAMPLES, TileId};
use engine_content::BlockId;
use engine_core::Budget;
use engine_core::cube_sphere::Face;
use engine_voxel::WorldSampleConfig;
use glam::DVec3;

const SAMPLE_COUNT: usize = (TILE_SAMPLES * TILE_SAMPLES) as usize;
const BUDGET_STRIDE: u32 = 64;

/// One tile's baked surface samples in row-major order.
pub struct TileData {
    pub id: TileId,
    /// Metres above sea level; the sea floor is kept under water.
    pub height_m: Vec<i16>,
    pub material: Vec<u8>,
    /// 1 where the column surface is below the water level.
    pub water: Vec<u8>,
    /// Heat and wetness scaled to 0..255.
    pub climate: Vec<[u8; 2]>,
}

impl TileData {
    /// Allocates a zeroed tile.
    pub fn empty(id: TileId) -> TileData {
        TileData {
            id,
            height_m: vec![0; SAMPLE_COUNT],
            material: vec![0; SAMPLE_COUNT],
            water: vec![0; SAMPLE_COUNT],
            climate: vec![[0; 2]; SAMPLE_COUNT],
        }
    }
}

/// Resume point of a partially baked tile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BakeCursor {
    pub next_sample: u32,
}

/// Maps a surface block to the globe material palette.
pub fn material_index(surface: BlockId) -> u8 {
    match surface {
        BlockId::Grass => 0,
        BlockId::Dirt => 1,
        BlockId::Stone => 2,
        BlockId::Sand => 3,
        BlockId::SnowGrass => 4,
        BlockId::Gravel => 5,
        BlockId::Mud | BlockId::Clay => 6,
        BlockId::Sandstone => 7,
        BlockId::Leaves | BlockId::PineLeaves => 8,
        BlockId::Log => 9,
        BlockId::Ice => 10,
        _ => 255,
    }
}

/// Voxel column of tile sample `(i, j)`.
pub fn sample_coord(cfg: &WorldSampleConfig, id: TileId, i: u32, j: u32) -> (i32, i32) {
    let st = id.sample_chart(i, j);
    let face = Face::ALL[id.face as usize];
    let p = chart::join(face, DVec3::new(st.x, 0.0, st.y));
    let vm = cfg.voxel_meters() as f64;
    ((p.x / vm).floor() as i32, (p.z / vm).floor() as i32)
}

fn unit_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Bakes samples from the cursor on; returns true once the tile is complete.
pub fn bake_tile(
    cfg: &WorldSampleConfig,
    id: TileId,
    data: &mut TileData,
    cursor: &mut BakeCursor,
    budget: Budget,
) -> bool {
    let total = TILE_SAMPLES * TILE_SAMPLES;
    let water_level = cfg.water_level();
    let vm = cfg.voxel_meters();
    while cursor.next_sample < total {
        let k = cursor.next_sample;
        let (x, z) = sample_coord(cfg, id, k % TILE_SAMPLES, k / TILE_SAMPLES);
        let column = engine_worldgen::sample_column(cfg, x, z);
        let height = cfg.world_y(column.height);
        let (heat, wet, _) =
            engine_worldgen::landscape::visual_ecology(cfg, x as f32 * vm, z as f32 * vm, height);
        let idx = k as usize;
        data.height_m[idx] = height.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        data.material[idx] = material_index(column.surface);
        data.water[idx] = u8::from(column.height < water_level);
        data.climate[idx] = [unit_byte(heat), unit_byte(wet)];
        cursor.next_sample += 1;
        if cursor.next_sample % BUDGET_STRIDE == 0 && cursor.next_sample < total && !budget.has_time() {
            return false;
        }
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::VoxelScalePreset;
    use engine_voxel::WorldType;
    use std::time::Duration;

    fn cfg() -> WorldSampleConfig {
        WorldSampleConfig { generator_version: 53, seed: 74, world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter }
    }

    #[test]
    fn baked_heights_and_materials_equal_the_voxel_columns() {
        let id = TileId { face: 1, level: 6, x: 33, y: 20 };
        let mut data = TileData::empty(id);
        let mut cursor = BakeCursor::default();
        assert!(bake_tile(&cfg(), id, &mut data, &mut cursor, Budget::UNBOUNDED));
        for (i, j) in [(0, 0), (64, 64), (17, 41), (64, 0)] {
            let (x, z) = sample_coord(&cfg(), id, i, j);
            let column = engine_worldgen::sample_column(&cfg(), x, z);
            let k = (j * TILE_SAMPLES + i) as usize;
            assert_eq!(data.height_m[k] as i32, cfg().world_y(column.height).round() as i32);
            assert_eq!(data.material[k], material_index(column.surface));
            assert_eq!(data.water[k], u8::from(column.height < cfg().water_level()));
        }
    }

    #[test]
    fn a_bake_interrupted_by_its_budget_resumes_to_the_same_result() {
        let id = TileId { face: 4, level: 5, x: 3, y: 30 };
        let mut whole = TileData::empty(id);
        bake_tile(&cfg(), id, &mut whole, &mut BakeCursor::default(), Budget::UNBOUNDED);
        let mut pieces = TileData::empty(id);
        let mut cursor = BakeCursor::default();
        let mut rounds = 0;
        while !bake_tile(&cfg(), id, &mut pieces, &mut cursor, Budget::wall(Duration::from_micros(200))) {
            rounds += 1;
            assert!(rounds < 10_000);
        }
        assert!(rounds > 0);
        assert_eq!(whole.height_m, pieces.height_m);
        assert_eq!(whole.material, pieces.material);
    }

    #[test]
    fn negative_and_far_face_coordinates_bake_without_overflow() {
        for id in [TileId { face: 5, level: 10, x: 1023, y: 0 }, TileId { face: 0, level: 10, x: 0, y: 1023 }] {
            let mut data = TileData::empty(id);
            assert!(bake_tile(&cfg(), id, &mut data, &mut BakeCursor::default(), Budget::UNBOUNDED));
            assert!(data.height_m.iter().all(|h| (-2_000..4_000).contains(&(*h as i32))));
        }
    }
}
