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
/// Coarser cells use the horizon's far-field evaluator. The exact column builds a
/// site/drainage plan per region it touches, about 5.6 ms cold for each sample of a
/// 500 m root tile, so a root took 23 s; the far-field evaluator costs microseconds.
pub const EXACT_MAX_CELL_M: f64 = 64.0;

/// One tile's baked surface samples in row-major order.
pub struct TileData {
    pub id: TileId,
    /// Metres above sea level; the sea floor is kept under water.
    pub height_m: Vec<i16>,
    pub material: Vec<u8>,
    /// 0 dry, 1 ocean, 2..128 inland (encoded depth plus one).
    pub water: Vec<u8>,
    /// Heat/suspension, wetness, canopy coverage and local sky visibility.
    pub climate: Vec<[u8; 4]>,
    /// Octahedrally encoded body-space normals, derived once from this height tile.
    pub normals: Vec<[u8; 2]>,
}

impl TileData {
    /// Allocates a zeroed tile.
    pub fn empty(id: TileId) -> TileData {
        TileData {
            id,
            height_m: vec![0; SAMPLE_COUNT],
            material: vec![0; SAMPLE_COUNT],
            water: vec![0; SAMPLE_COUNT],
            climate: vec![[0, 0, 0, 255]; SAMPLE_COUNT],
            normals: vec![[128, 128]; SAMPLE_COUNT],
        }
    }
}

/// Resume point of a partially baked tile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BakeCursor {
    pub next_sample: u32,
}

/// Index of a block without a globe colour; the shader draws it in its fallback slot.
pub const UNMAPPED: u8 = 255;

/// Maps a surface block to the globe material palette.
pub fn material_index(surface: BlockId) -> u8 {
    match surface {
        BlockId::Grass => 0,
        BlockId::Dirt => 1,
        BlockId::Stone => 2,
        BlockId::Sand => 3,
        BlockId::SnowGrass => 4,
        BlockId::Gravel => 5,
        // Crater lakes report Water; the bed colour shows where shore samples blend.
        BlockId::Mud | BlockId::Clay | BlockId::Water => 6,
        BlockId::Sandstone => 7,
        BlockId::Leaves | BlockId::PineLeaves => 8,
        BlockId::Log => 9,
        BlockId::Ice => 10,
        BlockId::Hellstone => 11,
        BlockId::Basalt => 12,
        BlockId::Ash => 13,
        BlockId::Lava => 14,
        BlockId::Limestone => 15,
        BlockId::GroveMoss => 16,
        _ => UNMAPPED,
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

fn height_i16(metres: f32) -> i16 {
    metres.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

fn unit_byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub(crate) fn surface_at(cfg: &WorldSampleConfig, x: i32, z: i32, cell_m: f64) -> (i32, BlockId) {
    if cell_m > EXACT_MAX_CELL_M && !cfg.is_hell() {
        let (top, surface) = engine_worldgen::landscape::preview_surface_lod(cfg, x, z, cell_m as f32);
        ((top - 1.0).round() as i32, surface)
    } else {
        let column = engine_worldgen::sample_column(cfg, x, z);
        (column.height, column.surface)
    }
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
    let cell_m = id.cell_m();
    let spherical = engine_worldgen::planetary::active(cfg);
    // The spherical generator already filters relief by footprint. Requesting
    // a metre under a 23-64 m orbital texel wasted local erosion work and aliased
    // small rock/soil features into whole distant pixels.
    let footprint = if spherical || cell_m > EXACT_MAX_CELL_M { (cell_m as f32).max(vm) } else { vm };
    let hell = cfg.is_hell();
    let tidal = engine_worldgen::planetary::tidal_coasts(cfg);
    // The modern Superflat generator returns the same ground column at every
    // coordinate. Climate still varies and is sampled at every texel.
    let flat_surface = (cfg.world_type == engine_voxel::WorldType::Superflat
        && cfg.generator_version >= 2).then(|| surface_at(cfg, 0, 0, cell_m));
    while cursor.next_sample < total {
        let k = cursor.next_sample;
        let (x, z) = sample_coord(cfg, id, k % TILE_SAMPLES, k / TILE_SAMPLES);
        let (column_height, surface, inland_head, suspended, canopy, sampled_climate) = if spherical {
            let (column, head, ground) = engine_worldgen::planetary::orbital_surface(cfg, x, z, footprint);
            // Coverage follows the actual admission field, including ecological
            // clearings, river banks, substrate and exposed mountain exclusions.
            let canopy = if head.is_none() && column.height >= water_level {
                1.0 - (-2.8 * column.vegetation_density).exp()
            } else { 0.0 };
            (column.height, column.surface, head, ground.suspended, canopy, Some((ground.temperature, ground.moisture)))
        } else {
            let (height, surface) = flat_surface.unwrap_or_else(|| surface_at(cfg, x, z, cell_m));
            (height, surface, None, 0.0, 0.0, None)
        };
        let height = cfg.world_y(column_height);
        let (heat, wet) = if hell { (1.0, 0.0) } else if let Some(climate) = sampled_climate { climate } else {
            let (heat, wet) = engine_worldgen::landscape::lowland_climate(cfg, x as f32 * vm, z as f32 * vm);
            (engine_worldgen::landscape::lapse_heat(cfg, heat, height), wet)
        };
        let idx = k as usize;
        data.height_m[idx] = height_i16(height);
        let frozen = column_height < water_level
            && cfg.world_type == engine_voxel::WorldType::Planet
            && engine_worldgen::planet::sea_ice_at(cfg, x, z);
        data.material[idx] = if frozen { material_index(BlockId::Ice) } else { material_index(surface) };
        // Sea ice rests on the marine datum, not on the bathymetric bed.
        if frozen { data.height_m[idx] = height_i16(cfg.world_y(water_level)); }
        data.water[idx] = if frozen || hell { 0 }
            else if let Some(head) = inland_head.filter(|h| *h > cfg.world_y(water_level)) {
                1 + (head - f32::from(data.height_m[idx])).round().clamp(1.0, 127.0) as u8
            } else { u8::from(column_height < water_level) };
        let optical = if tidal && data.water[idx] != 0 { suspended } else { heat };
        data.climate[idx] = [unit_byte(optical), unit_byte(wet), unit_byte(canopy), 255];
        cursor.next_sample += 1;
        if cursor.next_sample % BUDGET_STRIDE == 0 && cursor.next_sample < total && !budget.has_time() {
            return false;
        }
    }
    super::surface::finish(data, cfg.world_y(water_level));
    true
}

#[cfg(test)]
#[path = "bake_palette_tests.rs"]
mod palette_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::VoxelScalePreset;
    use engine_voxel::WorldType;
    use std::time::Duration;

    fn cfg() -> WorldSampleConfig {
        WorldSampleConfig { planet: None, generator_version: 53, seed: 74, world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter }
    }

    #[test]
    fn coarse_tiles_bake_from_the_far_field_evaluator() {
        let id = TileId::root(0);
        assert!(id.cell_m() > EXACT_MAX_CELL_M);
        let mut data = TileData::empty(id);
        assert!(bake_tile(&cfg(), id, &mut data, &mut BakeCursor::default(), Budget::UNBOUNDED));
        for k in [0u32, 2_112, 4_224] {
            let (x, z) = sample_coord(&cfg(), id, k % TILE_SAMPLES, k / TILE_SAMPLES);
            let (top, surface) = engine_worldgen::landscape::preview_surface_lod(&cfg(), x, z, id.cell_m() as f32);
            let h = (top - 1.0).round() as i32;
            assert_eq!(data.height_m[k as usize] as i32, cfg().world_y(h).round() as i32);
            assert_eq!(data.material[k as usize], material_index(surface));
        }
    }

    #[test]
    fn baked_heights_and_materials_equal_the_voxel_columns() {
        let id = TileId { face: 1, level: 6, x: 33, y: 20 };
        let mut data = TileData::empty(id);
        let mut cursor = BakeCursor::default();
        assert!(bake_tile(&cfg(), id, &mut data, &mut cursor, Budget::UNBOUNDED));
        for j in 0..TILE_SAMPLES {
            for i in 0..TILE_SAMPLES {
                let (x, z) = sample_coord(&cfg(), id, i, j);
                let column = engine_worldgen::sample_column(&cfg(), x, z);
                let k = (j * TILE_SAMPLES + i) as usize;
                assert_eq!(data.height_m[k] as i32, cfg().world_y(column.height).round() as i32);
                assert_eq!(data.material[k], material_index(column.surface));
                assert_eq!(data.water[k], u8::from(column.height < cfg().water_level()));
            }
        }
        let vm = cfg().voxel_meters();
        for (i, j) in [(0, 0), (64, 64), (17, 41), (64, 0)] {
            let (x, z) = sample_coord(&cfg(), id, i, j);
            let column = engine_worldgen::sample_column(&cfg(), x, z);
            let (heat, wet, _) = engine_worldgen::landscape::visual_ecology(
                &cfg(),
                x as f32 * vm,
                z as f32 * vm,
                cfg().world_y(column.height),
            );
            let k = (j * TILE_SAMPLES + i) as usize;
            assert_eq!(data.climate[k][..2], [unit_byte(heat), unit_byte(wet)]);
        }
    }

    #[test]
    fn heights_beyond_i16_clamp() {
        assert_eq!(height_i16(40_000.0), i16::MAX);
        assert_eq!(height_i16(-40_000.0), i16::MIN);
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
        assert!(rounds >= 2);
        assert_eq!(whole.height_m, pieces.height_m);
        assert_eq!(whole.material, pieces.material);
        assert_eq!(whole.water, pieces.water);
        assert_eq!(whole.climate, pieces.climate);
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
