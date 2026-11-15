//! One spherical height source for voxel columns and orbital terrain.
use crate::{BlockId, ColumnSample, WorldSampleConfig};
use engine_core::cube_sphere::{Projection, atlas};
use engine_core::planet_frame::PlanetAxis;
use glam::DVec3;

/// Continuous 3D noise avoids independent terrain at face boundaries.
fn noise(p: DVec3, seed: u32) -> f64 {
    let cell = p.floor().as_ivec3();
    let f = p - p.floor();
    let f = f * f * (DVec3::splat(3.0) - 2.0 * f);
    let mut result = 0.0;
    for z in 0..2 {
        for y in 0..2 {
            for x in 0..2 {
                let mut h = seed ^ (cell.x.wrapping_add(x) as u32).wrapping_mul(0x9e3779b9)
                    ^ (cell.y.wrapping_add(y) as u32).wrapping_mul(0x85ebca6b)
                    ^ (cell.z.wrapping_add(z) as u32).wrapping_mul(0xc2b2ae35);
                h = (h ^ (h >> 16)).wrapping_mul(0x7feb352d);
                h = (h ^ (h >> 15)).wrapping_mul(0x846ca68b);
                let value = ((h ^ (h >> 16)) >> 8) as f64 / 16_777_215.0;
                let w = |i: i32, a: f64| if i == 0 { 1.0 - a } else { a };
                result += value * w(x, f.x) * w(y, f.y) * w(z, f.z);
            }
        }
    }
    result
}

pub(crate) fn noise_at(p: DVec3, seed: u32) -> f64 {
    noise(p, seed)
}

/// Climate, biome cover, caves and woodland. Planet saves below this keep the shell.
pub const BIOME_VERSION: u32 = 120;

pub fn height(seed: u32, dir: DVec3) -> f64 {
    let continent = noise(dir * 3.2, seed);
    let hills = noise(dir * 28.0, seed.wrapping_add(91));
    let detail = noise(dir * 220.0, seed.wrapping_add(173));
    ((continent - 0.46) * 850.0 + hills * hills * 110.0 + detail * 8.0)
        .clamp(-48.0, 440.0)
}

#[derive(Clone, Copy)]
struct Climate {
    temperature: f64,
    moisture: f64,
    /// Present from `planet_climate::VERSION`.
    axis: Option<crate::planet_climate::Climate>,
}

/// Fine relief, ridged ranges, strata and ore bodies.
pub const DETAIL_VERSION: u32 = 123;

/// Dungeons, caverns and dry caves with aquifer pockets. Dormant until the live version reaches it.
pub const UNDERGROUND_VERSION: u32 = 125;

/// Hills authored sites on the planet surface and below. Dormant until the live version reaches it.
// 127 ships tree detail only. No supported 127 saves existed before that
// release; keep sites dormant there and retain their behaviour in 128+ worlds.
pub const SITES_VERSION: u32 = crate::gates::SURVEYOR_SITES_GENERATOR;

/// Latitude climate and banded cover. Older planets keep the one-octave field.
pub const CLIMATE_VERSION: u32 = 121;

/// Direction on the sphere, so a biome continues across a face edge.
fn climate(cfg: &WorldSampleConfig, dir: DVec3) -> Climate {
    let seed = cfg.seed;
    if cfg.generator_version < CLIMATE_VERSION {
        return Climate {
            temperature: noise(dir * 6.0, seed.wrapping_add(11)),
            moisture: noise(dir * 7.0, seed.wrapping_add(29)),
            axis: None,
        };
    }
    if cfg.generator_version >= crate::planet_climate::VERSION {
        let s = cfg.planet_settings();
        let scale = s.map_or(1.0, |s| 2.0_f64.powf((0.5 - s.unit(1)) * 2.0));
        let sea_line = s.map_or(0.535, |s| 0.535 + (s.unit(0) - 0.5) * 0.5);
        let land = (noise(dir * (3.2 * scale), seed) - sea_line) * 4.0;
        let mut c = crate::planet_climate::at(seed, &PlanetAxis::EARTHLIKE, dir, land);
        if let Some(s) = s {
            let bias = (s.unit(4) - 0.5) * 40.0;
            let dials = crate::planet_profile::profile(seed);
            let swing = c.winter_c - c.annual_c;
            c.annual_c += bias - f64::from(dials.temperature - 0.5) * 10.0;
            c.winter_c = c.annual_c + swing * (s.unit(6) * 47.0).to_radians().sin() / 0.3978;
            c.wet = ((c.wet - f64::from(dials.rainfall - 0.5) * 0.3) * s.unit(5) * 2.0).clamp(0.0, 1.0);
        }
        return Climate { temperature: c.heat01(), moisture: c.wet, axis: Some(c) };
    }
    // Blended value noise piles up near 0.5; stretching restores the tails so
    // every band in `biome_cover` is reachable.
    let stretch = |n: f64, k: f64| ((n - 0.5) * k + 0.5).clamp(0.0, 1.0);
    let octaves = |f: f64, salt: u32| {
        noise(dir * f, seed.wrapping_add(salt)) * 0.65 + noise(dir * f * 3.1, seed.wrapping_add(salt + 7)) * 0.35
    };
    let latitude = 1.0 - dir.y.abs().powf(2.2);
    Climate {
        temperature: (latitude * 0.72 + stretch(octaves(5.0, 11), 2.2) * 0.28).clamp(0.0, 1.0),
        moisture: stretch(octaves(6.0, 29), 2.6),
        axis: None,
    }
}

fn biome_height(cfg: &WorldSampleConfig, dir: DVec3, climate: Climate) -> f64 {
    let seed = cfg.seed;
    // From the climate gate, a higher sea line leaves most of the sphere ocean.
    let sea_line = if cfg.generator_version >= CLIMATE_VERSION { 0.535 } else { 0.46 };
    let s = cfg.planet_settings();
    let scale = s.map_or(1.0, |s| 2.0_f64.powf((0.5 - s.unit(1)) * 2.0));
    let sea_line = sea_line + s.map_or(0.0, |s| (s.unit(0) - 0.5) * 0.5);
    let continent = noise(dir * (3.2 * scale), seed);
    let hills = noise(dir * 12.0, seed.wrapping_add(91));
    let detail = noise(dir * 48.0, seed.wrapping_add(173));
    let alpine = ((0.40 - climate.temperature) / 0.40).clamp(0.0, 1.0);
    let arid = ((0.42 - climate.moisture) / 0.42).clamp(0.0, 1.0);
    let relief = 0.28 + (1.0 - arid) * 0.22 + alpine * 0.85;
    let base = (continent - sea_line) * 620.0 + hills * hills * 180.0 * relief + detail * (3.0 + alpine * 8.0);
    let sea = f64::from(cfg.world_y(cfg.water_level()));
    let base = s.map_or(base, |s| sea + (base - sea) * (80.0 + s.unit(2) * 400.0) / 520.0);
    if cfg.generator_version < DETAIL_VERSION {
        return base.clamp(-48.0, 440.0);
    }
    let sea = f64::from(cfg.world_y(cfg.water_level()));
    let relief = fine_relief(seed, dir, base, alpine) * s.map_or(1.0, |s| 1.5 - s.unit(3));
    if cfg.generator_version >= crate::planet_ocean::VERSION && base + relief < sea {
        // Coast stays where it was; the raised shelf must not grow islets out of land relief.
        let floor = sea - crate::planet_ocean::depth(seed, dir, base, sea, alpine);
        return (floor + relief).min(sea - 0.5).clamp(-60.0, 560.0);
    }
    (base + relief).clamp(-60.0, 560.0)
}

/// Ridged ranges and hills below the 4 km continental octaves.
///
/// Sampled in body metres, so relief is continuous across face edges.
fn fine_relief(seed: u32, dir: DVec3, base: f64, alpine: f64) -> f64 {
    let p = dir * atlas::RADIUS_M;
    let n = |wavelength: f64, salt: u32| noise(p / wavelength, seed.wrapping_add(salt));
    let signed = |wavelength: f64, salt: u32| n(wavelength, salt) * 2.0 - 1.0;
    let ridge = |wavelength: f64, salt: u32| (1.0 - signed(wavelength, salt).abs()).powi(2);
    // A slow belt mask turns ridges into chains instead of uniform speckle.
    let belt = ((n(5200.0, 301) - 0.47) * 3.5 + alpine * 0.6).clamp(0.0, 1.0);
    // Sea floors stay calm; relief ramps in over the first 80 m of land.
    let land = ((base + 20.0) / 80.0).clamp(0.0, 1.0);
    let ridged = ridge(1500.0, 307) * 0.65 + ridge(620.0, 311) * 0.35;
    belt * ridged * 280.0 * land
        + signed(380.0, 313) * 24.0 * (0.3 + 0.7 * land)
        + signed(110.0, 317) * 7.0 * land
        + signed(28.0, 331) * 1.6
}

fn biome_cover(cfg: &WorldSampleConfig, climate: Climate, metres: f64) -> (BlockId, BlockId, f32) {
    let sea = f64::from(cfg.world_y(cfg.water_level()));
    if cfg.generator_version >= crate::planet_ocean::VERSION
        && let Some(cover) = crate::planet_ocean::floor_cover(sea - metres)
    {
        return cover;
    }
    if let Some(axis) = climate.axis {
        return crate::planet_climate::cover(axis, metres, f64::from(cfg.world_y(cfg.water_level())) + 2.0);
    }
    if cfg.generator_version >= CLIMATE_VERSION {
        return banded_cover(cfg, climate, metres);
    }
    let shore = f64::from(cfg.world_y(cfg.water_level())) + 2.0;
    if metres <= shore {
        return if climate.temperature < 0.34 {
            (BlockId::SnowGrass, BlockId::Dirt, 0.04)
        } else {
            (BlockId::Sand, BlockId::Sandstone, 0.0)
        };
    }
    if metres > 280.0 || climate.temperature < 0.28 {
        return (BlockId::SnowGrass, BlockId::Dirt, 0.12);
    }
    if climate.temperature > 0.62 && climate.moisture < 0.38 {
        return (BlockId::Sand, BlockId::Sandstone, 0.02);
    }
    if climate.moisture > 0.66 && metres < 48.0 {
        return (BlockId::Mud, BlockId::Clay, 0.40);
    }
    if climate.temperature > 0.55 && climate.moisture > 0.62 {
        return (BlockId::Grass, BlockId::Dirt, 0.82);
    }
    if climate.temperature < 0.42 && climate.moisture > 0.40 {
        return (BlockId::SnowGrass, BlockId::Dirt, 0.58);
    }
    if climate.moisture < 0.32 {
        return (BlockId::Gravel, BlockId::Stone, 0.06);
    }
    let woods = if climate.moisture > 0.48 { 0.62 } else { 0.28 };
    (BlockId::Grass, BlockId::Dirt, woods)
}

/// Surface, filler and woodland density for one climate at one height.
fn banded_cover(cfg: &WorldSampleConfig, c: Climate, metres: f64) -> (BlockId, BlockId, f32) {
    use BlockId::*;
    let shore = f64::from(cfg.world_y(cfg.water_level())) + 2.0;
    // Altitude cools: about one band per 200 m of climb.
    let t = c.temperature - (metres - shore).max(0.0) / 900.0;
    let m = c.moisture;
    if t < 0.08 {
        return (Ice, Stone, 0.0);
    }
    if metres <= shore {
        return if t < 0.30 { (Gravel, Stone, 0.0) } else if m > 0.70 { (Mud, Clay, 0.10) } else { (Sand, Sandstone, 0.0) };
    }
    if metres > 320.0 || t < 0.18 {
        return (SnowGrass, Stone, 0.02);
    }
    if metres > 220.0 {
        return if t > 0.62 && m < 0.40 { (Basalt, Basalt, 0.0) } else { (Stone, Gravel, 0.04) };
    }
    if t < 0.34 {
        return if m > 0.45 { (SnowGrass, Dirt, 0.60) } else { (SnowGrass, Dirt, 0.08) };
    }
    if t > 0.66 {
        return match m {
            m if m < 0.22 => (Sand, Sandstone, 0.0),
            m if m < 0.36 => (Sandstone, Clay, 0.02),
            m if m < 0.55 => (Grass, Clay, 0.14),
            _ => (Grass, Dirt, 0.92),
        };
    }
    match m {
        m if m < 0.24 => (Gravel, Stone, 0.05),
        m if m > 0.74 && metres < shore + 40.0 => (Mud, Clay, 0.45),
        m if m > 0.50 => (Grass, Dirt, 0.70),
        _ => (Grass, Dirt, 0.24),
    }
}

/// Warm wet woodland patches turn to moss-floored old growth.
fn enchanted(cfg: &WorldSampleConfig, c: Climate, dir: DVec3, cover: (BlockId, BlockId, f32)) -> (BlockId, BlockId, f32) {
    let woods = cfg.generator_version >= crate::volcano::VERSION && cover.0 == BlockId::Grass && cover.2 > 0.5;
    if woods && (0.40..0.78).contains(&c.temperature) && noise(dir * 40.0, cfg.seed.wrapping_add(631)) > 0.62 {
        return (BlockId::GroveMoss, BlockId::Dirt, 0.96);
    }
    cover
}

fn direction(cfg: &WorldSampleConfig, x: i32, z: i32) -> DVec3 {
    let p = DVec3::new(cfg.world_x(x) as f64, 0.0, cfg.world_z(z) as f64);
    let (face, local) = atlas::split(p);
    Projection::Tangent.to_sphere(face, local.x / atlas::HALF_M, local.z / atlas::HALF_M)
}

/// Whether the sea surface over a shell column is frozen. False before the axis climate gate.
pub fn sea_ice_at(cfg: &WorldSampleConfig, x: i32, z: i32) -> bool {
    if crate::planetary::active(cfg) {
        return crate::planet_climate::sea_ice(crate::planetary::climate_at(cfg, direction(cfg, x, z)));
    }
    if cfg.generator_version < crate::planet_climate::VERSION {
        return false;
    }
    climate(cfg, direction(cfg, x, z)).axis.is_some_and(crate::planet_climate::sea_ice)
}

pub fn column(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    if crate::planetary::active(cfg) { return crate::planetary::column(cfg, x, z, cfg.voxel_meters()); }
    match crate::basin::column(cfg, x, z, || shell_column(cfg, x, z)) {
        Some(solved) if cfg.generator_version >= crate::volcano::VERSION => solved_cones(cfg, x, z, solved),
        Some(solved) => solved,
        None => shell_column(cfg, x, z),
    }
}

/// Cones raised on the solved face. Its drainage solve already owns the plates.
fn solved_cones(cfg: &WorldSampleConfig, x: i32, z: i32, solved: ColumnSample) -> ColumnSample {
    let vm = f64::from(cfg.voxel_meters());
    let metres = f64::from(cfg.world_y(solved.height));
    let base_at = |d: DVec3| {
        let p = atlas::from_body(d * atlas::RADIUS_M);
        f64::from(cfg.world_y(crate::landscape::column(cfg, (p.x / vm).floor() as i32, (p.z / vm).floor() as i32).height))
    };
    let shaped = crate::volcano::shape(cfg.seed, direction(cfg, x, z), metres, false, base_at);
    if shaped.lake.is_none() && (shaped.metres - metres).abs() < vm * 0.5 {
        return solved;
    }
    let metres = shaped.lake.map_or(shaped.metres, |(level, _)| level);
    let height = cfg.clamp_height((metres / vm).floor() as i32);
    match crate::volcano::cover(&shaped, noise(direction(cfg, x, z) * 900.0, cfg.seed.wrapping_add(619))) {
        Some((surface, filler, vegetation_density)) => ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone, height, surface, filler, vegetation_density, cave: crate::cave_column(cfg, x, z, height) },
        None => ColumnSample { height, cave: crate::cave_column(cfg, x, z, height), ..solved },
    }
}

/// A crater lake on the solved face: its fluid layer comes from the planet block path.
pub fn is_crater_lake(cfg: &WorldSampleConfig, column: &ColumnSample) -> bool {
    cfg.generator_version >= crate::volcano::VERSION
        && column.surface == column.filler
        && matches!(column.surface, BlockId::Lava | BlockId::Water)
}

pub(crate) fn shell_column(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    let dir = direction(cfg, x, z);
    if cfg.generator_version < BIOME_VERSION {
        let metres = height(cfg.seed, dir);
        let height = cfg.clamp_height((metres / cfg.voxel_meters() as f64).floor() as i32);
        let surface = if height <= cfg.water_level() + cfg.blocks_for_span(2.0) { BlockId::Sand }
            else if dir.y.abs() > 0.86 || cfg.world_y(height) > 330.0 { BlockId::SnowGrass }
            else { BlockId::Grass };
        // The shell mesh samples every column. This field is not carved.
        return ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone, height, surface, filler: BlockId::Dirt, vegetation_density: 0.22,
            cave: Default::default() };
    }
    let climate = climate(cfg, dir);
    let mut metres = biome_height(cfg, dir, climate);
    let mut cover = None;
    if cfg.generator_version >= crate::volcano::VERSION {
        let shaped = crate::volcano::shape(cfg.seed, dir, metres, true, |d| biome_height(cfg, d, self::climate(cfg, d)));
        metres = shaped.lake.map_or(shaped.metres, |(level, _)| level);
        let shore = f64::from(cfg.world_y(cfg.water_level())) + 2.0;
        if shaped.lake.is_some() || metres > shore {
            cover = crate::volcano::cover(&shaped, noise(dir * 900.0, cfg.seed.wrapping_add(619)));
        }
    }
    let height = cfg.clamp_height((metres / cfg.voxel_meters() as f64).floor() as i32);
    let (surface, filler, vegetation_density) = cover.unwrap_or_else(|| enchanted(cfg, climate, dir, biome_cover(cfg, climate, metres)));
    let vegetation_density = (vegetation_density * cfg.planet_settings().map_or(1.0, |s| (s.unit(7) * 2.0) as f32)).min(1.0);
    ColumnSample { deposit_m: 0.0, deposit_bed_surface: crate::BlockId::Stone, height, surface, filler, vegetation_density, cave: crate::cave_column(cfg, x, z, height) }
}

pub fn block(cfg: &WorldSampleConfig, p: glam::IVec3, column: ColumnSample) -> BlockId {
    block_with(cfg, p, column, &crate::CachedProfiles)
}

pub(crate) fn block_with(
    cfg: &WorldSampleConfig,
    p: glam::IVec3,
    column: ColumnSample,
    profiles: &impl crate::Profiles,
) -> BlockId {
    if p.y <= cfg.min_world_y() + cfg.blocks_for_span(2.0) { return BlockId::Bedrock; }
    // A crater's molten layer owns its cells ahead of surface trails and turf.
    if cfg.generator_version >= engine_voxel::VOLCANIC_LANDFORM_PLANET_GENERATOR
        && column.surface == BlockId::Lava && column.filler == BlockId::Lava
        && (column.height - cfg.blocks_for_span(3.0)..=column.height).contains(&p.y)
    {
        return BlockId::Lava;
    }
    // Gate 125 already ran the cavern pass, so the site chain skips it.
    if cfg.generator_version >= SITES_VERSION && let Some(block) = crate::authored_block(cfg, p, column, false) {
        return block;
    }
    let underground = cfg.generator_version >= UNDERGROUND_VERSION;
    if underground && let Some(block) = crate::sites::cavern(cfg, p, column.height) {
        return block;
    }
    // Authored sites remain authoritative. Natural bedrock caves and dikes
    // cannot replace the younger surface deposit carried by this same column.
    if column.deposit_m > 0.0 && p.y <= column.height
        && (column.height - p.y) as f32 * cfg.voxel_meters() < column.deposit_m
    {
        return column.surface;
    }
    // Reuse the full existing substrate policy at the original voxel bed.
    let column = if column.deposit_m > 0.0 {
        ColumnSample { height: column.height - cfg.blocks_for_span(column.deposit_m),
            surface: column.deposit_bed_surface, deposit_m: 0.0, ..column }
    } else { column };
    if cfg.generator_version >= BIOME_VERSION && crate::cave_carves(cfg, p, &column) {
        if underground {
            let pocket = crate::aquifer_pocket(cfg, p, &column);
            if pocket == BlockId::Air && cfg.generator_version >= crate::deep::VERSION {
                let block = crate::deep::cave_air(cfg, p, &column);
                // Historical worlds keep the complete deep-cave result. On
                // the shared line, flora may fill only its remaining dry air.
                if block != BlockId::Air || !crate::features::shared_line(cfg) {
                    return block;
                }
            }
            if pocket == BlockId::Air && crate::features::enabled(cfg, crate::features::Feature::CaveFlora) && crate::flora::active(cfg) {
                let plant = crate::flora::cave(cfg, p, |q| crate::flora::rock_at(cfg, q));
                if plant != BlockId::Air {
                    return plant;
                }
            }
            return pocket;
        }
        return if p.y <= cfg.water_level() { BlockId::Water } else { BlockId::Air };
    }
    if p.y > column.height {
        if crate::planetary::active(cfg)
            && profiles.river_top(cfg, p.x, p.z).is_some_and(|head| cfg.world_y(p.y) < head)
        {
            return BlockId::Water;
        }
        if p.y == cfg.water_level() && sea_ice_at(cfg, p.x, p.z) {
            return BlockId::Ice;
        }
        return if p.y <= cfg.water_level() { BlockId::Water } else { BlockId::Air };
    }
    // A dike is younger than the rock and soil it cuts, so it is read first.
    if crate::planetary::dike_at(cfg, p, &column) { return BlockId::Basalt; }
    if p.y == column.height {
        if cfg.generator_version >= crate::deep::VERSION {
            return crate::deep::surface_tell(cfg, p.x, p.z, &column).unwrap_or(column.surface);
        }
        column.surface
    }
    else if p.y >= column.height - cfg.blocks_for_span(3.0) {
        if cfg.generator_version >= engine_voxel::BEDROCK_RIVER_GENERATOR
            && (column.surface == BlockId::CoalOre || column.filler == BlockId::CoalOre)
        {
            return crate::planetary::rock_at(cfg, p);
        }
        if crate::planetary::ground_ice(cfg, p, &column) {
            BlockId::Ice
        } else if column.filler == BlockId::Paydirt && p.y > column.height - cfg.blocks_for_span(2.0) {
            BlockId::Gravel
        } else {
            column.filler
        }
    }
    else if cfg.generator_version >= DETAIL_VERSION {
        let host = if crate::planetary::active(cfg) { crate::planetary::rock_at(cfg, p) }
            else { crate::landscape::geological_stratum(cfg, p, column.height, column.height - p.y) };
        if cfg.generator_version >= crate::deep::VERSION { crate::deep::rock(cfg, p, &column, host) } else { host }
    }
    else { BlockId::Stone }
}

#[cfg(test)]
mod tests {
    use engine_core::VoxelScalePreset;

    #[test]
    fn dimension_previews_match_the_played_column() {
        for kind in [crate::WorldType::Planet, crate::WorldType::Nether, crate::WorldType::Manyfold, crate::WorldType::Hall] {
            let cfg = crate::WorldSampleConfig {
                planet: None,
                generator_version: crate::CURRENT_GENERATOR_VERSION,
                seed: 7,
                world_type: kind,
                voxel_scale: VoxelScalePreset::OneMeter,
            };
            for (x, z) in [(0, 0), (128, -64), (-40, 96)] {
                let exact = crate::distant_surface(&cfg, x, z);
                assert_eq!(crate::landscape::preview_surface(&cfg, x, z), exact, "{kind:?} preview {x},{z}");
                assert_eq!(crate::landscape::preview_surface_canonical(&cfg, x, z), exact, "{kind:?} horizon {x},{z}");
                let metres = crate::landscape::preview_height_lod(&cfg, x, z, 32.0);
                assert!((metres - exact.0 * cfg.voxel_meters()).abs() < 1e-4, "{kind:?} height {x},{z}");
                assert!(crate::tree_profile_uncached(&cfg, x, z).is_none(), "{kind:?} grew a phantom tree");
            }
        }
    }

    #[test]
    fn biome_planets_vary_cover_and_can_grow_woodland() {
        let cfg = crate::WorldSampleConfig {
            planet: None,
            generator_version: super::BIOME_VERSION,
            seed: 7,
            world_type: crate::WorldType::Planet,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        let mut surfaces = Vec::new();
        let mut woods = false;
        for z in -4..=4 {
            for x in -4..=4 {
                let (wx, wz) = (x * 2_000, z * 2_000);
                let column = crate::sample_column(&cfg, wx, wz);
                if !surfaces.contains(&column.surface) {
                    surfaces.push(column.surface);
                }
                if !woods && crate::tree_profile_uncached(&cfg, wx.div_euclid(12), wz.div_euclid(12)).is_some() {
                    woods = true;
                }
            }
        }
        assert!(surfaces.len() >= 3, "cover {surfaces:?}");
        assert!(woods, "no woodland on a biome planet");
        let shell = crate::WorldSampleConfig { generator_version: 74, ..cfg };
        let legacy = crate::sample_column(&shell, 0, 0);
        assert!(matches!(legacy.surface, crate::BlockId::Grass | crate::BlockId::Sand | crate::BlockId::SnowGrass));
    }

    #[test]
    fn latitude_climate_spreads_cover_across_the_planet() {
        use engine_core::cube_sphere::atlas;
        let survey = |generator_version| {
            let cfg = crate::WorldSampleConfig {
                planet: None,
                generator_version,
                seed: 7,
                world_type: crate::WorldType::Planet,
                voxel_scale: VoxelScalePreset::OneMeter,
            };
            let step = atlas::HALF_M / 6.0;
            // Woodland density separates forest, meadow and rainforest on the same blocks.
            let mut counts: Vec<((crate::BlockId, crate::BlockId, u8), u32)> = Vec::new();
            for gz in -18..18 {
                for gx in -18..18 {
                    let (x, z) = ((f64::from(gx) * step) as i32, (f64::from(gz) * step) as i32);
                    let c = crate::sample_column(&cfg, x, z);
                    if c.height <= cfg.water_level() {
                        continue;
                    }
                    let key = (c.surface, c.filler, (c.vegetation_density * 10.0) as u8);
                    match counts.iter_mut().find(|e| e.0 == key) {
                        Some(e) => e.1 += 1,
                        None => counts.push((key, 1)),
                    }
                }
            }
            counts
        };
        let (old, new) = (survey(120), survey(super::CLIMATE_VERSION));
        eprintln!("v120 {old:?}\nv121 {new:?}");
        let share = |c: &[((crate::BlockId, crate::BlockId, u8), u32)]| {
            let total: u32 = c.iter().map(|e| e.1).sum();
            c.iter().map(|e| e.1).max().unwrap_or(0) as f64 / f64::from(total.max(1))
        };
        assert!(new.len() >= 8, "only {} cover bands: {new:?}", new.len());
        assert!(share(&new) < 0.35, "one band covers {:.0}% of land", share(&new) * 100.0);
        assert!(share(&new) < share(&old), "v120 {:.2} v121 {:.2}", share(&old), share(&new));
    }
    #[test]
    fn detail_planets_gain_short_relief_and_ore_bodies() {
        let cfg = |generator_version| crate::WorldSampleConfig {
            planet: None,
            generator_version,
            seed: 7,
            world_type: crate::WorldType::Planet,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        let (old, new) = (cfg(super::CLIMATE_VERSION), cfg(super::DETAIL_VERSION));
        // Neighbouring atlas face: face 0 is the solved continent from gate 122.
        const FACE: i32 = engine_core::cube_sphere::atlas::PITCH_M as i32;
        // Mean height step over 48 m on land outside the solved spawn face.
        let roughness = |c: &crate::WorldSampleConfig| {
            let (mut sum, mut n) = (0i64, 0i64);
            for gz in -30..30 {
                for gx in -30..30 {
                    let (x, z) = (FACE + gx * 700 + 13, gz * 700 - 29);
                    if crate::basin::covers(c, x, z) {
                        continue;
                    }
                    let a = crate::sample_column(c, x, z).height;
                    if a <= c.water_level() {
                        continue;
                    }
                    sum += i64::from((crate::sample_column(c, x + 48, z).height - a).abs());
                    n += 1;
                }
            }
            assert!(n > 100, "only {n} land samples");
            sum as f64 / n as f64
        };
        let (rough_old, rough_new) = (roughness(&old), roughness(&new));
        assert!(rough_new > rough_old * 1.5, "48 m relief {rough_old:.2} -> {rough_new:.2}");

        let mut rock = Vec::new();
        for gz in -6..6 {
            for gx in -6..6 {
                let (x, z) = (FACE + gx * 1_900 + 7, gz * 1_900 + 3);
                let column = crate::sample_column(&new, x, z);
                for depth in 6..90 {
                    let block = super::block(&new, glam::IVec3::new(x, column.height - depth, z), column);
                    if !rock.contains(&block) {
                        rock.push(block);
                    }
                }
            }
        }
        assert!(rock.len() >= 5, "underground holds only {rock:?}");
        assert!(rock.iter().any(|b| crate::ore_geology::is_scattered_ore(*b) || format!("{b:?}").contains("Ore")),
            "no ore in {rock:?}");
    }

    #[test]
    fn underground_planets_hold_dungeons_and_dry_caves() {
        let at = |v| crate::WorldSampleConfig {
            planet: None,
            generator_version: v,
            seed: 7,
            world_type: crate::WorldType::Planet,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        let (old, new) = (at(super::UNDERGROUND_VERSION - 1), at(super::UNDERGROUND_VERSION));
        let face = engine_core::cube_sphere::atlas::PITCH_M as f32;
        let anchors = crate::dungeon::anchors(&new, glam::Vec3::new(face + 400.0, 0.0, 300.0), 3_000.0);
        assert!(!anchors.is_empty(), "no dungeon complex near the probe");
        let mut changed = 0;
        for (_, point) in anchors.iter().take(4) {
            let (x, z) = (point.x.floor() as i32, point.z.floor() as i32);
            let column = crate::sample_column(&new, x, z);
            for y in (point.y.floor() as i32 - 3)..(point.y.floor() as i32 + 3) {
                let p = glam::IVec3::new(x, y, z);
                if super::block(&new, p, column) != super::block(&old, p, column) {
                    changed += 1;
                }
            }
        }
        assert!(changed > 0, "dungeon rooms left the planet rock unchanged");

        let (mut carved, mut wet) = (0, 0);
        for gz in -8..8 {
            for gx in -8..8 {
                let (x, z) = (face as i32 + gx * 211 + 5, gz * 211 + 9);
                let column = crate::sample_column(&new, x, z);
                for y in (column.height - 60).max(new.min_world_y() + 4)..column.height {
                    let p = glam::IVec3::new(x, y, z);
                    if crate::cave_carves(&new, p, &column) && crate::sites::cavern(&new, p, column.height).is_none() {
                        carved += 1;
                        wet += (super::block(&new, p, column) == crate::BlockId::Water) as u32;
                    }
                }
            }
        }
        assert!(carved > 50, "only {carved} carved cells");
        assert!(wet * 10 < carved, "{wet} of {carved} cave cells are water");
    }

    #[test]
    fn site_planets_build_hills_sites_on_their_own_ground() {
        let at = |v| crate::WorldSampleConfig {
            planet: None,
            generator_version: v,
            seed: 7,
            world_type: crate::WorldType::Planet,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        let (old, new) = (at(super::SITES_VERSION - 1), at(super::SITES_VERSION));
        let face = engine_core::cube_sphere::atlas::PITCH_M as f32;
        let sites = crate::sites::encounters(&new, glam::Vec3::new(face + 400.0, 0.0, 300.0));
        assert!(!sites.is_empty(), "no site near the probe");
        let mut changed = 0;
        for (_, point, _) in sites.iter().take(4) {
            for dz in (-12..=12).step_by(3) {
                for dx in (-12..=12).step_by(3) {
                    let (x, z) = (point.x.floor() as i32 + dx, point.z.floor() as i32 + dz);
                    let column = crate::sample_column(&new, x, z);
                    for y in column.height - 4..column.height + 10 {
                        let p = glam::IVec3::new(x, y, z);
                        changed += (super::block(&new, p, column) != super::block(&old, p, column)) as u32;
                    }
                }
            }
        }
        assert!(changed > 20, "sites changed only {changed} cells");
    }

    #[test]
    fn climate_planets_are_mostly_ocean() {
        use engine_core::cube_sphere::atlas;
        use glam::DVec3;
        let cfg = crate::WorldSampleConfig {
            planet: None,
            generator_version: super::CLIMATE_VERSION,
            seed: 7,
            world_type: crate::WorldType::Planet,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        let (mut wet, mut all) = (0, 0);
        // Uniform latitude sine and longitude sample the whole sphere equally.
        for latitude in 0..36 {
            let y = 2.0 * (f64::from(latitude) + 0.5) / 36.0 - 1.0;
            let radius = (1.0 - y * y).sqrt();
            for longitude in 0..72 {
                let angle = std::f64::consts::TAU * (f64::from(longitude) + 0.5) / 72.0;
                let direction = DVec3::new(radius * angle.cos(), y, radius * angle.sin());
                let p = atlas::from_body(direction * atlas::RADIUS_M);
                let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
                all += 1;
                wet += (crate::sample_column(&cfg, x, z).height <= cfg.water_level()) as u32;
            }
        }
        let ocean = f64::from(wet) / f64::from(all);
        eprintln!("ocean share {ocean:.2}");
        assert!((0.55..0.80).contains(&ocean), "ocean share {ocean:.2}");
    }
}
