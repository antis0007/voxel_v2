//! World generation: every function here is a pure function of
//! `WorldSampleConfig` (seed, generator version, world type, voxel scale) and a
//! position. No runtime state. Edits, regions and streaming live in the app.
use engine_content::BlockId;
use engine_core::{CHUNK_SIZE, PADDED_CHUNK_SIZE, floor_div};
#[cfg(test)]
use engine_core::VoxelScalePreset;
pub use engine_voxel::{
    CURRENT_GENERATOR_VERSION, ChunkData, WATER_LEVEL_METERS, WORLD_MAX_Y_METERS, WORLD_MIN_Y_METERS,
    WorldSampleConfig, WorldType, RealmKind,
};
use glam::{IVec3, Vec3};
use std::collections::HashSet;
pub use engine_core::hash::{hash01, hash_u32};

/// Ribs, camps, hall hazard shifts and manyfold beacons. Older versions keep their samplers.
pub const SEEDED_REALMS: u32 = 111;
/// Original leaf-relief authoring version, also available in tree-only release 127.
pub const TREE_FOLIAGE_VERSION: u32 = trees::foliage::VERSION;

pub mod amberroot;
mod column_memo;
pub mod nether;
pub mod hell_gate;
pub mod planet;
pub mod planet_freight;
pub mod planetary;
pub mod plants;
pub mod flora;
mod deadwood;
pub mod wayside;
pub mod planet_climate;
pub mod planet_ocean;
pub mod planet_profile;
pub mod surveyor;
pub mod basin;
pub mod deep;
pub mod placer;
pub mod volcano;
pub mod vistas;
pub mod manyfold;
pub mod moon;
pub mod station;
pub mod hall;
pub mod lightless;
pub mod highfold;
pub mod gloaming;
pub mod gilded;
pub mod anchor_studio;
pub mod architecture;
pub mod bedrock;
pub mod biome;
pub mod cistern;
pub mod fernwake;
pub mod households;
pub mod civilization;
pub mod public_works;
pub mod karst;
pub mod dungeon;
pub mod habitats_region;
#[cfg(test)]
mod gen_golden;
#[cfg(test)]
mod seam_route;
#[cfg(test)]
mod gen_probe;
#[cfg(test)]
mod map_probe;
#[cfg(test)]
mod planet_probe;
#[cfg(test)]
mod horizon_agreement;
mod generation_cache;
pub mod ground_exposure;
pub mod landscape;
pub mod ecology;
pub mod drainage;
pub mod gates;
pub mod features;
pub mod geo;
pub mod sphere;
pub mod lakes;
pub mod river_ice;
pub mod lore;
pub mod meridian_terminal;
mod ore_geology;
pub mod prism_sanctum;
pub mod ruins;
/// Eye and target along the east road link of region (rx, rz), for review.
pub fn road_review_view(cfg: &WorldSampleConfig, rx: i32, rz: i32) -> Option<(Vec3, Vec3)> {
    surface::road_review_view(cfg, rx, rz)
}

/// Highest generator version a benchmark may request: the newest dormant gate.
pub const REVIEW_GENERATOR_VERSION: u32 = ruins::VERSION;
pub mod sites;
#[cfg(test)]
mod floating_check;
pub mod surface;
pub mod terrain_variety;
mod terrain_detail;
mod landforms;
pub mod toll_crossing;
pub mod trees;
pub mod undercrown;
pub mod valley;
pub mod vegetation;
pub mod zones;

/// Stand-in for the app's `World` in generator tests: a fresh, unedited world
/// reads exactly what the generator produces.
#[cfg(test)]
pub mod test_world {
    use super::*;

    /// World seed for invariant tests: random per process, `VOXEL_TEST_SEED=<n>` replays one.
    ///
    /// Fixed seeds let every test pass on the one world it was tuned on; a fresh seed per run
    /// is what finds the edge cases. Golden and pinned-hash tests keep their literal seeds.
    pub fn seed() -> u32 {
        static SEED: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
        let seed = *SEED.get_or_init(|| {
            std::env::var("VOXEL_TEST_SEED").ok().and_then(|s| s.trim().parse().ok()).unwrap_or_else(|| {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos() as u64);
                engine_core::hash::hash_u32((nanos ^ (nanos >> 32)) as u32 ^ std::process::id())
            })
        });
        // Captured per test and printed only when that test fails.
        eprintln!("worldgen test seed {seed} (replay: VOXEL_TEST_SEED={seed})");
        seed
    }

    pub struct World {
        pub generator_version: u32,
        pub seed: u32,
        pub world_type: WorldType,
        pub voxel_scale: VoxelScalePreset,
    }
    impl World {
        pub fn with_seed(world_type: WorldType, voxel_scale: VoxelScalePreset, seed: u32) -> Self {
            Self { generator_version: CURRENT_GENERATOR_VERSION, seed, world_type, voxel_scale }
        }
        pub fn sample_config(&self) -> WorldSampleConfig {
            WorldSampleConfig {
                planet: None,
                generator_version: self.generator_version,
                seed: self.seed,
                world_type: self.world_type,
                voxel_scale: self.voxel_scale,
            }
        }
        pub fn get(&self, p: IVec3) -> BlockId {
            sample_block(&self.sample_config(), p)
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct BiomeWeights {
    pub plains: f32,
    pub hills: f32,
    pub desert: f32,
    pub snow: f32,
}

#[derive(Clone, Copy)]
pub struct ClimateSample {
    temperature: f32,
    moisture: f32,
    continentalness: f32,
}

#[derive(Clone, Copy)]
pub struct TreeProfile {
    conifer: bool,
    cell_x: i32,
    cell_z: i32,
    anchor_x: i32,
    anchor_z: i32,
    ground_y: i32,
    trunk_h: i32,
    canopy_radius: i32,
    canopy_height: i32,
    branch_count: i32,
    branch_len: i32,
    branch_start_y: i32,
    lean_x: i32,
    lean_z: i32,
    form: trees::TreeForm,
}

impl TreeProfile {
    /// Box outside which `tree_block` is always Air: Chebyshev reach from the
    /// anchor and the y range. `gen_probe_tree_reach_holds` checks it against
    /// every voxel of every tree over a wide region, versions and scales.
    fn reach(&self, cfg: &WorldSampleConfig) -> (i32, i32, i32) {
        let margin = cfg.blocks_for_span(8.0);
        let across = self.canopy_radius + self.branch_len + self.lean_x.abs() + self.lean_z.abs();
        let up = self.trunk_h + self.canopy_height + self.branch_len;
        (across + margin, self.ground_y - margin, self.ground_y + up + margin)
    }

    fn may_fill(&self, cfg: &WorldSampleConfig, p: IVec3) -> bool {
        let (across, low, high) = self.reach(cfg);
        (low..=high).contains(&p.y)
            && (p.x - self.anchor_x).abs() <= across
            && (p.z - self.anchor_z).abs() <= across
    }
}

#[derive(Clone, Copy)]
pub struct BushProfile {
    anchor_x: i32,
    anchor_z: i32,
    ground_y: i32,
    base_radius: i32,
    top_dy: i32,
    lean_x: i32,
    lean_z: i32,
}


/// Column-constant terms of the cave field. Computing these once per column
/// removes four 2D noise evaluations from every underground voxel.
#[derive(Clone, Copy, Default)]
pub struct CaveColumn {
    opening: landforms::Opening,
    height_above_water: f32,
    ravine: f32,
    ravine_center: f32,
    ravine_half_h: f32,
    mx: f32,
    mz: f32,
}

#[derive(Clone, Copy)]
pub struct ColumnSample {
    /// Physical surface-deposit thickness above the eroded bed; derived, never saved.
    pub deposit_m: f32,
    /// Surface buried beneath deposit_m; filler retains the buried soil.
    pub deposit_bed_surface: BlockId,
    pub height: i32,
    pub surface: BlockId,
    pub filler: BlockId,
    pub vegetation_density: f32,
    pub cave: CaveColumn,
}

pub fn cave_column(cfg: &WorldSampleConfig, x: i32, z: i32, height: i32) -> CaveColumn {
    let seed = cfg.seed;
    let mx = cfg.world_x(x);
    let mz = cfg.world_z(z);
    CaveColumn {
        opening: landforms::opening(cfg, mx, mz),
        height_above_water: (height - cfg.water_level()) as f32 * cfg.voxel_meters(),
        ravine: ridge(value_noise2(mx * 0.010, mz * 0.010, seed.wrapping_add(571))),
        ravine_center: cfg.world_y(height)
            - 16.0
            - value_noise2(mx * 0.020, mz * 0.020, seed.wrapping_add(593)) * 10.0,
        ravine_half_h: 4.0 + value_noise2(mx * 0.018, mz * 0.018, seed.wrapping_add(617)) * 4.5,
        mx,
        mz,
    }
}

pub fn saturate(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = saturate((x - edge0) / (edge1 - edge0).max(1e-6));
    t * t * (3.0 - 2.0 * t)
}

pub fn lerp_color(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = saturate(t);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

pub fn hash2(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = seed ^ (x as u32).wrapping_mul(374761393) ^ (z as u32).wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

pub fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = seed
        ^ (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(1103515245)
        ^ (z as u32).wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

pub fn value_noise2(x: f32, z: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let z0 = z.floor() as i32;
    let x1 = x0 + 1;
    let z1 = z0 + 1;

    let tx = x - x0 as f32;
    let tz = z - z0 as f32;
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sz = tz * tz * (3.0 - 2.0 * tz);

    let n00 = hash2(x0, z0, seed);
    let n10 = hash2(x1, z0, seed);
    let n01 = hash2(x0, z1, seed);
    let n11 = hash2(x1, z1, seed);

    let nx0 = n00 + (n10 - n00) * sx;
    let nx1 = n01 + (n11 - n01) * sx;
    nx0 + (nx1 - nx0) * sz
}

pub fn value_noise3(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let z0 = z.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let z1 = z0 + 1;

    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let tz = z - z0 as f32;
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let sz = tz * tz * (3.0 - 2.0 * tz);

    let n000 = hash3(x0, y0, z0, seed);
    let n100 = hash3(x1, y0, z0, seed);
    let n010 = hash3(x0, y1, z0, seed);
    let n110 = hash3(x1, y1, z0, seed);
    let n001 = hash3(x0, y0, z1, seed);
    let n101 = hash3(x1, y0, z1, seed);
    let n011 = hash3(x0, y1, z1, seed);
    let n111 = hash3(x1, y1, z1, seed);

    let nx00 = n000 + (n100 - n000) * sx;
    let nx10 = n010 + (n110 - n010) * sx;
    let nx01 = n001 + (n101 - n001) * sx;
    let nx11 = n011 + (n111 - n011) * sx;
    let nxy0 = nx00 + (nx10 - nx00) * sy;
    let nxy1 = nx01 + (nx11 - nx01) * sy;
    nxy0 + (nxy1 - nxy0) * sz
}

pub fn fbm2(x: f32, z: f32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    for i in 0..4 {
        sum += value_noise2(x * freq, z * freq, seed + i) * amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum
}

pub fn ridge(x: f32) -> f32 {
    1.0 - (2.0 * x - 1.0).abs()
}

pub fn smooth_band(value: f32, center: f32, width: f32) -> f32 {
    saturate(1.0 - ((value - center).abs() / width))
}

pub fn branch_direction(cell_x: i32, cell_z: i32, branch_idx: i32, seed: u32) -> (i32, i32) {
    match ((hash3(cell_x, branch_idx, cell_z, seed) * 8.0) as i32).min(7) {
        0 => (1, 0),
        1 => (1, 1),
        2 => (0, 1),
        3 => (-1, 1),
        4 => (-1, 0),
        5 => (-1, -1),
        6 => (0, -1),
        _ => (1, -1),
    }
}

pub fn tree_trunk_center(tree: TreeProfile, y: i32) -> (i32, i32) {
    if y <= tree.branch_start_y {
        return (tree.anchor_x, tree.anchor_z);
    }

    let bend_span = (tree.trunk_h - (tree.branch_start_y - tree.ground_y)).max(1);
    let bend_t = (y - tree.branch_start_y) as f32 / bend_span as f32;
    (
        tree.anchor_x + (tree.lean_x as f32 * bend_t).round() as i32,
        tree.anchor_z + (tree.lean_z as f32 * bend_t).round() as i32,
    )
}

pub fn tree_block(cfg: &WorldSampleConfig, tree: TreeProfile, p: IVec3) -> BlockId {
    if cfg.generator_version >= 6 {
        return trees::block(cfg, tree, p);
    }
    if cfg.generator_version >= 5 {
        return vegetation::tree_block(cfg, tree, p);
    }
    if tree.conifer {
        let dy = cfg.world_y(p.y - tree.ground_y);
        let height = tree.trunk_h as f32 * cfg.voxel_meters();
        if dy <= 0.0 || dy > height + 1.0 {
            return BlockId::Air;
        }
        let dx = cfg.world_x(p.x - tree.anchor_x).abs();
        let dz = cfg.world_z(p.z - tree.anchor_z).abs();
        if dx < 0.35 && dz < 0.35 && dy <= height {
            return BlockId::Log;
        }
        let taper = (1.0 - dy / (height + 1.0)).max(0.0);
        let tier = (dy / 1.6).fract();
        let radius = taper * 3.5 * (0.65 + 0.35 * (1.0 - tier));
        if dy > 1.8 && dx.max(dz) <= radius && dx + dz < radius * 1.65 {
            return BlockId::PineLeaves;
        }
        return BlockId::Air;
    }
    let top_y = tree.ground_y + tree.trunk_h;
    if p.y <= tree.ground_y || p.y > top_y + tree.canopy_height + 1 {
        return BlockId::Air;
    }

    let (trunk_x, trunk_z) = tree_trunk_center(tree, p.y.min(top_y));
    let trunk_radius = if cfg.generator_version >= 2 {
        cfg.blocks_for_height(0.3)
    } else {
        0
    };
    if (p.x - trunk_x).abs() <= trunk_radius
        && (p.z - trunk_z).abs() <= trunk_radius
        && p.y <= top_y
    {
        return BlockId::Log;
    }

    for branch_idx in 0..tree.branch_count {
        let (dir_x, dir_z) = branch_direction(
            tree.cell_x,
            tree.cell_z,
            branch_idx,
            cfg.seed.wrapping_add(613),
        );
        let start_y = tree.branch_start_y + branch_idx.min((top_y - tree.branch_start_y).max(0));
        let (start_x, start_z) = tree_trunk_center(tree, start_y.min(top_y));
        for step in 1..=tree.branch_len {
            let rise = if step > 1 { (step - 1) / 2 } else { 0 };
            let bx = start_x + dir_x * step;
            let by = (start_y + rise).min(top_y + tree.canopy_height / 2);
            let bz = start_z + dir_z * step;
            if p == IVec3::new(bx, by, bz) {
                return BlockId::Log;
            }

            let twig_len = cfg.blocks_for_span(0.75).min(2);
            for twig_step in 1..=twig_len {
                let tx = bx + dir_x * twig_step;
                let ty = by + twig_step / 2;
                let tz = bz + dir_z * twig_step;
                if p == IVec3::new(tx, ty, tz) {
                    return BlockId::Log;
                }
            }
        }
    }

    let crown_center_y = top_y + tree.canopy_height / 3;
    let dy = p.y - crown_center_y;
    let crown_dx = (p.x - trunk_x).abs();
    let crown_dz = (p.z - trunk_z).abs();
    let canopy_rx = tree.canopy_radius.max(1) as f32;
    let canopy_ry = (tree.canopy_height.max(2) as f32 * 0.72).max(1.0);
    let crown_shape = (crown_dx as f32 / canopy_rx).powi(2)
        + (dy as f32 / canopy_ry).powi(2)
        + (crown_dz as f32 / canopy_rx).powi(2);
    let trim = hash3(p.x, p.y, p.z, cfg.seed.wrapping_add(631));
    if crown_shape <= 1.18 && !(crown_shape > 0.78 && trim > 0.82) {
        return BlockId::Leaves;
    }

    for branch_idx in 0..tree.branch_count {
        let (dir_x, dir_z) = branch_direction(
            tree.cell_x,
            tree.cell_z,
            branch_idx,
            cfg.seed.wrapping_add(613),
        );
        let start_y = tree.branch_start_y + branch_idx.min((top_y - tree.branch_start_y).max(0));
        let (start_x, start_z) = tree_trunk_center(tree, start_y.min(top_y));
        let tip_x = start_x + dir_x * (tree.branch_len + cfg.blocks_for_span(0.5));
        let tip_y = (start_y + tree.branch_len / 2 + 1).min(top_y + tree.canopy_height);
        let tip_z = start_z + dir_z * (tree.branch_len + cfg.blocks_for_span(0.5));
        let dx = (p.x - tip_x).abs();
        let dy = (p.y - tip_y).abs();
        let dz = (p.z - tip_z).abs();
        let tuft_radius = cfg.blocks_for_span(1.2);
        if dx.max(dy).max(dz) <= tuft_radius && (dx + dy + dz <= tuft_radius * 2 + 1 || trim < 0.35)
        {
            return BlockId::Leaves;
        }
    }

    BlockId::Air
}

pub fn sample_climate(cfg: &WorldSampleConfig, x: i32, z: i32) -> ClimateSample {
    let seed = cfg.seed;
    let mx = cfg.world_x(x);
    let mz = cfg.world_z(z);
    let warp_x = (fbm2(mx * 0.0012, mz * 0.0012, seed.wrapping_add(7)) - 0.5) * 220.0;
    let warp_z = (fbm2(mx * 0.0011, mz * 0.0011, seed.wrapping_add(9)) - 0.5) * 220.0;
    let wx = mx + warp_x;
    let wz = mz + warp_z;

    let temperature = fbm2(wx * 0.0022, wz * 0.0022, seed.wrapping_add(11));
    let moisture = fbm2(wx * 0.0020, wz * 0.0020, seed.wrapping_add(23));
    let continentalness = fbm2(wx * 0.0015, wz * 0.0015, seed.wrapping_add(37));

    ClimateSample {
        temperature: saturate(temperature),
        moisture: saturate(moisture),
        continentalness: saturate(continentalness),
    }
}

pub fn biome_weights(cfg: &WorldSampleConfig, x: i32, z: i32) -> BiomeWeights {
    let climate = sample_climate(cfg, x, z);
    let mx = cfg.world_x(x);
    let mz = cfg.world_z(z);
    let erosion = fbm2(mx * 0.0044, mz * 0.0044, cfg.seed.wrapping_add(51));

    let cold = saturate((0.42 - climate.temperature) / 0.23);
    let hot = saturate((climate.temperature - 0.57) / 0.20);
    let humid = saturate((climate.moisture - 0.47) / 0.28);
    let arid = saturate((0.52 - climate.moisture) / 0.25);

    let desert = hot * arid * smooth_band(climate.continentalness, 0.58, 0.44);
    let snow = cold * (0.42 + humid * 0.58) * smooth_band(climate.continentalness, 0.52, 0.50);
    let hills =
        smooth_band(erosion, 0.77, 0.27) * smooth_band(climate.continentalness, 0.62, 0.38) * 0.90;
    let plains =
        0.25 + (1.0 - (desert + snow).min(0.92)) * smooth_band(climate.moisture, 0.55, 0.42);

    let sum = (desert + snow + hills + plains).max(0.001);
    BiomeWeights {
        plains: plains / sum,
        hills: hills / sum,
        desert: desert / sum,
        snow: snow / sum,
    }
}

pub fn terrain_height(cfg: &WorldSampleConfig, x: i32, z: i32) -> i32 {
    sample_column(cfg, x, z).height
}

/// Heightfield approximation uses the same generated columns and sea level as chunks.
pub fn distant_surface(cfg: &WorldSampleConfig, x: i32, z: i32) -> (f32, BlockId) {
    let column = sample_column(cfg, x, z);
    (column.height as f32 + 1.0, column.surface)
}


pub fn sample_column(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    generation_cache::column(cfg, x, z)
}

pub fn sample_column_uncached(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    if cfg.world_type == WorldType::Hall { return hall::column(cfg, x, z); }
    if cfg.world_type == WorldType::Realm(RealmKind::Lightless) { return lightless::column(cfg, x, z); }
    if cfg.world_type == WorldType::Realm(RealmKind::Gilded) { return gilded::column(cfg, x, z); }
    if cfg.world_type == WorldType::Realm(RealmKind::Highfold) { return highfold::column(cfg, x, z); }
    if cfg.world_type == WorldType::Realm(RealmKind::Gloaming) { return gloaming::column(cfg, x, z); }
    if cfg.world_type == WorldType::Manyfold { return manyfold::column(cfg, x, z); }
    if cfg.world_type == WorldType::Planet { return planet::column(cfg, x, z); }
    if cfg.world_type == WorldType::Nether { return nether::column(cfg,x,z); }
    if cfg.generator_version >= 2 {
        return landscape::column(cfg, x, z);
    }
    let weights = biome_weights(cfg, x, z);
    let seed = cfg.seed;
    let climate = sample_climate(cfg, x, z);
    let mx = cfg.world_x(x);
    let mz = cfg.world_z(z);
    let erosion = fbm2(mx * 0.0048, mz * 0.0048, seed.wrapping_add(51));
    let macro_shape = fbm2(mx * 0.0090, mz * 0.0090, seed.wrapping_add(71));
    let detail = fbm2(mx * 0.026, mz * 0.026, seed.wrapping_add(83));
    let ridge_shape = ridge(fbm2(mx * 0.017, mz * 0.017, seed.wrapping_add(97)));
    let river = smooth_band(
        fbm2(mx * 0.0036, mz * 0.0036, seed.wrapping_add(109)),
        0.50,
        0.05,
    );
    let lake = smooth_band(
        fbm2(mx * 0.0019, mz * 0.0019, seed.wrapping_add(127)),
        0.50,
        0.028,
    ) * smooth_band(climate.continentalness, 0.56, 0.34);
    let mountain_mask =
        smooth_band(climate.continentalness, 0.72, 0.20) * smooth_band(ridge_shape, 0.68, 0.40);
    let mountain_boost = mountain_mask
        * (8.0 + ridge(fbm2(mx * 0.0105, mz * 0.0105, seed.wrapping_add(137))) * 20.0);
    let valley_cut =
        river * river * (5.0 + climate.moisture * 7.0) + lake * (3.0 + climate.moisture * 4.0);

    let plains_h = 8.0 + macro_shape * 5.5 + detail * 2.5;
    let hills_h = 11.0 + macro_shape * 11.0 + ridge(erosion) * 9.0 + detail * 4.0;
    let desert_h = 6.0 + macro_shape * 4.0 + ridge(detail) * 3.0;
    let snow_h = 12.0 + macro_shape * 7.5 + ridge_shape * 10.0 + detail * 3.5;

    let blended = weights.plains * plains_h
        + weights.hills * hills_h
        + weights.desert * desert_h
        + weights.snow * snow_h;
    let coast = (climate.continentalness - 0.5) * 14.0;

    let world_modifier = match cfg.world_type {
        WorldType::Superflat => 2.0 + detail * 0.5,
        WorldType::Hills | WorldType::Planet | WorldType::Manyfold => blended + coast + mountain_boost * 0.65 - valley_cut,
        WorldType::Ridged => {
            blended + coast * 0.45 + ridge_shape * 18.0 + mountain_boost - valley_cut * 0.35
        }
        WorldType::Nether | WorldType::Hall | WorldType::Realm(_) => unreachable!("dimension columns use their own generator"),
    };

    let height = cfg.clamp_height(cfg.blocks_for_height(world_modifier));
    let water_level = cfg.water_level();
    let altitude_snow = saturate((cfg.world_y(height) - 18.0) / 18.0);
    let snow_bias = (weights.snow + altitude_snow * 0.60).clamp(0.0, 1.0);
    let surface = if height <= water_level + 1 && weights.snow > 0.55 {
        BlockId::SnowGrass
    } else if height <= water_level + 1 {
        BlockId::Sand
    } else if snow_bias > 0.50 && weights.desert < 0.26 {
        BlockId::SnowGrass
    } else {
        BlockId::Grass
    };
    let filler = if surface == BlockId::Sand {
        BlockId::Sandstone
    } else {
        BlockId::Dirt
    };

    ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone,
        height,
        surface,
        filler,
        vegetation_density: (weights.plains * 0.95 + weights.hills * 0.40 + weights.snow * 0.08)
            .clamp(0.0, 1.0)
            * (1.0 - weights.desert * 0.90),
        cave: cave_column(cfg, x, z, height),
    }
}

/// Carved cell: water only in a sparse one-cell pocket on the water table.
pub(crate) fn aquifer_pocket(cfg: &WorldSampleConfig, p: IVec3, column: &ColumnSample) -> BlockId {
    let table = water_table(cfg, p.x, p.z);
    if cfg.world_type == WorldType::Planet && planetary::watershed_erosion(cfg.generator_version) {
        return if p.y <= table.min(column.height - cfg.blocks_for_span(7.0)) {
            BlockId::Water
        } else {
            BlockId::Air
        };
    }
    // The pocket noise matters only on the table row; skipping it elsewhere
    // cannot change the result.
    if p.y == table
        && fbm2(cfg.world_x(p.x)*0.025, cfg.world_z(p.z)*0.025, cfg.seed.wrapping_add(1871)) > 0.72
        && p.y > cfg.min_world_y()+1
        && !cave_carves(cfg, p-IVec3::Y, column) {
        BlockId::Water
    } else { BlockId::Air }
}

pub fn water_table(cfg: &WorldSampleConfig, x: i32, z: i32) -> i32 {
    thread_local! {
        static MEMO: column_memo::ColumnMemo<i32> = const { column_memo::ColumnMemo::new() };
    }
    MEMO.with(|memo| memo.get(cfg, x, z, || water_table_uncached(cfg, x, z)))
}

fn water_table_uncached(cfg: &WorldSampleConfig, x: i32, z: i32) -> i32 {
    if matches!(cfg.world_type, WorldType::Nether | WorldType::Hall | WorldType::Realm(_)) { return cfg.min_world_y(); }
    if cfg.world_type == WorldType::Planet && planetary::watershed_erosion(cfg.generator_version) {
        return planetary::aquifer_head(cfg, x, z);
    }
    let climate = sample_climate(cfg, x, z);
    let coast = saturate((0.54 - climate.continentalness) / 0.18);
    let aquifer = fbm2(
        cfg.world_x(x) * 0.0042,
        cfg.world_z(z) * 0.0042,
        cfg.seed.wrapping_add(509),
    ) - 0.5;
    // Never allow the underground water table to rise above sea level, or
    // shallow coastal cave pockets will place perched water lines on beaches.
    cfg.blocks_for_height(
        (WORLD_MIN_Y_METERS + 6.0) * (1.0 - coast)
            + (WATER_LEVEL_METERS - 1.0) * coast
            + aquifer * 1.5
            + climate.moisture * 1.2,
    )
    .min(cfg.water_level())
}

pub fn cave_carves(cfg: &WorldSampleConfig, p: IVec3, column: &ColumnSample) -> bool {
    let caves = cfg.planet_settings().map_or(1.0, |s| s.unit(8) as f32 * 2.0);
    if caves == 0.0 { return false; }
    if planetary::active(cfg) { return planetary::cave_carves(cfg, p, column); }
    if matches!(cfg.world_type, WorldType::Superflat) {
        return false;
    }
    if p.y > column.height || p.y <= cfg.min_world_y() + cfg.blocks_for_span(0.75) {
        return false;
    }

    let seed = cfg.seed;
    let depth = (column.height - p.y) as f32 * cfg.voxel_meters();
    let height_above_water = column.cave.height_above_water;
    if cfg.generator_version >= 39 && column.cave.opening.carves(depth, height_above_water) {
        return true;
    }
    // Protect beaches, shorelines, and shallow lowland turf from surface-breaking
    // cave slits and flooded sinkholes; only high hills/mountains allow cave mouths.
    if height_above_water < 9.0 && depth < 5.5 {
        return false;
    }
    if height_above_water < 18.0 && depth < 2.5 {
        return false;
    }
    let mx = column.cave.mx;
    let my = cfg.world_y(p.y);
    let mz = column.cave.mz;
    let spaghetti = value_noise3(mx * 0.060, my * 0.095, mz * 0.060, seed.wrapping_add(301));
    let cheese = value_noise3(mx * 0.028, my * 0.040, mz * 0.028, seed.wrapping_add(347));
    let pillar = ridge(value_noise3(
        mx * 0.020,
        my * 0.055,
        mz * 0.020,
        seed.wrapping_add(389),
    ));
    let ravine = column.cave.ravine;
    let ravine_center = column.cave.ravine_center;
    let ravine_half_h = column.cave.ravine_half_h;
    let bias = saturate(depth / 24.0) * 0.12 + (caves - 1.0) * 0.10;
    let surface_guard = saturate((10.0 - depth) / 10.0) * 0.16;

    spaghetti > 0.86 + surface_guard - bias
        || cheese > 0.80 + surface_guard - bias
        || (depth > 3.5 && pillar > 0.93 + surface_guard * 0.6)
        || (depth > 6.0 && ravine > 0.975 && (my - ravine_center).abs() < ravine_half_h)
}

pub fn tree_profile_for_cell(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<TreeProfile> {
    generation_cache::tree(cfg, cell_x, cell_z)
}

/// Intermediate woodland uses the same cached spawn decision as editable trees.
/// Dimensions are metres, so quarter-metre worlds do not lose their forest LOD.
#[derive(Clone,Copy,Debug,PartialEq)]
pub enum TreeCanopy { Broadleaf, Conifer, Cycad, Fungal, Rainforest, Redwood, Cypress }

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct TreeVisual {
    pub base: Vec3,
    pub height: f32,
    /// Outer leaf reach from the trunk, metres (see `trees::crown_reach`).
    pub radius: f32,
    pub crown_height: f32,
    pub conifer: bool,
    pub canopy: TreeCanopy,
    pub foliage: BlockId,
    pub seed: u32,
    /// Branch-group crown masses (generator 137 broadleaf), metres from `base`.
    pub lobes: [TreeLobe; 8],
    pub lobe_count: u8,
}

pub use trees::structure::Lobe as TreeLobe;
pub use trees::WOODLAND_HABITAT_PREVIEW;

pub fn tree_visual(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<TreeVisual> {
    let tree = tree_profile_for_cell(cfg, cell_x, cell_z)?;
    Some(tree_visual_from_profile(cfg, tree))
}

/// Same anchor, placement lottery, soil/biome policy and crown profile as the
/// authoritative tree source. Only sub-footprint ground relief is filtered.
pub fn tree_visual_lod(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32, footprint: f32) -> Option<TreeVisual> {
    let tree = tree_profile_evaluated(cfg, cell_x, cell_z, Some(footprint))?;
    Some(tree_visual_from_profile(cfg, tree))
}

pub fn tree_visual_from_profile(cfg: &WorldSampleConfig, tree: TreeProfile) -> TreeVisual {
    let scale = cfg.voxel_meters();
    let ((reach, top), lobes, lobe_count) = if trees::structure::applies(cfg, &tree) {
        let (bounds, mut lobes, count) = trees::structure::visual(cfg, &tree);
        // Lobes are metres above the ground voxel; the proxy base is one voxel up.
        for lobe in &mut lobes[..count] {
            lobe.centre.y -= scale;
        }
        (bounds, lobes, count)
    } else {
        (trees::crown_bounds(cfg, &tree), [TreeLobe::default(); 8], 0)
    };
    TreeVisual {
        // The first trunk voxel sits one cell above the surface voxel, so the
        // visible trunk base is the top face of ground_y, not its centre.
        base: Vec3::new(
            tree.anchor_x as f32,
            (tree.ground_y + 1) as f32,
            tree.anchor_z as f32,
        ) * scale,
        height: tree.trunk_h as f32 * scale,
        // True leaf footprint in metres, from the same limb math as `block()`.
        // Lobes reach well past `canopy_radius`, so a proxy sized from the
        // radius alone shrinks the tree when its LOD changes.
        radius: reach,
        // Proxy crown height is set so base + height + crown_height is the true
        // leaf top, measured from the same base the proxy is drawn from.
        crown_height: (top - tree.trunk_h as f32 * scale).max(scale),
        conifer: tree.conifer,
        canopy: match tree.form {
            trees::TreeForm::Cycad => TreeCanopy::Cycad,
            trees::TreeForm::Fungal => TreeCanopy::Fungal,
            trees::TreeForm::Cypress => TreeCanopy::Cypress,
            form if form.is_rainforest() => TreeCanopy::Rainforest,
            trees::TreeForm::Redwood if trees::is_tall_redwood(cfg, &tree) => TreeCanopy::Redwood,
            _ if tree.conifer => TreeCanopy::Conifer,
            _ => TreeCanopy::Broadleaf,
        },
        foliage: if tree.form==trees::TreeForm::Fungal {
            if hash2(tree.cell_x,tree.cell_z,cfg.seed.wrapping_add(19431))>0.5 {BlockId::UmberCap} else {BlockId::AmberCap}
        } else if tree.conifer {BlockId::PineLeaves} else {BlockId::Leaves},
        seed: (tree.cell_x as u32).wrapping_mul(73856093)
            ^ (tree.cell_z as u32).wrapping_mul(19349663)
            ^ cfg.seed,
        lobes,
        lobe_count: lobe_count as u8,
    }
}

pub fn tree_profile_uncached(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<TreeProfile> {
    if matches!(cfg.world_type, WorldType::Nether | WorldType::Hall | WorldType::Realm(_)) { return None; }
    tree_profile_evaluated(cfg, cell_x, cell_z, None)
}

/// Trunk voxel column of a tree cell, before any placement test.
pub(crate) fn tree_anchor(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> (i32, i32) {
    let cell_size = cfg.blocks_for_span(12.0);
    let offset_x = (hash2(cell_x, cell_z, cfg.seed.wrapping_add(401)) * cell_size as f32) as i32;
    let offset_z = (hash2(cell_x, cell_z, cfg.seed.wrapping_add(419)) * cell_size as f32) as i32;
    (cell_x * cell_size + offset_x.min(cell_size - 1), cell_z * cell_size + offset_z.min(cell_size - 1))
}

pub fn tree_profile_evaluated(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32, footprint: Option<f32>) -> Option<TreeProfile> {
    // Resident blocks in these worlds never grow woodland. A hills-profile
    // crown would stand in the distance and vanish on approach.
    // Version 74 planets are a bare shell. Later planets grow the column biome's
    // woodland in atlas space, so the shell projection places it on the ground.
    let planet_woods = cfg.world_type == WorldType::Planet && cfg.generator_version >= planet::BIOME_VERSION;
    if matches!(cfg.world_type, WorldType::Nether | WorldType::Planet | WorldType::Manyfold | WorldType::Hall | WorldType::Realm(_))
        && !planet_woods
    {
        return None;
    }
    let (anchor_x, anchor_z) = tree_anchor(cfg, cell_x, cell_z);

    let column_at = |x, z| footprint.map_or_else(|| sample_column(cfg, x, z), |step| landscape::preview_column_lod(cfg, x, z, step));
    if features::enabled(cfg, features::Feature::GiantTrees) && trees::giant::candidate(cfg, cell_x, cell_z)
        && !(cfg.generator_version >= 54 && sites::tree_reserved(cfg,cfg.world_x(anchor_x),cfg.world_z(anchor_z)))
        && let Some(giant) = trees::giant::profile(cfg, (cell_x, cell_z), (anchor_x, anchor_z))
    {
        return Some(giant);
    }
    // Column density is capped at one. Reject impossible rolls before
    // evaluating terrain, while keeping giant placement independent of them.
    let chance = hash2(cell_x, cell_z, cfg.seed.wrapping_add(433));
    let density_factor = if cfg.generator_version >= 2 { 0.88 } else { 0.22 };
    if chance > density_factor {
        return None;
    }
    if ruins::clears_vegetation(cfg, cfg.world_x(anchor_x), cfg.world_z(anchor_z))
        || civilization::clears_vegetation(cfg, cfg.world_x(anchor_x), cfg.world_z(anchor_z))
    {
        return None;
    }
    let (column, preview) = footprint.map_or_else(
        || (sample_column(cfg, anchor_x, anchor_z), None),
        |step| landscape::preview_column_and_landscape(cfg, anchor_x, anchor_z, step),
    );
    let ground_y = column.height;
    let woodland_soil = cfg.generator_version >= 11
        && matches!(
            column.surface,
            BlockId::Dirt | BlockId::Mud | BlockId::Gravel
        )
        && column.vegetation_density > 0.20;
    if column.surface != BlockId::Grass
        && !(cfg.generator_version >= 2 && column.surface == BlockId::SnowGrass)
        && !woodland_soil
        && !(cfg.generator_version >= 94
            && matches!(column.surface, BlockId::GroveMoss | BlockId::FungalSoil)
            && column.vegetation_density > 0.20)
    {
        return None;
    }

    if chance > column.vegetation_density * density_factor {
        return None;
    }

    if cfg.generator_version >= 54 && sites::tree_reserved(cfg,cfg.world_x(anchor_x),cfg.world_z(anchor_z)) {
        return None;
    }
    // Gate 136 riparian woods crowd the banks; their trunks stay out of the water.
    if geo::corridor::active(cfg) && footprint.is_none()
        && geo::ground(cfg, cfg.world_x(anchor_x), cfg.world_z(anchor_z), cfg.voxel_meters()).water_m.is_some()
    {
        return None;
    }
    let slope_limit = cfg.blocks_for_span(0.9);
    if ground_y <= cfg.water_level() + cfg.blocks_for_span(2.2).max(2)
        || [(-1, 0), (1, 0), (0, -1), (0, 1)].into_iter().any(|(dx, dz)| {
            (column_at(anchor_x + dx, anchor_z + dz).height - ground_y).abs() > slope_limit
        })
    {
        return None;
    }

    let trunk_h =
        cfg.blocks_for_span(4.0 + hash2(cell_x, cell_z, cfg.seed.wrapping_add(449)) * 3.2);
    let canopy_radius = if hash2(cell_x, cell_z, cfg.seed.wrapping_add(457)) > 0.66 {
        cfg.blocks_for_span(2.9)
    } else {
        cfg.blocks_for_span(2.3)
    };
    let canopy_height =
        cfg.blocks_for_span(3.0 + hash2(cell_x, cell_z, cfg.seed.wrapping_add(463)) * 1.8);
    let branch_count = 3 + (hash2(cell_x, cell_z, cfg.seed.wrapping_add(487)) * 3.0) as i32;
    let branch_len =
        cfg.blocks_for_span(1.5 + hash2(cell_x, cell_z, cfg.seed.wrapping_add(499)) * 1.5);
    let branch_start_y = ground_y + trunk_h - cfg.blocks_for_span(2.2);
    let lean_idx = ((hash2(cell_x, cell_z, cfg.seed.wrapping_add(521)) * 5.0) as i32).min(4);
    let (lean_x, lean_z) = match lean_idx {
        1 => (1, 0),
        2 => (-1, 0),
        3 => (0, 1),
        4 => (0, -1),
        _ => (0, 0),
    };

    let mut tree = TreeProfile {
        conifer: cfg.generator_version >= 2 && column.surface == BlockId::SnowGrass,
        cell_x,
        cell_z,
        anchor_x,
        anchor_z,
        ground_y,
        trunk_h,
        canopy_radius,
        canopy_height,
        branch_count,
        branch_len,
        branch_start_y,
        lean_x,
        lean_z,
        form: trees::TreeForm::Classic,
    };
    if cfg.generator_version >= 6 {
        if let Some(climate) = preview {
            trees::profile_from_landscape(cfg, &mut tree, climate);
        } else if let Some(step) = footprint {
            trees::profile_lod(cfg, &mut tree, step);
        } else {
            trees::profile(cfg, &mut tree);
        }
    } else if cfg.generator_version >= 5 {
        vegetation::shape_profile(cfg, &mut tree);
    }
    if ruins::tree_conflicts(
        cfg,
        cfg.world_x(anchor_x),
        cfg.world_z(anchor_z),
        cfg.world_x(tree.canopy_radius.max(tree.branch_len)) * 1.65 + 2.0,
    ) {
        return None;
    }
    if cfg.generator_version >= trees::giant::VERSION
        && trees::giant::crowds(cfg, &tree, |x, z| tree_anchor(cfg, x, z))
    {
        return None;
    }
    Some(tree)
}

/// Highest voxel of any landmark tree whose crown reaches the voxel box
/// `min..=max` (x, z), so residency can keep the whole crown loaded.
pub fn giant_tree_top_near(cfg: &WorldSampleConfig, min: (i32, i32), max: (i32, i32)) -> Option<i32> {
    trees::giant::top_near(cfg, min, max)
}

pub fn bush_profile_for_cell(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<BushProfile> {
    generation_cache::bush(cfg, cell_x, cell_z)
}

pub fn bush_profile_uncached(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<BushProfile> {
    if matches!(cfg.world_type, WorldType::Nether | WorldType::Hall | WorldType::Realm(_)) { return None; }
    let cell_size = cfg.blocks_for_span(8.0);
    let border = cfg
        .blocks_for_span(1.5)
        .min((cell_size - 1).max(1) / 2)
        .max(1);
    let inner_span = (cell_size - border * 2).max(1);

    let offset_x = border
        + ((hash2(cell_x, cell_z, cfg.seed.wrapping_add(467)) * inner_span as f32) as i32)
            .min(inner_span - 1);
    let offset_z = border
        + ((hash2(cell_x, cell_z, cfg.seed.wrapping_add(479)) * inner_span as f32) as i32)
            .min(inner_span - 1);
    let anchor_x = cell_x * cell_size + offset_x;
    let anchor_z = cell_z * cell_size + offset_z;

    if ruins::clears_vegetation(cfg, cfg.world_x(anchor_x), cfg.world_z(anchor_z))
        || civilization::clears_vegetation(cfg, cfg.world_x(anchor_x), cfg.world_z(anchor_z))
    {
        return None;
    }
    let column = sample_column(cfg, anchor_x, anchor_z);
    let ground_y = column.height;
    // Gate 136: closed stands carry an understory on their own litter and
    // moss, about three bushes in four cells where density saturates.
    let understory = geo::corridor::active(cfg);
    if column.vegetation_density <= 0.24 || ground_y <= cfg.water_level() + 1 {
        return None;
    }
    let jungle = trees::rainforest::thicket(cfg, anchor_x, anchor_z);
    let colonies = cfg.generator_version >= engine_voxel::CANOPY_UNDERSTORY_GENERATOR;
    let mossy_soil = colonies && column.vegetation_density > 0.45
        && matches!(column.surface, BlockId::GroveMoss | BlockId::Dirt | BlockId::SnowGrass);
    let floor_soil = matches!(column.surface, BlockId::Dirt | BlockId::Mud | BlockId::GroveMoss)
        && (jungle > 0.0 || (understory && column.vegetation_density > 0.8));
    if column.surface != BlockId::Grass && !floor_soil && !mossy_soil {
        return None;
    }

    if understory && geo::ground(cfg, cfg.world_x(anchor_x), cfg.world_z(anchor_z), cfg.voxel_meters()).water_m.is_some() {
        return None;
    }
    let chance = hash2(cell_x, cell_z, cfg.seed.wrapping_add(461));
    let thicket = if understory { smoothstep(0.75, 1.0, column.vegetation_density) * 0.5 } else { 0.0 };
    let colony = if colonies {
        let patch = value_noise2(cfg.world_x(anchor_x) / 23.0, cfg.world_z(anchor_z) / 23.0, cfg.seed.wrapping_add(194_033));
        smoothstep(0.40, 0.74, patch) * column.vegetation_density.min(1.0)
    } else { 0.0 };
    let cover = column.vegetation_density * 0.26 + thicket + jungle * 0.44 + colony * 0.20;
    let swale = jungle > 0.0 && column.surface == BlockId::Mud && trees::rainforest::forest_floor(cfg);
    if chance > cover * if swale { 0.42 } else { 1.0 } {
        return None;
    }

    let h_nx = terrain_height(cfg, anchor_x - 1, anchor_z);
    let h_px = terrain_height(cfg, anchor_x + 1, anchor_z);
    let h_nz = terrain_height(cfg, anchor_x, anchor_z - 1);
    let h_pz = terrain_height(cfg, anchor_x, anchor_z + 1);
    let slope = (h_nx - ground_y)
        .abs()
        .max((h_px - ground_y).abs())
        .max((h_nz - ground_y).abs())
        .max((h_pz - ground_y).abs());
    if slope > cfg.blocks_for_span(0.85) {
        return None;
    }

    let spread = hash2(cell_x, cell_z, cfg.seed.wrapping_add(487));
    let mut base_radius = if jungle > 0.0 {
        cfg.blocks_for_span(1.2 + jungle * (0.8 + spread * 1.6))
    } else if spread > 0.58 {
        cfg.blocks_for_span(1.6)
    } else {
        cfg.blocks_for_span(1.1)
    };
    let height = hash2(cell_x, cell_z, cfg.seed.wrapping_add(503));
    let mut top_dy = if jungle > 0.0 {
        cfg.blocks_for_span(0.9 + jungle * height * 1.6)
    } else if height > 0.74 {
        cfg.blocks_for_span(1.7)
    } else {
        cfg.blocks_for_span(1.1)
    };
    if colony > 0.0 && !swale {
        base_radius = base_radius.max(cfg.blocks_for_span(1.35 + colony * (1.65 + spread * 1.5) + jungle * 0.55));
        top_dy = top_dy.max(cfg.blocks_for_span(0.8 + colony * (0.55 + height * 1.2) + jungle * 0.45));
    }

    let lean_index = ((hash2(cell_x, cell_z, cfg.seed.wrapping_add(509)) * 5.0) as i32).min(4);
    let (lean_x, lean_z) = match lean_index {
        1 => (1, 0),
        2 => (-1, 0),
        3 => (0, 1),
        4 => (0, -1),
        _ => (0, 0),
    };

    Some(BushProfile {
        anchor_x,
        anchor_z,
        ground_y,
        base_radius,
        top_dy,
        lean_x,
        lean_z,
    })
}

pub fn bush_contains(cfg: &WorldSampleConfig, bush: BushProfile, p: IVec3) -> bool {
    let dy = p.y - (bush.ground_y + 1);
    if dy < 0 || dy > bush.top_dy {
        return false;
    }
    if cfg.generator_version >= engine_voxel::CANOPY_UNDERSTORY_GENERATOR {
        let (x, z) = (p.x - bush.anchor_x, p.z - bush.anchor_z);
        let r = bush.base_radius as f32;
        if x.abs().max(z.abs()) as f32 > r * 1.20 {
            return false;
        }
        let (lx, lz) = if bush.lean_x == 0 && bush.lean_z == 0 { (1, 0) } else { (bush.lean_x, bush.lean_z) };
        let along = (x * lx + z * lz) as f32 / r;
        let across = (z * lx - x * lz) as f32 / r;
        let y = dy as f32 / (bush.top_dy + 1) as f32;
        // A larger mound with a second offset shoulder, both joined at the base.
        let main = ((along + 0.14) / 0.79).powi(2) + (across / 0.81).powi(2) + ((y - 0.29) / 0.69).powi(2);
        let wing = ((along - 0.49) / 0.59).powi(2) + ((across - 0.16) / 0.60).powi(2) + ((y - 0.12) / 0.57).powi(2);
        let shape = main.min(wing);
        if shape < 0.87 || shape >= 1.10 {
            return shape < 0.87;
        }
        let seed = cfg.seed.wrapping_add(521);
        let edge = if cfg.generator_version >= engine_voxel::DISTINCT_FOLIAGE_GENERATOR {
            // Keep clusters of leaves coherent as the grid gets finer.
            let frequency = cfg.voxel_meters() / 1.4;
            value_noise3(x as f32 * frequency, dy as f32 * frequency, z as f32 * frequency,
                seed)
        } else {
            hash3(p.x, p.y, p.z, seed)
        };
        return shape < 0.87 + 0.23 * edge;
    }

    let (center_x, center_z) = if dy >= 1 {
        (bush.anchor_x + bush.lean_x, bush.anchor_z + bush.lean_z)
    } else {
        (bush.anchor_x, bush.anchor_z)
    };
    let dx = (p.x - center_x).abs();
    let dz = (p.z - center_z).abs();
    let trim = hash3(p.x, p.y, p.z, cfg.seed.wrapping_add(521));
    if cfg.generator_version >= 5 {
        // Low spreading understorey with rounded shoulders and irregular tips.
        let r = bush.base_radius.max(1) as f32;
        let height = (bush.top_dy + 1) as f32;
        let shape = (dx as f32 / r).powi(2)
            + (dz as f32 / (r * 0.84)).powi(2)
            + ((dy as f32 - height * 0.22) / (height * 0.80)).powi(2);
        return shape < 0.87 + trim * 0.23;
    }

    match dy {
        0 if bush.base_radius > 1 => {
            let manhattan = dx + dz;
            manhattan <= 2 || (manhattan == 3 && dx.max(dz) == 2 && trim < 0.35)
        }
        0 => dx.max(dz) <= 1 && (dx != 1 || dz != 1 || trim < 0.45),
        1 => dx.max(dz) <= 1 && (dx != 1 || dz != 1 || trim < 0.32),
        _ => dx + dz == 0 || (bush.lean_x == 0 && bush.lean_z == 0 && dx + dz == 1 && trim < 0.18),
    }
}

/// Where generation reads tree and bush profiles and river surfaces from.
/// Single-voxel queries use the per-thread caches; whole chunks use
/// `RegionProfiles`, which resolves each profile and column once.
pub trait Profiles {
    fn hell_ruin(&self, cfg: &WorldSampleConfig, x: i32, z: i32, c: ColumnSample) -> Option<nether::RuinColumn> {
        nether::ruin_column(cfg, x, z, c)
    }
    fn tree(&self, cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<TreeProfile>;
    /// Highest y any tree this sampler can return may fill; `None` when unknown.
    fn tree_ceiling(&self, _cfg: &WorldSampleConfig) -> Option<i32> {
        None
    }
    fn bush(&self, cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<BushProfile>;
    fn river_top(&self, cfg: &WorldSampleConfig, x: i32, z: i32) -> Option<f32>;
}

pub struct CachedProfiles;

/// Generated inland water occupies the surface column above its bed.
/// Cache both wet and dry columns so simulation and meshing can reuse the plan.
pub fn steady_water_column(cfg: &WorldSampleConfig, x: i32, z: i32) -> Option<(i32, f32)> {
    use std::{cell::RefCell, collections::HashMap};
    type Columns = HashMap<(i32, i32), Option<(i32, f32)>>;
    thread_local! {
        static COLUMNS: RefCell<(Option<WorldSampleConfig>, Columns)> = RefCell::default();
    }
    if cfg.generator_version < 79
        || !matches!(cfg.world_type, WorldType::Hills | WorldType::Ridged | WorldType::Planet)
    {
        return None;
    }
    COLUMNS.with(|cache| {
        let mut cache = cache.borrow_mut();
        let (previous, columns) = &mut *cache;
        if *previous != Some(*cfg) || columns.len() >= 8192 {
            columns.clear();
            *previous = Some(*cfg);
        }
        *columns.entry((x, z)).or_insert_with(|| {
            let head = CachedProfiles.river_top(cfg, x, z)? / cfg.voxel_meters();
            let bed = terrain_height(cfg, x, z);
            (head > (bed + 1).max(cfg.water_level() + 1) as f32).then_some((bed, head))
        })
    })
}

/// Fractional starting stock at a natural river's free surface. Call only for
/// a known water voxel; earlier saves retain their whole-cell inventory.
pub fn river_water_amount(cfg: &WorldSampleConfig, p: IVec3) -> Option<u64> {
    if cfg.generator_version < 79 || p.y <= cfg.water_level()
        || cfg.world_type == WorldType::Superflat { return None; }
    let head = CachedProfiles.river_top(cfg, p.x, p.z)? / cfg.voxel_meters();
    fractional_water_stock(cfg, head, p.y)
}

fn fractional_water_stock(cfg: &WorldSampleConfig, head: f32, y: i32) -> Option<u64> {
    let fill = f64::from(head - y as f32);
    if fill <= 0.0 || fill >= 1.0 { return None; }
    let volume = engine_core::environment::cell_volume(cfg.voxel_scale);
    Some((fill * volume as f64).round().clamp(1.0, volume as f64) as u64)
}

impl Profiles for CachedProfiles {
    fn tree(&self, cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<TreeProfile> {
        tree_profile_for_cell(cfg, cell_x, cell_z)
    }
    fn bush(&self, cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<BushProfile> {
        bush_profile_for_cell(cfg, cell_x, cell_z)
    }
    fn river_top(&self, cfg: &WorldSampleConfig, x: i32, z: i32) -> Option<f32> {
        if geo::active(cfg) {
            return geo::ground(cfg, cfg.world_x(x), cfg.world_z(z), cfg.voxel_meters()).water_m;
        }
        let inherited = valley::river_top(cfg, cfg.world_x(x), cfg.world_z(z));
        let regional = drainage::sample(cfg, cfg.world_x(x), cfg.world_z(z)).water_m;
        let lake = lakes::sample(cfg, cfg.world_x(x), cfg.world_z(z)).water_m;
        let surfaces = [inherited, regional, lake].into_iter().flatten();
        if cfg.generator_version >= 62 { surfaces.reduce(f32::min) }
        else { surfaces.reduce(f32::max) }
    }
}

type HellRuinSites = std::collections::HashMap<(i32, i32), Option<(f32, f32, u32)>>;

/// Dense profile grids and river surfaces for a box of columns.
///
/// Every air voxel above ground checks 25 tree cells and 9 bush cells. Through
/// the hashed thread-local cache that was ~19 ms per padded chunk, and the
/// river surface (which depends only on x/z) was re-evaluated for every voxel
/// of the column. Each cell is filled on first use and then reused, so a chunk
/// pays only for the cells its voxels actually reach.
pub struct RegionProfiles {
    hell_sites: std::cell::RefCell<HellRuinSites>,
    hell_columns: Vec<std::cell::OnceCell<Option<nether::RuinColumn>>>,
    tree_min: (i32, i32),
    tree_w: i32,
    trees: Vec<std::cell::OnceCell<Option<TreeProfile>>>,
    bush_min: (i32, i32),
    bush_w: i32,
    bushes: Vec<std::cell::OnceCell<Option<BushProfile>>>,
    column_min: (i32, i32),
    column_w: i32,
    rivers: Vec<std::cell::OnceCell<Option<f32>>>,
    tree_ceiling: std::cell::OnceCell<i32>,
}

impl RegionProfiles {
    /// Covers columns `min_x..min_x + size` by `min_z..min_z + size`.
    fn new(cfg: &WorldSampleConfig, min_x: i32, min_z: i32, size: i32) -> Self {
        // Cell ranges match `structure_block`: trees +-2 cells, bushes +-1.
        let grid = |cell: i32, reach: i32| {
            let lo = (floor_div(min_x, cell) - reach, floor_div(min_z, cell) - reach);
            let w = floor_div(min_x + size - 1, cell) + reach - lo.0 + 1;
            let h = floor_div(min_z + size - 1, cell) + reach - lo.1 + 1;
            (lo, w, h)
        };
        let (tree_min, tree_w, tree_h) = grid(cfg.blocks_for_span(12.0), 2);
        let (bush_min, bush_w, bush_h) = grid(cfg.blocks_for_span(8.0), 1);
        let rivers_on = cfg.generator_version >= 14;
        Self {
            hell_sites: Default::default(),
            hell_columns: if cfg.hell_ruins() {
                (0..size * size).map(|_| std::cell::OnceCell::new()).collect()
            } else { Vec::new() },
            tree_min,
            tree_w,
            trees: (0..tree_w * tree_h).map(|_| std::cell::OnceCell::new()).collect(),
            bush_min,
            bush_w,
            bushes: (0..bush_w * bush_h).map(|_| std::cell::OnceCell::new()).collect(),
            column_min: (min_x, min_z),
            column_w: size,
            rivers: (0..if rivers_on { size * size } else { 0 })
                .map(|_| std::cell::OnceCell::new())
                .collect(),
            tree_ceiling: std::cell::OnceCell::new(),
        }
    }
}

impl Profiles for RegionProfiles {
    fn hell_ruin(&self, cfg: &WorldSampleConfig, x: i32, z: i32, c: ColumnSample) -> Option<nether::RuinColumn> {
        let index = (z - self.column_min.1) * self.column_w + (x - self.column_min.0);
        *self.hell_columns[index as usize].get_or_init(|| {
            nether::ruin_column_with(cfg, x, z, c, |ix, iz| {
                *self.hell_sites.borrow_mut().entry((ix, iz))
                    .or_insert_with(|| nether::ruin_site(cfg, ix, iz))
            })
        })
    }
    fn tree(&self, cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<TreeProfile> {
        let (x, z) = (cell_x - self.tree_min.0, cell_z - self.tree_min.1);
        *self.trees[(z * self.tree_w + x) as usize]
            .get_or_init(|| tree_profile_for_cell(cfg, cell_x, cell_z))
    }
    // The grid holds every cell `structure_block` asks about for this region.
    fn tree_ceiling(&self, cfg: &WorldSampleConfig) -> Option<i32> {
        Some(*self.tree_ceiling.get_or_init(|| {
            let rows = self.trees.len() as i32 / self.tree_w.max(1);
            (0..rows).flat_map(|z| (0..self.tree_w).map(move |x| (x, z)))
                .filter_map(|(x, z)| self.tree(cfg, x + self.tree_min.0, z + self.tree_min.1))
                .map(|tree| tree.reach(cfg).2)
                .max()
                .unwrap_or(i32::MIN)
        }))
    }
    fn bush(&self, cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<BushProfile> {
        let (x, z) = (cell_x - self.bush_min.0, cell_z - self.bush_min.1);
        *self.bushes[(z * self.bush_w + x) as usize]
            .get_or_init(|| bush_profile_for_cell(cfg, cell_x, cell_z))
    }
    fn river_top(&self, cfg: &WorldSampleConfig, x: i32, z: i32) -> Option<f32> {
        let (x, z) = (x - self.column_min.0, z - self.column_min.1);
        *self.rivers[(z * self.column_w + x) as usize]
            .get_or_init(|| CachedProfiles.river_top(cfg, x + self.column_min.0, z + self.column_min.1))
    }
}

pub fn structure_block(
    cfg: &WorldSampleConfig,
    p: IVec3,
    column: ColumnSample,
    profiles: &impl Profiles,
) -> BlockId {
    if p.y <= column.height || cfg.world_type == WorldType::Superflat {
        return BlockId::Air;
    }
    if column.surface == BlockId::Gravel && p.y - column.height <= cfg.blocks_for_span(2.2)
        && planetary::trail_reserves(cfg, cfg.world_x(p.x), cfg.world_z(p.z), 1.8)
    {
        return BlockId::Air;
    }

    let cell_size = cfg.blocks_for_span(12.0);
    let base_cell_x = floor_div(p.x, cell_size);
    let base_cell_z = floor_div(p.z, cell_size);

    let mut overlapping_foliage = BlockId::Air;
    // A tree outside its reach box returns Air, which this loop ignores.
    let above_trees = profiles.tree_ceiling(cfg).is_some_and(|top| p.y > top);
    for cell_z in (base_cell_z - 2)..=(base_cell_z + 2) {
        if above_trees {
            break;
        }
        for cell_x in (base_cell_x - 2)..=(base_cell_x + 2) {
            let Some(tree) = profiles.tree(cfg, cell_x, cell_z) else {
                continue;
            };
            if !tree.may_fill(cfg, p) {
                continue;
            }
            let block = tree_block(cfg, tree, p);
            // Giants keep their crowns over trails; only head height is cleared.
            let cleared = |p: IVec3| {
                (tree.form != trees::TreeForm::Giant || p.y <= tree.ground_y + cfg.blocks_for_span(4.5))
                    && surface::trail_clears(cfg, p.as_vec3() * cfg.voxel_meters())
            };
            if block != BlockId::Air && !cleared(p) {
                // Wide crowns can cross several placement cells. Prefer a
                // neighboring bole to foliage when both occupy this voxel.
                let wide_crown = tree.form.is_rainforest()
                    || matches!(tree.form, trees::TreeForm::OldOak | trees::TreeForm::CloudCrown | trees::TreeForm::PlateauCrown)
                    || trees::is_tall_redwood(cfg, &tree);
                if wide_crown && block != BlockId::Log {
                    if overlapping_foliage == BlockId::Air || matches!(block, BlockId::Leaves | BlockId::PineLeaves) {
                        overlapping_foliage = block;
                    }
                    continue;
                }
                return block;
            }
        }
    }
    if overlapping_foliage != BlockId::Air {
        return overlapping_foliage;
    }

    let fallen = deadwood::block(cfg, p, column);
    if fallen != BlockId::Air {
        return fallen;
    }

    let find = wayside::block(cfg, p, column);
    if find != BlockId::Air {
        return find;
    }

    let bush_cell_size = cfg.blocks_for_span(8.0);
    let bush_cell_x = floor_div(p.x, bush_cell_size);
    let bush_cell_z = floor_div(p.z, bush_cell_size);
    for cell_z in (bush_cell_z - 1)..=(bush_cell_z + 1) {
        for cell_x in (bush_cell_x - 1)..=(bush_cell_x + 1) {
            if let Some(bush) = profiles.bush(cfg, cell_x, cell_z)
                && bush_contains(cfg, bush, p)
                    && !surface::trail_clears(cfg, p.as_vec3() * cfg.voxel_meters())
                {
                    return BlockId::Bush;
                }
        }
    }

    if plants::active(cfg) {
        let plant = plants::block_in_column(cfg, p, column);
        if plant != BlockId::Air && !surface::trail_clears(cfg, p.as_vec3() * cfg.voxel_meters()) {
            return plant;
        }
    }

    if flora::active(cfg) {
        let plant = flora::block(cfg, p, column);
        if plant != BlockId::Air && !surface::trail_clears(cfg, p.as_vec3() * cfg.voxel_meters()) {
            return plant;
        }
    }

    BlockId::Air
}

pub fn sample_block(cfg: &WorldSampleConfig, p: IVec3) -> BlockId {
    if p.y < cfg.min_world_y() || p.y >= cfg.max_world_y() {
        return BlockId::Air;
    }
    sample_block_in_column(cfg, p, sample_column(cfg, p.x, p.z))
}

pub fn sample_block_in_column(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> BlockId {
    sample_block_with(cfg, p, column, &CachedProfiles)
}

/// Blocks and bed data for a chunk plus its one-voxel halo, before meshing.
pub struct PaddedTerrain {
    pub coord: IVec3,
    pub origin: IVec3,
    pub blocks: Vec<BlockId>,
    /// Terrain surface height per padded column, even below this chunk.
    pub bed_height: Vec<i32>,
    /// Fraction of the bed cell under water per padded column; empty when no column holds water.
    pub bed_frac: Vec<f32>,
    /// Initial volume of partial natural surface cells, including the halo.
    pub partial_water: Vec<(IVec3, u64)>,
    /// Highest sky-occluding cell per padded column (terrain height when it lies above the chunk).
    pub sky_ceiling: Vec<i32>,
}

impl PaddedTerrain {
    fn sky_occluder(block: BlockId) -> bool {
        block.is_opaque() && !matches!(block, BlockId::Leaves | BlockId::PineLeaves | BlockId::Bush)
    }

    /// World y of the highest occluder in padded column (x, z), or one below the chunk when none.
    /// Like `local_roof`, ignoring occluders at or above world `limit_y`.
    fn roof_below(&self, x: i32, z: i32, limit_y: i32) -> i32 {
        let end = limit_y.saturating_sub(self.origin.y).clamp(0, PADDED_CHUNK_SIZE);
        self.roof_in_column(x, z, end)
    }

    pub fn local_roof(&self, x: i32, z: i32) -> i32 {
        self.roof_in_column(x, z, PADDED_CHUNK_SIZE)
    }

    fn roof_in_column(&self, x: i32, z: i32, end: i32) -> i32 {
        let size = PADDED_CHUNK_SIZE;
        let base = (z * size + x) as usize;
        let plane = (size * size) as usize;
        (0..end)
            .rev()
            .find(|&y| Self::sky_occluder(self.blocks[base + y as usize * plane]))
            .map_or(self.origin.y - 1, |y| self.origin.y + y)
    }
}

pub fn generate_padded(
    cfg: &WorldSampleConfig,
    coord: IVec3,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Option<PaddedTerrain> {
    let size = PADDED_CHUNK_SIZE;
    let origin = coord * CHUNK_SIZE - IVec3::ONE;
    let columns = generation_cache::footprint(cfg, origin.x, origin.z, size, cancelled)?;
    let mut blocks = vec![BlockId::Air; (size * size * size) as usize];
    let mut bed_height = vec![0; (size * size) as usize];
    let profiles = RegionProfiles::new(cfg, origin.x, origin.z, size);
    let min_y = cfg.min_world_y().saturating_sub(origin.y).clamp(0, size);
    let end_y = cfg.max_world_y().saturating_sub(origin.y).clamp(0, size);
    let plane = (size * size) as usize;
    let mut wet_columns = [false; (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as usize];
    let mut occupied_columns = [false; (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as usize];
    for z in 0..size {
        for x in 0..size {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }
            let column_index = (z * size + x) as usize;
            let column = columns[column_index];
            bed_height[column_index] = column.height;
            let world_x = origin.x + x;
            let world_z = origin.z + z;
            let basin = basin::covers(cfg, world_x, world_z);
            let mut wet = false;
            let mut occupied = false;
            let slots = blocks[column_index..].iter_mut().step_by(plane);
            for (y, slot) in (min_y..end_y).zip(slots.skip(min_y as usize)) {
                // The loop bounds already exclude voxels outside the world.
                let block = sample_block_in_world(
                    cfg, IVec3::new(world_x, origin.y + y, world_z), column, &profiles, Some(basin),
                );
                *slot = block;
                wet |= block == BlockId::Water;
                occupied |= block != BlockId::Air;
            }
            wet_columns[column_index] = wet;
            occupied_columns[column_index] = occupied;
        }
    }
    let mut bed_frac = Vec::new();
    let mut partial_water = Vec::new();
    if cfg.generator_version >= 30 && wet_columns.contains(&true) {
        let water_level = cfg.water_level();
        let check_river = cfg.generator_version >= 79
            && cfg.world_type != WorldType::Superflat
            && origin.y + size - 1 > water_level;
        let voxel_meters = cfg.voxel_meters();
        // Only columns that hold water need the fraction; it costs one extra
        // landscape sample each and is identical on both sides of a chunk border.
        bed_frac = vec![0.0f32; (size * size) as usize];
        for z in 0..size {
            for x in 0..size {
                let i = (z * size + x) as usize;
                if wet_columns[i] {
                    bed_frac[i] = landscape::bed_fraction(cfg, origin.x + x, origin.z + z, bed_height[i]);
                    if check_river
                        && let Some(level) = profiles.river_top(cfg, origin.x + x, origin.z + z)
                    {
                        let head = level / voxel_meters;
                        let y = head.floor() as i32;
                        let local_y = y - origin.y;
                        if y > water_level && (0..size).contains(&local_y)
                            && blocks[((local_y * size + z) * size + x) as usize] == BlockId::Water
                            && let Some(amount) = fractional_water_stock(cfg, head, y)
                        {
                            partial_water.push((IVec3::new(origin.x + x, y, origin.z + z), amount));
                        }
                    }
                }
            }
        }
    }
    let mut terrain = PaddedTerrain { coord, origin, blocks, bed_height, bed_frac, partial_water, sky_ceiling: Vec::new() };
    let mut sky_ceiling = terrain.bed_height.clone();
    let voxel_meters = cfg.voxel_meters();
    let top = origin.y + size;
    let no_local_roof = origin.y - 1;
    let local_roof = |x, z, i: usize| {
        if occupied_columns[i] { terrain.local_roof(x, z) } else { no_local_roof }
    };
    for z in 0..size {
        for x in 0..size {
            let i = (z * size + x) as usize;
            let span = vistas::span_soffit(cfg,
                ((origin.x + x) as f32 + 0.5) * voxel_meters, ((origin.z + z) as f32 + 0.5) * voxel_meters);
            if let Some(soffit) = span {
                sky_ceiling[i] = if occupied_columns[i] {
                    terrain.roof_below(x, z, (soffit / voxel_meters).floor() as i32)
                } else {
                    no_local_roof
                };
            } else if sky_ceiling[i] < top {
                sky_ceiling[i] = local_roof(x, z, i);
            } else if karst::skylight(cfg, origin.x+x, origin.z+z) {
                // Daylight through a real shaft must not depend on upper-chunk residency.
                let column = columns[i];
                let clear = (top..=column.height).rev().all(|y| {
                    !PaddedTerrain::sky_occluder(sample_block_with(cfg,
                        IVec3::new(origin.x+x,y,origin.z+z),column,&profiles))
                });
                if clear { sky_ceiling[i] = local_roof(x, z, i); }
            }
        }
    }
    terrain.sky_ceiling = sky_ceiling;
    Some(terrain)
}

/// Freight market, surveyor sites, rebuilt ruins and generated settlements, in
/// that order. Each module's `features` gate decides which surfaces see it.
fn settlement_block(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample, freight: bool, with_ruins: bool) -> Option<BlockId> {
    if freight && let Some(block) = planet_freight::block(cfg, p, column) {
        return Some(block);
    }
    if let Some(block) = surveyor::block(cfg, p, column) {
        return Some(block);
    }
    if with_ruins && cfg.generator_version >= ruins::REBUILT_VERSION
        && let Some(block) = ruins::block(cfg, p, (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters(), cfg.world_y(column.height + 1))
    {
        return Some(block);
    }
    civilization::block(cfg, p, column)
}

/// Hand-placed sites in hills order; `cavern` adds the underground site pass.
pub(crate) fn authored_block(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample, cavern: bool) -> Option<BlockId> {
    let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
    vistas::priority_block(cfg,p,column)
        .or_else(|| fernwake::sample(cfg, m))
        .or_else(|| households::sample(cfg, m))
        .or_else(|| meridian_terminal::block(cfg, p, column.height))
        .or_else(|| undercrown::block(cfg, p, column.height))
        .or_else(|| amberroot::block(cfg, p, column.height))
        .or_else(|| sites::block(cfg, p, column.height))
        .or_else(|| if cavern { sites::cavern(cfg, p, column.height) } else { None })
        .or_else(|| vistas::block(cfg,p,column))
}

pub fn sample_block_with(
    cfg: &WorldSampleConfig,
    p: IVec3,
    column: ColumnSample,
    profiles: &impl Profiles,
) -> BlockId {
    if p.y < cfg.min_world_y() || p.y >= cfg.max_world_y() {
        return BlockId::Air;
    }
    sample_block_in_world(cfg, p, column, profiles, None)
}

fn sample_block_in_world(
    cfg: &WorldSampleConfig,
    p: IVec3,
    column: ColumnSample,
    profiles: &impl Profiles,
    basin: Option<bool>,
) -> BlockId {
    if let Some(block) = planetary::reference_relief::block(cfg, p, column) { return block; }
    if cfg.world_type == WorldType::Hall { return hall::block(cfg, p); }
    if cfg.world_type == WorldType::Realm(RealmKind::Lightless) { return lightless::block(cfg, p); }
    if cfg.world_type == WorldType::Realm(RealmKind::Gilded) { return gilded::block(cfg, p); }
    if cfg.world_type == WorldType::Realm(RealmKind::Highfold) { return highfold::block(cfg, p); }
    if cfg.world_type == WorldType::Realm(RealmKind::Gloaming) { return gloaming::block(cfg, p); }
    if let Some(block) = hell_gate::block(cfg, p, column) { return block; }
    if cfg.world_type == WorldType::Nether { return nether::block_with(cfg, p, column, profiles); }
    if cfg.world_type == WorldType::Manyfold { return manyfold::block(cfg,p); }
    // The solved face shares the hills block path but none of its authored sites.
    let basin = basin.unwrap_or_else(|| basin::covers(cfg, p.x, p.z));
    // The elevator exists on every planet generator, including the planetary
    // solver that bypasses the legacy basin. Its editable shaft must own this
    // footprint before ruins/terrain, just as its fittings and boarding do.
    if (basin || cfg.world_type == WorldType::Planet)
        && let Some(block) = meridian_terminal::block(cfg, p, column.height)
    {
        return block;
    }
    // Planet caverns reach the solved face too; the hills site chain below is skipped there.
    if basin && cfg.generator_version >= planet::UNDERGROUND_VERSION
        && let Some(block) = sites::cavern(cfg, p, column.height)
    {
        return block;
    }
    let planetary = cfg.world_type == WorldType::Planet;
    let crater_lake = planetary && planet::is_crater_lake(cfg, &column);
    if planetary && (!basin || crater_lake) {
        if let Some(block) = settlement_block(cfg, p, column, !crater_lake, true) {
            return block;
        }
        let block = planet::block_with(cfg, p, column, profiles);
        if block == BlockId::Air && p.y > column.height && cfg.generator_version >= planet::BIOME_VERSION {
            let structure = structure_block(cfg, p, column, profiles);
            if structure != BlockId::Air {
                return structure;
            }
        }
        return block;
    }

    if cfg.generator_version >= bedrock::VERSION
        && p.y <= bedrock::ceiling(cfg)
        && p.y <= bedrock::top(cfg, p.x, p.z)
    {
        return BlockId::Bedrock;
    }

    if cfg.generator_version >= 3 && !basin {
        // This patch belongs to generation, so rendering and collision agree.
        let m = p.as_vec3() * cfg.voxel_meters();
        if cfg.world_type != WorldType::Superflat && m.x.abs() < 10.0 && m.z.abs() < 10.0 {
            if cfg.generator_version >= surface::SPAWN_ROAD_VERSION
                && let Some(road) = surface::block(cfg, p, column.height, cfg.world_y(column.height + 1))
            {
                return road;
            }
            if p.y > column.height {
                return BlockId::Air;
            }
            if p.y >= column.height - cfg.blocks_for_span(6.0) {
                return if p.y == column.height {
                    BlockId::Grass
                } else {
                    BlockId::Dirt
                };
            }
        }
        if let Some(block) = authored_block(cfg, p, column, true) {
            return block;
        }
    }
    // Flat ruins already come through the site chain above.
    if !basin && features::shared_line(cfg)
        && let Some(block) = settlement_block(cfg, p, column, true, false)
    {
        return block;
    }

    if p.y > column.height {
        let structure = structure_block(cfg, p, column, profiles);
        if structure != BlockId::Air {
            return structure;
        }
        if p.y <= cfg.water_level() {
            return BlockId::Water;
        }
        if cfg.generator_version >= 14
            && let Some(surface) = profiles.river_top(cfg, p.x, p.z)
                && cfg.world_y(p.y) < surface {
                    let depth = surface - cfg.world_y(p.y);
                    if depth <= river_ice::MAX_M && depth <= river_ice::thickness_m(cfg, p.x, p.z, surface) {
                        return BlockId::Ice;
                    }
                    return BlockId::Water;
                }
        return BlockId::Air;
    }

    if cave_carves(cfg, p, &column) {
        // Small aquifer pockets replace continuous wet gallery floors.
        // Previous worlds retain their generated cells and drainage behavior.
        if cfg.generator_version >= 73 {
            let pocket = aquifer_pocket(cfg, p, &column);
            if pocket == BlockId::Air && basin && cfg.generator_version >= deep::VERSION {
                return deep::cave_air(cfg, p, &column);
            }
            if pocket == BlockId::Air && flora::active(cfg) {
                let plant = flora::cave(cfg, p, |q| flora::rock_at(cfg, q));
                if plant != BlockId::Air {
                    return plant;
                }
            }
            return pocket;
        }
        // Generator 17 leaves a wet floor instead of filling the gallery.
        // Older worlds still drown every carve that dips under the aquifer.
        if cfg.generator_version >= 17 {
            let below = IVec3::new(p.x, p.y - 1, p.z);
            if p.y <= water_table(cfg, p.x, p.z)
                && p.y > cfg.min_world_y() + 1
                && !cave_carves(cfg, below, &column)
            {
                return BlockId::Water;
            }
            return BlockId::Air;
        }
        if p.y <= water_table(cfg, p.x, p.z) {
            return BlockId::Water;
        }
        return BlockId::Air;
    }

    let depth = column.height - p.y;
    if geo::active(cfg)
        && let Some(block) = geo::subsurface(cfg, cfg.world_x(p.x), cfg.world_z(p.z), depth as f32 * cfg.voxel_meters())
    {
        return block;
    }
    if basin && depth <= cfg.blocks_for_span(1.5)
        && placer::grade(cfg, cfg.world_x(p.x), cfg.world_z(p.z)).is_some_and(|g| g > placer::PAYDIRT_GRADE)
        && profiles.river_top(cfg, p.x, p.z).is_some_and(|w| w > cfg.world_y(column.height))
    {
        return BlockId::Paydirt;
    }
    // Lode veins show in rock and on bare slopes, never through turf.
    if basin && (depth >= cfg.blocks_for_span(2.0)
            || matches!(column.surface, BlockId::Stone | BlockId::Gravel | BlockId::Limestone | BlockId::Basalt | BlockId::Sandstone))
        && placer::vein(cfg, (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters())
    {
        return BlockId::GoldQuartz;
    }
    // Generator 15 lets rocky, shingle and mud shores keep their own surface.
    let sandy_shore = cfg.generator_version < 15
        || matches!(
            column.surface,
            BlockId::Sand | BlockId::Grass | BlockId::Dirt
        );
    if column.height <= cfg.water_level() + 1 && sandy_shore {
        if depth == 0 {
            return BlockId::Sand;
        }
        if depth
            <= if cfg.generator_version >= 2 {
                cfg.blocks_for_span(2.2)
            } else {
                2
            }
        {
            return if hash3(p.x, p.y, p.z, cfg.seed.wrapping_add(541)) > 0.78 {
                BlockId::Sandstone
            } else {
                BlockId::Sand
            };
        }
    } else if column.height <= cfg.water_level() + cfg.blocks_for_span(2.5).max(2)
        && column.surface == BlockId::Sand
    {
        if depth == 0 {
            return BlockId::Sand;
        }
        if depth <= cfg.blocks_for_span(2.5).max(2) {
            return if depth == 1 {
                BlockId::Sand
            } else {
                BlockId::Sandstone
            };
        }
    }

    if depth == 0 {
        if basin && cfg.generator_version >= deep::VERSION {
            return deep::surface_tell(cfg, p.x, p.z, &column).unwrap_or(column.surface);
        }
        column.surface
    } else if depth
        <= if cfg.generator_version >= 2 {
            cfg.blocks_for_span(2.0)
        } else {
            3
        }
    {
        column.filler
    } else if depth
        <= if cfg.generator_version >= 2 {
            cfg.blocks_for_span(3.0)
        } else {
            5
        }
        && hash3(p.x, p.y, p.z, cfg.seed.wrapping_add(563)) > 0.76
    {
        BlockId::Gravel
    } else if basin && cfg.generator_version >= deep::VERSION {
        deep::rock(cfg, p, &column, landscape::geological_stratum(cfg, p, column.height, depth))
    } else if cfg.generator_version >= 2 {
        landscape::geological_stratum(cfg, p, column.height, depth)
    } else {
        BlockId::Stone
    }
}



/// Generate one chunk of terrain from the configuration alone.
pub fn generate(cfg: &WorldSampleConfig, chunk_coord: IVec3) -> ChunkData {
    let mut blocks = Box::new([BlockId::Air; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize]);
    let chunk_origin = chunk_coord * CHUNK_SIZE;
    let profiles = RegionProfiles::new(cfg, chunk_origin.x, chunk_origin.z, CHUNK_SIZE);
    // Torch index feeds the light list; it must match the torch blocks written here.
    let mut torches = HashSet::new();

    for z in 0..CHUNK_SIZE {
        for x in 0..CHUNK_SIZE {
            let column = sample_column(cfg, chunk_origin.x + x, chunk_origin.z + z);
            for y in 0..CHUNK_SIZE {
                let world_pos = chunk_origin + IVec3::new(x, y, z);
                let index = ChunkData::local_index(x, y, z);
                let block = sample_block_with(cfg, world_pos, column, &profiles);
                if block.is_light_source() {
                    torches.insert(world_pos);
                }
                blocks[index] = block;
            }
        }
    }

    ChunkData {
        blocks,
        modified_bounds: None,
        structural_bounds: None,
        light: Box::new([0; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize]),
        torches,
    }
}

#[cfg(test)]
mod generated_torches {
    use super::*;

    /// Generated torch blocks must be indexed, or they mesh but never light the scene.
    #[test]
    fn generated_torch_index_matches_torch_blocks() {
        let cfg = crate::test_world::World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 7).sample_config();
        let mut found = 0usize;
        for z in -2..3 {
            for x in -2..3 {
                for y in 0..4 {
                    let coord = IVec3::new(x, y, z);
                    let data = generate(&cfg, coord);
                    let origin = coord * CHUNK_SIZE;
                    let mut expected = HashSet::new();
                    for (i, block) in data.blocks.iter().enumerate() {
                        if block.is_light_source() {
                            let i = i as i32;
                            expected.insert(
                                origin
                                    + IVec3::new(
                                        i % CHUNK_SIZE,
                                        i / (CHUNK_SIZE * CHUNK_SIZE),
                                        (i / CHUNK_SIZE) % CHUNK_SIZE,
                                    ),
                            );
                        }
                    }
                    found += expected.len();
                    assert_eq!(data.torches, expected, "torch index for chunk {coord:?}");
                }
            }
        }
        assert!(found > 0, "scan window held no generated torches; widen it");
    }
}

#[cfg(test)]
mod generation_bench {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    #[ignore]
    fn air_voxel_stage_costs() {
        let cfg = crate::test_world::World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 7).sample_config();
        let origin = IVec3::new(64, 32, 64);
        let mut voxels = Vec::new();
        for z in 0..34 {
            for x in 0..34 {
                let column = sample_column(&cfg, origin.x + x, origin.z + z);
                for y in 0..34 {
                    voxels.push((origin + IVec3::new(x, y, z), column));
                }
            }
        }
        let time = |name: &str, f: &dyn Fn(IVec3, ColumnSample) -> u32| {
            let mut acc = 0u32;
            for &(p, c) in &voxels {
                acc = acc.wrapping_add(f(p, c));
            }
            let start = std::time::Instant::now();
            for &(p, c) in &voxels {
                acc = acc.wrapping_add(f(p, c));
            }
            println!(
                "{name:>12}: {:7.2} ms ({acc})",
                start.elapsed().as_secs_f64() * 1e3
            );
        };
        time("column", &|p, _| {
            sample_column(&cfg, p.x, p.z).height as u32
        });
        time("undercrown", &|p, c| {
            undercrown::block(&cfg, p, c.height).is_some() as u32
        });
        time("sites", &|p, c| {
            sites::block(&cfg, p, c.height).is_some() as u32
        });
        time("cavern", &|p, c| {
            sites::cavern(&cfg, p, c.height).is_some() as u32
        });
        time("structure", &|p, c| {
            structure_block(&cfg, p, c, &CachedProfiles).as_u32()
        });
        time("river_top", &|p, _| {
            valley::river_top(&cfg, cfg.world_x(p.x), cfg.world_z(p.z)).is_some() as u32
        });
        time("full", &|p, c| sample_block_in_column(&cfg, p, c).as_u32());
    }

    /// Chunk generation reads profiles from dense per-chunk grids; it must
    /// produce exactly what single-voxel sampling through the caches does.
    #[test]
    fn region_profiles_match_single_voxel_sampling() {
        let never = AtomicBool::new(false);
        for (world_type, scale) in [
            (WorldType::Hills, VoxelScalePreset::OneMeter),
            (WorldType::Ridged, VoxelScalePreset::OneMeter),
            (WorldType::Hills, VoxelScalePreset::QuarterMeter),
        ] {
            let cfg = crate::test_world::World::with_seed(world_type, scale, 7).sample_config();
            let mut structures = 0;
            let crowns = (0..400)
                .filter_map(|i| tree_visual(&cfg, i % 20 - 10, i / 20 - 10))
                .take(3)
                .map(|t| {
                    ((t.base + Vec3::Y * t.height) / cfg.voxel_meters())
                        .floor()
                        .as_ivec3()
                        .div_euclid(IVec3::splat(CHUNK_SIZE))
                });
            let fixed = [
                IVec3::new(2, 1, -3),
                IVec3::new(-5, 2, 4),
                IVec3::new(0, 4, 1),
            ];
            for c in fixed.into_iter().chain(crowns) {
                let chunk = generate_padded(&cfg, c, &never).unwrap();
                let data = generate(&cfg, c);
                for (i, &b) in chunk.blocks.iter().enumerate() {
                    let i = i as i32;
                    let n = PADDED_CHUNK_SIZE;
                    let p = chunk.origin + IVec3::new(i % n, i / (n * n), (i / n) % n);
                    assert_eq!(
                        b,
                        sample_block(&cfg, p),
                        "{world_type:?} {scale:?} padded {p}"
                    );
                    structures += usize::from(matches!(
                        b,
                        BlockId::Leaves | BlockId::PineLeaves | BlockId::Log | BlockId::Bush
                    ));
                }
                for (i, &b) in data.blocks.iter().enumerate() {
                    let p = c * CHUNK_SIZE
                        + IVec3::new(
                            i as i32 % CHUNK_SIZE,
                            i as i32 / (CHUNK_SIZE * CHUNK_SIZE),
                            (i as i32 / CHUNK_SIZE) % CHUNK_SIZE,
                        );
                    assert_eq!(
                        b,
                        sample_block(&cfg, p),
                        "{world_type:?} {scale:?} chunk {p}"
                    );
                }
            }
            println!("{world_type:?} {scale:?}: {structures} structure voxels compared");
            assert!(structures > 200, "test chunks missed the tree crowns");
        }
    }

    /// `cargo test --release generation_bench -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn padded_chunk_generation_rate() {
        let cfg = crate::test_world::World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 7).sample_config();
        let never = AtomicBool::new(false);
        let coords: Vec<IVec3> = (-4..4)
            .flat_map(|x| (-4..4).flat_map(move |z| (1..3).map(move |y| IVec3::new(x, y, z))))
            .collect();
        let start = std::time::Instant::now();
        let mut solid = 0usize;
        for &c in &coords {
            let chunk = generate_padded(&cfg, c, &never).unwrap();
            solid += chunk.blocks.iter().filter(|b| !b.is_empty()).count();
        }
        let elapsed = start.elapsed();
        println!(
            "{} chunks in {:.1} ms ({:.2} ms/chunk, {solid} non-air)",
            coords.len(),
            elapsed.as_secs_f64() * 1e3,
            elapsed.as_secs_f64() * 1e3 / coords.len() as f64
        );
    }
}

#[cfg(test)]
mod tree_lod_accuracy {
    use super::*;
    use engine_core::VoxelScalePreset;

    /// `cargo test --release tree_lod_accuracy -- --ignored --nocapture`
    /// Voxel crown extents against the LOD proxy's `TreeVisual` for the same tree.
    #[test]
    #[ignore]
    fn voxel_trees_match_lod_proxy() {
        for scale in [VoxelScalePreset::OneMeter, VoxelScalePreset::QuarterMeter] {
            let cfg = crate::test_world::World::with_seed(WorldType::Hills, scale, 7).sample_config();
            let m = cfg.voxel_meters();
            let (mut n, mut top_err, mut rad_err, mut base_err, mut rad_bias) = (0, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
            let mut under = 0;
            let mut top_bias = 0.0f32;
            let mut worst_under = 0.0f32;
            let mut worst = (0.0f32, IVec3::ZERO);
            for cz in -30..30 {
                for cx in -30..30 {
                    let Some(v) = tree_visual(&cfg, cx, cz) else { continue };
                    let Some(profile) = tree_profile_for_cell(&cfg, cx, cz) else { continue };
                    let anchor = (v.base / m).round().as_ivec3();
                    let reach = (v.radius * 1.6 / m).ceil() as i32 + 2;
                    let top_guess = ((v.base.y + v.height + v.crown_height) / m).ceil() as i32 + 3;
                    let (mut top, mut max_r, mut low) = (i32::MIN, 0.0f32, i32::MAX);
                    let mut trunk_base = i32::MAX;
                    for y in anchor.y..=top_guess {
                        for z in -reach..=reach {
                            for x in -reach..=reach {
                                let p = anchor + IVec3::new(x, y - anchor.y, z);
                                // This tree alone: neighbours' crowns overlap at 12 m spacing.
                                let b = tree_block(&cfg, profile, p);
                                if matches!(b, BlockId::Leaves | BlockId::PineLeaves) {
                                    top = top.max(y);
                                    low = low.min(y);
                                    max_r = max_r.max(glam::Vec2::new(x as f32, z as f32).length());
                                } else if b == BlockId::Log {
                                    trunk_base = trunk_base.min(y);
                                }
                            }
                        }
                    }
                    if top == i32::MIN {
                        continue;
                    }
                    n += 1;
                    let true_top = (top + 1) as f32 * m;
                    let proxy_top = v.base.y + v.height + v.crown_height;
                    top_bias += proxy_top - true_top;
                    let terr = (proxy_top - true_top).abs();
                    top_err += terr;
                    if terr > worst.0 {
                        worst = (terr, anchor);
                    }
                    let true_r = max_r * m + m;
                    if true_r > v.radius + 0.01 {
                        under += 1;
                        worst_under = worst_under.max(true_r - v.radius);
                    }
                    rad_bias += true_r - v.radius;
                    rad_err += (max_r * m + m - v.radius).abs();
                    if trunk_base != i32::MAX {
                        base_err += (trunk_base as f32 * m - v.base.y).abs();
                    }
                }
            }
            println!("{scale:?}: under-covered {under}/{n}, worst short by {worst_under:.2} m, top bias {:+.2} m", top_bias / n as f32);
            println!(
                "{scale:?}: {n} trees, mean |top| err {:.2} m, |radius| err {:.2} m (bias {:+.2}), |trunk base| err {:.2} m, worst top {:.2} m at {:?}",
                top_err / n as f32, rad_err / n as f32, rad_bias / n as f32, base_err / n as f32, worst.0, worst.1
            );
        }
    }
}

#[cfg(test)]
mod v16_stage_split {
    use super::*;
    use engine_core::VoxelScalePreset;

    /// `cargo test --release v16_stage_split -- --ignored --nocapture`
    /// Where generator v16 spends its time, with the same per-chunk profile
    /// grids `generate_padded` uses, on the seed the cost test uses.
    #[test]
    #[ignore]
    fn v16_stage_split() {
        for version in [15u32, 16] {
            let cfg = WorldSampleConfig {
                planet: None,
                generator_version: version,
                seed: 424242,
                world_type: WorldType::Hills,
                voxel_scale: VoxelScalePreset::OneMeter,
            };
            let mut chunks = Vec::new();
            for cz in -2..2 {
                for cx in 6..10 {
                    for cy in -2..3 {
                        let origin = IVec3::new(cx * CHUNK_SIZE - 1, cy * CHUNK_SIZE - 1, cz * CHUNK_SIZE - 1);
                        let profiles = RegionProfiles::new(&cfg, origin.x, origin.z, 34);
                        let mut voxels = Vec::new();
                        for z in 0..34 {
                            for x in 0..34 {
                                let column = sample_column(&cfg, origin.x + x, origin.z + z);
                                for y in 0..34 {
                                    voxels.push((origin + IVec3::new(x, y, z), column));
                                }
                            }
                        }
                        chunks.push((profiles, voxels));
                    }
                }
            }
            let run = |f: &dyn Fn(IVec3, ColumnSample, &RegionProfiles) -> u32| {
                let mut acc = 0u32;
                for (profiles, voxels) in &chunks {
                    for &(p, c) in voxels {
                        acc = acc.wrapping_add(f(p, c, profiles));
                    }
                }
                acc
            };
            let time = |name: &str, f: &dyn Fn(IVec3, ColumnSample, &RegionProfiles) -> u32| {
                run(f);
                let t = std::time::Instant::now();
                let acc = run(f);
                println!("v{version} {name:>10}: {:8.1} ms ({acc})", t.elapsed().as_secs_f64() * 1e3);
            };
            time("column", &|p, _, _| sample_column(&cfg, p.x, p.z).height as u32);
            time("uncached", &|p, _, _| landscape::column(&cfg, p.x, p.z).height as u32);
            time("undercrown", &|p, c, _| undercrown::block(&cfg, p, c.height).is_some() as u32);
            time("sites", &|p, c, _| sites::block(&cfg, p, c.height).is_some() as u32);
            time("cavern", &|p, c, _| sites::cavern(&cfg, p, c.height).is_some() as u32);
            time("structure", &|p, c, pr| structure_block(&cfg, p, c, pr).as_u32());
            time("cave", &|p, c, _| cave_carves(&cfg, p, &c) as u32);
            time("dungeon", &|p, c, _| {
                let m = (p.as_vec3() + Vec3::splat(0.5)) * cfg.voxel_meters();
                (dungeon::block(&cfg, m, c.height).is_some() as u32) + (dungeon::reserved(&cfg, m) as u32)
            });
            time("full", &|p, c, pr| sample_block_with(&cfg, p, c, pr).as_u32());
        }
    }
}

