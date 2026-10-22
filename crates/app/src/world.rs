use glam::{IVec3, Vec3};
use std::collections::HashSet;
use std::sync::Arc;

use crate::fluid::VoxelScalePreset;
pub use crate::material::BlockId;

pub mod activity;
pub mod adventure;
mod collision;
pub mod expedition;
pub mod factory;
pub mod habitats;
pub mod interaction;
pub mod inventory;
pub mod recipe_index;
pub mod mood;
pub mod orbital;
pub mod regions;
mod sample_cache;
pub mod storage;
pub(crate) mod trace;
pub mod undercrown;
mod water;
pub(crate) use water::WaterSnapshot;
mod weather;
pub use collision::{
    collides_world, collides_world_local, player_aabb, raycast_block, raycast_block_precise,
    resolve_penetration_local, try_move_local, try_step_up_local,
};

#[allow(unused_imports)]
pub use weather::{
    CloudWeatherField, CloudWeatherUploadManifest, WEATHER_CLIPMAP_LEVEL_COUNT,
    WEATHER_TEXTURE_RESOLUTION, WeatherClipmapUploadBatch, WeatherRuntimeState, WeatherUploadRect,
};

/// How deep inside a starlit cosmos chamber a point (in metres) is, 0..1.
pub fn cosmos_at(cfg: &WorldSampleConfig, p: Vec3) -> f32 {
    dungeon::cosmos_at(cfg, p)
}

pub use engine_core::{CHUNK_SIZE, chunk_coord_from_world_block};
pub use engine_voxel::{
    CURRENT_GENERATOR_VERSION, ChunkData, WATER_LEVEL_METERS, WorldSampleConfig, WorldType,
};
pub use engine_worldgen::{
    TreeVisual, biome_weights, distant_surface, dungeon, landscape, lore, prism_sanctum, sample_block, sites,
    terrain_height, toll_crossing, tree_visual, tree_visual_lod, zones,
};
use engine_worldgen::{lerp_color, sample_column, saturate, smoothstep};
pub const PLAYER_WIDTH: f32 = 0.6;
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const EYE_HEIGHT: f32 = 1.62;
pub const STEP_HEIGHT: f32 = 0.6;
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

pub fn speed_blocks(voxel_scale: VoxelScalePreset, meters_per_second: f32) -> f32 {
    blocks_from_meters(voxel_scale, meters_per_second)
}

pub fn acceleration_blocks(voxel_scale: VoxelScalePreset, meters_per_second_sq: f32) -> f32 {
    blocks_from_meters(voxel_scale, meters_per_second_sq)
}


pub fn hotbar_block(slot: usize) -> Option<BlockId> {
    crate::material::hotbar_block(slot)
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
    /// x: cosmos chamber (fog becomes starfield), y: aurora, z: meteor shower,
    /// w: rainbow. Zero unless `with_mood` or the camera says otherwise.
    pub mood: [f32; 4],
    /// Whole-frame colour grade for sky events: rgb multiplier, w: amount.
    pub mood_tint: [f32; 4],
    /// Seasonal multipliers for the meadow and canopy material families.
    pub grass_tint: [f32; 3],
    pub foliage_tint: [f32; 3],
    /// Living air: fireflies, pollen, diamond dust, embers. Each 0..1.
    pub ambience: [f32; 4],
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
        let sun_dir = Vec3::new(
            angle.cos() * solar_tilt.cos(),
            angle.sin() * solar_tilt.cos(),
            solar_tilt.sin(),
        );
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
        let advection = cloud_controls.advection.clamp(0.0, 1.0);
        let noise_scale = cloud_controls.noise_scale.clamp(0.0, 1.0);
        let wind_speed = cloud_controls.wind_speed.clamp(0.0, 1.0);
        let detail_strength = cloud_controls.detail_strength.clamp(0.0, 1.0);

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

        let haze =(low_cloudiness * 0.10
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

        let ambient_night = [0.105, 0.125, 0.185];
        // Keep midday fill restrained so the sun still shapes slopes and ridges.
        // The previous value flattened the terrain into a pale blue wash.
        let ambient_day = [0.155, 0.195, 0.268];
        let mut ambient_color = lerp_color(ambient_night, ambient_day, daylight);
        ambient_color = lerp_color(
            ambient_color,
            [0.18, 0.12, 0.14],
            twilight * (1.0 - overcast * 0.68) * 0.18,
        );
        let overcast_ambient = [0.17, 0.19, 0.22];
        ambient_color = lerp_color(ambient_color, overcast_ambient, overcast * daylight * 0.48);
        ambient_color = lerp_color(
            ambient_color,
            [0.065, 0.075, 0.095],
            storminess * daylight * 0.62,
        );

        let (day_top, day_horizon, sunset_hue) = day_sky_palette(day_index, weather_seed);
        let night_top = [0.012, 0.018, 0.048];
        let night_horizon = [0.028, 0.040, 0.085];
        let mut sky_top = lerp_color(night_top, day_top, daylight);
        let mut sky_horizon = lerp_color(night_horizon, day_horizon, daylight);

        // Weather classes move the whole dome. A single gray-blue mix made
        // clear, marine, storm, and winter days look like the same peach sky.
        let overcast_top = [0.30, 0.36, 0.44];
        let overcast_horizon = [0.50, 0.56, 0.62];
        sky_top = lerp_color(
            sky_top,
            lerp_color(night_top, overcast_top, daylight),
            overcast * 0.86,
        );
        sky_horizon = lerp_color(
            sky_horizon,
            lerp_color(night_horizon, overcast_horizon, daylight),
            overcast * 0.90,
        );
        let storm_top = [0.07, 0.10, 0.15];
        let storm_horizon = [0.15, 0.22, 0.23];
        sky_top = lerp_color(
            sky_top,
            lerp_color(night_top, storm_top, daylight),
            storminess * 0.92,
        );
        sky_horizon = lerp_color(
            sky_horizon,
            lerp_color(night_horizon, storm_horizon, daylight),
            storminess * 0.94,
        );
        let winter = saturate(
            stratus * 0.55 + cirrus * 0.45 + altostratus * 0.25
                - cumulus * 0.70
                - cumulonimbus
                - overcast * 0.55,
        );
        let winter_top = [0.42, 0.50, 0.62];
        let winter_horizon = [0.72, 0.78, 0.86];
        sky_top = lerp_color(
            sky_top,
            lerp_color(night_top, winter_top, daylight),
            winter * 0.78,
        );
        sky_horizon = lerp_color(
            sky_horizon,
            lerp_color(night_horizon, winter_horizon, daylight),
            winter * 0.82,
        );
        sky_horizon = lerp_color(
            sky_horizon,
            [0.80, 0.76, 0.66],
            haze * daylight * (1.0 - storminess),
        );
        // Ozone lives in the zenith only. The saturated sunset is painted in
        // the shader as an aureole and an antisolar belt; this horizon shift
        // is just enough for the fog to warm with the sun.
        let ozone = hash01(weather_seed ^ day_index.wrapping_mul(0x6C8E_9CF5));
        sky_top = lerp_color(
            sky_top,
            [
                sky_top[0] * 0.80,
                sky_top[1] * 0.86,
                (sky_top[2] * 1.14).min(1.0),
            ],
            ozone * daylight * (1.0 - overcast) * (1.0 - storminess) * 0.42,
        );
        let sunset_amt = (twilight
            * (1.0 - overcast * 0.92)
            * (1.0 - storminess * 0.78)
            * (1.0 - winter * 0.40))
            .clamp(0.0, 0.34);
        sky_horizon = lerp_color(sky_horizon, sunset_hue, sunset_amt);
        sky_top = lerp_color(
            sky_top,
            [
                sunset_hue[0] * 0.22 + day_top[0] * 0.78,
                sunset_hue[1] * 0.12 + day_top[1] * 0.88,
                day_top[2],
            ],
            sunset_amt * sunset_amt * 0.12,
        );

        let mut fog_color = lerp_color([0.030, 0.040, 0.075], sky_horizon, daylight * 0.90);
        fog_color = lerp_color(fog_color, sky_top, 0.22 + haze * 0.08);
        // Shade takes the colour of this day's sky, so a cobalt noon and a
        // dusty noon do not share the same grey shadows.
        ambient_color = lerp_color(
            ambient_color,
            [
                sky_top[0] * 0.40 + 0.045,
                sky_top[1] * 0.34 + 0.050,
                sky_top[2] * 0.46 + 0.030,
            ],
            daylight * (1.0 - overcast * 0.70) * (1.0 - storminess * 0.40) * 0.64,
        );

        let sun_elev = smoothstep(-0.08, 0.85, sun_height);
        let solar_extinction = 1.0 - smoothstep(-0.18, 0.10, sun_height);
        let mut sun_intensity =
            daylight * (2.1 + 7.9 * sun_elev + twilight * 0.38) * (1.0 - solar_extinction * 0.30);

        let sun_occlusion = saturate(
            low_cloudiness * 0.32
                + mid_cloudiness * 0.18
                + high_cloudiness * 0.08
                + overcast * 0.20
                + storminess * 0.22,
        );
        sun_intensity *= 1.0 - sun_occlusion * 0.65;

        let moon_base = night * smoothstep(-0.16, 0.26, moon_height) * 0.62;
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
            (0.465 + daylight * 0.400 + twilight * 0.22) * (1.0 - overcast * 0.08) + haze * 0.008;
        let saturation =
            (0.84 + daylight * 0.16) * (1.0 - overcast * 0.34 - storminess * 0.24 - stratus * 0.08);

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
            saturation: saturation.clamp(0.40, 1.12),
            fog_height,
            fog_falloff,
            mood: [0.0; 4],
            mood_tint: [1.0, 1.0, 1.0, 0.0],
            grass_tint: [1.0; 3],
            foliage_tint: [1.0; 3],
            ambience: [0.0; 4],
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
    overcast: f32,
    wind_angle: f32,
    wind_speed: f32,
    turbulence: f32,
}

pub(crate) use engine_core::hash::{hash01, hash_u32};

/// Noon zenith, noon horizon, and the low-sun hue for one calendar day.
/// Neighbouring days share a family, then Rayleigh and aerosol shift that family
/// so two cobalt days are not the same sky.
fn day_sky_palette(day_index: u32, weather_seed: u32) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let hue = hash01(weather_seed ^ day_index.rotate_left(9) ^ 0x51A7_C3E1) * 6.0;
    let index = (hue.floor() as usize) % 6;
    let next = (index + 1) % 6;
    let blend = {
        let t = hue.fract();
        t * t * (3.0 - 2.0 * t)
    };
    const PALETTES: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
        (
            [0.030, 0.140, 0.660],
            [0.500, 0.720, 0.960],
            [1.00, 0.46, 0.10],
        ),
        (
            [0.050, 0.300, 0.780],
            [0.580, 0.840, 0.980],
            [1.00, 0.66, 0.32],
        ),
        (
            [0.090, 0.240, 0.460],
            [0.660, 0.760, 0.820],
            [0.98, 0.40, 0.28],
        ),
        (
            [0.200, 0.300, 0.450],
            [0.860, 0.740, 0.520],
            [1.00, 0.48, 0.14],
        ),
        (
            [0.220, 0.380, 0.620],
            [0.800, 0.880, 0.960],
            [0.86, 0.50, 0.66],
        ),
        (
            [0.015, 0.150, 0.820],
            [0.300, 0.580, 0.990],
            [0.96, 0.32, 0.44],
        ),
    ];
    let mut top = lerp_color(PALETTES[index].0, PALETTES[next].0, blend * 0.55);
    let mut horizon = lerp_color(PALETTES[index].1, PALETTES[next].1, blend * 0.55);
    let sunset = lerp_color(PALETTES[index].2, PALETTES[next].2, blend * 0.55);
    let rayleigh = hash01(weather_seed ^ day_index.wrapping_mul(0x27BB_2EE6));
    let aerosol = hash01(weather_seed ^ day_index.wrapping_mul(0x1656_67B1));
    top[0] *= 1.08 - rayleigh * 0.28;
    top[1] *= 0.96 + rayleigh * 0.06;
    top[2] = (top[2] * (0.86 + rayleigh * 0.32)).min(1.0);
    horizon = lerp_color(
        horizon,
        [
            (horizon[0] + 0.06).min(1.0),
            horizon[1],
            (horizon[2] * 0.82).max(0.05),
        ],
        (aerosol - 0.35).clamp(0.0, 1.0) * 0.55,
    );
    (top, horizon, sunset)
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
            overcast: 0.0,
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
            overcast: 0.04 + j1 * 0.06,
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
            overcast: 0.03 + j2 * 0.06,
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
            overcast: 0.08 + j0 * 0.08,
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
            overcast: 0.24 + j0 * 0.16,
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
            overcast: 0.50 + j0 * 0.22,
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
            overcast: 0.32 + j0 * 0.20,
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
            overcast: 0.28 + j0 * 0.22,
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
        .with_mood(self.weather_seed, self.day_index, self.time_of_day)
    }

    /// Review hook: jump to a calendar day (season, sky event, weather bucket).
    pub fn set_day_index(&mut self, day: u32) {
        self.day_index = day;
    }

    /// The sky event running right now, for the HUD announcement.
    pub fn sky_event(&self) -> Option<mood::ActiveEvent> {
        mood::active_event(self.weather_seed, self.day_index, self.time_of_day)
    }
}

#[derive(Clone)]
pub struct WorldSnapshot {
    pub regions: regions::RegionView,
    pub water_levels: Arc<water::WaterSnapshot>,
    pub cfg: WorldSampleConfig,
    pub chunks: Arc<crate::fast_hash::FastMap<IVec3, Arc<ChunkData>>>,
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
}

pub fn suggested_spawn(cfg: &WorldSampleConfig) -> Option<Vec3> {
    if cfg.generator_version >= 3 && cfg.world_type != WorldType::Superflat {
        // The generator reserves a dry 20 m clearing on connected inland terrain.
        let ground = sample_column(cfg, 0, 0).height;
        return Some(Vec3::new(0.5, ground as f32 + 1.0, 0.5));
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

/// Generated terrain for `coord` plus its halo, as the mesher reads it.
pub fn generate_padded(
    cfg: &WorldSampleConfig,
    coord: IVec3,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Option<crate::mesh::GeneratedChunk> {
    let terrain = engine_worldgen::generate_padded(cfg, coord, cancelled)?;
    let mut chunk = crate::mesh::GeneratedChunk::new(coord, terrain.blocks);
    chunk.voxel_meters = cfg.voxel_meters();
    chunk.bed_height = terrain.bed_height;
    chunk.bed_frac = terrain.bed_frac;
    chunk.sky_ceiling = terrain.sky_ceiling;
    Some(chunk)
}

mod resident;

pub struct World {
    pub habitats: habitats::State,
    pub orbital: orbital::State,
    pub inventory: inventory::SurvivalInventory,
    pub sandbox: crate::simulation::sandbox::Sandbox,
    pub adventure: adventure::Adventure,
    pub undercrown: undercrown::State,
    pub factory: factory::Factory,
    pub expedition: expedition::Expedition,
    pub regions: regions::RegionStream,
    pub water_levels: water::WaterLevels,
    /// Runtime-only surface ripples. Not saved; rebuilt from impacts.
    pub ripples: crate::fluid::ripples::RippleField,
    pub activity: activity::Activity,
    pub generator_version: u32,
    sample_cache: std::cell::RefCell<sample_cache::SampleCache>,
    pub chunks: resident::ResidentEdits,
    snapshot_cache: WorldSnapshot,
    snapshot_revision: u64,
    snapshot_range: Option<(IVec3, i32)>,
    full_snapshot_cache: Option<(u64, WorldSnapshot)>,
    pub world_type: WorldType,
    pub voxel_scale: VoxelScalePreset,
    pub seed: u32,
    revision: u64,
}

impl World {
    #[cfg(test)]
    pub fn new(world_type: WorldType, voxel_scale: VoxelScalePreset) -> Self {
        Self::with_seed(world_type, voxel_scale, default_seed(world_type))
    }

    pub fn with_seed(world_type: WorldType, voxel_scale: VoxelScalePreset, seed: u32) -> Self {
        let regions = regions::RegionStream::new();
        let region_view = regions.view.clone();
        let mut world = Self {
            habitats: habitats::State::default(),
            orbital: orbital::State::default(),
            inventory: {
                let mut inv = inventory::SurvivalInventory::default();
                inv.voxel_m = voxel_scale.meters();
                inv
            },
            regions,
            expedition: expedition::Expedition::default(),
            factory: factory::Factory::default(),
            adventure: adventure::Adventure::default(),
            undercrown: undercrown::State::default(),
            sandbox: Default::default(),
            generator_version: CURRENT_GENERATOR_VERSION,
            water_levels: water::WaterLevels::default(),
            ripples: Default::default(),
            activity: activity::Activity::default(),
            sample_cache: Default::default(),
            chunks: resident::ResidentEdits::default(),
            snapshot_cache: WorldSnapshot {
                regions: region_view,
                water_levels: Arc::default(),
                cfg: WorldSampleConfig {
                    generator_version: CURRENT_GENERATOR_VERSION,
                    seed,
                    world_type,
                    voxel_scale,
                },
                chunks: Arc::default(),
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
        // Noita starts the player with a wand. The old crafted/altar-only path
        // left new players unarmed with no visible route to spellcasting.
        world.ensure_starter_wand();
        world
    }

    /// Grants the first wand to a fresh world or to a pre-wand save. Later
    /// chassis upgrades keep their existing altar/workbench progression.
    /// Returns true only when a wand was actually added.
    pub fn ensure_starter_wand(&mut self) -> bool {
        if self.adventure.wand > 0 {
            return false;
        }
        self.adventure.wand = 1;
        let log = self.adventure.spell_log;
        adventure::wands::seed_unlocked_slots_known(&mut self.adventure.loadout, 1, &|id| {
            log.grants(id)
        });
        true
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
            self.factory.kits[kind as usize] = self.inventory.count_item(Item::Block(kind.block()));
        }
        self.adventure.rations = self
            .inventory
            .count_item(Item::Food(FoodKind::FungalRation));
        self.adventure.weapons = (1..=5u8).fold(0, |mask, tier| {
            mask | if self.inventory.count_item(Item::Weapon(tier)) > 0 {
                1 << (tier - 1)
            } else {
                0
            }
        });
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
                let _ = self
                    .inventory
                    .remove_item(item, current_whole - desired_whole);
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

        for tier in [1u8, 2u8, 3u8, 4u8, 5u8] {
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

    pub fn eat_food(&mut self, food: inventory::FoodKind, preferred_hotbar: Option<usize>) -> bool {
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

    pub(super) fn invalidate_snapshot_at(&mut self, c: IVec3) {
        self.full_snapshot_cache = None;
        if self.snapshot_range.is_none_or(|(center, radius)| {
            c.x.abs_diff(center.x).max(c.z.abs_diff(center.z)) <= (radius + 1) as u32
        }) {
            self.snapshot_revision = u64::MAX;
        }
    }
    pub fn snapshot(&mut self) -> WorldSnapshot {
        // Horizon and nearby streaming request different views each frame.
        // Keep their maps separate rather than rebuilding both on every call.
        let cfg = self.sample_config();
        if self
            .full_snapshot_cache
            .as_ref()
            .is_none_or(|(revision, cached)| *revision != self.revision || cached.cfg != cfg)
        {
            self.full_snapshot_cache = Some((
                self.revision,
                WorldSnapshot {
                    regions: self.regions.view.clone(),
                    water_levels: Arc::new(self.water_levels.snapshot_all()),
                    cfg,
                    chunks: Arc::new(self.chunks.clone()),
                },
            ));
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

    fn write_water_cell(&mut self, p: IVec3, volume: u8) -> Option<IVec3> {
        let cfg = self.sample_config();
        if p.y < cfg.min_world_y()
            || p.y >= cfg.max_world_y()
            || self
                .factory
                .nodes
                .contains_key(&factory::anchor(p, factory::step(self)))
        {
            return None;
        }
        let coord = chunk_coord_from_world_block(p);
        if self.resident_edit(coord).is_err() {
            return None;
        }
        let volume = volume.min(8);
        let id = if volume == 0 {
            BlockId::Air
        } else {
            BlockId::Water
        };
        let current = self.chunks.get(&coord).map_or(BlockId::Air, |chunk| {
            chunk.blocks[ChunkData::local_index(p.x, p.y, p.z)]
        });
        if current != id {
            if let Some(mut chunk) = self.chunks.get_mut(&coord) {
                let chunk = Arc::make_mut(&mut chunk);
                chunk.blocks[ChunkData::local_index(p.x, p.y, p.z)] = id;
                chunk.note_modified(p,current,id);
            }
            if current == BlockId::Fire {
                self.sandbox.clear_fire(p);
            }
            self.water_levels.remove_velocity(&p);
            if let Some(cell) = self
                .sandbox
                .cells
                .get_mut(&crate::simulation::sandbox::key(p))
            {
                cell.fuel = 0.0;
                cell.gas = 0;
            }
        }
        if volume > 0 && volume < 8 {
            self.water_levels.insert(p, volume);
        } else {
            self.water_levels.remove(&p);
            if volume == 0 {
                self.water_levels.remove_velocity(&p);
            }
            // Full water and air are not thermal carriers. Partial water keeps
            // any temperature associated with the cell.
            if let Some(cell) = self
                .sandbox
                .cells
                .get_mut(&crate::simulation::sandbox::key(p))
            {
                cell.fuel = 0.0;
                cell.gas = 0;
            }
        }
        self.regions.mark_dirty(coord);
        Some(coord)
    }

    /// Commit both halves of one conservative water transfer as a single
    /// transaction. The cellular solver rewrites two cells per move; routing
    /// each through `set_water_volume` duplicated revision bumps, snapshot
    /// invalidation, sandbox cleanup and ten-chunk wake sweeps. Those costs
    /// dominated wide or deep water scenes even though the rule work was tiny.
    pub fn apply_water_pair(
        &mut self,
        from: IVec3,
        from_volume: u8,
        to: IVec3,
        to_volume: u8,
        moved: u8,
    ) {
        if from == to || moved == 0 {
            return;
        }
        let from_chunk = chunk_coord_from_world_block(from);
        let to_chunk = chunk_coord_from_world_block(to);
        if from_chunk == to_chunk && self.chunks.contains_key(&from_chunk) {
            // Most flow is intra-chunk. Commit both cells through one mutable
            // chunk borrow, one dirty mark and one snapshot invalidation, all
            // still under the same bounds/assembly admission as the general
            // path.
            let cfg = self.sample_config();
            let step = factory::step(self);
            let valid = [from, to].iter().all(|&p| {
                p.y >= cfg.min_world_y()
                    && p.y < cfg.max_world_y()
                    && !self.factory.nodes.contains_key(&factory::anchor(p, step))
            });
            if !valid {
                return;
            }
            let entries = [(from, from_volume.min(8)), (to, to_volume.min(8))];
            let mut block_changed = [false; 2];
            {
                let mut chunk = self.chunks.get_mut(&from_chunk).unwrap();
                let chunk = Arc::make_mut(&mut chunk);
                for (i, &(p, volume)) in entries.iter().enumerate() {
                    let id = if volume == 0 {
                        BlockId::Air
                    } else {
                        BlockId::Water
                    };
                    let index = ChunkData::local_index(p.x, p.y, p.z);
                    let previous=chunk.blocks[index];
                    block_changed[i] = previous != id;
                    chunk.blocks[index] = id;
                    if block_changed[i] { chunk.note_modified(p,previous,id); }
                }
            }
            for (i, &(p, volume)) in entries.iter().enumerate() {
                if volume > 0 && volume < 8 {
                    self.water_levels.insert(p, volume);
                } else {
                    self.water_levels.remove(&p);
                    if volume == 0 {
                        self.water_levels.remove_velocity(&p);
                    }
                }
                if block_changed[i] {
                    self.water_levels.remove_velocity(&p);
                }
                if (block_changed[i] || volume == 0 || volume == 8)
                    && let Some(cell) = self
                        .sandbox
                        .cells
                        .get_mut(&crate::simulation::sandbox::key(p))
                    {
                        cell.fuel = 0.0;
                        cell.gas = 0;
                    }
            }
            self.regions.mark_dirty(from_chunk);
            self.wake_acting(from);
            self.wake_acting(to);
            self.invalidate_snapshot_at(from_chunk);
            self.revision = self.revision.wrapping_add(1);
            return;
        }
        let Some(from_chunk) = self.write_water_cell(from, from_volume) else {
            return;
        };
        let Some(to_chunk) = self.write_water_cell(to, to_volume) else {
            // The second half could be refused (missing/corrupt resident edit).
            // Restore the first half to its pre-transfer volume.
            self.write_water_cell(from, from_volume.saturating_add(moved));
            return;
        };
        self.wake_acting(from);
        self.wake_acting(to);
        self.invalidate_snapshot_at(from_chunk);
        if to_chunk != from_chunk {
            self.invalidate_snapshot_at(to_chunk);
        }
        self.revision = self.revision.wrapping_add(1);
    }

    /// Nearest exposed water surface to a metric point. Splash effects use the
    /// voxel water volume rather than assuming a flat sea level, so contact
    /// rings sit on partial cells and cave pools as well as the ocean.
    pub(crate) fn water_surface_at(&self, point: Vec3) -> Option<Vec3> {
        water::surface_at(self, point, self.voxel_scale.meters())
    }

    /// Y of the exposed water surface voxel at `column`, searching `hint` and
    /// its vertical neighbours. Roofed or submerged water is not a surface.
    pub(crate) fn water_surface_top(&self, column: glam::IVec2, hint: i32) -> Option<i32> {
        [hint, hint + 1, hint - 1].into_iter().find(|&y| {
            let p = IVec3::new(column.x, y, column.y);
            if self.get(p) != BlockId::Water || self.water_volume(p) == 0 {
                return false;
            }
            let above = self.get(p + IVec3::Y);
            above != BlockId::Water && !above.is_solid()
        })
    }

    /// Put a ripple on the water surface under a metric point. `strength` is
    /// a 0..1 impact magnitude. Returns `false` if there is no exposed water
    /// there or the ripple field is full.
    pub(crate) fn disturb_water_surface(&mut self, point: Vec3, strength: f32) -> bool {
        let voxel = point / self.voxel_scale.meters();
        let column = glam::IVec2::new(voxel.x.floor() as i32, voxel.z.floor() as i32);
        let Some(top) = self.water_surface_top(column, voxel.y.floor() as i32) else {
            return false;
        };
        let amount = crate::fluid::ripples::IMPACT_VOXELS * strength.clamp(0.0, 1.0);
        self.ripples.disturb(column, top, amount)
    }

    /// Advance surface ripples by `dt` seconds against the live water.
    pub(crate) fn advance_ripples(&mut self, dt: f32) {
        // Take the field out so its closure can read the rest of the world.
        let mut ripples = std::mem::take(&mut self.ripples);
        ripples.advance(dt, |column, hint| self.water_surface_top(column, hint));
        self.ripples = ripples;
    }

    pub(crate) fn water_velocity(&self, p: IVec3) -> glam::Vec2 {
        self.water_levels.velocity(p)
    }

    pub(crate) fn set_water_velocity(&mut self, p: IVec3, v: glam::Vec2) {
        self.water_levels.set_velocity(p, v);
    }

    pub(crate) fn remove_water_velocity(&mut self, p: IVec3) {
        self.water_levels.remove_velocity(&p);
    }

    pub fn set_water_volume(&mut self, p: IVec3, volume: u8) {
        let volume = volume.min(8);
        if self.water_volume(p) == volume {
            return;
        }
        let Some(coord) = self.write_water_cell(p, volume) else {
            return;
        };
        self.wake_acting(p);
        self.invalidate_snapshot_at(coord);
        self.revision = self.revision.wrapping_add(1);
    }

    /// Wakes `p` and each neighbour the scheduler can act on (`simulation::acts`).
    /// Neighbours in chunks that are not resident are woken unconditionally:
    /// classifying them would load their region from disk on every edit.
    fn wake_acting(&mut self, p: IVec3) {
        for d in std::iter::once(IVec3::ZERO).chain(activity::WAKE_NEIGHBORS) {
            let q = p + d;
            if !self.chunks.contains_key(&chunk_coord_from_world_block(q))
                || crate::simulation::acts(self, q)
            {
                self.activity.wake(q);
            }
        }
    }

    pub fn set_many(&mut self, edits: &[(IVec3, BlockId)]) {
        let mut changed = false;
        for &(p, id) in edits {
            // Assemblies are dismantled as one metre-sized object. Builder and
            // physics edits cannot silently orphan their inventories or networks.
            if self
                .factory
                .nodes
                .contains_key(&factory::anchor(p, factory::step(self)))
            {
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
            self.sandbox.clear_fire(p);
            if let Some(cell) = self
                .sandbox
                .cells
                .get_mut(&crate::simulation::sandbox::key(p))
            {
                cell.fuel = 0.0;
                cell.gas = 0;
            }
            self.water_levels.remove(&p);
            self.water_levels.remove_velocity(&p);
            if id == BlockId::Fire {
                self.sandbox.note_fire(p);
            }
            self.regions.mark_dirty(chunk_coord);

            if let Some(mut chunk) = self.chunks.get_mut(&chunk_coord) {
                let chunk_mut = Arc::make_mut(&mut chunk);
                chunk_mut.blocks[ChunkData::local_index(p.x, p.y, p.z)] = id;
                chunk_mut.note_modified(p,current,id);
                if id.is_torch() {
                    chunk_mut.torches.insert(p);
                } else {
                    chunk_mut.torches.remove(&p);
                }
            }

            self.wake_acting(p);
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
        // Collision must not depend on a bottom chunk being streamed. Below the
        // authored world floor the player meets an invisible bedrock plane
        // instead of an infinite void.
        if p.y <= self.sample_config().min_world_y() {
            return true;
        }
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

#[cfg(test)]
fn default_seed(world_type: WorldType) -> u32 {
    match world_type {
        WorldType::Superflat => 0x051f_1001,
        WorldType::Hills => 0x051f_1002,
        WorldType::Ridged => 0x051f_1003,
    }
}

#[cfg(test)]
mod undercrown_generation {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use glam::Vec3;

    fn hash_blocks(cfg: &WorldSampleConfig) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for z in (-48..=48).step_by(8) {
            for x in (-48..=48).step_by(8) {
                for y in (-24..=48).step_by(4) {
                    hash ^= sample_block(cfg, IVec3::new(x, y, z)).as_u32() as u64;
                    hash = hash.wrapping_mul(0x100000001b3);
                }
            }
        }
        hash
    }

    #[test]
    fn generator_19_authors_outside_the_locked_grid() {
        let base = WorldSampleConfig {
            generator_version: 18,
            seed: 17,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
        };
        let next = WorldSampleConfig {
            generator_version: undercrown::FIRST_VERSION,
            ..base
        };
        let locked = hash_blocks(&base);
        assert_eq!(locked, hash_blocks(&base));
        assert!(undercrown::plan(&base).is_empty());
        let authored = undercrown::plan(&next);
        assert!(!authored.is_empty());
        assert!(!undercrown::reserved(&next, Vec3::ZERO));
        assert!(undercrown::reserved(&next, authored.anchor));
        assert!(authored.anchor.x > 48.0);
    }

    #[test]
    fn fresh_world_starts_with_a_seeded_lumen_twig_and_can_upgrade_unarmed_saves() {
        let mut world = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 17);
        assert_eq!(world.adventure.wand, 1);
        assert_ne!(world.adventure.loadout.slot(0), adventure::wands::EMPTY);
        assert!(!world.ensure_starter_wand());

        // A loaded pre-wand save reaches the same state through the explicit
        // migration hook used by storage.
        world.adventure.wand = 0;
        world.adventure.loadout = Default::default();
        assert!(world.ensure_starter_wand());
        assert_eq!(world.adventure.wand, 1);
        assert_ne!(world.adventure.loadout.slot(2), adventure::wands::EMPTY);
        assert!(!world.ensure_starter_wand());
    }

    #[test]
    fn collision_has_a_solid_floor_at_and_below_the_world_minimum() {
        for scale in [
            VoxelScalePreset::OneMeter,
            VoxelScalePreset::HalfMeter,
            VoxelScalePreset::QuarterMeter,
        ] {
            let world = World::with_seed(WorldType::Superflat, scale, 17);
            let min = world.sample_config().min_world_y();
            assert!(world.solid(IVec3::new(0, min, 0)));
            assert!(world.solid(IVec3::new(-3, min - 1, 7)));
        }
    }
}

#[cfg(test)]
mod sky_variety {
    use super::*;

    fn color_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
        (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
    }

    fn luma(color: [f32; 3]) -> f32 {
        color[0] * 0.2126 + color[1] * 0.7152 + color[2] * 0.0722
    }

    #[test]
    fn noon_sky_changes_with_the_day_and_the_weather() {
        let clear = CloudControls {
            coverage: 0.15,
            density: 0.40,
            cumulus: 0.45,
            stratus: 0.02,
            cumulonimbus: 0.0,
            cirrus: 0.20,
            altostratus: 0.03,
            ..CloudControls::default()
        };
        let mut distinct = Vec::new();
        for day in 0..24 {
            let sky = LightingState::from_time_and_day(0.50, day, 0xA341_316C, clear).sky_top;
            if distinct
                .iter()
                .all(|previous| color_distance(*previous, sky) > 0.06)
            {
                distinct.push(sky);
            }
            assert!(
                sky[2] > sky[0] + 0.12,
                "clear noon zenith should stay blue, got {sky:?}"
            );
        }
        assert!(
            distinct.len() >= 4,
            "expected several day palettes, got {}",
            distinct.len()
        );

        let storm = CloudControls {
            coverage: 0.94,
            density: 0.90,
            cumulus: 0.50,
            stratus: 0.30,
            cumulonimbus: 0.82,
            storm_thickness: 0.92,
            anvil_spread: 0.72,
            cirrus: 0.03,
            ..clear
        };
        let overcast = CloudControls {
            coverage: 0.92,
            density: 0.82,
            cumulus: 0.10,
            stratus: 0.75,
            cumulonimbus: 0.0,
            altostratus: 0.45,
            cirrus: 0.04,
            ..clear
        };
        let winter = CloudControls {
            coverage: 0.62,
            density: 0.60,
            cumulus: 0.10,
            stratus: 0.52,
            cumulonimbus: 0.0,
            cirrus: 0.65,
            altostratus: 0.38,
            ..clear
        };
        let noon = LightingState::from_time_and_day(0.50, 0, 0xA341_316C, clear);
        let storm_sky = LightingState::from_time_and_day(0.50, 0, 0xA341_316C, storm);
        let overcast_sky = LightingState::from_time_and_day(0.50, 0, 0xA341_316C, overcast);
        let winter_sky = LightingState::from_time_and_day(0.50, 0, 0xA341_316C, winter);
        assert!(storm_sky.sky_top[2] + 0.12 < noon.sky_top[2]);
        assert!(luma(storm_sky.sky_top) + 0.03 < luma(noon.sky_top));
        assert!(color_distance(overcast_sky.sky_top, noon.sky_top) > 0.12);
        assert!(color_distance(winter_sky.sky_top, storm_sky.sky_top) > 0.12);
        assert!(color_distance(overcast_sky.sky_horizon, winter_sky.sky_horizon) > 0.08);

        let dusk = LightingState::from_time_and_day(0.72, 0, 0xA341_316C, clear);
        assert!(color_distance(dusk.sky_horizon, noon.sky_horizon) > 0.06);
        assert!(
            dusk.sky_top[2] > dusk.sky_top[0],
            "sunset should leave the zenith blue, got {:?}",
            dusk.sky_top
        );
    }
}

#[cfg(test)]
mod spawn_profile {
    use super::*;
    use crate::fluid::VoxelScalePreset;

    /// `cargo test --release spawn_profile -- --ignored --nocapture`
    /// Height profile of the spawn area, to see whether the visible slabs are generator steps.
    #[test]
    #[ignore]
    fn spawn_height_profile() {
        let cfg = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 2738958700).sample_config();
        let mut line = Vec::new();
        for x in (-120..=120).step_by(2) {
            line.push(terrain_height(&cfg, x, 40));
        }
        let steps: Vec<i32> = line.windows(2).map(|w| w[1] - w[0]).collect();
        let big = steps.iter().filter(|s| s.abs() >= 3).count();
        println!("heights {:?}", &line[..20]);
        println!("steps>=3 voxels: {big} of {}; max step {}", steps.len(), steps.iter().map(|s| s.abs()).max().unwrap());
    }
}

#[cfg(test)]
mod far_ring_seam {
    use super::*;
    use crate::fluid::VoxelScalePreset;

    /// Generator 23 has no step where the far-field shore treatment used to
    /// switch on at 110 m. Heights either side of that radius must agree.
    #[test]
    fn no_step_at_the_old_far_field_ring() {
        let cfg = World::with_seed(WorldType::Hills, VoxelScalePreset::OneMeter, 2738958700).sample_config();
        assert!(cfg.generator_version >= 23);
        let mut worst = 0;
        for i in 0..64 {
            let a = i as f32 / 64.0 * std::f32::consts::TAU;
            let at = |r: f32| {
                let (x, z) = ((a.cos() * r).round() as i32, (a.sin() * r).round() as i32);
                terrain_height(&cfg, x, z)
            };
            // Every adjacent radius pair from 100 m to 120 m, so the old 110 m ring is straddled.
            for r in 100..120 {
                worst = worst.max((at(r as f32 + 1.0) - at(r as f32)).abs());
            }
        }
        assert!(worst <= 1, "ring step of {worst} voxels around the 110 m radius");
    }
}
