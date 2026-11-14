//! Ice cover on rivers and valley lakes where the banks would hold snow.
//! Sea water is left open: the horizon draws the sea as one liquid sheet.
use super::landscape::{self, Biome};
use super::{WorldSampleConfig, ecology, smoothstep, value_noise2};

/// Generator 54: snowy reaches of rivers and lakes carry a top layer of ice.
pub const RIVER_ICE_GENERATOR: u32 = 54;
/// Ice thickness per degree of mean air temperature below freezing, metres.
const M_PER_C: f32 = 0.08;
pub const MAX_M: f32 = 1.5;
const SNOWLINE_START_M: f32 = 104.0;

/// The generator-9 snow cover of a flat, sheltered column (`landscape.rs`
/// surface pass), so ice appears exactly where level banks turn white.
fn snowy(cfg: &WorldSampleConfig, x: i32, z: i32) -> bool {
    let s = landscape::sample(cfg, x, z);
    if !matches!(s.biome, Biome::Alpine | Biome::Tundra | Biome::Taiga | Biome::Forest | Biome::Meadow | Biome::Heathland) {
        return false;
    }
    let patch = value_noise2(cfg.world_x(x) / 19.0, cfg.world_z(z) / 19.0, cfg.seed.wrapping_add(9341));
    let cold = 1.0 - smoothstep(0.08, 0.52, s.temperature);
    let snowline = smoothstep(SNOWLINE_START_M, 158.0, s.height);
    cold * 0.20 + snowline * 0.58 + patch * 0.32 + 0.12 > 0.64
}

/// Ice thickness in metres on water whose surface is `surface_m` at column `x, z`.
pub fn thickness_m(cfg: &WorldSampleConfig, x: i32, z: i32, surface_m: f32) -> f32 {
    // Cover above 0.64 needs the snowline term, so ground above 104 m; the
    // carved bank lies below the water surface. Most reaches stop here.
    if cfg.generator_version < RIVER_ICE_GENERATOR || surface_m <= SNOWLINE_START_M || !snowy(cfg, x, z) {
        return 0.0;
    }
    let (heat, _) = landscape::lowland_climate(cfg, cfg.world_x(x), cfg.world_z(z));
    let mean_c = ecology::heat_celsius(landscape::lapse_heat(cfg, heat, surface_m));
    (-mean_c * M_PER_C).clamp(cfg.voxel_meters(), MAX_M)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockId, VoxelScalePreset, WorldType, sample_block};
    use glam::IVec3;

    fn cfg(version: u32, voxel_scale: VoxelScalePreset) -> WorldSampleConfig {
        WorldSampleConfig { planet: None, generator_version: version, seed: crate::test_world::seed(), world_type: WorldType::Hills, voxel_scale }
    }

    /// Inland river columns with their water surface, in a fixed search window.
    fn river_columns(c: &WorldSampleConfig) -> Vec<(i32, i32, f32)> {
        let step = (16.0 / c.voxel_meters()) as i32;
        let mut out = Vec::new();
        for z in (-160..=160).map(|i| i * step) {
            for x in (-160..=160).map(|i| i * step) {
                if let Some(top) = crate::drainage::sample(c, c.world_x(x), c.world_z(z)).water_m
                    && top > c.world_y(c.water_level() + 1)
                {
                    out.push((x, z, top));
                }
            }
        }
        out
    }

    #[test]
    fn river_ice_caps_snowy_reaches_and_leaves_the_rest_open() {
        let c = cfg(RIVER_ICE_GENERATOR, VoxelScalePreset::OneMeter);
        let rivers = river_columns(&c);
        let (mut frozen, mut open) = (0, 0);
        for &(x, z, top) in &rivers {
            let t = thickness_m(&c, x, z, top);
            let p = IVec3::new(x, (top / c.voxel_meters()).ceil() as i32 - 1, z);
            match sample_block(&c, p) {
                BlockId::Ice => {
                    assert!(t > 0.0, "{p}: ice on a reach without snow");
                    frozen += 1;
                }
                BlockId::Water => {
                    assert_eq!(t, 0.0, "{p}: open water on a snowy reach");
                    open += 1;
                }
                _ => {}
            }
        }
        println!("inland river columns: {frozen} frozen, {open} open of {}", rivers.len());
        assert!(open > frozen, "most rivers stay open");
    }

    #[test]
    fn river_ice_is_off_before_its_generator() {
        let old = cfg(RIVER_ICE_GENERATOR - 1, VoxelScalePreset::OneMeter);
        assert!(river_columns(&old).iter().all(|&(x, z, top)| thickness_m(&old, x, z, top) == 0.0));
    }

    #[test]
    fn river_ice_is_one_cover_at_every_voxel_size() {
        let coarse = cfg(RIVER_ICE_GENERATOR, VoxelScalePreset::OneMeter);
        let fine = WorldSampleConfig { voxel_scale: VoxelScalePreset::QuarterMeter, ..coarse };
        for (x, z, top) in river_columns(&coarse).into_iter().take(300) {
            let a = thickness_m(&coarse, x, z, top);
            let b = thickness_m(&fine, x * 4, z * 4, top);
            assert_eq!(a > 0.0, b > 0.0, "{x},{z}: frozen at one scale only");
            if a > 0.0 {
                assert!(b <= a && b >= fine.voxel_meters());
            }
        }
    }

    #[test]
    #[ignore]
    fn probe_river_ice_cover() {
        // Frozen reaches to review, per seed.
        for seed in [2738958700u32, 173, 9001, 424242] {
            let c = WorldSampleConfig { seed, ..cfg(RIVER_ICE_GENERATOR, VoxelScalePreset::OneMeter) };
            let rivers = river_columns(&c);
            let frozen: Vec<_> = rivers.iter().filter(|&&(x, z, t)| thickness_m(&c, x, z, t) > 0.0).collect();
            let near = frozen.iter().min_by_key(|(x, z, _)| x.abs() + z.abs());
            println!("seed {seed}: {} of {} inland river columns frozen, nearest {near:?}", frozen.len(), rivers.len());
        }
    }
}
