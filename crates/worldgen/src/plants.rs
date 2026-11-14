//! Wild stalk plants: bamboo in warm forest groves and cane on freshwater margins.
use crate::{
    ColumnSample, WorldSampleConfig, WorldType, floor_div, hash2, landscape, sample_column,
    smoothstep, value_noise2,
};
use engine_content::BlockId;
use engine_voxel::SURFACE_FLORA_GENERATOR;
use glam::IVec3;

/// Gate 139: bamboo groves. Dormant until `CURRENT_GENERATOR_VERSION` reaches it.
pub const VERSION: u32 = 139;
/// One culm per cell of this size; heights are metres so groves match at every scale.
const CULM_SPACING_METRES: f32 = 1.5;
const MIN_METRES: f32 = 3.0;
const MAX_METRES: f32 = 6.0;
/// Historical cover threshold for worlds through generator 176.
const GROVE_DENSITY: f32 = 0.85;

pub fn active(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= VERSION
}

/// The plant block at `p`, or `Air`.
pub fn block(cfg: &WorldSampleConfig, p: IVec3) -> BlockId {
    block_in_column(cfg, p, sample_column(cfg, p.x, p.z))
}

pub fn block_in_column(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> BlockId {
    let bamboo = if cfg.generator_version >= SURFACE_FLORA_GENERATOR {
        grove_bamboo(cfg, p, column)
    } else {
        legacy_bamboo(cfg, p, column)
    };
    if bamboo != BlockId::Air { return bamboo; }
    if cfg.generator_version < engine_voxel::HILLS_CANE_GENERATOR
        || cfg.world_type != crate::WorldType::Hills
        || p.y <= column.height || p.y > column.height + cfg.blocks_for_span(3.0)
        || !matches!(column.surface, BlockId::Grass | BlockId::Dirt | BlockId::Sand | BlockId::Mud)
    { return BlockId::Air; }
    let span = cfg.blocks_for_span(2.0);
    let (cx, cz) = (floor_div(p.x, span), floor_div(p.z, span));
    let seed = cfg.seed ^ 0x5c41_81c3;
    let ax = cx * span + ((hash2(cx, cz, seed) * span as f32) as i32).min(span - 1);
    let az = cz * span + ((hash2(cx, cz, seed ^ 31) * span as f32) as i32).min(span - 1);
    if (p.x, p.z) != (ax, az) { return BlockId::Air; }
    let (x, z) = (cfg.world_x(ax), cfg.world_z(az));
    let height = cfg.blocks_for_span(1.6 + 1.4 * hash2(cx, cz, seed ^ 73));
    if p.y <= column.height + height && value_noise2(x * 0.06, z * 0.06, seed) >= 0.46
        && crate::geo::cane_bank(cfg, x, z)
    {
        BlockId::Sugarcane
    } else { BlockId::Air }
}

fn grove_bamboo(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> BlockId {
    let height = p.y - column.height;
    let tall = cfg.generator_version >= engine_voxel::CANOPY_UNDERSTORY_GENERATOR;
    if !(1..=cfg.blocks_for_span(if tall { 8.0 } else { 4.5 })).contains(&height)
        || !matches!(cfg.world_type, WorldType::Hills | WorldType::Ridged | WorldType::Planet)
        || !matches!(column.surface, BlockId::Grass | BlockId::GroveMoss)
        || column.height <= cfg.water_level() + cfg.blocks_for_span(1.5)
    {
        return BlockId::Air;
    }

    let spacing = cfg.blocks_for_span(CULM_SPACING_METRES);
    let (cx, cz) = (floor_div(p.x, spacing), floor_div(p.z, spacing));
    let seed = cfg.seed.wrapping_add(177_139);
    let offset = |roll: f32| ((roll * spacing as f32) as i32).min(spacing - 1);
    if p.x.rem_euclid(spacing) != offset(hash2(cx, cz, seed))
        || p.z.rem_euclid(spacing) != offset(hash2(cx, cz, seed ^ 31))
    {
        return BlockId::Air;
    }

    // Keep groves wholly inside their home squares, with clear ground between them.
    let (x, z) = (cfg.world_x(p.x), cfg.world_z(p.z));
    let (gx, gz) = ((x / 36.0).floor() as i32, (z / 36.0).floor() as i32);
    if hash2(gx, gz, seed ^ 47) < 0.48 {
        return BlockId::Air;
    }
    let center_x = gx as f32 * 36.0 + 10.0 + 16.0 * hash2(gx, gz, seed ^ 61);
    let center_z = gz as f32 * 36.0 + 10.0 + 16.0 * hash2(gx, gz, seed ^ 79);
    let radius = if tall { 5.5 + 4.5 * hash2(gx, gz, seed ^ 107) } else { 4.5 + 4.5 * hash2(gx, gz, seed ^ 107) };
    let distance = (x - center_x).hypot(z - center_z) / radius;
    let roll = hash2(cx, cz, seed ^ 127);
    let culm_height = if tall { 2.1 + 5.9 * roll } else { 1.5 + 3.0 * roll };
    if distance >= 1.0 || height > cfg.blocks_for_span(culm_height) {
        return BlockId::Air;
    }

    let site = landscape::sample(cfg, p.x, p.z);
    let suitability = smoothstep(0.56, 0.70, site.temperature)
        * smoothstep(0.68, 0.83, site.moisture)
        * (1.0 - smoothstep(0.22, 0.48, site.slope));
    let cover = if tall { 0.59 } else { 0.50 } * suitability * (1.0 - smoothstep(0.55, 1.0, distance));
    if hash2(cx, cz, seed ^ 113) < cover {
        BlockId::Bamboo
    } else {
        BlockId::Air
    }
}

fn legacy_bamboo(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> BlockId {
    if p.y <= column.height || p.y > column.height + cfg.blocks_for_span(MAX_METRES).max(1) {
        return BlockId::Air;
    }
    let cell = cfg.blocks_for_span(CULM_SPACING_METRES).max(1);
    let (cx, cz) = (floor_div(p.x, cell), floor_div(p.z, cell));
    let seed = cfg.seed.wrapping_add(0xba_b0);
    let ax = cx * cell + ((hash2(cx, cz, seed) * cell as f32) as i32).min(cell - 1);
    let az = cz * cell + ((hash2(cx, cz, seed ^ 0x51) * cell as f32) as i32).min(cell - 1);
    // Most voxels stop here, before any column is sampled.
    if p.x != ax || p.z != az {
        return BlockId::Air;
    }
    // Patches about 25 m across, with gaps between them.
    let (mx, mz) = (cfg.world_x(ax), cfg.world_z(az));
    if value_noise2(mx * 0.04, mz * 0.04, seed ^ 0x9e) < 0.55 {
        return BlockId::Air;
    }
    if column.surface != BlockId::Grass
        || column.vegetation_density < GROVE_DENSITY
        || column.height <= cfg.water_level() + 1
    {
        return BlockId::Air;
    }
    let metres = MIN_METRES + hash2(cx, cz, seed ^ 0x77) * (MAX_METRES - MIN_METRES);
    let top = column.height + cfg.blocks_for_span(metres).max(1);
    if p.y > column.height && p.y <= top { BlockId::Bamboo } else { BlockId::Air }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::VoxelScalePreset;

    #[test]
    fn bamboo_waits_for_its_gate() {
        let mut cfg = WorldSampleConfig {
            planet: None, generator_version: VERSION - 1, seed: 7,
            world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::OneMeter,
        };
        assert!(!active(&cfg));
        cfg.generator_version = VERSION;
        assert!(active(&cfg));
    }

    #[test]
    fn culms_stand_on_their_cell_anchor_only() {
        let cfg = WorldSampleConfig {
            planet: None, generator_version: VERSION, seed: 7,
            world_type: WorldType::Hills, voxel_scale: VoxelScalePreset::QuarterMeter,
        };
        let cell = cfg.blocks_for_span(CULM_SPACING_METRES);
        for (x, z) in [(0, 0), (-41, 77), (1 << 18, -(1 << 18))] {
            let culms = (0..cell)
                .flat_map(|dx| (0..cell).map(move |dz| (dx, dz)))
                .filter(|&(dx, dz)| {
                    let (px, pz) = (x * cell + dx, z * cell + dz);
                    let h = sample_column(&cfg, px, pz).height;
                    (1..=cfg.blocks_for_span(MAX_METRES)).any(|y| block(&cfg, IVec3::new(px, h + y, pz)) == BlockId::Bamboo)
                })
                .count();
            assert!(culms <= 1, "{culms} culms in one cell at {x},{z}");
        }
    }
}
