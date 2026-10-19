use glam::{IVec3, Vec3};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::fluid::VoxelScalePreset;
pub use crate::material::BlockId;

pub mod activity;
mod water;
mod collision;
mod generation_cache;
mod vegetation;
mod trees;
pub mod landscape;
pub mod sites;
pub mod expedition;
pub mod factory;
pub mod adventure;
pub mod inventory;
pub mod regions;
mod sample_cache;
pub mod storage;
mod weather;
pub use collision::{raycast_block_precise, collides_world_local, try_move_local, try_step_up_local, collides_world, player_aabb, raycast_block, try_move, try_step_up};

#[allow(unused_imports)]
pub use weather::{
    CloudWeatherField, CloudWeatherUploadManifest, WEATHER_CLIPMAP_LEVEL_COUNT,
    WEATHER_TEXTURE_RESOLUTION, WeatherClipmapUploadBatch, WeatherRuntimeState, WeatherUploadRect,
};

pub const CHUNK_SIZE: i32 = 32;
/// New-world generator and maximum readable version. Keep historical version
/// gates literal: bumping this must never change how an older save generates.
pub const CURRENT_GENERATOR_VERSION: u32 = 13;
pub const PLAYER_WIDTH: f32 = 0.6;
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const EYE_HEIGHT: f32 = 1.62;
pub const STEP_HEIGHT: f32 = 0.6;
pub const REACH_DISTANCE: f32 = 5.0;
pub const WALK_SPEED: f32 = 5.5;
pub const SPRINT_SPEED: f32 = 8.5;
pub const SNEAK_SPEED: f32 = 2.2;
pub const GRAVITY: f32 = 28.0;
pub const JUMP_VELOCITY: f32 = 8.4;
pub const GROUND_ACCEL: f32 = 45.0;
pub const AIR_ACCEL: f32 = 12.0;
pub const GROUND_FRICTION: f32 = 14.0;
pub const CREATIVE_FLY_SPEED: f32 = 12.0;
pub const CREATIVE_FLY_SPRINT_SPEED: f32 = 24.0;
pub const NOCLIP_FLY_SPEED: f32 = 18.0;
pub const NOCLIP_FLY_FAST_SPEED: f32 = 30.0;
pub const FLY_ACCEL: f32 = 32.0;
pub const WORLD_MIN_Y: i32 = -64;
pub const WORLD_MAX_Y: i32 = 2048;
pub const WORLD_MIN_Y_METERS: f32 = -16.0;
pub const WORLD_MAX_Y_METERS: f32 = 512.0;
pub const WATER_LEVEL_METERS: f32 = 12.0;
pub const DEFAULT_DAY_LENGTH_SECONDS: f32 = 480.0;
pub const DEFAULT_START_TIME_OF_DAY: f32 = 0.23;

fn blocks_from_meters(voxel_scale: VoxelScalePreset, meters: f32) -> f32 {
    meters / voxel_scale.meters().max(1e-6)
}

pub fn player_width_blocks(voxel_scale: VoxelScalePreset, player_scale: f32) -> f32 {
    blocks_from_meters(voxel_scale, PLAYER_WIDTH * player_scale.max(0.1))
}

pub fn player_height_blocks(voxel_scale: VoxelScalePreset, player_scale: f32) -> f32 {
    blocks_from_meters(voxel_scale, PLAYER_HEIGHT * player_scale.max(0.1))
}

pub fn player_eye_height_blocks(voxel_scale: VoxelScalePreset, player_scale: f32) -> f32 {
    blocks_from_meters(voxel_scale, EYE_HEIGHT * player_scale.max(0.1))
}

pub fn player_step_height_blocks(voxel_scale: VoxelScalePreset, player_scale: f32) -> f32 {
    blocks_from_meters(voxel_scale, STEP_HEIGHT * player_scale.max(0.1))
}

pub fn reach_distance_blocks(voxel_scale: VoxelScalePreset) -> f32 {
    blocks_from_meters(voxel_scale, REACH_DISTANCE)
}

pub fn speed_blocks(voxel_scale: VoxelScalePreset, meters_per_second: f32) -> f32 {
    blocks_from_meters(voxel_scale, meters_per_second)
}

pub fn acceleration_blocks(voxel_scale: VoxelScalePreset, meters_per_second_sq: f32) -> f32 {
    blocks_from_meters(voxel_scale, meters_per_second_sq)
}

#[derive(Clone)]
pub struct ChunkData {
    pub blocks: Box<[BlockId; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize]>,
    pub light: Box<[u8; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize]>,
    pub torches: HashSet<IVec3>,
}

impl ChunkData {
    pub fn local_index(x: i32, y: i32, z: i32) -> usize {
        let lx = x.rem_euclid(CHUNK_SIZE) as usize;
        let ly = y.rem_euclid(CHUNK_SIZE) as usize;
        let lz = z.rem_euclid(CHUNK_SIZE) as usize;
        (ly * CHUNK_SIZE as usize * CHUNK_SIZE as usize) + (lz * CHUNK_SIZE as usize) + lx
    }

    pub fn generate(cfg: &WorldSampleConfig, chunk_coord: IVec3) -> Self {
        let mut blocks = Box::new([BlockId::Air; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize]);
        let chunk_origin = chunk_coord * CHUNK_SIZE;

        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let column = sample_column(cfg, chunk_origin.x + x, chunk_origin.z + z);
                for y in 0..CHUNK_SIZE {
                    let world_pos = chunk_origin + IVec3::new(x, y, z);
                    let index = Self::local_index(x, y, z);
                    blocks[index] = sample_block_in_column(cfg, world_pos, column);
                }
            }
        }

        Self {
            blocks,
            light: Box::new([0; (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize]),
            torches: HashSet::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorldType {
    Superflat,
    Hills,
    Ridged,
}

pub fn hotbar_block(slot: usize) -> Option<BlockId> {
    crate::material::hotbar_block(slot)
}

pub fn cycle_hotbar_block(current: BlockId, dir: i32) -> BlockId {
    crate::material::cycle_hotbar_block(current, dir)
}

#[derive(Clone, Copy, Debug)]
pub struct LightingState {
    pub sun_dir: Vec3,
    pub moon_dir: Vec3,
    pub sun_color: [f32; 3],
    pub moon_color: [f32; 3],
    pub ambient_color: [f32; 3],
    pub sky_top: [f32; 3],
    pub sky_horizon: [f32; 3],
    pub fog_color: [f32; 3],
    pub sun_intensity: f32,
    pub moon_intensity: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    pub daylight: f32,
    pub twilight: f32,
    pub exposure: f32,
    pub saturation: f32,
    pub fog_height: f32,
    pub fog_falloff: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudControls {
    // --- Low-level clouds (0-2000m) ---
    pub coverage: f32,
    pub density: f32,
    pub cumulus: f32,
    pub stratus: f32,
    pub cumulonimbus: f32,
    pub base_height: f32,
    pub layer_thickness: f32,
    pub storm_thickness: f32,
    pub anvil_spread: f32,
    // --- Mid-level clouds (2000-7000m, altocumulus / altostratus) ---
    pub altocumulus: f32,
    pub altostratus: f32,
    pub mid_layer_height: f32,
    // --- High-level clouds (>6000m, cirrus family) ---
    pub cirrus: f32,
    pub layer_separation: f32,
    // --- Atmosphere-wide ---
    pub wind_shear: f32,
    pub erosion: f32,
    pub advection: f32,
    pub noise_scale: f32,
    pub wind_speed: f32,
    pub wind_dir_angle: f32,
    pub detail_strength: f32,
    pub silver_lining: f32,
}

impl Default for CloudControls {
    fn default() -> Self {
        Self {
            // Tuned for voxel world: fewer, closer, more dramatic clouds.
            coverage: 0.45,         // Reduced: clear sky gaps between cloud patches
            density: 0.75,          // Higher: clouds should be opaque, not ghostly
            cumulus: 0.72,          // Dominant: puffy cumulus = most recognizable cloud
            stratus: 0.08,          // Low: flat stratus is boring, keep minimal
            cumulonimbus: 0.02,     // Very rare: storms are special events
            base_height: 0.25,      // ~100m base: clouds visible overhead, not distant
            layer_thickness: 0.55,  // ~80m thick: visible 3D depth from ground
            storm_thickness: 0.50,  // Moderate storm towers
            anvil_spread: 0.20,     // Subtle anvils
            altocumulus: 0.15,      // Some mid-level patches
            altostratus: 0.04,      // Minimal: thin haze is not interesting
            mid_layer_height: 0.35, // Mid clouds at ~280m
            cirrus: 0.25,           // Light high wisps
            layer_separation: 0.30, // Good layer separation
            wind_shear: 0.18,       // Subtle shear
            erosion: 0.48,          // Good edge definition
            advection: 0.35,        // Gentle drift
            noise_scale: 0.5,       // Default size
            wind_speed: 0.30,       // Gentle breeze
            wind_dir_angle: 225.0,  // Southwest wind
            detail_strength: 0.70,  // Strong detail for nearby viewing
            silver_lining: 0.55,    // Visible silver lining
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeatherPreset {
    Clear,        // Nearly clear sky, 5-15% coverage
    PartlyCloudy, // Blue sky with cumulus patches, 40-60% coverage
    Overcast,     // Stratocumulus deck, 80-90% coverage
    Stormy,       // Cumulonimbus with storms, 85-95% coverage, high winds
    WinterSky,    // High cirrus with low stratus, 55-70% coverage
}

impl LightingState {
    pub fn from_time(time_of_day: f32) -> Self {
        Self::from_time_and_day(time_of_day, 0, 0xA341_316C, CloudControls::default())
    }

    pub fn from_time_and_day(
        time_of_day: f32,
        day_index: u32,
        weather_seed: u32,
        cloud_controls: CloudControls,
    ) -> Self {
        let cycle = time_of_day.rem_euclid(1.0);
        // Shift solar phase so 0.50 == noon, 0.00 == midnight, matching UI clock labels.
        let angle = (cycle - 0.25) * std::f32::consts::TAU;
        // Constant angular speed on a tilted celestial circle. Normalizing an
        // ellipse (x scaled to 0.33) sped the sun up threefold at the horizon,
        // squeezing nearly all golden/coral lighting into a few game minutes.
        let solar_tilt = 0.24_f32.atan();
        let sun_dir = Vec3::new(angle.cos() * solar_tilt.cos(),
            angle.sin() * solar_tilt.cos(), solar_tilt.sin());
        let moon_dir = -sun_dir;
        let sun_height = sun_dir.y;
        let moon_height = moon_dir.y;

        // Live authored cloud state is now the authority for atmospheric response.
        let coverage = cloud_controls.coverage.clamp(0.0, 1.0);
        let density = cloud_controls.density.clamp(0.0, 1.0);
        let cumulus = cloud_controls.cumulus.clamp(0.0, 1.0);
        let stratus = cloud_controls.stratus.clamp(0.0, 1.0);
        let cumulonimbus = cloud_controls.cumulonimbus.clamp(0.0, 1.0);
        let cirrus = cloud_controls.cirrus.clamp(0.0, 1.0);
        let altocumulus = cloud_controls.altocumulus.clamp(0.0, 1.0);
        let altostratus = cloud_controls.altostratus.clamp(0.0, 1.0);

        let base_height = cloud_controls.base_height.clamp(0.05, 0.72);
        let layer_thickness = cloud_controls.layer_thickness.clamp(0.12, 1.0);
        let storm_thickness = cloud_controls.storm_thickness.clamp(0.18, 1.0);
        let anvil_spread = cloud_controls.anvil_spread.clamp(0.0, 1.0);
        let mid_layer_height = cloud_controls.mid_layer_height.clamp(0.20, 0.65);
        let layer_separation = cloud_controls.layer_separation.clamp(0.08, 0.72);
        let wind_shear = cloud_controls.wind_shear.clamp(0.0, 1.0);
        let erosion = cloud_controls.erosion.clamp(0.0, 1.0);
        let advection = cloud_controls.advection.clamp(0.0, 1.0);
        let noise_scale = cloud_controls.noise_scale.clamp(0.0, 1.0);
        let wind_speed = cloud_controls.wind_speed.clamp(0.0, 1.0);
        let detail_strength = cloud_controls.detail_strength.clamp(0.0, 1.0);
        let silver_lining = cloud_controls.silver_lining.clamp(0.0, 1.0);

        // Tiny non-authoritative day/seed modulation so skies are not perfectly static,
        // but it no longer defines the actual weather regime.
        let day_variation = hash01(weather_seed ^ day_index.rotate_left(9) ^ 0xD1B5_4A32);

        let low_base_factor = 1.0 - smoothstep(0.18, 0.60, base_height);
        let low_cloudiness = saturate(
            coverage
                * density
                * (0.18 + stratus * 0.34 + cumulus * 0.42 + cumulonimbus * 0.24)
                * (0.70 + layer_thickness * 0.30),
        );
        let mid_cloudiness = saturate(
            coverage
                * density
                * (altocumulus * 0.46 + altostratus * 0.74)
                * (0.42 + mid_layer_height * 0.18 + layer_separation * 0.10),
        );
        let high_cloudiness = saturate(
            coverage * density * cirrus * (0.44 + layer_separation * 0.34 + wind_shear * 0.08),
        );

        let storminess =
            saturate(cumulonimbus * (0.60 + storm_thickness * 0.30 + anvil_spread * 0.10));

        let cloud_mass =
            saturate(low_cloudiness * 0.58 + mid_cloudiness * 0.24 + high_cloudiness * 0.18);

        let overcast = saturate(
            low_cloudiness * 0.54
                + mid_cloudiness * 0.28
                + storminess * 0.38
                + stratus * 0.18
                + altostratus * 0.16
                - detail_strength * 0.08,
        );

        let cloud_grayness = saturate(
            overcast * 0.42
                + stratus * 0.20
                + altostratus * 0.16
                + storminess * 0.24
                + (1.0 - silver_lining) * 0.10
                + (1.0 - erosion) * 0.06
                + day_variation * 0.04,
        );

        let haze = (low_cloudiness * 0.10
            + mid_cloudiness * 0.06
            + stratus * 0.08
            + altostratus * 0.08
            + storminess * 0.07
            + low_base_factor * 0.06
            + wind_speed * 0.02
            + day_variation * 0.015
            - noise_scale * 0.02
            - detail_strength * 0.02
            - advection * 0.015)
            .clamp(0.0, 0.35);

        let daylight = smoothstep(-0.28, 0.20, sun_height);
        let twilight_core = 1.0 - smoothstep(0.22, 0.62, sun_height.abs());
        let horizon_glow = 1.0 - smoothstep(0.04, 0.34, sun_height.abs());
        let golden_hour = 1.0 - smoothstep(0.03, 0.42, sun_height.abs());
        let twilight = (twilight_core * 0.68 + horizon_glow * 0.62 + golden_hour * 0.42)
            * (1.0 - daylight * 0.46);
        let night = 1.0 - daylight;

        // Solar source stays nearly white. Atmospheric transmittance already
        // removes blue/green at low elevation in sky, cloud and terrain shaders.
        // Tinting the source orange as well made the entire sunset sky brown.
        let sun_color = [1.00, 0.98, 0.95];
        let moon_color = [0.50, 0.58, 0.74];

        let ambient_night = [0.040, 0.050, 0.080];
        let ambient_day = [0.18, 0.22, 0.30];
        let mut ambient_color = lerp_color(ambient_night, ambient_day, daylight);
        ambient_color = lerp_color(
            ambient_color,
            [0.18, 0.12, 0.14],
            twilight * (1.0 - overcast * 0.68) * 0.18,
        );
        let overcast_ambient = [0.17, 0.19, 0.22];
        ambient_color = lerp_color(ambient_color, overcast_ambient, overcast * daylight * 0.48);

        let mut sky_top = lerp_color([0.020, 0.040, 0.100], [0.09, 0.36, 0.92], daylight);
        let mut sky_horizon = lerp_color([0.050, 0.085, 0.170], [0.44, 0.72, 1.00], daylight);

        let cloud_tint_top = [0.28, 0.40, 0.64];
        let cloud_tint_horizon = [0.48, 0.62, 0.80];
        sky_top = lerp_color(
            sky_top,
            cloud_tint_top,
            saturate(overcast * 0.42 + cloud_grayness * 0.18),
        );
        sky_horizon = lerp_color(
            sky_horizon,
            cloud_tint_horizon,
            saturate(overcast * 0.55 + cloud_grayness * 0.24),
        );
        sky_horizon = lerp_color(
            sky_horizon,
            [1.00, 0.58, 0.18],
            twilight * (1.0 - overcast * 0.58) * 1.04,
        );
        sky_top = lerp_color(sky_top, [0.66, 0.30, 0.22], twilight * 0.24);

        let fog_night = [0.050, 0.060, 0.100];
        let fog_day = [0.50, 0.68, 0.96];
        let mut fog_color = lerp_color(fog_night, fog_day, daylight);
        fog_color = lerp_color(
            fog_color,
            [0.56, 0.66, 0.80],
            saturate(overcast * 0.34 + haze * 0.14),
        );
        fog_color = lerp_color(
            fog_color,
            [1.00, 0.66, 0.30],
            twilight * (1.0 - overcast * 0.54) * 0.44,
        );
        fog_color = lerp_color(fog_color, sky_horizon, 0.12 + haze * 0.16 + overcast * 0.08);

        let sun_elev = smoothstep(-0.08, 0.85, sun_height);
        let solar_extinction = 1.0 - smoothstep(-0.18, 0.10, sun_height);
        let mut sun_intensity =
            daylight * (2.0 + 7.1 * sun_elev + twilight * 0.38) * (1.0 - solar_extinction * 0.30);

        let sun_occlusion = saturate(
            low_cloudiness * 0.32
                + mid_cloudiness * 0.18
                + high_cloudiness * 0.08
                + overcast * 0.20
                + storminess * 0.22,
        );
        sun_intensity *= 1.0 - sun_occlusion * 0.65;

        let moon_base = night * smoothstep(-0.16, 0.26, moon_height) * 0.32;
        let moon_occlusion = saturate(
            low_cloudiness * 0.28
                + mid_cloudiness * 0.16
                + high_cloudiness * 0.10
                + overcast * 0.18,
        );
        let moon_intensity = moon_base * (1.0 - moon_occlusion * 0.62);

        let fog_scale = 1.0 + cloud_mass * (0.30 + overcast * 0.30 + haze * 0.12);
        let fog_start = (75.0 + daylight * 271.0) / fog_scale;
        let fog_end = (320.0 + daylight * 842.0) / fog_scale.powf(0.86);

        let exposure =
            (0.28 + daylight * 0.66 + twilight * 0.24) * (1.0 - overcast * 0.08) + haze * 0.008;
        let saturation = (0.74 + daylight * 0.18 + twilight * 0.08)
            * (1.0 - overcast * 0.10 - stratus * 0.06 - altostratus * 0.04);

        let fog_height = WATER_LEVEL_METERS + 14.0 + haze * 9.0 + low_base_factor * 4.0;
        let fog_falloff = 0.036 + night * 0.010 + cloud_mass * 0.008 + haze * 0.006;

        Self {
            sun_dir,
            moon_dir,
            sun_color,
            moon_color,
            ambient_color,
            sky_top,
            sky_horizon,
            fog_color,
            sun_intensity,
            moon_intensity,
            fog_start,
            fog_end,
            daylight,
            twilight,
            exposure,
            saturation: saturation.clamp(0.52, 1.00),
            fog_height,
            fog_falloff,
        }
    }
}
fn sanitize_cloud_controls(mut c: CloudControls) -> CloudControls {
    c.coverage = c.coverage.clamp(0.02, 0.98);
    c.density = c.density.clamp(0.02, 1.0);

    c.cumulus = c.cumulus.clamp(0.0, 1.0);
    c.cirrus = c.cirrus.clamp(0.0, 0.75);
    c.stratus = c.stratus.clamp(0.0, 1.0);
    c.cumulonimbus = c.cumulonimbus.clamp(0.0, 1.0);

    c.base_height = c.base_height.clamp(0.05, 0.82);
    c.layer_separation = c.layer_separation.clamp(0.05, 0.95);
    c.layer_thickness = c.layer_thickness.clamp(0.10, 1.00);
    c.storm_thickness = c.storm_thickness.clamp(0.10, 1.00);
    c.wind_shear = c.wind_shear.clamp(0.0, 1.0);
    c.anvil_spread = c.anvil_spread.clamp(0.0, 1.0);
    c.erosion = c.erosion.clamp(0.0, 1.0);
    c.advection = c.advection.clamp(0.0, 1.0);
    c.altocumulus = c.altocumulus.clamp(0.0, 1.0);
    c.altostratus = c.altostratus.clamp(0.0, 1.0);
    c.mid_layer_height = c.mid_layer_height.clamp(0.20, 0.65);
    c.noise_scale = c.noise_scale.clamp(0.0, 1.0);
    c.wind_speed = c.wind_speed.clamp(0.0, 1.0);
    c.wind_dir_angle = c.wind_dir_angle.rem_euclid(360.0);
    c.detail_strength = c.detail_strength.clamp(0.0, 1.0);
    c.silver_lining = c.silver_lining.clamp(0.0, 1.0);

    // Storm clouds need enough body and vertical range to read correctly.
    if c.cumulonimbus > 0.35 {
        c.density = c.density.max(0.45);
        c.layer_thickness = c.layer_thickness.max(0.42);
        c.storm_thickness = c.storm_thickness.max(0.55);
        c.anvil_spread = c.anvil_spread.max(0.25);
    }

    // Heavy stratus should flatten and lower the deck a bit.
    if c.stratus > 0.55 {
        c.base_height = c.base_height.min(0.52);
        c.layer_thickness = c.layer_thickness.max(0.28);
        c.erosion = c.erosion.min(0.45);
    }

    // Strong cirrus should not simultaneously make the whole sky opaque.
    if c.cirrus > 0.45 {
        c.density = c.density.min(0.72);
        c.cirrus = c.cirrus.min(0.65);
    }

    c
}
fn cloud_controls_from_weather(weather: WeatherProfile) -> CloudControls {
    let storm = weather.cumulonimbus.clamp(0.0, 1.0);
    let stratus = weather.stratus.clamp(0.0, 1.0);
    let cirrus = weather.cirrus.clamp(0.0, 1.0);
    let cumulus = weather.cumulus.clamp(0.0, 1.0);
    let overcast = weather.overcast.clamp(0.0, 1.0);
    let density = weather.density.clamp(0.0, 1.0);
    let turbulence = weather.turbulence.clamp(0.0, 1.0);

    sanitize_cloud_controls(CloudControls {
        coverage: weather.coverage.clamp(0.06, 0.96),
        density: (density * 1.2 + 0.10).clamp(0.12, 0.95), // boost: clouds should be visible
        cumulus,
        cirrus: cirrus * (1.0 - overcast * 0.30),
        stratus,
        cumulonimbus: storm,
        // Lower base for more approachable clouds. Stratus/overcast pushes them down further.
        base_height: (0.35 - stratus * 0.18 - overcast * 0.12 + cirrus * 0.10 - storm * 0.06)
            .clamp(0.08, 0.65),
        layer_separation: (0.18 + cirrus * 0.34 + storm * 0.10 - stratus * 0.06).clamp(0.08, 0.85),
        // Thicker layers: cumulus and storm need vertical depth for immersive fly-through.
        layer_thickness: (0.35 + density * 0.22 + stratus * 0.22 + cumulus * 0.18 + storm * 0.20)
            .clamp(0.25, 0.95),
        storm_thickness: (0.30 + storm * 0.58 + overcast * 0.18).clamp(0.22, 1.0),
        wind_shear: (0.18
            + weather.wind_speed * 0.22
            + turbulence * 0.34
            + cirrus * 0.16
            + storm * 0.12)
            .clamp(0.0, 1.0),
        anvil_spread: (0.04 + storm * 0.82 + weather.wind_speed * 0.08).clamp(0.0, 1.0),
        erosion: (0.30 + turbulence * 0.24 + cirrus * 0.06 + cumulus * 0.12
            - stratus * 0.14
            - overcast * 0.10)
            .clamp(0.08, 0.85),
        advection: (0.10 + weather.wind_speed * 0.50 + turbulence * 0.12).clamp(0.04, 0.90),
        noise_scale: (0.22 + cumulus * 0.28 + cirrus * 0.14 + turbulence * 0.12
            - stratus * 0.06
            - overcast * 0.08)
            .clamp(0.10, 0.78),
        altocumulus: (0.18 + turbulence * 0.24 + cumulus * 0.12).clamp(0.0, 0.85),
        altostratus: (0.06 + overcast * 0.28 + stratus * 0.18).clamp(0.0, 0.75),
        mid_layer_height: (0.36 + storm * 0.06 - stratus * 0.08).clamp(0.20, 0.65),
        wind_speed: weather.wind_speed.clamp(0.0, 1.0),
        wind_dir_angle: weather.wind_angle.to_degrees().rem_euclid(360.0),
        detail_strength: (0.42 + turbulence * 0.28 + cumulus * 0.18 - stratus * 0.16)
            .clamp(0.15, 0.92),
        silver_lining: (0.35 + cumulus * 0.28 + cirrus * 0.15 - overcast * 0.18).clamp(0.12, 0.82),
    })
}

#[derive(Clone, Copy)]
struct WeatherProfile {
    coverage: f32,
    density: f32,
    cumulus: f32,
    cirrus: f32,
    stratus: f32,
    cumulonimbus: f32,
    haze: f32,
    overcast: f32,
    cloud_grayness: f32,
    wind_angle: f32,
    wind_speed: f32,
    turbulence: f32,
}

fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

fn hash01(x: u32) -> f32 {
    hash_u32(x) as f32 / u32::MAX as f32
}

fn weather_profile_for_day(day_index: u32, weather_seed: u32) -> WeatherProfile {
    let day_key = day_index
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(weather_seed.rotate_left(11));

    let roll = hash01(day_key ^ 0xA3C5_9AC3);
    let j0 = hash01(day_key ^ 0xB529_7A4D);
    let j1 = hash01(day_key ^ 0x68E3_1DA4);
    let j2 = hash01(day_key ^ 0x1B56_C4E9);

    if roll < 0.14 {
        // Crisp clear / very light fair weather — nearly empty sky
        WeatherProfile {
            coverage: 0.05 + j0 * 0.12,
            density: 0.08 + j1 * 0.10,
            cumulus: 0.10 + j1 * 0.18,
            cirrus: 0.04 + j2 * 0.10,
            stratus: 0.00 + j0 * 0.02,
            cumulonimbus: 0.0,
            haze: 0.01 + j2 * 0.03,
            overcast: 0.0,
            cloud_grayness: 0.01 + j1 * 0.03,
            wind_angle: j0 * std::f32::consts::TAU,
            wind_speed: 0.12 + j1 * 0.18,
            turbulence: 0.06 + j2 * 0.10,
        }
    } else if roll < 0.26 {
        // Thin high cirrus / jetstream day — wispy ice clouds
        WeatherProfile {
            coverage: 0.15 + j0 * 0.20,
            density: 0.14 + j1 * 0.16,
            cumulus: 0.06 + j2 * 0.12,
            cirrus: 0.55 + j0 * 0.30,
            stratus: 0.02 + j1 * 0.04,
            cumulonimbus: 0.0,
            haze: 0.02 + j2 * 0.05,
            overcast: 0.04 + j1 * 0.06,
            cloud_grayness: 0.05 + j0 * 0.08,
            wind_angle: (0.85 + j0 * 0.35) * std::f32::consts::PI,
            wind_speed: 0.38 + j1 * 0.28,
            turbulence: 0.12 + j2 * 0.14,
        }
    } else if roll < 0.46 {
        // Fair weather cumulus — puffy clouds, blue sky dominant
        WeatherProfile {
            coverage: 0.30 + j0 * 0.30,
            density: 0.35 + j1 * 0.25,
            cumulus: 0.55 + j0 * 0.30,
            cirrus: 0.06 + j2 * 0.12,
            stratus: 0.02 + j1 * 0.06,
            cumulonimbus: 0.00 + j2 * 0.04,
            haze: 0.02 + j1 * 0.04,
            overcast: 0.03 + j2 * 0.06,
            cloud_grayness: 0.04 + j1 * 0.06,
            wind_angle: (0.15 + j0 * 0.80) * std::f32::consts::PI,
            wind_speed: 0.22 + j1 * 0.26,
            turbulence: 0.14 + j2 * 0.16,
        }
    } else if roll < 0.62 {
        // Broken cumulus / dramatic textured sky — photogenic clouds
        WeatherProfile {
            coverage: 0.45 + j0 * 0.30,
            density: 0.40 + j1 * 0.25,
            cumulus: 0.60 + j2 * 0.25,
            cirrus: 0.14 + j0 * 0.16,
            stratus: 0.06 + j1 * 0.10,
            cumulonimbus: 0.02 + j2 * 0.06,
            haze: 0.03 + j1 * 0.06,
            overcast: 0.08 + j0 * 0.08,
            cloud_grayness: 0.06 + j1 * 0.08,
            wind_angle: (0.35 + j0 * 1.00) * std::f32::consts::PI,
            wind_speed: 0.28 + j1 * 0.30,
            turbulence: 0.20 + j2 * 0.18,
        }
    } else if roll < 0.78 {
        // Stratocumulus / marine deck — grey blanket with gaps
        WeatherProfile {
            coverage: 0.58 + j0 * 0.28,
            density: 0.52 + j1 * 0.25,
            cumulus: 0.16 + j2 * 0.14,
            cirrus: 0.06 + j0 * 0.08,
            stratus: 0.46 + j1 * 0.28,
            cumulonimbus: 0.02 + j2 * 0.06,
            haze: 0.06 + j1 * 0.08,
            overcast: 0.24 + j0 * 0.16,
            cloud_grayness: 0.16 + j1 * 0.12,
            wind_angle: (1.00 + j0 * 0.60) * std::f32::consts::PI,
            wind_speed: 0.32 + j1 * 0.26,
            turbulence: 0.22 + j2 * 0.14,
        }
    } else if roll < 0.88 {
        // Overcast / nimbostratus — heavy grey sky
        WeatherProfile {
            coverage: 0.78 + j0 * 0.20,
            density: 0.72 + j1 * 0.22,
            cumulus: 0.08 + j2 * 0.10,
            cirrus: 0.04 + j0 * 0.08,
            stratus: 0.60 + j1 * 0.24,
            cumulonimbus: 0.08 + j2 * 0.14,
            haze: 0.10 + j1 * 0.10,
            overcast: 0.50 + j0 * 0.22,
            cloud_grayness: 0.26 + j1 * 0.14,
            wind_angle: (1.25 + j0 * 0.50) * std::f32::consts::PI,
            wind_speed: 0.40 + j1 * 0.28,
            turbulence: 0.22 + j2 * 0.14,
        }
    } else if roll < 0.96 {
        // Frontal storm / shelf cloud day — dramatic
        WeatherProfile {
            coverage: 0.68 + j0 * 0.26,
            density: 0.75 + j1 * 0.22,
            cumulus: 0.35 + j2 * 0.20,
            cirrus: 0.08 + j0 * 0.12,
            stratus: 0.20 + j1 * 0.18,
            cumulonimbus: 0.50 + j2 * 0.30,
            haze: 0.08 + j1 * 0.10,
            overcast: 0.32 + j0 * 0.20,
            cloud_grayness: 0.18 + j1 * 0.14,
            wind_angle: (1.45 + j0 * 0.35) * std::f32::consts::PI,
            wind_speed: 0.55 + j1 * 0.30,
            turbulence: 0.42 + j2 * 0.22,
        }
    } else {
        // Severe convective / dramatic thunderstorm — towering clouds
        WeatherProfile {
            coverage: 0.62 + j0 * 0.30,
            density: 0.80 + j1 * 0.18,
            cumulus: 0.42 + j2 * 0.24,
            cirrus: 0.12 + j0 * 0.14,
            stratus: 0.14 + j1 * 0.12,
            cumulonimbus: 0.70 + j2 * 0.26,
            haze: 0.10 + j1 * 0.10,
            overcast: 0.28 + j0 * 0.22,
            cloud_grayness: 0.22 + j1 * 0.14,
            wind_angle: (1.60 + j0 * 0.30) * std::f32::consts::PI,
            wind_speed: 0.68 + j1 * 0.30,
            turbulence: 0.55 + j2 * 0.24,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct DayNightCycle {
    time_of_day: f32,
    day_length_seconds: f32,
    day_index: u32,
    weather_seed: u32,
    cloud_controls: CloudControls,
}

impl Default for DayNightCycle {
    fn default() -> Self {
        let time_of_day = DEFAULT_START_TIME_OF_DAY;
        let day_length_seconds = DEFAULT_DAY_LENGTH_SECONDS;
        let weather_seed = hash_u32(0xA341_316C);
        Self {
            time_of_day,
            day_length_seconds,
            day_index: 0,
            weather_seed,
            cloud_controls: sanitize_cloud_controls(cloud_controls_from_weather(
                weather_profile_for_day(0, weather_seed),
            )),
        }
    }
}

impl DayNightCycle {
    pub fn new(start_time_of_day: f32, day_length_seconds: f32) -> Self {
        let start = start_time_of_day.rem_euclid(1.0);
        let day_len = day_length_seconds.max(1.0);
        let weather_seed = hash_u32(start.to_bits() ^ day_len.to_bits().rotate_left(13));
        Self {
            time_of_day: start,
            day_length_seconds: day_len,
            day_index: 0,
            weather_seed,
            cloud_controls: sanitize_cloud_controls(cloud_controls_from_weather(
                weather_profile_for_day(0, weather_seed),
            )),
        }
    }

    pub fn reset(&mut self, start_time_of_day: f32) {
        self.time_of_day = start_time_of_day.rem_euclid(1.0);
        self.day_index = 0;
        self.weather_seed = hash_u32(self.weather_seed ^ 0x9E37_79B9);
        self.cloud_controls = sanitize_cloud_controls(cloud_controls_from_weather(
            weather_profile_for_day(self.day_index, self.weather_seed),
        ));
    }

    pub fn set_day_length_seconds(&mut self, seconds: f32) {
        self.day_length_seconds = seconds.max(1.0);
    }

    pub fn day_length_seconds(&self) -> f32 {
        self.day_length_seconds
    }

    pub fn set_time_of_day(&mut self, time_of_day: f32) {
        self.time_of_day = time_of_day.rem_euclid(1.0);
    }

    pub fn time_of_day(&self) -> f32 {
        self.time_of_day
    }

    pub fn weather_seed(&self) -> u32 {
        self.weather_seed
    }

    pub fn set_cloud_controls(&mut self, controls: CloudControls) {
        self.cloud_controls = sanitize_cloud_controls(controls);
    }

    pub fn cloud_controls(&self) -> CloudControls {
        self.cloud_controls
    }

    pub fn randomize_weather(&mut self) {
        self.weather_seed = hash_u32(
            self.weather_seed
                ^ self.day_index.rotate_left(7)
                ^ self.time_of_day.to_bits().rotate_left(13)
                ^ 0x7F4A_7C15,
        );
        // Also advance the day index so we pick a different weather profile bucket
        self.day_index = self
            .day_index
            .wrapping_add(hash_u32(self.weather_seed) % 11 + 2);
        let mut controls =
            cloud_controls_from_weather(weather_profile_for_day(self.day_index, self.weather_seed));

        // Post-randomization jitter: shake individual controls by ±30% of their range
        // so each Randomize press produces a visibly distinct sky, not a subtle tweak.
        let j0 = hash01(self.weather_seed ^ 0x1234_ABCD);
        let j1 = hash01(self.weather_seed ^ 0x5678_EF01);
        let j2 = hash01(self.weather_seed ^ 0x9ABC_DEF0);
        let j3 = hash01(self.weather_seed ^ 0x3456_7890);
        let j4 = hash01(self.weather_seed ^ 0xDEAD_BEEF);
        let j5 = hash01(self.weather_seed ^ 0xCAFE_BABE);
        let j6 = hash01(self.weather_seed ^ 0xF00D_CAFE);
        let j7 = hash01(self.weather_seed ^ 0xBAAD_F00D);
        let j8 = hash01(self.weather_seed ^ 0x0DDB_0D1E);
        let j9 = hash01(self.weather_seed ^ 0xABCD_1234);
        let j10 = hash01(self.weather_seed ^ 0xFEED_FACE);
        let j11 = hash01(self.weather_seed ^ 0x1337_C0DE);
        let j12 = hash01(self.weather_seed ^ 0x4242_4242);

        controls.coverage = (controls.coverage + (j0 - 0.5) * 0.40).clamp(0.05, 0.98);
        controls.density = (controls.density + (j1 - 0.5) * 0.35).clamp(0.08, 0.95);
        controls.cumulus = (controls.cumulus + (j2 - 0.5) * 0.45).clamp(0.0, 1.0);
        controls.stratus = (controls.stratus + (j3 - 0.5) * 0.40).clamp(0.0, 1.0);
        controls.cirrus = (controls.cirrus + (j4 - 0.5) * 0.30).clamp(0.0, 0.75);
        controls.cumulonimbus = (controls.cumulonimbus + (j5 - 0.5) * 0.30).clamp(0.0, 1.0);
        controls.base_height = (controls.base_height + (j6 - 0.5) * 0.25).clamp(0.18, 0.72);
        controls.layer_thickness = (controls.layer_thickness + (j7 - 0.5) * 0.30).clamp(0.12, 1.0);
        controls.wind_speed = (controls.wind_speed + (j8 - 0.5) * 0.35).clamp(0.0, 1.0);
        controls.wind_dir_angle = (controls.wind_dir_angle + (j9 - 0.5) * 180.0).rem_euclid(360.0);
        controls.erosion = (controls.erosion + (j10 - 0.5) * 0.30).clamp(0.0, 1.0);
        controls.altocumulus = (controls.altocumulus + (j11 - 0.5) * 0.35).clamp(0.0, 1.0);
        controls.altostratus = (controls.altostratus + (j12 - 0.5) * 0.30).clamp(0.0, 1.0);

        self.cloud_controls = sanitize_cloud_controls(controls);
    }

    pub fn set_weather_preset(&mut self, preset: WeatherPreset) {
        self.cloud_controls = sanitize_cloud_controls(match preset {
            WeatherPreset::Clear => CloudControls {
                coverage: 0.15, // Very sparse coverage — few scattered puffs
                density: 0.40,  // Enough opacity to be visible against blue sky
                cumulus: 0.45,  // Visible cumulus character (was 0.06 — invisible)
                cirrus: 0.35,   // Some high wisps
                stratus: 0.02,  // Almost no stratus
                cumulonimbus: 0.0,
                base_height: 0.32,
                layer_thickness: 0.28, // Slightly thicker so clouds accumulate density
                storm_thickness: 0.35,
                anvil_spread: 0.0,
                altocumulus: 0.08,
                altostratus: 0.03,
                mid_layer_height: 0.42,
                layer_separation: 0.25,
                wind_shear: 0.13,
                erosion: 0.22,
                advection: 0.25,
                noise_scale: 0.38,
                wind_speed: 0.10,
                wind_dir_angle: 180.0,
                detail_strength: 0.45,
                silver_lining: 0.30,
            },
            WeatherPreset::PartlyCloudy => CloudControls {
                coverage: 0.50, // Half the sky covered
                density: 0.70,  // Solid opacity for dramatic cumulus
                cumulus: 0.82,  // Strong cumulus character
                cirrus: 0.42,   // Distinct high wisps above the cumulus field
                stratus: 0.03,  // Very little stratus
                cumulonimbus: 0.0,
                base_height: 0.25, // Slightly lower for better ground visibility
                layer_thickness: 0.65, // Thick enough for volumetric depth
                storm_thickness: 0.45,
                anvil_spread: 0.08,
                altocumulus: 0.26,
                altostratus: 0.10,
                mid_layer_height: 0.48,
                layer_separation: 0.28,
                wind_shear: 0.17,
                erosion: 0.42, // More edge definition
                advection: 0.36,
                noise_scale: 0.52,
                wind_speed: 0.25,
                wind_dir_angle: 225.0,
                detail_strength: 0.68, // More detail
                silver_lining: 0.52,
            },
            WeatherPreset::Overcast => CloudControls {
                coverage: 0.92, // Nearly full coverage — minimal blue sky
                density: 0.82,  // Thick opaque deck
                cumulus: 0.10,  // Minimal individual clouds
                cirrus: 0.04,   // Reduced high clouds
                stratus: 0.75,  // Strong stratus base — flat sheet
                cumulonimbus: 0.0,
                base_height: 0.20,     // Low base — oppressive ceiling
                layer_thickness: 0.82, // Very thick layer
                storm_thickness: 0.50,
                anvil_spread: 0.06,
                altocumulus: 0.32, // Moderate mid-layer
                altostratus: 0.45, // Strong mid haze for depth
                mid_layer_height: 0.42,
                layer_separation: 0.30,
                wind_shear: 0.19,
                erosion: 0.20, // Less erosion — smoother overcast deck
                advection: 0.31,
                noise_scale: 0.42,
                wind_speed: 0.21,
                wind_dir_angle: 270.0,
                detail_strength: 0.40, // Less detail — flat overcast character
                silver_lining: 0.35,
            },
            WeatherPreset::Stormy => CloudControls {
                coverage: 0.94,        // Nearly total coverage
                density: 0.90,         // Very dark/thick clouds
                cumulus: 0.50,         // Towering cumulus buildups
                cirrus: 0.03,          // Minimal high clouds
                stratus: 0.30,         // Some base stratus
                cumulonimbus: 0.82,    // Heavy storm character
                base_height: 0.14,     // Very low clouds — threatening feel
                layer_thickness: 0.92, // Extremely thick
                storm_thickness: 0.92,
                anvil_spread: 0.72,
                altocumulus: 0.08,
                altostratus: 0.06,
                mid_layer_height: 0.40,
                layer_separation: 0.25,
                wind_shear: 0.50,
                erosion: 0.48,
                advection: 0.70,
                noise_scale: 0.64,
                wind_speed: 0.80,
                wind_dir_angle: 315.0,
                detail_strength: 0.70,
                silver_lining: 0.55,
            },
            WeatherPreset::WinterSky => CloudControls {
                coverage: 0.62, // Moderate coverage — winter grey
                density: 0.60,  // Moderate opacity for visible layers
                cumulus: 0.10,  // Almost no cumulus — flat winter sky
                cirrus: 0.65,   // Heavy high cirrus — icy wisps
                stratus: 0.52,  // Strong low stratus deck
                cumulonimbus: 0.0,
                base_height: 0.18, // Low base — winter ceiling
                layer_thickness: 0.45,
                storm_thickness: 0.40,
                anvil_spread: 0.03,
                altocumulus: 0.22,
                altostratus: 0.38, // Strong mid haze — layered look
                mid_layer_height: 0.40,
                layer_separation: 0.35,
                wind_shear: 0.24,
                erosion: 0.25,
                advection: 0.23,
                noise_scale: 0.44,
                wind_speed: 0.17,
                wind_dir_angle: 45.0,
                detail_strength: 0.45,
                silver_lining: 0.40,
            },
        });
    }

    pub fn advance(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let total = self.time_of_day + dt / self.day_length_seconds;
        let wraps = total.floor().max(0.0) as u32;
        self.day_index = self.day_index.wrapping_add(wraps);
        self.time_of_day = total.rem_euclid(1.0);
    }

    pub fn lighting(&self) -> LightingState {
        LightingState::from_time_and_day(
            self.time_of_day,
            self.day_index,
            self.weather_seed,
            self.cloud_controls,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldSampleConfig {
    pub generator_version: u32,
    pub seed: u32,
    pub world_type: WorldType,
    pub voxel_scale: VoxelScalePreset,
}

impl WorldSampleConfig {
    pub fn voxel_meters(self) -> f32 {
        self.voxel_scale.meters()
    }

    pub fn inv_voxel_meters(self) -> f32 {
        self.voxel_meters().recip()
    }

    pub fn world_x(self, x: i32) -> f32 {
        x as f32 * self.voxel_meters()
    }

    pub fn world_y(self, y: i32) -> f32 {
        y as f32 * self.voxel_meters()
    }

    pub fn world_z(self, z: i32) -> f32 {
        z as f32 * self.voxel_meters()
    }

    pub fn blocks_for_height(self, meters: f32) -> i32 {
        (meters * self.inv_voxel_meters()).round() as i32
    }

    pub fn blocks_for_span(self, meters: f32) -> i32 {
        ((meters * self.inv_voxel_meters()).ceil() as i32).max(1)
    }

    pub fn min_world_y(self) -> i32 {
        (if self.generator_version >= 2 {
            -64.0
        } else {
            WORLD_MIN_Y_METERS
        } * self.inv_voxel_meters())
        .floor() as i32
    }

    pub fn max_world_y(self) -> i32 {
        (WORLD_MAX_Y_METERS * self.inv_voxel_meters()).ceil() as i32
    }

    pub fn water_level(self) -> i32 {
        self.blocks_for_height(WATER_LEVEL_METERS)
    }

    pub fn clamp_height(self, y: i32) -> i32 {
        y.clamp(self.min_world_y() + 4, self.max_world_y() - 3)
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
struct ClimateSample {
    temperature: f32,
    moisture: f32,
    continentalness: f32,
}

#[derive(Clone, Copy)]
struct TreeProfile {
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
}

#[derive(Clone, Copy)]
struct BushProfile {
    anchor_x: i32,
    anchor_z: i32,
    ground_y: i32,
    base_radius: i32,
    top_dy: i32,
    lean_x: i32,
    lean_z: i32,
}

#[derive(Clone)]
pub struct WorldSnapshot {
    pub regions: regions::RegionView,
    pub water_levels: Arc<HashMap<IVec3, u8>>,
    pub cfg: WorldSampleConfig,
    pub chunks: Arc<HashMap<IVec3, Arc<ChunkData>>>,
}

impl WorldSnapshot {
    pub fn edited_coords_near(
        &self,
        center: IVec3,
        radius: i32,
    ) -> impl Iterator<Item = IVec3> + '_ {
        self.chunks.keys().copied().chain(
            self.regions
                .records
                .nearby(center, radius)
                .filter(|c| !self.chunks.contains_key(c))
                .copied(),
        )
    }
    pub fn edits_ready(&self, c: IVec3) -> bool {
        (-1..=1).all(|z| {
            (-1..=1).all(|y| {
                (-1..=1).all(|x| {
                    let p = c + IVec3::new(x, y, z);
                    !self.regions.records.contains_key(&p) || self.chunks.contains_key(&p)
                })
            })
        })
    }
    pub fn get(&self, p: IVec3) -> BlockId {
        let chunk_coord = chunk_coord_from_world_block(p);
        if let Some(chunk) = self.chunks.get(&chunk_coord) {
            chunk.blocks[ChunkData::local_index(p.x, p.y, p.z)]
        } else if self.regions.records.contains_key(&chunk_coord) {
            self.regions
                .load(chunk_coord)
                .ok()
                .flatten()
                .map_or(BlockId::Stone, |c| {
                    c.chunk.blocks[ChunkData::local_index(p.x, p.y, p.z)]
                })
        } else {
            sample_block(&self.cfg, p)
        }
    }

    pub fn solid(&self, p: IVec3) -> bool {
        self.get(p).is_solid()
    }
}

#[derive(Clone, Copy)]
struct ColumnSample {
    height: i32,
    surface: BlockId,
    filler: BlockId,
    vegetation_density: f32,
}

fn saturate(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = saturate((x - edge0) / (edge1 - edge0).max(1e-6));
    t * t * (3.0 - 2.0 * t)
}

fn lerp_color(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = saturate(t);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn hash2(x: i32, z: i32, seed: u32) -> f32 {
    let mut h = seed ^ (x as u32).wrapping_mul(374761393) ^ (z as u32).wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut h = seed
        ^ (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(1103515245)
        ^ (z as u32).wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    ((h ^ (h >> 16)) & 0xffff) as f32 / 65535.0
}

fn value_noise2(x: f32, z: f32, seed: u32) -> f32 {
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

fn value_noise3(x: f32, y: f32, z: f32, seed: u32) -> f32 {
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

fn fbm2(x: f32, z: f32, seed: u32) -> f32 {
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

fn ridge(x: f32) -> f32 {
    1.0 - (2.0 * x - 1.0).abs()
}

fn smooth_band(value: f32, center: f32, width: f32) -> f32 {
    saturate(1.0 - ((value - center).abs() / width))
}

fn branch_direction(cell_x: i32, cell_z: i32, branch_idx: i32, seed: u32) -> (i32, i32) {
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

fn tree_trunk_center(tree: TreeProfile, y: i32) -> (i32, i32) {
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

fn tree_block(cfg: &WorldSampleConfig, tree: TreeProfile, p: IVec3) -> BlockId {
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

fn sample_climate(cfg: &WorldSampleConfig, x: i32, z: i32) -> ClimateSample {
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
pub(crate) fn distant_surface(cfg: &WorldSampleConfig, x: i32, z: i32) -> (f32, BlockId) {
    let column = sample_column(cfg, x, z);
    (column.height as f32 + 1.0, column.surface)
}

fn sample_column(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
    generation_cache::column(cfg, x, z)
}

fn sample_column_uncached(cfg: &WorldSampleConfig, x: i32, z: i32) -> ColumnSample {
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
        WorldType::Hills => blended + coast + mountain_boost * 0.65 - valley_cut,
        WorldType::Ridged => {
            blended + coast * 0.45 + ridge_shape * 18.0 + mountain_boost - valley_cut * 0.35
        }
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

    ColumnSample {
        height,
        surface,
        filler,
        vegetation_density: (weights.plains * 0.95 + weights.hills * 0.40 + weights.snow * 0.08)
            .clamp(0.0, 1.0)
            * (1.0 - weights.desert * 0.90),
    }
}

fn water_table(cfg: &WorldSampleConfig, x: i32, z: i32) -> i32 {
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

fn cave_carves(cfg: &WorldSampleConfig, p: IVec3, column_height: i32) -> bool {
    if matches!(cfg.world_type, WorldType::Superflat) {
        return false;
    }
    if p.y > column_height || p.y <= cfg.min_world_y() + cfg.blocks_for_span(0.75) {
        return false;
    }

    let seed = cfg.seed;
    let depth = (column_height - p.y) as f32 * cfg.voxel_meters();
    let height_above_water = (column_height - cfg.water_level()) as f32 * cfg.voxel_meters();
    // Protect beaches, shorelines, and shallow lowland turf from surface-breaking
    // cave slits and flooded sinkholes; only high hills/mountains allow cave mouths.
    if height_above_water < 9.0 && depth < 5.5 {
        return false;
    }
    if height_above_water < 18.0 && depth < 2.5 {
        return false;
    }
    let mx = cfg.world_x(p.x);
    let my = cfg.world_y(p.y);
    let mz = cfg.world_z(p.z);
    let spaghetti = value_noise3(mx * 0.060, my * 0.095, mz * 0.060, seed.wrapping_add(301));
    let cheese = value_noise3(mx * 0.028, my * 0.040, mz * 0.028, seed.wrapping_add(347));
    let pillar = ridge(value_noise3(
        mx * 0.020,
        my * 0.055,
        mz * 0.020,
        seed.wrapping_add(389),
    ));
    let ravine = ridge(value_noise2(mx * 0.010, mz * 0.010, seed.wrapping_add(571)));
    let ravine_center = cfg.world_y(column_height)
        - 16.0
        - value_noise2(mx * 0.020, mz * 0.020, seed.wrapping_add(593)) * 10.0;
    let ravine_half_h = 4.0 + value_noise2(mx * 0.018, mz * 0.018, seed.wrapping_add(617)) * 4.5;
    let bias = saturate(depth / 24.0) * 0.12;
    let surface_guard = saturate((10.0 - depth) / 10.0) * 0.16;

    spaghetti > 0.86 + surface_guard - bias
        || cheese > 0.80 + surface_guard - bias
        || (depth > 3.5 && pillar > 0.93 + surface_guard * 0.6)
        || (depth > 6.0 && ravine > 0.975 && (my - ravine_center).abs() < ravine_half_h)
}

fn tree_profile_for_cell(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<TreeProfile> {
    generation_cache::tree(cfg, cell_x, cell_z)
}

/// Intermediate woodland uses the same cached spawn decision as editable trees.
/// Dimensions are metres, so quarter-metre worlds do not lose their forest LOD.
pub(crate) struct TreeVisual {
    pub base: Vec3,
    pub height: f32,
    pub radius: f32,
    pub crown_height: f32,
    pub conifer: bool,
    pub seed: u32,
}

pub(crate) fn tree_visual(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<TreeVisual> {
    let tree = tree_profile_for_cell(cfg, cell_x, cell_z)?;
    let scale = cfg.voxel_meters();
    Some(TreeVisual {
        base: Vec3::new(tree.anchor_x as f32, tree.ground_y as f32, tree.anchor_z as f32) * scale,
        height: tree.trunk_h as f32 * scale,
        radius: tree.canopy_radius as f32 * scale,
        crown_height: tree.canopy_height as f32 * scale,
        conifer: tree.conifer,
        seed: (cell_x as u32).wrapping_mul(73856093) ^ (cell_z as u32).wrapping_mul(19349663) ^ cfg.seed,
    })
}

fn tree_profile_uncached(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<TreeProfile> {
    let cell_size = cfg.blocks_for_span(12.0);
    let offset_x = (hash2(cell_x, cell_z, cfg.seed.wrapping_add(401)) * cell_size as f32) as i32;
    let offset_z = (hash2(cell_x, cell_z, cfg.seed.wrapping_add(419)) * cell_size as f32) as i32;
    let anchor_x = cell_x * cell_size + offset_x.min(cell_size - 1);
    let anchor_z = cell_z * cell_size + offset_z.min(cell_size - 1);

    let column = sample_column(cfg, anchor_x, anchor_z);
    let ground_y = column.height;
    let woodland_soil = cfg.generator_version >= 11
        && matches!(column.surface, BlockId::Dirt | BlockId::Mud | BlockId::Gravel)
        && column.vegetation_density > 0.20;
    if column.surface != BlockId::Grass
        && !(cfg.generator_version >= 2 && column.surface == BlockId::SnowGrass)
        && !woodland_soil
    {
        return None;
    }

    let chance = hash2(cell_x, cell_z, cfg.seed.wrapping_add(433));
    if chance
        > column.vegetation_density
            * if cfg.generator_version >= 2 {
                0.88
            } else {
                0.22
            }
    {
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
    if slope > cfg.blocks_for_span(0.9) || ground_y <= cfg.water_level() + cfg.blocks_for_span(2.2).max(2) {
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
    };
    if cfg.generator_version >= 6 {
        trees::profile(cfg, &mut tree);
    } else if cfg.generator_version >= 5 {
        vegetation::shape_profile(cfg, &mut tree);
    }
    Some(tree)
}

fn bush_profile_for_cell(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<BushProfile> {
    generation_cache::bush(cfg, cell_x, cell_z)
}

fn bush_profile_uncached(cfg: &WorldSampleConfig, cell_x: i32, cell_z: i32) -> Option<BushProfile> {
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

    let column = sample_column(cfg, anchor_x, anchor_z);
    let ground_y = column.height;
    if column.surface != BlockId::Grass
        || column.vegetation_density <= 0.24
        || ground_y <= cfg.water_level() + 1
    {
        return None;
    }

    let chance = hash2(cell_x, cell_z, cfg.seed.wrapping_add(461));
    if chance > column.vegetation_density * 0.26 {
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

    let base_radius = if hash2(cell_x, cell_z, cfg.seed.wrapping_add(487)) > 0.58 {
        cfg.blocks_for_span(1.6)
    } else {
        cfg.blocks_for_span(1.1)
    };
    let top_dy = if hash2(cell_x, cell_z, cfg.seed.wrapping_add(503)) > 0.74 {
        cfg.blocks_for_span(1.7)
    } else {
        cfg.blocks_for_span(1.1)
    };

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

fn bush_contains(cfg: &WorldSampleConfig, bush: BushProfile, p: IVec3) -> bool {
    let dy = p.y - (bush.ground_y + 1);
    if dy < 0 || dy > bush.top_dy {
        return false;
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
        let shape = (dx as f32 / r).powi(2) + (dz as f32 / (r * 0.84)).powi(2)
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

fn structure_block(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> BlockId {
    if p.y <= column.height || cfg.world_type == WorldType::Superflat {
        return BlockId::Air;
    }

    let cell_size = cfg.blocks_for_span(12.0);
    let base_cell_x = floor_div(p.x, cell_size);
    let base_cell_z = floor_div(p.z, cell_size);

    for cell_z in (base_cell_z - 2)..=(base_cell_z + 2) {
        for cell_x in (base_cell_x - 2)..=(base_cell_x + 2) {
            let Some(tree) = tree_profile_for_cell(cfg, cell_x, cell_z) else {
                continue;
            };
            let block = tree_block(cfg, tree, p);
            if block != BlockId::Air {
                return block;
            }
        }
    }

    let bush_cell_size = cfg.blocks_for_span(8.0);
    let bush_cell_x = floor_div(p.x, bush_cell_size);
    let bush_cell_z = floor_div(p.z, bush_cell_size);
    for cell_z in (bush_cell_z - 1)..=(bush_cell_z + 1) {
        for cell_x in (bush_cell_x - 1)..=(bush_cell_x + 1) {
            if let Some(bush) = bush_profile_for_cell(cfg, cell_x, cell_z) {
                if bush_contains(cfg, bush, p) {
                    return BlockId::Bush;
                }
            }
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

pub fn generate_padded(
    cfg: &WorldSampleConfig,
    coord: IVec3,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Option<crate::mesh::GeneratedChunk> {
    use crate::mesh::{GeneratedChunk, PADDED_CHUNK_SIZE};
    let size = PADDED_CHUNK_SIZE;
    let origin = coord * CHUNK_SIZE - IVec3::ONE;
    let mut blocks = vec![BlockId::Air; (size * size * size) as usize];
    let mut sky_ceiling = vec![0; (size * size) as usize];
    for z in 0..size {
        for x in 0..size {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }
            let column = sample_column(cfg, origin.x + x, origin.z + z);
            sky_ceiling[(z * size + x) as usize] = column.height;
            for y in 0..size {
                blocks[((y * size + z) * size + x) as usize] =
                    sample_block_in_column(cfg, origin + IVec3::new(x, y, z), column);
            }
        }
    }
    let mut chunk = GeneratedChunk::new(coord, blocks);
    chunk.voxel_meters = cfg.voxel_meters();
    for z in 0..size { for x in 0..size {
        let i = (z * size + x) as usize;
        if sky_ceiling[i] < origin.y + size {
            sky_ceiling[i] = chunk.local_roof(x,z);
        }
    } }
    chunk.sky_ceiling = sky_ceiling;
    Some(chunk)
}

fn sample_block_in_column(cfg: &WorldSampleConfig, p: IVec3, column: ColumnSample) -> BlockId {
    if p.y < cfg.min_world_y() || p.y >= cfg.max_world_y() {
        return BlockId::Air;
    }

    if cfg.generator_version >= 3 {
        // This patch belongs to generation, so rendering and collision agree.
        let m = p.as_vec3() * cfg.voxel_meters();
        if cfg.world_type != WorldType::Superflat && m.x.abs() < 10.0 && m.z.abs() < 10.0 {
            if p.y > column.height { return BlockId::Air; }
            if p.y >= column.height - cfg.blocks_for_span(6.0) {
                return if p.y == column.height {BlockId::Grass} else {BlockId::Dirt};
            }
        }
        if let Some(block) = sites::block(cfg, p).or_else(|| sites::cavern(cfg, p, column.height)) {
            return block;
        }
    }

    if p.y > column.height {
        let structure = structure_block(cfg, p, column);
        if structure != BlockId::Air {
            return structure;
        }
        if p.y <= cfg.water_level() {
            return BlockId::Water;
        }
        return BlockId::Air;
    }

    if cave_carves(cfg, p, column.height) {
        if p.y <= water_table(cfg, p.x, p.z) {
            return BlockId::Water;
        }
        return BlockId::Air;
    }

    let depth = column.height - p.y;
    if column.height <= cfg.water_level() + 1 {
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
    } else if cfg.generator_version >= 2 {
        landscape::geological_stratum(cfg, p, column.height, depth)
    } else {
        BlockId::Stone
    }
}

pub fn suggested_spawn(cfg: &WorldSampleConfig) -> Option<Vec3> {
    if cfg.generator_version >= 3 && cfg.world_type != WorldType::Superflat {
        // The generator reserves a dry 20 m clearing on connected inland terrain.
        let ground = sample_column(cfg,0,0).height;
        return Some(Vec3::new(0.5,ground as f32+1.0,0.5));
    }
    let mut candidates = Vec::new();
    let probe_stride = cfg.blocks_for_span(6.0);
    for radius in 0_i32..24_i32 {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if radius > 0 && dx.abs() != radius && dz.abs() != radius {
                    continue;
                }
                candidates.push((dx * probe_stride, dz * probe_stride));
            }
        }
        for (x, z) in candidates.drain(..) {
            // Sample the column once — avoids 5 redundant sample_column calls per
            // candidate that the original sample_block + terrain_height pattern caused.
            let col = sample_column(cfg, x, z);
            let ground_y = col.height;
            if ground_y <= cfg.water_level() || ground_y < cfg.min_world_y() {
                continue;
            }
            let pos = Vec3::new(x as f32 + 0.5, ground_y as f32 + 1.0, z as f32 + 0.5);
            let (min, max) = player_aabb(pos, cfg.voxel_scale, 1.0, 0.0);
            if max.y >= cfg.max_world_y() as f32 {
                continue;
            }
            let low = min.floor().as_ivec3();
            let high = (max - Vec3::splat(0.001)).floor().as_ivec3();
            let support = (low.z..=high.z).all(|z| {
                (low.x..=high.x).all(|x| sample_block(cfg, IVec3::new(x, ground_y, z)).is_solid())
            });
            let body_ok = (low.z..=high.z).all(|z| {
                (low.y..=high.y).all(|y| {
                    (low.x..=high.x).all(|x| sample_block(cfg, IVec3::new(x, y, z)) == BlockId::Air)
                })
            });
            if support && body_ok {
                return Some(pos);
            }
        }
    }

    None
}

mod resident;

pub struct World {
    pub inventory: inventory::SurvivalInventory,
    pub sandbox: crate::simulation::sandbox::Sandbox,
    pub adventure: adventure::Adventure,
    pub factory: factory::Factory,
    pub expedition: expedition::Expedition,
    pub regions: regions::RegionStream,
    pub water_levels: water::WaterLevels,
    pub activity: activity::Activity,
    pub generator_version: u32,
    sample_cache: std::cell::RefCell<sample_cache::SampleCache>,
    pub chunks: resident::ResidentEdits,
    snapshot_cache: WorldSnapshot,
    snapshot_revision: u64,
    snapshot_range: Option<(IVec3,i32)>,
    full_snapshot_cache: Option<(u64, WorldSnapshot)>,
    pub world_type: WorldType,
    pub voxel_scale: VoxelScalePreset,
    pub seed: u32,
    revision: u64,
}

impl World {
    pub fn new(world_type: WorldType, voxel_scale: VoxelScalePreset) -> Self {
        Self::with_seed(world_type, voxel_scale, default_seed(world_type))
    }

    pub fn with_seed(world_type: WorldType, voxel_scale: VoxelScalePreset, seed: u32) -> Self {
        let regions = regions::RegionStream::new();
        let region_view = regions.view.clone();
        let mut world = Self {
            inventory: { let mut inv = inventory::SurvivalInventory::default(); inv.voxel_m = voxel_scale.meters(); inv },
            regions,
            expedition: expedition::Expedition::default(),
            factory: factory::Factory::default(),
            adventure: adventure::Adventure::default(),
            sandbox: Default::default(),
            generator_version: CURRENT_GENERATOR_VERSION,
            water_levels: water::WaterLevels::default(),
            activity: activity::Activity::default(),
            sample_cache: Default::default(),
            chunks: resident::ResidentEdits::default(),
            snapshot_cache: WorldSnapshot {
                regions: region_view,
                water_levels: Arc::new(HashMap::new()),
                cfg: WorldSampleConfig {
                    generator_version: CURRENT_GENERATOR_VERSION,
                    seed,
                    world_type,
                    voxel_scale,
                },
                chunks: Arc::new(HashMap::new()),
            },
            snapshot_revision: u64::MAX,
            snapshot_range: None,
            full_snapshot_cache: None,
            world_type,
            voxel_scale,
            seed,
            revision: 0,
        };
        world.sync_from_inventory();
        world
    }

    /// Synchronizes legacy resource/kit/ration/weapon fields from the authoritative survival inventory.
    pub fn sync_from_inventory(&mut self) {
        use inventory::{FoodKind, Item};
        for i in 0..factory::RESOURCE_COUNT {
            let whole = self.inventory.count_item(Item::Resource(i as u8));
            let rem = self.inventory.resource_remainders[i] % 64;
            self.expedition.resources[i] = whole.saturating_mul(64).saturating_add(rem);
        }
        for kind in factory::Kind::ALL {
            self.factory.kits[kind as usize] =
                self.inventory.count_item(Item::Block(kind.block()));
        }
        self.adventure.rations = self
            .inventory
            .count_item(Item::Food(FoodKind::FungalRation));
        self.adventure.weapons=(1..=4u8).fold(0,|mask,tier|mask|if self.inventory.count_item(Item::Weapon(tier))>0 {1<<(tier-1)}else{0});
        if self.adventure.weapon != 0 && !self.adventure.owns_weapon(self.adventure.weapon) {
            self.adventure.weapon = 0;
        }
        if self.inventory.has_lumen_helm() {
            self.expedition.lamp = true;
        }
    }

    /// Reconciles changes made by expedition/factory/adventure actions into the survival slot inventory.
    pub fn reconcile_legacy_to_inventory(&mut self) {
        use inventory::{FoodKind, Item, ItemStack};
        for i in 0..factory::RESOURCE_COUNT {
            let desired_whole = self.expedition.resources[i] / 64;
            let desired_rem = self.expedition.resources[i] % 64;
            self.inventory.resource_remainders[i] = desired_rem;
            let item = Item::Resource(i as u8);
            let current_whole = self.inventory.count_item(item);
            if desired_whole > current_whole {
                let delta = desired_whole - current_whole;
                let (inserted, rem) = self.inventory.try_insert(ItemStack { item, count: delta });
                if inserted > 0 {
                    self.inventory.notify_pickup(item, inserted);
                }
                if rem > 0 {
                    self.inventory.notify_overflow(item);
                }
                self.expedition.resources[i] =
                    (current_whole + inserted).saturating_mul(64) + desired_rem;
            } else if desired_whole < current_whole {
                let _ = self.inventory.remove_item(item, current_whole - desired_whole);
            }
        }

        for kind in factory::Kind::ALL {
            let idx = kind as usize;
            let item = Item::Block(kind.block());
            let desired = self.factory.kits[idx];
            let current = self.inventory.count_item(item);
            if desired > current {
                let (inserted, rem) = self.inventory.try_insert(ItemStack {
                    item,
                    count: desired - current,
                });
                if inserted > 0 {
                    self.inventory.notify_pickup(item, inserted);
                }
                if rem > 0 {
                    self.inventory.notify_overflow(item);
                }
                self.factory.kits[idx] = current + inserted;
            } else if desired < current {
                let _ = self.inventory.remove_item(item, current - desired);
            }
        }

        let ration_item = Item::Food(FoodKind::FungalRation);
        let desired_rations = self.adventure.rations;
        let current_rations = self.inventory.count_item(ration_item);
        if desired_rations > current_rations {
            let (inserted, rem) = self.inventory.try_insert(ItemStack {
                item: ration_item,
                count: desired_rations - current_rations,
            });
            if inserted > 0 {
                self.inventory.notify_pickup(ration_item, inserted);
            }
            if rem > 0 {
                self.inventory.notify_overflow(ration_item);
            }
            self.adventure.rations = current_rations + inserted;
        } else if desired_rations < current_rations {
            let _ = self
                .inventory
                .remove_item(ration_item, current_rations - desired_rations);
        }

        for tier in [1u8, 2u8, 3u8, 4u8] {
            let item = Item::Weapon(tier);
            let owns = (self.adventure.weapons & (1 << (tier - 1))) != 0;
            let count = self.inventory.count_item(item);
            if owns && count == 0 {
                let (inserted, _) = self.inventory.try_insert(ItemStack::new(item, 1));
                if inserted > 0 {
                    self.inventory.notify_pickup(item, 1);
                }
            }
        }
    }

    /// Migrates pre-V13 saves into the survival inventory without losing resources, kits, rations, or weapons.
    pub fn migrate_legacy_save_to_inventory(&mut self) {
        self.inventory = inventory::SurvivalInventory::new_starter();
        self.inventory.voxel_m = self.voxel_scale.meters();
        // Match starter ration count before reconciling so legacy ration count is respected exactly.
        let starter_ration = inventory::Item::Food(inventory::FoodKind::FungalRation);
        let current = self.inventory.count_item(starter_ration);
        if self.adventure.rations < current {
            let _ = self
                .inventory
                .remove_item(starter_ration, current - self.adventure.rations);
        }
        self.reconcile_legacy_to_inventory();
        self.sync_from_inventory();
    }

    pub fn eat_food(
        &mut self,
        food: inventory::FoodKind,
        preferred_hotbar: Option<usize>,
    ) -> bool {
        let max_hp = self.adventure.max_health();
        if self.adventure.hunger >= 99.5 && self.adventure.health >= max_hp - 0.1 {
            self.expedition.message = "Already well-fed.".into();
            self.expedition.message_seconds = 2.0;
            return false;
        }
        let item = inventory::Item::Food(food);
        let consumed = if let Some(slot) = preferred_hotbar {
            self.inventory.consume_from_hotbar(slot, item)
        } else {
            self.inventory.remove_item(item, 1)
        };
        if !consumed {
            return false;
        }
        self.adventure.hunger = (self.adventure.hunger + food.hunger_restore()).min(100.0);
        self.adventure.health = (self.adventure.health + food.health_restore()).min(max_hp);
        self.inventory.eat_timer = 1.35;
        self.sync_from_inventory();
        self.expedition.message = format!(
            "Ate {} (+{:.0} hunger, +{:.0} health).",
            food.name(),
            food.hunger_restore(),
            food.health_restore()
        );
        self.expedition.message_seconds = 2.4;
        true
    }

    pub fn sample_config(&self) -> WorldSampleConfig {
        WorldSampleConfig {
            generator_version: self.generator_version,
            seed: self.seed,
            world_type: self.world_type,
            voxel_scale: self.voxel_scale,
        }
    }

    pub(super) fn invalidate_snapshot_at(&mut self,c:IVec3) {
        self.full_snapshot_cache = None;
        if self.snapshot_range.is_none_or(|(center,radius)| c.x.abs_diff(center.x).max(c.z.abs_diff(center.z))<=(radius+1) as u32) {self.snapshot_revision=u64::MAX;}
    }
    pub fn snapshot(&mut self) -> WorldSnapshot {
        // Horizon and nearby streaming request different views each frame.
        // Keep their maps separate rather than rebuilding both on every call.
        let cfg = self.sample_config();
        if self.full_snapshot_cache.as_ref().is_none_or(|(revision, cached)|
            *revision != self.revision || cached.cfg != cfg)
        {
            self.full_snapshot_cache = Some((self.revision, WorldSnapshot {
                regions: self.regions.view.clone(),
                water_levels: Arc::new(self.water_levels.iter().map(|(&p,&v)|(p,v)).collect()),
                cfg,
                chunks: Arc::new(self.chunks.clone()),
            }));
        }
        let cached = &mut self.full_snapshot_cache.as_mut().unwrap().1;
        // Region publications can change without resident voxel mutations.
        // Replace reader pins without copying unchanged chunk/water maps.
        cached.regions = self.regions.view.clone();
        cached.clone()
    }

    pub fn get(&self, p: IVec3) -> BlockId {
        let chunk_coord = chunk_coord_from_world_block(p);
        if let Some(chunk) = self.chunks.get(&chunk_coord) {
            chunk.blocks[ChunkData::local_index(p.x, p.y, p.z)]
        } else if self.regions.view.records.contains_key(&chunk_coord) {
            self.regions
                .view
                .load(chunk_coord)
                .ok()
                .flatten()
                .map_or(BlockId::Stone, |c| {
                    c.chunk.blocks[ChunkData::local_index(p.x, p.y, p.z)]
                })
        } else {
            self.sample_cache.borrow_mut().get(self.sample_config(), p)
        }
    }

    pub fn set(&mut self, p: IVec3, id: BlockId) {
        self.set_many(&[(p, id)]);
    }

    /// Eight volume units per voxel. Unedited procedural water starts full.
    pub fn water_volume(&self, p: IVec3) -> u8 {
        if self.get(p) != BlockId::Water {
            return 0;
        }
        self.water_levels
            .get(&p)
            .copied()
            .or_else(|| {
                let c = chunk_coord_from_world_block(p);
                if self.chunks.contains_key(&c) {
                    None
                } else {
                    self.regions
                        .view
                        .load(c)
                        .ok()
                        .flatten()
                        .and_then(|data| data.water.get(&p).copied())
                }
            })
            .unwrap_or(8)
    }

    pub fn set_water_volume(&mut self, p: IVec3, volume: u8) {
        let cfg=self.sample_config();
        if p.y<cfg.min_world_y() || p.y>=cfg.max_world_y() || self.factory.nodes.contains_key(&factory::anchor(p,factory::step(self))) {return;}
        let volume = volume.min(8);
        if self.water_volume(p) == volume {
            return;
        }
        let coord = chunk_coord_from_world_block(p);
        if self.resident_edit(coord).is_err() {
            return;
        }
        self.set(
            p,
            if volume == 0 {
                BlockId::Air
            } else {
                BlockId::Water
            },
        );
        if volume > 0 && volume < 8 {
            self.water_levels.insert(p, volume);
        } else {
            if let Some(cell) = self.sandbox.cells.get_mut(&crate::simulation::sandbox::key(p)) { cell.fuel=0.0; cell.gas=0; }
            self.water_levels.remove(&p);
        }
        self.regions.mark_dirty(chunk_coord_from_world_block(p));
        self.activity.wake_neighbors(p);
        self.invalidate_snapshot_at(chunk_coord_from_world_block(p));
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn set_many(&mut self, edits: &[(IVec3, BlockId)]) {
        let mut changed = false;
        for &(p, id) in edits {
            // Assemblies are dismantled as one metre-sized object. Builder and
            // physics edits cannot silently orphan their inventories or networks.
            if self.factory.nodes.contains_key(&factory::anchor(p, factory::step(self))) {
                continue;
            }
            let cfg = self.sample_config();
            if p.y < cfg.min_world_y() || p.y >= cfg.max_world_y() {
                continue;
            }
            let current = self.get(p);
            if current == id {
                continue;
            }

            let chunk_coord = chunk_coord_from_world_block(p);
            if self.resident_edit(chunk_coord).is_err() {
                // A missing/corrupt edited record must never become procedural data.
                continue;
            }
            if let Some(cell) = self.sandbox.cells.get_mut(&crate::simulation::sandbox::key(p)) { cell.fuel=0.0; cell.gas=0; }
            self.water_levels.remove(&p);
            self.regions.mark_dirty(chunk_coord);

            if let Some(mut chunk) = self.chunks.get_mut(&chunk_coord) {
                let chunk_mut = Arc::make_mut(&mut chunk);
                chunk_mut.blocks[ChunkData::local_index(p.x, p.y, p.z)] = id;
                if id.is_torch() {
                    chunk_mut.torches.insert(p);
                } else {
                    chunk_mut.torches.remove(&p);
                }
            }

            self.activity.wake_neighbors(p);
        self.invalidate_snapshot_at(chunk_coord_from_world_block(p));
            if id.is_torch() {
                propagate_light(self, p, 14);
            }
            changed = true;
        }

        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn solid(&self, p: IVec3) -> bool {
        self.get(p).is_solid()
    }

    pub fn terrain_height_at(&self, x: i32, z: i32) -> i32 {
        terrain_height(&self.sample_config(), x, z)
    }

    pub fn suggested_spawn(&mut self) -> Vec3 {
        let cfg = self.sample_config();
        if let Some(spawn) = suggested_spawn(&cfg) {
            return spawn;
        }
        // An all-water or obstructed search area must still produce a safe
        // playable world. Generate a small dry support area only in that case.
        let body_height = player_height_blocks(self.voxel_scale, 1.0).ceil() as i32;
        let radius = (player_width_blocks(self.voxel_scale, 1.0) * 0.5).ceil() as i32 + 1;
        let feet_y = (terrain_height(&cfg, 0, 0).max(cfg.water_level()) + 2)
            .clamp(cfg.min_world_y() + 1, cfg.max_world_y() - body_height - 2);
        let mut edits = Vec::new();
        for z in -radius..=radius {
            for x in -radius..=radius {
                edits.push((IVec3::new(x, feet_y - 1, z), BlockId::Stone));
                for y in feet_y..=feet_y + body_height {
                    edits.push((IVec3::new(x, y, z), BlockId::Air));
                }
            }
        }
        self.set_many(&edits);
        Vec3::new(0.5, feet_y as f32, 0.5)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

pub fn propagate_light(world: &mut World, start_pos: IVec3, light_level: u8) {
    let mut queue = std::collections::VecDeque::new();
    queue.push_back((start_pos, light_level));

    while let Some((pos, level)) = queue.pop_front() {
        if level <= 1 {
            continue;
        }

        let neighbors = [
            pos + IVec3::X,
            pos - IVec3::X,
            pos + IVec3::Y,
            pos - IVec3::Y,
            pos + IVec3::Z,
            pos - IVec3::Z,
        ];

        for n in neighbors {
            let chunk_coord = chunk_coord_from_world_block(n);
            if let Some(mut chunk) = world.chunks.get_mut(&chunk_coord) {
                let chunk_mut = Arc::make_mut(&mut chunk);
                let idx = ChunkData::local_index(n.x, n.y, n.z);

                if !chunk_mut.blocks[idx].is_solid() {
                    let current_light = chunk_mut.light[idx] & 0x0F;
                    let new_light = level - 1;
                    if new_light > current_light {
                        chunk_mut.light[idx] = (chunk_mut.light[idx] & 0xF0) | new_light;
                        world.regions.mark_dirty(chunk_coord);
                        queue.push_back((n, new_light));
                    }
                }
            }
        }
    }
}

fn default_seed(world_type: WorldType) -> u32 {
    match world_type {
        WorldType::Superflat => 0x51f1_001,
        WorldType::Hills => 0x51f1_002,
        WorldType::Ridged => 0x51f1_003,
    }
}

pub fn floor_div(a: i32, b: i32) -> i32 {
    let mut q = a / b;
    let r = a % b;
    if r != 0 && ((r > 0) != (b > 0)) {
        q -= 1;
    }
    q
}

pub fn chunk_coord_from_world_block(p: IVec3) -> IVec3 {
    IVec3::new(
        floor_div(p.x, CHUNK_SIZE),
        floor_div(p.y, CHUNK_SIZE),
        floor_div(p.z, CHUNK_SIZE),
    )
}

pub fn mark_chunk_and_neighbors_dirty(dirty: &mut HashSet<IVec3>, block: IVec3) {
    let center = chunk_coord_from_world_block(block);
    let local = block.rem_euclid(IVec3::splat(CHUNK_SIZE));
    // Face culling and ambient occlusion read a complete one-voxel halo,
    // including diagonals at chunk edges and corners.
    for z in -1..=1 {
        for y in -1..=1 {
            for x in -1..=1 {
                let offset = IVec3::new(x, y, z);
                let touches = (0..3).all(|axis| match offset[axis] {
                    -1 => local[axis] == 0,
                    1 => local[axis] == CHUNK_SIZE - 1,
                    _ => true,
                });
                if touches {
                    dirty.insert(center + offset);
                }
            }
        }
    }
}
