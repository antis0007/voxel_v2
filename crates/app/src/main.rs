mod dungeon_review;
mod workshop_review;
mod industrial_review;
use anyhow::Result;
use glam::{DVec2, IVec3, Mat4, Vec3, Vec3Swizzles};
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use winit::{
    dpi::PhysicalSize,
    event::*,
    event_loop::{ControlFlow, DeviceEvents, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowAttributes},
};

use engine_audio as audio;
mod crash_log;
mod altitude;
mod creative;
mod egui_ui;
use engine_core::fast_hash;
mod fluid;
mod headless;
mod geology_cache;
mod relief_cache;
mod ca_window;
mod comfort;
mod gamepad;
mod keybinds;
use keybinds::Action as BindAction;
mod screenshot;
use engine_content as material;
mod material_studio;
mod mesh;
#[cfg(feature = "multiplayer")]
mod multiplayer;
#[path = "multiplayer/social.rs"]
pub mod social;
mod local_commands;
mod dev_panel;
mod world_commands;
mod perf;
mod player;
mod planet;
mod preferences;
mod quality;
mod reference_environment;
mod render;
// TEMP A/B switch, removed before commit.
pub(crate) static AB_OLD: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var_os("VOXEL_AB_OLD").is_some());
mod sandbox_review;
mod receiver_review;
mod shader_sources;
mod simulation;
mod soundscape;
mod space_nav;
mod tool_effects;
mod torch_review;
mod terrain_review;
mod horizon_review;
mod low_spec;
mod loading_report;
#[cfg(test)]
mod repo_layout;
#[cfg(test)]
mod shader_contract_tests;

#[cfg(test)]
mod interaction_tests;
mod wand_review;
mod latency_probe;
mod gameplay_trace;
mod picking_review_gate;
mod viewmodel_review;
mod wand_appearance;
mod wand_style;
mod water_review;
mod river_review;
mod world;
mod manyfold;

use creative::CreativeState;
use egui_ui::{
    EguiUi, LoadingUiState, PhysicsUiState, RenderUiState, TimeUiState, UiAction, UiMode,
};
use fluid::VoxelScalePreset;
use mesh::build_outline_vertices;
use perf::{
    CpuFrameStats, FrameCounters, PerfSample, PerfSnapshot, PerfTracker, snapshot_from_samples,
};
use player::{FlyMode, FpsCounter, Input, Player};
use render::{
    AtmosphereMode, ChunkCache, ChunkStreamer, Gpu, GpuBootstrap, GpuPendingInit, MAX_TORCH_LIGHTS,
    RenderDebugView, TorchLight, build_draw_list_with_coverage, render_distance_settings,
    set_render_distance_settings,
};
use simulation::WorldSimulation;
use world::{
    BlockId, DEFAULT_DAY_LENGTH_SECONDS, DEFAULT_START_TIME_OF_DAY, DayNightCycle, WeatherPreset,
    WeatherRuntimeState, World, WorldType, chunk_coord_from_world_block,
    mark_chunk_and_neighbors_dirty,
};

enum AppMode {
    WorldSelect,
    Paused,
    Playing,
}

enum StartupState {
    WaitingForWindow,
    BuildingRenderer {
        pending: GpuPendingInit,
    },
    BuildingChunkStreamer {
        gpu: Gpu,
    },
    BuildingWorldSimulation {
        gpu: Gpu,
        chunk_streamer: ChunkStreamer,
    },
    Ready {
        gpu: Gpu,
        chunk_streamer: ChunkStreamer,
        fluid_sim: WorldSimulation,
    },
    Failed(String),
}

#[derive(Clone, Copy, Debug)]
struct BenchmarkConfig {
    tollkeeper_review: bool,
    ossuary_review: bool,
    hart_review: Option<(u8, u8)>,
    sandbox_review: bool,
    water_review: bool,
    torch_review: bool,
    reference_review: bool,
    saved_review: bool,
    enabled: bool,
    world_type: WorldType,
    voxel_scale: VoxelScalePreset,
    seed: u32,
    weather_preset: Option<WeatherPreset>,
    width: u32,
    height: u32,
    view_distance_chunks: Option<i32>,
    cpu_mib: Option<usize>,
    gpu_mib: Option<usize>,
    profile_physics: bool,
    adaptive_clouds: bool,
    warmup_seconds: f32,
    duration_seconds: f32,
    motion: BenchmarkMotionMode,
    capture_frames: bool,
    capture_hud: bool,
    capture_interval: f32,
    resize_review: bool,
    review_start_seconds: f32,
    review_time_of_day: Option<f32>,
    review_time_end: Option<f32>,
    review_sky_time: Option<f32>,
    /// Absolute world days for reproducible orbital poses/passes, independent
    /// of the sun and cloud clocks. Held at the start throughout warmup.
    review_orbital_time: Option<f64>,
    review_orbital_time_end: Option<f64>,
    /// Calendar day for reviews of seasons and sky events.
    review_day: Option<u32>,
    terrain_camera: Option<[f32; 5]>,
    terrain_sweep: bool,
    horizon_sweep: bool,
    debug_view: RenderDebugView,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BenchmarkMotionMode {
    Static,
    Orbit,
    CloudReview,
    CloudReviewFar,
    Ascent,
    LodTravel,
}

impl BenchmarkMotionMode {
    fn label(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Orbit => "orbit",
            Self::CloudReview => "cloud-review",
            Self::CloudReviewFar => "cloud-review-far",
            Self::Ascent => "ascent",
            Self::LodTravel => "lod-travel",
        }
    }
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            tollkeeper_review: false,
            ossuary_review: false,
            hart_review: None,
            torch_review: false,
            reference_review: false,
            saved_review: false,
            water_review: false,
            sandbox_review: false,
            enabled: false,
            world_type: WorldType::Planet,
            voxel_scale: VoxelScalePreset::OneMeter,
            seed: 0xA341_316C,
            weather_preset: None,
            width: 1280,
            height: 720,
            view_distance_chunks: None,
            cpu_mib: None,
            gpu_mib: None,
            profile_physics: false,
            adaptive_clouds: false,
            warmup_seconds: 4.0,
            duration_seconds: 8.0,
            motion: BenchmarkMotionMode::Static,
            capture_frames: false,
            capture_hud: false,
            capture_interval: 5.0,
            resize_review: false,
            review_start_seconds: 0.0,
            review_time_of_day: None,
            review_time_end: None,
            review_sky_time: None,
            review_orbital_time: None,
            review_orbital_time_end: None,
            review_day: None,
            terrain_camera: None,
            terrain_sweep: false,
            horizon_sweep: false,
            debug_view: RenderDebugView::None,
        }
    }
}

#[derive(Debug)]
struct BenchmarkState {
    config: BenchmarkConfig,
    auto_started: bool,
    measuring: bool,
    phase_started_at: Instant,
    motion_started_at: Instant,
    motion_anchor_blocks: Vec3,
    lod_travel_profile: Option<(i32, Vec<f32>)>,
    lod_travel_worker: Option<std::thread::JoinHandle<(i32, Vec<f32>)>>,
    cloud_review_phase: usize,
    last_capture_key: usize,
    last_periodic_capture: Option<usize>,
    last_resize_phase: usize,
    last_profile_second: usize,
    measured_samples: Vec<PerfSample>,
    route_timing: Option<perf::route::RouteTiming>,
    horizon_world_switches: u8,
    horizon_route_started_at: Option<Instant>,
    scripted_stream_velocity: Option<Vec3>,
}

fn ui_mode(mode: &AppMode) -> UiMode {
    match mode {
        AppMode::WorldSelect => UiMode::WorldSelect,
        AppMode::Paused => UiMode::Paused,
        AppMode::Playing => UiMode::Playing,
    }
}

fn default_random_seed() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0_u128, |d| d.as_nanos());
    let mut x = (nanos as u64) ^ ((nanos >> 64) as u64) ^ ((std::process::id() as u64) << 32);
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^= x >> 31;
    let seed = (x as u32) ^ ((x >> 32) as u32);
    if seed == 0 { 0xA341_316C } else { seed }
}

static BENCHMARK_GENERATOR_VERSION:std::sync::atomic::AtomicU32=std::sync::atomic::AtomicU32::new(0);

fn new_planet_generator(custom: bool) -> u32 {
    static TERRAIN_OVERRIDE: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    if let Some(version) = *TERRAIN_OVERRIDE.get_or_init(|| {
        if std::env::args().any(|arg| arg == "--dungeon-mechanisms") {
            Some(engine_voxel::DUNGEON_MECHANISMS_GENERATOR)
        } else if std::env::args().any(|arg| arg == "--branching-watersheds") {
            Some(engine_worldgen::planetary::BRANCHING_WATERSHEDS_GENERATOR)
        } else if std::env::args().any(|arg| arg == "--resolved-history") {
            Some(engine_worldgen::planetary::RESOLVED_HISTORY_GENERATOR)
        } else if std::env::args().any(|arg| arg == "--evolved-watersheds") {
            Some(engine_worldgen::planetary::EVOLVED_WATERSHEDS_GENERATOR)
        } else if std::env::args().any(|arg| matches!(arg.as_str(), "--rifted-margins" | "--geological-history")) {
            Some(engine_worldgen::planetary::GEOLOGICAL_HISTORY_GENERATOR)
        } else if std::env::args().any(|arg| arg == "--watershed-terrain") {
            Some(engine_worldgen::planetary::LAYERED_CANYON_GENERATOR)
        } else if std::env::args().any(|arg| arg == "--volcanic-terrain") {
            Some(engine_worldgen::planetary::VOLCANIC_SECTOR_GENERATOR)
        } else if std::env::args().any(|arg| arg == "--tectonic-terrain") {
            Some(engine_voxel::WATERSHED_EROSION_GENERATOR)
        } else if std::env::args().any(|arg| arg == "--real-terrain") {
            Some(engine_worldgen::planetary::reference_relief::VERSION)
        } else {
            None
        }
    }) {
        return version;
    }
    if custom {
        return engine_voxel::CUSTOM_PLANET_GENERATOR_VERSION;
    }
    let review = BENCHMARK_GENERATOR_VERSION.load(std::sync::atomic::Ordering::Relaxed);
    if review != 0 { review } else { engine_voxel::PLANET_GENERATOR_VERSION }
}

fn review_configured_world(kind:WorldType,scale:VoxelScalePreset,seed:u32)->World {
    let mut world=World::with_seed(kind,scale,seed);
    let version=BENCHMARK_GENERATOR_VERSION.load(std::sync::atomic::Ordering::Relaxed);
    if version!=0 { world.generator_version=version; }
    world
}

fn parse_benchmark_config() -> BenchmarkConfig {
    let mut config = BenchmarkConfig::default();

    for arg in std::env::args().skip(1) {
        if arg == "--city-quarter" || arg == "--city-livelihood" {
            config.enabled = true;
            config.world_type = WorldType::Superflat;
            config.voxel_scale = VoxelScalePreset::OneMeter;
            config.profile_physics = true;
            config.duration_seconds = 86400.0;
            continue;
        }
        if arg == "--benchmark-adaptive-clouds" {
            config.enabled = true;
            config.adaptive_clouds = true;
            continue;
        }
        if arg == "--benchmark-ossuary-review" {
            config.ossuary_review=true;
            config.enabled=true;
            config.world_type=WorldType::Superflat;
            config.view_distance_chunks=Some(4);
            config.duration_seconds=32.0;
            continue;
        }
        if arg == "--benchmark-tollkeeper-review" {
            config.tollkeeper_review=true;
            config.enabled=true;
            config.world_type=WorldType::Superflat;
            config.view_distance_chunks=Some(4);
            config.duration_seconds=32.0;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-hart-review=") {
            let mut values = value.split(',').filter_map(|v| v.parse::<u8>().ok());
            config.hart_review = Some((values.next().unwrap_or(0).min(2), values.next().unwrap_or(0).min(2)));
            config.enabled = true;
            config.world_type = WorldType::Superflat;
            config.view_distance_chunks = Some(4);
            config.duration_seconds = 32.0;
            continue;
        }
        if arg == "--benchmark-saved-world" {
            config.enabled = true;
            config.saved_review = true;
            continue;
        }
        if arg == "--benchmark-no-multidraw" {
            config.enabled = true;
            render::set_multidraw_enabled(false);
            continue;
        }
        if arg == "--benchmark-no-indirect" {
            config.enabled = true;
            render::set_indirect_lighting_enabled(false);
            continue;
        }
        if arg == "--benchmark-no-clouds" {
            config.enabled = true;
            render::set_clouds_enabled(false);
            continue;
        }
        if arg == "--benchmark-no-shadows" {
            config.enabled = true;
            render::shadow::set_enabled(false);
            continue;
        }
        if arg == "--benchmark-reference-review" {
            config.enabled = true;
            config.reference_review = true;
            config.world_type = WorldType::Superflat;
            config.view_distance_chunks = Some(8);
            config.warmup_seconds = 1.0;
            config.duration_seconds = 66.0;
            continue;
        }
        if arg == "--benchmark-torch-review" {
            config.enabled = true;
            config.torch_review = true;
            config.world_type = WorldType::Superflat;
            config.view_distance_chunks = Some(4);
            config.warmup_seconds = 1.0;
            config.duration_seconds = 50.0;
            continue;
        }
        if arg == "--benchmark-sandbox-review" {
            config.enabled = true;
            config.sandbox_review = true;
            config.profile_physics = true;
            config.world_type = WorldType::Superflat;
            config.view_distance_chunks = Some(4);
            config.warmup_seconds = 3.0;
            config.duration_seconds = 65.0;
            continue;
        }
        if arg == "--benchmark-water-review" || arg == "--benchmark-water-block" {
            config.enabled = true;
            config.water_review = true;
            config.world_type = WorldType::Superflat;
            config.view_distance_chunks = Some(4);
            config.warmup_seconds = 2.0;
            config.duration_seconds = 24.0;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-cpu-mib=") {
            config.cpu_mib = value.parse().ok();
            config.enabled = true;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-cloud-divisor=") {
            if let Ok(divisor) = value.parse::<u32>() { render::set_cloud_render_scale(divisor); }
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-render-scale=") {
            if let Ok(percent) = value.parse::<u32>() { render::set_scene_scale_percent(percent); }
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-gpu-mib=") {
            config.gpu_mib = value.parse().ok();
            config.enabled = true;
            continue;
        }
        if arg == "--benchmark-physics" || arg == "--benchmark-profile-physics" {
            config.profile_physics = true;
            config.enabled = true;
            continue;
        }
        if arg == "--benchmark-horizon-sweep" {
            config.enabled = true;
            config.horizon_sweep = true;
            config.motion = BenchmarkMotionMode::CloudReview;
            config.terrain_camera = Some([0.0, 0.0, 90.0, 0.0, -0.12]);
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-view-chunks=") {
            if let Ok(chunks) = value.parse::<i32>() {
                config.enabled = true;
                config.view_distance_chunks = Some(chunks.clamp(4, 64));
            }
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-terrain-camera=") {
            let values: Vec<f32> = value
                .split(',')
                .filter_map(|v| v.parse::<f32>().ok())
                .collect();
            if values.len() == 5 && values.iter().all(|v| v.is_finite()) {
                config.terrain_camera =
                    Some([values[0], values[1], values[2], values[3], values[4]]);
            }
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-review-time-end=") {
            config.review_time_end = value
                .parse::<f32>()
                .ok()
                .filter(|time| time.is_finite())
                .map(|time| time.rem_euclid(1.0));
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-sky-time=") {
            config.review_sky_time = value.parse::<f32>().ok().filter(|v| v.is_finite() && *v >= 0.0);
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-orbital-time=") {
            config.review_orbital_time = value.parse::<f64>().ok()
                .filter(|v| v.is_finite() && *v >= 0.0 && *v < u32::MAX as f64);
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-orbital-time-end=") {
            config.review_orbital_time_end = value.parse::<f64>().ok()
                .filter(|v| v.is_finite() && *v >= 0.0 && *v < u32::MAX as f64);
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-review-day=") {
            config.review_day = value.parse::<u32>().ok();
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-review-time=") {
            config.review_time_of_day = value
                .parse::<f32>()
                .ok()
                .filter(|time| time.is_finite())
                .map(|time| time.rem_euclid(1.0));
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-review-start=") {
            config.review_start_seconds = value.parse::<f32>().unwrap_or(0.0).max(0.0);
            continue;
        }
        if arg == "--benchmark-terrain-sweep" {
            config.terrain_sweep = true;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-debug=") {
            config.debug_view = match value {
                "cloud-alpha" => RenderDebugView::CloudAlpha,
                "cloud-density" => RenderDebugView::CloudLayerDensity,
                "cloud-shadow" => RenderDebugView::CloudShadowDensity,
                "cloud-bounds" => RenderDebugView::SunShadowVisibility,
                "cloud-steps" => RenderDebugView::CloudStepCount,
                "cloud-weather" => RenderDebugView::CloudWeatherDecode,
                "cloud-history" => RenderDebugView::CloudTemporalWeight,
                "weather-near" => RenderDebugView::WeatherClipmapNear,
                "weather-mid" => RenderDebugView::WeatherClipmapMid,
                "weather-far" => RenderDebugView::WeatherClipmapFar,
                "terrain-shadow" => RenderDebugView::SunShadowVisibility,
                "cascades" => RenderDebugView::ShadowCascadeIndex,
                "albedo" => RenderDebugView::Albedo,
                "material-id" => RenderDebugView::MaterialId,
                "geometric-normal" => RenderDebugView::GeometricNormal,
                "shading-normal" => RenderDebugView::ShadingNormal,
                "roughness" => RenderDebugView::Roughness,
                "direct-light" => RenderDebugView::DirectLight,
                "ambient-diffuse" => RenderDebugView::AmbientDiffuse,
                "local-light" => RenderDebugView::LocalLight,
                "unfogged" => RenderDebugView::Unfogged,
                "projection" => RenderDebugView::Projection,
                "ao" => RenderDebugView::AmbientOcclusion,
                "sky-access" => RenderDebugView::SkyAccess,
                "mesh-kind" => RenderDebugView::MeshKind,
                "water-depth" => RenderDebugView::WaterDepth,
                "clay" => RenderDebugView::Clay,
                "shadow-split" => RenderDebugView::ShadowSplit,
                "sky-galaxy" => RenderDebugView::SkyGalaxy,
                "sky-stars" => RenderDebugView::SkyStars,
                "sky-visibility" => RenderDebugView::SkyVisibility,
                "surface-distance" => RenderDebugView::SurfaceDistance,
                other => {
                    eprintln!("[benchmark] unknown --benchmark-debug={other}; rendering without a debug view");
                    RenderDebugView::None
                }
            };
            continue;
        }
        if arg == "--benchmark-capture-hud" {
            config.enabled = true;
            config.capture_frames = true;
            config.capture_hud = true;
            continue;
        }
        if arg == "--benchmark-resize-review" {
            config.resize_review = true;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-capture-interval=") {
            if let Ok(seconds) = value.parse::<f32>()
                && seconds.is_finite() { config.capture_interval = seconds.clamp(0.05, 60.0); }
            continue;
        }
        if arg == "--benchmark-capture-frames" {
            config.enabled = true;
            config.capture_frames = true;
            continue;
        }
        if arg == "--benchmark" {
            config.enabled = true;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-world=") {
            config.enabled = true;
            config.world_type = match value.to_ascii_lowercase().as_str() {
                "superflat" => WorldType::Superflat,
                "planet" => WorldType::Planet,
                "manyfold" => WorldType::Manyfold,
                "hall" => WorldType::Hall,
                "lightless" => WorldType::Realm(engine_voxel::RealmKind::Lightless),
                "highfold" => WorldType::Realm(engine_voxel::RealmKind::Highfold),
                "gloaming" => WorldType::Realm(engine_voxel::RealmKind::Gloaming),
                "gilded" => WorldType::Realm(engine_voxel::RealmKind::Gilded),
                // Explicit legacy selections let reviews reproduce old terrain.
                "hills" => WorldType::Hills,
                "ridged" => WorldType::Ridged,
                "nether" => WorldType::Nether,
                _ => WorldType::Planet,
            };
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-scale=") {
            config.enabled = true;
            config.voxel_scale = match value.to_ascii_lowercase().as_str() {
                "half" | "halfmeter" | "0.5" => VoxelScalePreset::HalfMeter,
                "quarter" | "quartermeter" | "0.25" => VoxelScalePreset::QuarterMeter,
                _ => VoxelScalePreset::OneMeter,
            };
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-seed=") {
            if let Ok(seed) = value.parse::<u32>() {
                config.enabled = true;
                config.seed = seed;
            }
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-weather=") {
            config.enabled = true;
            config.weather_preset = match value.to_ascii_lowercase().as_str() {
                "clear" => Some(WeatherPreset::Clear),
                "partly" | "partlycloudy" | "partly_cloudy" => Some(WeatherPreset::PartlyCloudy),
                "overcast" => Some(WeatherPreset::Overcast),
                "storm" | "stormy" => Some(WeatherPreset::Stormy),
                "winter" | "winter_sky" | "wintersky" => Some(WeatherPreset::WinterSky),
                _ => None,
            };
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-width=") {
            if let Ok(width) = value.parse::<u32>() {
                config.enabled = true;
                config.width = width.max(320);
            }
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-height=") {
            if let Ok(height) = value.parse::<u32>() {
                config.enabled = true;
                config.height = height.max(240);
            }
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-warmup=") {
            if let Ok(seconds) = value.parse::<f32>() {
                config.enabled = true;
                config.warmup_seconds = seconds.max(0.5);
            }
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-duration=") {
            if let Ok(seconds) = value.parse::<f32>() {
                config.enabled = true;
                config.duration_seconds = seconds.max(1.0);
            }
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-motion=") {
            config.enabled = true;
            config.motion = match value.to_ascii_lowercase().as_str() {
                "orbit" | "moving" | "move" | "flyover" => BenchmarkMotionMode::Orbit,
                "cloud-review" => BenchmarkMotionMode::CloudReview,
                "cloud-review-far" => BenchmarkMotionMode::CloudReviewFar,
                "ascent" => BenchmarkMotionMode::Ascent,
                "lod-travel" => BenchmarkMotionMode::LodTravel,
                _ => BenchmarkMotionMode::Static,
            };
        }
    }

    config
}

/// Height in metres of the water surface above a submerged eye (eye in voxel units).
fn camera_medium(world: &World, eye_m: glam::DVec3) -> render::CameraMedium {
    use render::CameraMedium;
    let scale = f64::from(world.voxel_scale.meters());
    let eye = eye_m / scale;
    let mut cell = eye.floor().as_ivec3();
    match world.get(cell) {
        BlockId::Lava => {
            let recessed = world.get(cell + glam::IVec3::Y) != BlockId::Lava;
            if recessed && eye.y - f64::from(cell.y) >= 1.0 - f64::from(mesh::LAVA_SURFACE_INSET) {
                return CameraMedium::Air;
            }
            return CameraMedium::Lava;
        }
        BlockId::Water => {}
        _ => return CameraMedium::Air,
    }
    let mut fill = world.water_fill(cell);
    let mut above_fill = world.water_fill(cell + glam::IVec3::Y);
    let height = if above_fill > 0.0 { 1.0 } else { mesh::free_water_height(fill) };
    if height <= 0.0 || eye.y - f64::from(cell.y) >= f64::from(height) {
        return CameraMedium::Air;
    }
    // Bound lookups in deep columns. If the surface is farther away, this
    // lower bound is already well beyond the shader's visible water range.
    for _ in 0..256 {
        if above_fill <= 0.0 { break; }
        cell += glam::IVec3::Y;
        fill = above_fill;
        above_fill = world.water_fill(cell + glam::IVec3::Y);
    }
    let height = if above_fill > 0.0 { 1.0 } else { mesh::free_water_height(fill) };
    let depth_m = ((f64::from(cell.y) + f64::from(height) - eye.y) * scale) as f32;
    CameraMedium::Water { depth_m }
}

fn benchmark_motion_active(benchmark: Option<&BenchmarkState>) -> bool {
    if egui_ui::nav_console::manual_ready() { return false; }
    benchmark.is_some_and(|state| {
        state.auto_started
            && !state.config.tollkeeper_review
            && !state.config.ossuary_review
            && !(state.config.world_type == world::WorldType::Planet && planet::review::walking())
            && (state.config.torch_review
                || (state.config.hart_review.is_some() && std::env::var_os("VOXEL_HART_ORBIT").is_some())
                || state.config.terrain_camera.is_some()
                || !matches!(state.config.motion, BenchmarkMotionMode::Static))
    })
}

fn cloud_review_hills_target(terrain: world::WorldSampleConfig) -> (f32, f32) {
    static TARGET: std::sync::OnceLock<(f32, f32)> = std::sync::OnceLock::new();
    *TARGET.get_or_init(|| {
        let meters_per_block = terrain.voxel_scale.meters();
        let blocks_per_meter = meters_per_block.recip();
        let water_m = terrain.water_level() as f32 * meters_per_block;
        let preview_height = |x_m: f32, z_m: f32| {
            world::landscape::preview_surface(
                &terrain,
                (x_m * blocks_per_meter).round() as i32,
                (z_m * blocks_per_meter).round() as i32,
            )
            .0 * meters_per_block
        };
        let mut best = (16_500.0f32, 0.0f32);
        let mut best_score = f32::NEG_INFINITY;
        let mut best_height = 0.0f32;
        let mut best_relief = 0.0f32;
        // Search a narrow 16-20 km annulus cheaply with the same preview
        // surface used by the distant terrain. Prefer inland, locally rugged
        // candidates; the capture validator still checks the actual terrain.
        for radius in [16_500.0, 17_500.0, 18_500.0, 19_500.0] {
            for sector in 0..48 {
                let angle = std::f32::consts::TAU * sector as f32 / 48.0;
                let x = radius * angle.cos();
                let z = radius * angle.sin();
                let center = preview_height(x, z);
                let mut low = center;
                let mut high = center;
                for (dx, dz) in [(-128.0, 0.0), (128.0, 0.0), (0.0, -128.0), (0.0, 128.0)] {
                    let h = preview_height(x + dx, z + dz);
                    low = low.min(h);
                    high = high.max(h);
                }
                let relief = high - low;
                let inland = (center - water_m - 12.0).max(0.0);
                let score = center + relief * 1.4 + inland * 0.25;
                if score > best_score {
                    best_score = score;
                    best = (x, z);
                    best_height = center;
                    best_relief = relief;
                }
            }
        }
        println!(
            "[cloud-review-target] x_m={:.1} z_m={:.1} preview_ground_m={:.1} local_relief_m={:.1}",
            best.0, best.1, best_height, best_relief,
        );
        best
    })
}

// Use the same terrain knots as the live route, but finish cold terrain
// sampling before the timed flight. The benchmark camera only interpolates.
fn lod_travel_profile(cfg: world::WorldSampleConfig, anchor: Vec3) -> (i32, Vec<f32>) {
    let inv_scale = cfg.voxel_scale.meters().recip();
    let stride = 16.0 * inv_scale;
    let first = ((anchor.z - 1280.0 * inv_scale) / stride).floor() as i32;
    let last = (anchor.z / stride).floor() as i32 + 1;
    let x = anchor.x.floor() as i32;
    let heights = (first..=last).map(|segment| {
        world::terrain_height(&cfg, x, (segment as f32 * stride).floor() as i32) as f32
    }).collect();
    (first, heights)
}

fn apply_benchmark_camera_motion(
    state: &BenchmarkState,
    player: &mut Player,
    terrain: world::WorldSampleConfig,
) {
    if river_review::camera(player, &terrain) { return; }
    if dungeon_review::camera(player,&terrain,state.motion_started_at.elapsed().as_secs_f32()) {return;}
    if world::adventure::enemy_review_camera(player, terrain.voxel_scale.meters()) { return; }
    if state.config.hart_review.is_some() && std::env::var_os("VOXEL_HART_ORBIT").is_some() {
        world::adventure::hart_review_camera(player, terrain.voxel_scale.meters(), state.motion_started_at.elapsed().as_secs_f32());
        return;
    }
    if state.config.torch_review {
        torch_review::motion_camera(
            player,
            terrain.voxel_meters(),
            state.motion_started_at.elapsed().as_secs_f32(),
        );
        return;
    }
    if let Some([x, z, clearance, yaw, pitch]) = state.config.terrain_camera {
        let scale = terrain.voxel_scale.meters();
        let ground = world::terrain_height(
            &terrain,
            (x / scale).floor() as i32,
            (z / scale).floor() as i32,
        ) as f32;
        // A clearance at or below -500 is an absolute eye height in metres,
        // -(clearance + 500), for rooms whose floor is not under the local surface.
        let eye_y = if clearance <= -500.0 {
            -(clearance + 500.0) / scale
        } else {
            ground + clearance / scale
        };
        player.pos = Vec3::new(x / scale, eye_y, z / scale);
        player.yaw = yaw;
        player.pitch = pitch;
        // Physics does not step in benchmark reviews, so derive the submerged flag directly.
        player.underwater =
            world::sample_block(&terrain, player.eye(terrain.voxel_scale).floor().as_ivec3())
                == BlockId::Water;
        if state.config.horizon_sweep {
            let t = (state.motion_started_at.elapsed().as_secs_f32() - 20.0).max(0.0);
            player.pos.x += t * 12.0 / scale;
            player.pos.z += (t * 0.1).sin() * 32.0 / scale;
            let ground = world::terrain_height(
                &terrain,
                player.pos.x.floor() as i32,
                player.pos.z.floor() as i32,
            ) as f32;
            player.pos.y = ground + clearance / scale;
            player.yaw += t * std::f32::consts::TAU / 60.0;
        } else if state.config.terrain_sweep {
            // Allow residency to settle, then repeatedly return to the same
            // pose while translating and turning through shadow/LOD boundaries.
            let t = (state.motion_started_at.elapsed().as_secs_f32() - 15.0).max(0.0);
            let wave = (t * std::f32::consts::TAU / 30.0).sin();
            // Underground review stays inside a chamber while looking through
            // every direction; exterior review retains its wider travel path.
            player.pos.x += (if clearance < 0.0 { 3.0 } else { 24.0 }) * wave / scale;
            player.pos.z += (if clearance < 0.0 { 3.0 } else { 8.0 }) * wave / scale;
            player.yaw += if clearance < 0.0 {
                t * std::f32::consts::TAU / 24.0
            } else {
                0.65 * wave
            };
            player.pitch += 0.10 * wave;
        }
        player.vel = Vec3::ZERO;
        player.flying = true;
        player.fly_mode = FlyMode::Noclip;
        player.on_ground = false;
        player.crouch_t = 0.0;
        return;
    }
    if matches!(state.config.motion, BenchmarkMotionMode::Static) {
        return;
    }
    if state.config.motion == BenchmarkMotionMode::LodTravel {
        // Warm up at the first pose. Then fly out, reverse twice, and
        // descend through the same terrain detail bands at flight speed.
        let t = if state.measuring { state.phase_started_at.elapsed().as_secs_f32() } else { 0.0 };
        let (travel, speed) = if t < 4.0 {
            (t, 1.0)
        } else if t < 8.0 {
            (8.0 - t, -1.0)
        } else {
            ((t - 8.0).min(8.0), if t < 16.0 { 1.0 } else { 0.0 })
        };
        let descent = ((t - 8.0) / 6.0).clamp(0.0, 1.0);
        let inv_scale = terrain.voxel_scale.meters().recip();
        let anchor = state.motion_anchor_blocks;
        let z = anchor.z - 160.0 * travel * inv_scale;
        // Interpolate terrain clearance along the route so the same descent
        // also works above a rising planetary surface.
        let stride = 16.0 * inv_scale;
        let segment = (z / stride).floor();
        let Some((first, heights)) = state.lod_travel_profile.as_ref() else { return };
        let index = (segment as i32 - *first) as usize;
        let ground = glam::FloatExt::lerp(heights[index], heights[index + 1], z / stride - segment);
        player.pos = Vec3::new(anchor.x, ground + (180.0 - 135.0 * descent) * inv_scale, z);
        player.vel = Vec3::new(0.0, if t > 8.0 && t < 14.0 { -135.0 / 6.0 } else { 0.0 },
            if t > 0.0 { -160.0 * speed } else { 0.0 }) * inv_scale;
        player.yaw = 0.0;
        player.pitch = -0.65;
        player.flying = true;
        player.fly_mode = FlyMode::Noclip;
        player.on_ground = false;
        return;
    }
    if state.config.motion == BenchmarkMotionMode::Ascent {
        // Straight up from spawn at the boosted flight cap, ground kept in view.
        const ASCENT_SPEED_M_S: f32 = 256.0;
        const ASCENT_TOP_M: f32 = 40_000.0;
        let climb_m = (state.motion_started_at.elapsed().as_secs_f32() * ASCENT_SPEED_M_S).min(ASCENT_TOP_M);
        let anchor = state.motion_anchor_blocks;
        player.pos = Vec3::new(anchor.x, anchor.y + climb_m / terrain.voxel_scale.meters(), anchor.z);
        player.pitch = -1.2;
        player.vel = Vec3::ZERO;
        player.flying = true;
        player.fly_mode = FlyMode::Noclip;
        player.on_ground = false;
        return;
    }

    let inv_scale = terrain.voxel_scale.meters().recip().max(1e-6);
    let elapsed = state.motion_started_at.elapsed().as_secs_f32()
        + state.config.review_start_seconds
        + if state.config.motion == BenchmarkMotionMode::CloudReviewFar {
            90.0
        } else {
            0.0
        };
    if matches!(
        state.config.motion,
        BenchmarkMotionMode::CloudReview | BenchmarkMotionMode::CloudReviewFar
    ) {
        // Five presets near the origin, continuous travel, then the same five
        // presets at 16 km. Each 15-second shot includes translation and rotation.
        let travel = ((elapsed - 75.0) / 15.0).clamp(0.0, 1.0);
        let shot_time = elapsed % 15.0;
        let (target_x, target_z) = if state.config.motion == BenchmarkMotionMode::CloudReviewFar {
            cloud_review_hills_target(terrain)
        } else {
            (16_384.0, -8_192.0)
        };
        let x = travel * target_x + (shot_time * 0.18).sin() * 24.0;
        let z = travel * target_z + (shot_time * 0.18).cos() * 24.0;
        // A fixed Y put the previous Hills review underground. Follow generated
        // terrain, with room for nearby crests and the tallest tree canopies.
        let ground = [
            (-64.0, -64.0),
            (-64.0, 0.0),
            (-64.0, 64.0),
            (0.0, -64.0),
            (0.0, 0.0),
            (0.0, 64.0),
            (64.0, -64.0),
            (64.0, 0.0),
            (64.0, 64.0),
        ]
        .into_iter()
        .map(|(dx, dz)| {
            world::terrain_height(
                &terrain,
                ((x + dx) * inv_scale).floor() as i32,
                ((z + dz) * inv_scale).floor() as i32,
            ) as f32
        })
        .fold(f32::NEG_INFINITY, f32::max);
        player.pos = Vec3::new(x * inv_scale, ground + 14.0 * inv_scale, z * inv_scale);
        player.yaw = -std::f32::consts::FRAC_PI_2 + (shot_time * 0.6).sin() * 0.8;
        player.pitch = 0.12 + (shot_time * 0.4).sin() * 0.30;
        player.vel = Vec3::ZERO;
        player.flying = true;
        player.fly_mode = FlyMode::Noclip;
        player.on_ground = false;
        player.crouch_t = 0.0;
        return;
    }
    let base_altitude = 56.0 * inv_scale;
    let vertical_bob = 14.0 * inv_scale;
    let radius_x = 170.0 * inv_scale;
    let radius_z = 120.0 * inv_scale;
    let wobble = 26.0 * inv_scale;
    let angular_speed = 0.105;

    let sample_position = |t: f32| {
        let angle = angular_speed * t;
        let wobble_phase = angle * 0.47;
        Vec3::new(
            state.motion_anchor_blocks.x
                + angle.cos() * radius_x
                + (wobble_phase * 1.9).sin() * wobble,
            state.motion_anchor_blocks.y + base_altitude + (angle * 0.63).sin() * vertical_bob,
            state.motion_anchor_blocks.z
                + angle.sin() * radius_z
                + (wobble_phase * 2.3).cos() * wobble * 0.65,
        )
    };

    let pos = sample_position(elapsed);
    let future = sample_position(elapsed + 0.35);
    let tangent = (future - pos).normalize_or_zero();
    let look_center = state.motion_anchor_blocks + Vec3::new(0.0, 10.0 * inv_scale, 0.0);
    let to_center = (look_center - pos).normalize_or_zero();
    let forward = (tangent * 0.78 + to_center * 0.22).normalize_or_zero();

    player.pos = pos;
    player.vel = Vec3::ZERO;
    player.flying = true;
    player.fly_mode = FlyMode::Noclip;
    player.on_ground = false;
    player.crouch_t = 0.0;

    if forward.length_squared() > 1e-6 {
        player.yaw = forward.x.atan2(-forward.z);
        player.pitch = forward.y.clamp(-0.98, 0.98).asin();
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct BenchmarkTelemetrySummary {
    active_weather_frames: usize,
    avg_clipmap_batches: f32,
    avg_dirty_rects: f32,
    avg_uploaded_bytes: f32,
    avg_weather_upload_ms: f32,
    max_uploaded_bytes: usize,
    max_weather_upload_ms: f32,
    temporal_reuse_frames: usize,
    history_invalidated_frames: usize,
    history_camera_invalidated_frames: usize,
    history_hard_cut_invalidated_frames: usize,
    history_weather_invalidated_frames: usize,
    history_external_invalidated_frames: usize,
}

fn summarize_benchmark_telemetry(samples: &[PerfSample]) -> BenchmarkTelemetrySummary {
    if samples.is_empty() {
        return BenchmarkTelemetrySummary::default();
    }

    let mut summary = BenchmarkTelemetrySummary::default();
    for sample in samples {
        let weather = sample.weather;
        let render = sample.render;
        if weather.uploaded_bytes > 0 {
            summary.active_weather_frames += 1;
        }
        summary.avg_clipmap_batches += weather.clipmap_batches as f32;
        summary.avg_dirty_rects += weather.dirty_rects as f32;
        summary.avg_uploaded_bytes += weather.uploaded_bytes as f32;
        summary.avg_weather_upload_ms += weather.cpu_upload_ms;
        summary.max_uploaded_bytes = summary.max_uploaded_bytes.max(weather.uploaded_bytes);
        summary.max_weather_upload_ms = summary.max_weather_upload_ms.max(weather.cpu_upload_ms);
        summary.temporal_reuse_frames += render.cloud_temporal_passes.min(1);
        summary.history_invalidated_frames += render.cloud_history_invalidations.min(1);
        summary.history_camera_invalidated_frames +=
            render.cloud_history_camera_invalidations.min(1);
        summary.history_hard_cut_invalidated_frames +=
            render.cloud_history_hard_cut_invalidations.min(1);
        summary.history_weather_invalidated_frames +=
            render.cloud_history_weather_invalidations.min(1);
        summary.history_external_invalidated_frames +=
            render.cloud_history_external_invalidations.min(1);
    }

    let denom = samples.len() as f32;
    summary.avg_clipmap_batches /= denom;
    summary.avg_dirty_rects /= denom;
    summary.avg_uploaded_bytes /= denom;
    summary.avg_weather_upload_ms /= denom;
    summary
}

fn consume_tool_action(
    button_down: bool,
    was_down: &mut bool,
    repeat_timer: &mut f32,
    repeat_interval: f32,
    dt: f32,
) -> bool {
    if !button_down {
        *was_down = false;
        *repeat_timer = 0.0;
        return false;
    }

    if !*was_down {
        *was_down = true;
        *repeat_timer = repeat_interval;
        return true;
    }

    *repeat_timer = (*repeat_timer - dt).max(0.0);
    if *repeat_timer <= 0.0 {
        *repeat_timer = repeat_interval;
        return true;
    }

    false
}

fn reset_gameplay_input(
    input: &mut Input,
    place_was_down: &mut bool,
    break_was_down: &mut bool,
    place_repeat_timer: &mut f32,
    break_repeat_timer: &mut f32,
    pending_tool_action: &mut Option<egui_ui::ToolUseAction>,
) {
    input.clear();
    *place_was_down = false;
    *break_was_down = false;
    *place_repeat_timer = 0.0;
    *break_repeat_timer = 0.0;
    *pending_tool_action = None;
}

/// Local solar clock at which a new planet world begins (0.5 is noon).
const PLANET_START_LOCAL_TIME: f32 = 0.36;
const TORCH_LIGHT_RADIUS: f32 = 18.0;
// Warm lamplight keeps pale stone, timber and brass distinguishable; the
// visible flame retains its hotter core and orange edge in the material pass.
const TORCH_LIGHT_COLOR: [f32; 3] = [1.00, 0.64, 0.36];
const TORCH_LIGHT_STRENGTH: f32 = 4.40;
// Amber gills: a small warm pool under each clump, well below a torch.
const AMBER_LIGHT_RADIUS: f32 = 9.0;
const AMBER_LIGHT_COLOR: [f32; 3] = [1.00, 0.60, 0.26];
const AMBER_LIGHT_STRENGTH: f32 = 1.70;
/// Lightless foxfire and shafts: a cold teal glow, grouped like amber gills.
const FOXFIRE_LIGHT_RADIUS: f32 = 7.0;
const FOXFIRE_LIGHT_COLOR: [f32; 3] = [0.30, 0.95, 0.78];
const FOXFIRE_LIGHT_STRENGTH: f32 = 1.35;
/// Sunstone: warm white, wide, grouped per clump; hung high in cavern roofs.
const SUNSTONE_LIGHT_RADIUS: f32 = 32.0;
const SUNSTONE_LIGHT_COLOR: [f32; 3] = [1.00, 0.86, 0.62];
const SUNSTONE_LIGHT_STRENGTH: f32 = 4.6;
/// Gilded gate: a gold glow around the heart stone.
const GILDED_GATE_LIGHT_RADIUS: f32 = 10.0;
const GILDED_GATE_LIGHT_COLOR: [f32; 3] = [1.00, 0.74, 0.36];
const GILDED_GATE_LIGHT_STRENGTH: f32 = 2.0;

fn held_flame_can_burn(world: &world::World, player: &Player, flame_m: glam::DVec3) -> bool {
    let cabin = world.orbital.express.ride.filter(|ride| !ride.deck)
        .and_then(|_| world::orbital::habitat::readings(world, player));
    let atmosphere = cabin.or_else(|| world::orbital::habitat::readings_at(world, flame_m));
    let flame = flame_m / f64::from(world.voxel_scale.meters());
    !flame_in_liquid(world, flame, flame.floor().as_ivec3())
        && flame_has_air(world, flame_m, atmosphere)
}

fn flame_has_air(world: &world::World, flame_m: glam::DVec3, atmosphere: Option<(f64, f64)>) -> bool {
    atmosphere.map_or_else(
        || altitude::altitude_m(flame_m, altitude::planet_world(world)) < altitude::LOAD_STOP_M,
        |(pressure, oxygen)| pressure >= 30_000.0 && oxygen >= 12_000.0,
    )
}

fn placed_flame_can_burn(world: &world::World, cache: &ChunkCache, owner: IVec3, flame_m: glam::DVec3) -> bool {
    if !flame_has_air(world, flame_m, world::orbital::habitat::readings_at(world, flame_m)) {
        return false;
    }
    let flame = flame_m / f64::from(world.voxel_scale.meters());
    let cell = flame.floor().as_ivec3();
    let owner_source = cache.cpu_chunks.get(&chunk_coord_from_world_block(owner));
    let liquid = |p| {
        let coord = chunk_coord_from_world_block(p);
        if world.chunks.contains_key(&coord)
            || world.ca_view.as_ref().is_some_and(|view| view.cell(p).is_some())
        {
            let block = world.get(p);
            return (block, if block == BlockId::Water { world.water_fill(p) } else { 0.0 });
        }
        // Lighting is cosmetic. Prefer the same generated cells as the
        // terrain renderer, and defer checks across missing source borders.
        let source = cache.cpu_chunks.get(&coord).or(owner_source);
        source.map_or((BlockId::Air, 0.0), |source| {
            let block = source.get_world(p);
            (block, if block == BlockId::Water { source.water_fill(p) } else { 0.0 })
        })
    };
    if flame_in_sampled_liquid(flame, cell, &liquid) { return false; }
    // At metre scale a mounted torch can occupy the same row as the water,
    // displacing the water block in its own cell. Check the exposed sides.
    cell != owner || ![IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z].into_iter()
        .any(|side| flame_in_sampled_liquid(flame, owner + side, &liquid))
}

fn flame_in_liquid(world: &world::World, flame_voxels: glam::DVec3, cell: IVec3) -> bool {
    flame_in_sampled_liquid(flame_voxels, cell, &|p| {
        let block = world.get(p);
        (block, if block == BlockId::Water { world.water_fill(p) } else { 0.0 })
    })
}

fn flame_in_sampled_liquid(
    flame_voxels: glam::DVec3, cell: IVec3, read: &impl Fn(IVec3) -> (BlockId, f32),
) -> bool {
    let (block, fill) = read(cell);
    let height = match block {
        BlockId::Water => {
            if fill <= 0.0 { return false; }
            if read(cell + IVec3::Y).1 > 0.0 { 1.0 } else { mesh::free_water_height(fill) }
        }
        BlockId::Lava if read(cell + IVec3::Y).0 != BlockId::Lava => 1.0 - mesh::LAVA_SURFACE_INSET,
        block => return block.is_liquid(),
    };
    flame_voxels.y - f64::from(cell.y) < f64::from(height)
}

fn held_torch_light(world: &world::World, player: &Player) -> Option<TorchLight> {
    use mesh::models::held_item;
    if held_item::stowed(world, player) { return None; }
    let lumen_lantern = world.expedition.lamp_enabled();
    let forward = player.forward_dir();
    let right = forward.cross(player.up_dir()).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    let motion = player.viewmodel;
    let view = held_item::View {
        eye: Vec3::ZERO, forward, right, up,
        time: world.adventure.time, sway: motion.sway.value, gait: motion.gait, speed: motion.speed,
        clip: motion.pose(), lowered: 0.0, torch_lit: false,
    };
    let left_view = held_item::View { clip: Default::default(), ..view };
    let offhand = held_item::offhand_torch(world);
    let position = if lumen_lantern {
        held_item::torch_position(world, player, if offhand { &left_view } else { &view }, offhand)
    } else {
        [
            (offhand, &left_view, true),
            (held_item::main_hand_torch(world, player), &view, false),
        ].into_iter().filter(|(held, _, _)| *held)
            .map(|(_, view, left)| held_item::torch_position(world, player, view, left))
            .find(|&position| held_flame_can_burn(world, player, position))?
    };
    Some(TorchLight {
        position: position.as_vec3(),
        radius: if lumen_lantern { 22.0 } else { TORCH_LIGHT_RADIUS },
        color: if lumen_lantern {
            [0.88, 1.00, 0.90]
        } else {
            TORCH_LIGHT_COLOR
        },
        strength: TORCH_LIGHT_STRENGTH,
        moving: true,
    })
}

fn push_cast_light(
    world: &world::World,
    player: &Player,
    forward: Vec3,
    world_scale: f32,
    lights: &mut Vec<TorchLight>,
) {
    if let Some((position, radius, color, strength)) = world
        .adventure
        .cast_light(player.eye(world.voxel_scale) * world_scale, forward)
    {
        let position = mesh::models::wand_tip_light(world, player, world_scale)
            .map_or(position, |(tip, _, _)| tip);
        if lights.len() >= MAX_TORCH_LIGHTS {
            lights.pop();
        }
        lights.insert(
            0,
            TorchLight {
                position,
                radius,
                color,
                strength,
                moving: true,
            },
        );
    }
}

fn collect_nearby_torches(
    world: &world::World,
    snapshot: &world::WorldSnapshot,
    cache: &ChunkCache,
    player_pos: Vec3,
    world_scale: f32,
) -> (Vec<TorchLight>, Vec<[i32; 3]>) {
    let center = player_pos.floor().as_ivec3();
    let center_chunk = chunk_coord_from_world_block(center);
    // Match visible terrain range, with a bounded light-selection budget. Iterate
    // indexed sources instead of probing a large cube of empty chunk coordinates.
    let max_distance = (render::render_distance_settings().view_distance_chunks as f32
        * world::CHUNK_SIZE as f32
        * world_scale)
        .clamp(64.0, 192.0);
    let max_radius_blocks = max_distance / world_scale.max(1e-6);
    let max_radius_sq = max_radius_blocks * max_radius_blocks;
    let chunk_radius = render::render_distance_settings().view_distance_chunks.max(
        ((max_radius_blocks.ceil() as i32) + world::CHUNK_SIZE - 1) / world::CHUNK_SIZE,
    ) + 1;
    // Edited chunks sit in the snapshot; generated terrain lives only in the render
    // cache. Both feed the light list, so dedup by cell.
    let mut sources: Vec<(IVec3, BlockId)> = Vec::new();
    for (coord, chunk) in snapshot.chunks.iter() {
        if (*coord - center_chunk).abs().max_element() > chunk_radius {
            continue;
        }
        sources.extend(chunk.torches.iter().map(|&pos| (pos, snapshot.get(pos))));
    }
    cache.append_torch_cells_within(center_chunk, chunk_radius,
        |coord| !snapshot.chunks.contains_key(&coord), &mut sources);
    if snapshot.cfg.world_type == WorldType::Manyfold {
        sources.extend(engine_worldgen::manyfold::guide_lights(&snapshot.cfg).into_iter()
            .filter(|&p| snapshot.get(p) == BlockId::Crystal).map(|p| (p, BlockId::Crystal)));
    }
    // Amber gills grow in clumps: one light per 4 m cell, chosen from the
    // cell's own positions so the pick does not hop as the player moves.
    // Snapshot and cache own disjoint chunks. Only gills need grouping.
    let cell = (4.0 / world_scale.max(1e-6)).max(1.0) as i32;
    let mut amber_cells = fast_hash::FastMap::default();
    sources.retain(|&(pos, block)| {
        if !matches!(block, BlockId::AmberCap | BlockId::GlowBerries | BlockId::FoxfireMoss | BlockId::LightlessShaft | BlockId::Sunstone) { return true; }
        amber_cells.entry(pos.div_euclid(IVec3::splat(cell)))
            .and_modify(|best: &mut (IVec3, BlockId)| {
                if pos.to_array() < best.0.to_array() { *best = (pos, block); }
            }).or_insert((pos, block));
        false
    });
    sources.extend(amber_cells.into_values());
    // The torch model and its light are metre-sized at every scale. Below 1 m
    // a metre-wide fitting is many torch voxels; each one as its own light
    // fills the 64-light budget with one spot and every pixel near it walks
    // all of their shadow rays. One light per 2 m cell, lowest cell first so
    // the pick does not hop as the player moves.
    let torch_cell = if world_scale < 0.75 { (2.0 / world_scale.max(1e-6)) as i32 } else { 1 };
    let mut torch_cells = fast_hash::FastSet::default();
    if torch_cell > 1 {
        sources.sort_unstable_by_key(|&(pos, _)| pos.to_array());
    }
    let mut torches = Vec::new();
    let mut extinguished = Vec::new();
    for (pos, block) in sources {
        let position = if block.is_torch() {
            let position = mesh::placed_torch_flame(pos, block, world_scale, world.generator_version,
                world.world_type == WorldType::Manyfold);
            if !placed_flame_can_burn(world, cache, pos, position) {
                extinguished.push(pos.to_array());
                continue;
            }
            if torch_cell > 1 && !torch_cells.insert(pos.div_euclid(IVec3::splat(torch_cell))) {
                continue;
            }
            position.as_vec3()
        } else {
            (pos.as_vec3() + Vec3::splat(0.5)) * world_scale
        };
        let dist_sq = (pos.as_vec3() + Vec3::splat(0.5) - player_pos).length_squared();
        if dist_sq <= max_radius_sq {
            torches.push((dist_sq, pos, block, position));
        }
    }
    let compare = |a: &(f32, IVec3, BlockId, Vec3), b: &(f32, IVec3, BlockId, Vec3)| {
        a.0.total_cmp(&b.0)
            .then_with(|| (a.1.x, a.1.y, a.1.z).cmp(&(b.1.x, b.1.y, b.1.z)))
    };
    // Fade out at the budget boundary as well as the distance boundary. Sources
    // exchanging the final slot therefore enter with zero intensity.
    let budget = render::torch_light_budget();
    let fade_end = if torches.len() > budget {
        let (_, first_excluded, _) = torches.select_nth_unstable_by(budget, compare);
        (first_excluded.0.sqrt() * world_scale).min(max_distance)
    } else {
        max_distance
    };
    torches.truncate(budget);
    torches.sort_unstable_by(compare);
    let lights = torches
        .into_iter()
        .map(|(distance_sq, _, block, position)| {
            let amber = matches!(block, BlockId::AmberCap | BlockId::GlowBerries);
            let foxfire = matches!(block, BlockId::FoxfireMoss | BlockId::LightlessShaft);
            let sunstone = block == BlockId::Sunstone;
            let gate = block == BlockId::GildedGate;
            TorchLight {
                position,
                radius: if sunstone { SUNSTONE_LIGHT_RADIUS } else if gate { GILDED_GATE_LIGHT_RADIUS } else if foxfire { FOXFIRE_LIGHT_RADIUS } else if amber { AMBER_LIGHT_RADIUS } else { TORCH_LIGHT_RADIUS },
                color: if sunstone { SUNSTONE_LIGHT_COLOR } else if gate { GILDED_GATE_LIGHT_COLOR } else if foxfire { FOXFIRE_LIGHT_COLOR } else if amber { AMBER_LIGHT_COLOR } else { TORCH_LIGHT_COLOR },
                strength: {
                    let distance = distance_sq.sqrt() * world_scale;
                    let fade_start = (fade_end * 0.8).max(fade_end - 24.0);
                    let t = ((fade_end - distance) / (fade_end - fade_start).max(0.001))
                        .clamp(0.0, 1.0);
                    let peak = if sunstone { SUNSTONE_LIGHT_STRENGTH } else if gate { GILDED_GATE_LIGHT_STRENGTH } else if foxfire { FOXFIRE_LIGHT_STRENGTH } else if amber { AMBER_LIGHT_STRENGTH } else { TORCH_LIGHT_STRENGTH };
                    peak * t * t * (3.0 - 2.0 * t)
                },
                moving: false,
            }
        })
        .collect();
    extinguished.sort_unstable();
    (lights, extinguished)
}

/// Fire used to be emissive but did not illuminate the cave wall beside it.
/// Burn cells maintain a tiny bounded index in the sandbox, so this costs at
/// most 64 validation probes per frame instead of a world scan.
fn append_fire_lights(
    world: &World,
    player_pos: Vec3,
    world_scale: f32,
    lights: &mut Vec<TorchLight>,
) {
    let eye = player_pos * world_scale;
    let mut fires = Vec::with_capacity(world.sandbox.fire_lights.len());
    for &cell in &world.sandbox.fire_lights {
        if world.get(cell) != BlockId::Fire {
            continue;
        }
        let position = (cell.as_vec3() + Vec3::new(0.5, 0.55, 0.5)) * world_scale;
        let distance_sq = (position - eye).length_squared();
        if distance_sq < 26.0 * 26.0 {
            fires.push((distance_sq, position));
        }
    }
    fires.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (distance_sq, position) in fires {
        if lights.len() >= MAX_TORCH_LIGHTS {
            break;
        }
        let fade = (1.0 - distance_sq.sqrt() / 26.0).clamp(0.0, 1.0);
        let flicker = 0.88 + 0.12 * (position.x * 3.7 + position.z * 2.3).sin();
        lights.insert(
            0,
            TorchLight {
                position,
                radius: 7.0,
                color: [1.0, 0.43, 0.12],
                strength: 2.2 * fade * fade * flicker,
                moving: false,
            },
        );
    }
}

fn rescue_from_void(world: &mut World, player: &mut Player, last_safe: glam::Vec3) -> bool {
    let min_y = world.sample_config().min_world_y() as f32;
    if !player.pos.is_finite() || player.pos.y >= min_y + 2.0 {
        return false;
    }
    let mut target = last_safe;
    if !target.is_finite() || target.y <= min_y + 8.0 {
        // suggested_spawn is in block units, like player.pos.
        target = world.suggested_spawn();
    }
    player.pos = target;
    player.precise_pos = target.as_dvec3();
    player.vel = Vec3::ZERO;
    player.on_ground = false;
    player.jump_was_down = false;
    player.jump_tap_timer = 0.0;
    world
        .expedition
        .say("You fell out of the world. Returned to solid ground.");
    true
}

/// Execute the ordinary item-use path with the offhand as its supply. The
/// selected stack is restored before returning; queued brush edits never borrow it.
fn use_offhand(world: &mut World, player: &Player, slot: usize,
    hit: Option<(glam::IVec3, glam::IVec3)>,
    blocks: &mut HashSet<glam::IVec3>, chunks: &mut HashSet<glam::IVec3>) -> bool {
    use world::inventory::Item;
    let Some(stack) = world.inventory.offhand else { return false };
    std::mem::swap(&mut world.inventory.offhand, &mut world.inventory.hotbar[slot]);
    let used = (|| {
        if let Item::Food(food) = stack.item { return world.eat_food(food, Some(slot)); }
        let Some(selected) = stack.item.placeable_block() else { return false };
        let Some((cell, place)) = hit else { return false };
        let Some(block) = resolve_placement_block(selected, place-cell,
            player.look_axis(), player.up_dir()) else { return false };
        let (min, max) = player.bounds(world.voxel_scale);
        if creative::placement_traps_body(block, place, min, max) { return false; }
        let old = world.get(place);
        if !old.is_empty() && !old.is_liquid() { return false; }
        if world::fittings::place(world, place, block, player, Some(slot), blocks, chunks) { return true; }
        if world::factory::place(world, place, block, player, Some(stack.item), blocks, chunks) { return true; }
        if (block.is_torch() || engine_content::thin::is_climbable(block)) && !is_torch_supported(world, place, block) { return false; }
        world::effects::place(world, place, old, block, Some(slot), blocks, chunks).is_ok()
    })();
    std::mem::swap(&mut world.inventory.offhand, &mut world.inventory.hotbar[slot]);
    world.sync_from_inventory();
    used
}

fn resolve_placement_block(selected: BlockId, support_normal: glam::IVec3, forward: glam::IVec3, up: glam::Vec3) -> Option<BlockId> {
    if engine_content::shaped::is_shaped(selected) {
        engine_content::shaped::placed(selected, support_normal, forward)
    } else if engine_content::thin::is_climbable(selected) {
        engine_content::thin::attach(selected, support_normal)
    } else if selected.is_torch() {
        BlockId::torch_for_face(support_normal, up)
    } else {
        Some(selected)
    }
}

fn dominant_axis(v: glam::Vec3) -> glam::IVec3 {
    let a = v.abs();
    if a.y >= a.x && a.y >= a.z {
        glam::IVec3::new(0, if v.y < 0.0 { -1 } else { 1 }, 0)
    } else if a.x >= a.z {
        glam::IVec3::new(if v.x < 0.0 { -1 } else { 1 }, 0, 0)
    } else {
        glam::IVec3::new(0, 0, if v.z < 0.0 { -1 } else { 1 })
    }
}

fn is_torch_supported(world: &World, pos: glam::IVec3, block: BlockId) -> bool {
    let encoded = block
        .torch_support_offset()
        .or_else(|| engine_content::thin::support_offset(block));
    if encoded.is_none() && !block.is_torch() {
        return true;
    }
    if encoded.is_some_and(|offset| world.get(pos + offset).is_solid()) {
        return true;
    }
    // A standing torch on the shell is supported by the floor under local up,
    // which is not world down once the tube leaves the bottom.
    if block == BlockId::Torch {
        let metres = (pos.as_vec3() + glam::Vec3::splat(0.5)) * world.voxel_scale.meters();
        let g = engine_worldgen::manyfold::acceleration_for(world.generator_version, metres);
        if let Some(up) = (-g).try_normalize() {
            let down = dominant_axis(-up);
            if down != glam::IVec3::NEG_Y && world.get(pos + down).is_solid() {
                return true;
            }
        }
    }
    false
}

fn cleanup_detached_torches(
    world: &mut World,
    origin: glam::IVec3,
    dirty_blocks: &mut HashSet<glam::IVec3>,
    dirty_chunks: &mut HashSet<glam::IVec3>,
    mut history: Option<&mut world::edit_history::EditHistory>,
) {
    for check in [
        origin + glam::IVec3::Y,
        origin - glam::IVec3::Y,
        origin + glam::IVec3::X,
        origin - glam::IVec3::X,
        origin + glam::IVec3::Z,
        origin - glam::IVec3::Z,
    ] {
        let block = world.get(check);
        let hangs = block.is_torch() || engine_content::thin::is_climbable(block);
        if !hangs || is_torch_supported(world, check, block) {
            continue;
        }
        world.set(check, BlockId::Air);
        if let Some(history) = history.as_deref_mut() { history.record(world, check, block, BlockId::Air); }
        dirty_blocks.insert(check);
        mark_chunk_and_neighbors_dirty(dirty_chunks, check);
    }
}

fn start_world(
    selected_world: WorldType,
    selected_voxel_scale: VoxelScalePreset,
    selected_seed: u32,
    planet: Option<engine_voxel::PlanetSettings>,
    world: &mut Option<World>,
    player: &mut Player,
    day_night: &mut DayNightCycle,
    chunk_cache: &mut ChunkCache,
    chunk_streamer: &mut ChunkStreamer,
    fluid_sim: &mut WorldSimulation,
    dirty_blocks: &mut HashSet<glam::IVec3>,
    dirty_chunks: &mut HashSet<glam::IVec3>,
    mode: &mut AppMode,
    window: &Window,
    grabbed: &mut bool,
    focused: bool,
    last: &mut Instant,
    last_safe: &mut glam::Vec3,
) {
    // A new world invalidates any grounded position from the previous one.
    *last_safe = glam::Vec3::splat(f32::NAN);
    let mut new_world = review_configured_world(selected_world, selected_voxel_scale, selected_seed);
    if selected_world == WorldType::Planet {
        new_world.generator_version = new_planet_generator(planet.is_some());
        new_world.planet_settings = planet;
    }
    // A fresh adventure starts with only the starter spells found.
    new_world.adventure.begin_journey();
    world::workshop_fit::fit(&mut new_world);
    *player = Player::new();
    day_night.reset(DEFAULT_START_TIME_OF_DAY);
    day_night.set_day_length_seconds(DEFAULT_DAY_LENGTH_SECONDS);
    player.pos = new_world.suggested_spawn();
    if selected_world == WorldType::Planet {
        // The spawn is fixed by terrain, so the sun moves instead: begin in local mid-morning,
        // whatever longitude the spawn face sits at. Deterministic, no wall clock.
        let eye = player.pos.as_dvec3() * f64::from(new_world.voxel_scale.meters());
        planet::set_local_time(selected_world, eye, day_night, PLANET_START_LOCAL_TIME);
        // Face the widest view rather than whatever hillside happens to be at yaw 0.
        player.yaw = planet::spawn_yaw(&new_world.sample_config(), player.pos);
    }
    if std::env::args().any(|a| a == "--city-livelihood") {
        if std::env::var_os("VOXEL_SAVE_DIR").is_none_or(|p| p.is_empty()) {
            eprintln!("[city-livelihood] an explicit scratch VOXEL_SAVE_DIR is required");
        } else if let Err(error) = world::farming::jobs::production::scene::install(&mut new_world, player) {
            eprintln!("[city-livelihood] {error}");
        } else if let Some(steps) = std::env::args().find_map(|a| a.strip_prefix("--city-livelihood-preview-ticks=").and_then(|n| n.parse::<usize>().ok())) {
            world::farming::jobs::production::check::preview(&mut new_world, player, steps);
        }
    }
    if std::env::args().any(|a| a == "--city-livelihood")
        && let Some(speed) = std::env::args().find_map(|a| a.strip_prefix("--city-livelihood-speed=").and_then(|n| n.parse::<u8>().ok()))
        {
        world::farming::jobs::production::scene::set_speed(&mut new_world, speed);
    }
    if std::env::args().any(|a| a == "--city-quarter") {
        if std::env::var_os("VOXEL_SAVE_DIR").is_none() {
            eprintln!("[city-quarter] an explicit scratch VOXEL_SAVE_DIR is required");
        } else if let Err(error) = world::farming::jobs::quarter::install(&mut new_world, player) {
            eprintln!("[city-quarter] {error}");
        } else if let Some(ticks)=std::env::args().find_map(|a|a.strip_prefix("--city-quarter-preview-ticks=").and_then(|v|v.parse::<usize>().ok())) {
            world::farming::jobs::quarter::check::preview(&mut new_world,player,ticks);
        }
    }
    if std::env::var_os("VOXEL_REVIEW_CAUSEWAY").is_some()
        && let Some(pos) = world::sites::review_causeway_spawn(&new_world.sample_config()) {
            player.pos = pos;
            player.precise_pos = pos.as_dvec3();
            player.yaw = std::f32::consts::FRAC_PI_2;
            player.pitch = -0.12;
            eprintln!("[tool-review] natural causeway spawn {:?}", pos);
        }

    let start_chunk = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
    let snapshot =
        new_world.snapshot_near(start_chunk, render_distance_settings().view_distance_chunks);
    chunk_cache.reset_with_snapshot(start_chunk, &snapshot);
    chunk_streamer.reset();
    chunk_streamer.queue_needed_visible(chunk_cache, start_chunk);
    fluid_sim.reset_for_world(selected_voxel_scale);

    dirty_blocks.clear();
    dirty_chunks.clear();
    *world = Some(new_world);
    *mode = AppMode::Playing;
    capture_mouse(window, grabbed, false, focused);
    *last = Instant::now();
}

fn release_mouse(window: &Window, grabbed: &mut bool) {
    *grabbed = false;
    let _ = window.set_cursor_grab(CursorGrabMode::None);
    window.set_cursor_visible(true);
}

fn capture_mouse(window: &Window, grabbed: &mut bool, paused: bool, focused: bool) {
    if paused || !focused {
        return;
    }
    let capture = window
        .set_cursor_grab(CursorGrabMode::Locked)
        .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
    *grabbed = capture.is_ok();
    window.set_cursor_visible(!*grabbed);
}

fn sync_creative_ui_cursor(
    window: &Window,
    grabbed: &mut bool,
    focused: bool,
    mode: &AppMode,
    creative: &CreativeState,
) {
    if !matches!(mode, AppMode::Playing) {
        return;
    }
    if creative.any_ui_open() {
        if *grabbed {
            release_mouse(window, grabbed);
        }
    } else if !*grabbed {
        capture_mouse(window, grabbed, false, focused);
    }
}

fn log_benchmark_summary(snapshot: &PerfSnapshot, config: &BenchmarkConfig, vsync: bool) {
    render::flush_review_captures();
    if let Some(root) = std::env::var_os("VOXEL_CAPTURE_DIR") {
        let root = std::path::PathBuf::from(root);
        let write_samples = || -> std::io::Result<()> {
            use std::io::Write;
            std::fs::create_dir_all(&root)?;
            let mut output = std::io::BufWriter::new(std::fs::File::create(root.join("frames.csv"))?);
            writeln!(output, "frame,total_ms,gameplay_ms,stream_ms,render_cpu_ms,gpu_supported,gpu_pass_ms,cloud_ms,cloud_composite_ms,terrain_ms,render_frame_id,render_config_id,gpu_sample_count,gpu_frame_id,gpu_config_id,acquire_ms,submit_ms,present_ms,gpu_shadow_ms,terrain_draws,terrain_triangles,shadow_draws,chunk_upload_bytes,worker_pending,cpu_resident_bytes,gpu_resident_bytes,horizon_ms,visible_missing,near_missing,globe_ms,globe_upload_bytes,globe_wanted,parked_offscreen,draw_chunks,gpu_sky_probe_ms")?;
            for (index, sample) in snapshot.history.iter().enumerate() {
                let cpu = sample.cpu;
                let gpu = sample.render;
                write!(output, "{index},{:.4},{:.4},{:.4},{:.4},{},{:.4},{:.4},{:.4},{:.4},{},{},{},{},{}",
                    cpu.total_ms, cpu.gameplay_ms, cpu.streaming_ms, cpu.render_ms,
                    gpu.gpu_timestamps_supported, gpu.gpu_pass_total_ms,
                    gpu.gpu_cloud_raymarch_ms, gpu.gpu_cloud_composite_ms, gpu.gpu_terrain_ms,
                    gpu.frame_id, gpu.config_id, gpu.gpu_sample_count,
                    gpu.gpu_sample_frame_id, gpu.gpu_sample_config_id)?;
                let chunk = sample.chunk;
                let uploaded_bytes = chunk.uploaded_vertices * std::mem::size_of::<mesh::Vertex3>()
                    + chunk.uploaded_indices * std::mem::size_of::<u32>();
                writeln!(output, ",{:.4},{:.4},{:.4},{:.4},{},{},{},{},{},{},{},{:.4},{},{},{:.4},{},{},{},{},{:.4}",
                    gpu.surface_acquire_ms, gpu.submit_ms, gpu.present_ms, gpu.gpu_shadow_ms,
                    gpu.terrain_draw_calls, sample.counters.terrain_triangles, gpu.shadow_draw_calls,
                    uploaded_bytes, chunk.generating + chunk.meshing + chunk.pending_schedule,
                    chunk.cpu_resident_bytes, chunk.gpu_resident_bytes, cpu.horizon_ms,
                    sample.counters.visible_missing, sample.counters.near_missing,
                    cpu.globe_ms, cpu.globe_upload_bytes as usize, cpu.globe_wanted as usize,
                    chunk.parked_offscreen, sample.counters.draw_chunks, gpu.gpu_sky_probe_ms)?;
            }
            output.flush()
        };
        if let Err(error) = write_samples() { eprintln!("[benchmark] cannot save frame samples: {error}"); }
    }
    println!("[benchmark] cloud_divisor={} history_enabled=false sky_time={:?} present_request={}",
        render::effective_cloud_render_scale(snapshot.resolution[0], snapshot.resolution[1]), config.review_sky_time,
        if vsync { "auto-vsync" } else { "auto-no-vsync" });
    let telemetry = summarize_benchmark_telemetry(&snapshot.history);
    let sample_count = snapshot.sample_count.max(1) as f32;
    let percent = |frames: usize| 100.0 * frames as f32 / sample_count;
    println!(
        "[benchmark] adapter=\"{}\" res={}x{} view_chunks={} lod_start={} lod_levels={} motion={} samples={}",
        snapshot.adapter_summary,
        snapshot.resolution[0],
        snapshot.resolution[1],
        snapshot.render_settings.view_distance_chunks,
        snapshot.render_settings.lod_start_chunks,
        snapshot.render_settings.lod_levels,
        config.motion.label(),
        snapshot.sample_count
    );
    println!(
        "[benchmark] avg_fps={:.2} avg_frame_ms={:.3} p50_frame_ms={:.3} p95_frame_ms={:.3} p99_frame_ms={:.3} max_frame_ms={:.3} fps_1pct={:.2}",
        snapshot.fps_avg,
        snapshot.frame_ms_avg,
        snapshot.frame_ms_p50,
        snapshot.frame_ms_p95,
        snapshot.frame_ms_p99,
        snapshot.frame_ms_max,
        snapshot.fps_1p_low
    );
    println!(
        "[benchmark] cpu_avg_ms ui={:.3} gameplay={:.3} snapshot={:.3} stream={:.3} cull={:.3} torch={:.3} weather={:.3} uniforms={:.3} render_call={:.3} horizon={:.3}",
        snapshot.cpu_avg.ui_ms,
        snapshot.cpu_avg.gameplay_ms,
        snapshot.cpu_avg.snapshot_ms,
        snapshot.cpu_avg.streaming_ms,
        snapshot.cpu_avg.culling_ms,
        snapshot.cpu_avg.torch_ms,
        snapshot.cpu_avg.weather_ms,
        snapshot.cpu_avg.uniform_ms,
        snapshot.cpu_avg.render_ms,
        snapshot.cpu_avg.horizon_ms
    );
    println!(
        "[benchmark] render_avg_ms acquire={:.3} uploads={:.3} shadow={:.3} terrain={:.3} clouds={:.3} ui={:.3} overlay={:.3} submit={:.3} present={:.3}",
        snapshot.render_avg.surface_acquire_ms,
        snapshot.render_avg.stream_upload_ms,
        snapshot.render_avg.shadow_encode_ms,
        snapshot.render_avg.terrain_encode_ms,
        snapshot.render_avg.cloud_encode_ms,
        snapshot.render_avg.ui_encode_ms,
        snapshot.render_avg.overlay_encode_ms,
        snapshot.render_avg.submit_ms,
        snapshot.render_avg.present_ms
    );
    if snapshot.render_avg.gpu_timestamps_supported {
        println!("[benchmark] gpu_samples={} cpu_frames={} configuration_crossovers={}",
            snapshot.render_avg.gpu_sample_count, snapshot.sample_count,
            snapshot.history.iter().filter(|s| s.render.gpu_sample_count > 0
                && s.render.gpu_sample_config_id != s.render.config_id).count());
        println!(
            "[benchmark] gpu_avg_ms passes={:.3} shadow={:.3} terrain={:.3} indirect={:.3} raymarch={:.3} temporal={:.3} sky={:.3} composite={:.3} outline={:.3} postprocess={:.3} ui={:.3}",
            snapshot.render_avg.gpu_pass_total_ms,
            snapshot.render_avg.gpu_shadow_ms,
            snapshot.render_avg.gpu_terrain_ms,
            snapshot.render_avg.gpu_indirect_ms,
            snapshot.render_avg.gpu_cloud_raymarch_ms,
            snapshot.render_avg.gpu_cloud_temporal_ms,
            snapshot.render_avg.gpu_sky_ms,
            snapshot.render_avg.gpu_cloud_composite_ms,
            snapshot.render_avg.gpu_outline_ms,
            snapshot.render_avg.gpu_postprocess_ms,
            snapshot.render_avg.gpu_ui_ms
        );
    }
    println!(
        "[benchmark] latest_scene visible_chunks={} gpu_meshes={} draw_chunks={} terrain_tris={} torches={} shadow_draws={} terrain_draws={} cave_culled={} weather_rects={} weather_bytes={}",
        snapshot.counters_latest.visible_chunks,
        snapshot.counters_latest.visible_meshes,
        snapshot.counters_latest.draw_chunks,
        snapshot.counters_latest.terrain_triangles,
        snapshot.counters_latest.nearby_torches,
        snapshot.render_latest.shadow_draw_calls,
        snapshot.render_latest.terrain_draw_calls,
        crate::render::occlusion::last_culled(),
        snapshot.weather_latest.dirty_rects,
        snapshot.weather_latest.uploaded_bytes
    );
    println!(
        "[benchmark] latest_stream gen={} sched={} mesh_dispatch={} upload={} req={} gpu_gen={} cpu_mesh={} ready_chunk={} ready_mesh={} pending={} parked_offscreen={}",
        snapshot.chunk_latest.terrain_generated,
        snapshot.chunk_latest.terrain_scheduled,
        snapshot.chunk_latest.mesh_dispatched,
        snapshot.chunk_latest.meshes_uploaded,
        snapshot.chunk_latest.requested,
        snapshot.chunk_latest.generating,
        snapshot.chunk_latest.meshing,
        snapshot.chunk_latest.ready_chunks,
        snapshot.chunk_latest.ready_meshes,
        snapshot.chunk_latest.pending_schedule,
        snapshot.chunk_latest.parked_offscreen,
    );
    println!(
        "[benchmark] weather_avg batches={:.2} rects={:.2} upload_kb={:.1} upload_cpu_ms={:.3} active_frames={:.1}% peak_upload_kb={:.1} peak_cpu_ms={:.3}",
        telemetry.avg_clipmap_batches,
        telemetry.avg_dirty_rects,
        telemetry.avg_uploaded_bytes / 1024.0,
        telemetry.avg_weather_upload_ms,
        percent(telemetry.active_weather_frames),
        telemetry.max_uploaded_bytes as f32 / 1024.0,
        telemetry.max_weather_upload_ms
    );
    println!(
        "[benchmark] cloud_history temporal_reuse={:.1}% invalidated={:.1}% camera={:.1}% hard_cut={:.1}% weather={:.1}% external={:.1}%",
        percent(telemetry.temporal_reuse_frames),
        percent(telemetry.history_invalidated_frames),
        percent(telemetry.history_camera_invalidated_frames),
        percent(telemetry.history_hard_cut_invalidated_frames),
        percent(telemetry.history_weather_invalidated_frames),
        percent(telemetry.history_external_invalidated_frames)
    );
}

#[cfg(all(axiom_native_edit_review, feature = "multiplayer"))]
fn main() -> Result<()> {
    multiplayer::edit_admission_review::run(std::env::var_os("VOXEL_ADMISSION_BASELINE").is_none())
}

#[cfg(not(all(axiom_native_edit_review, feature = "multiplayer")))]
fn main() -> Result<()> {
    // Before the startup checks: a draft spell table must load first.
    if std::env::args().any(|arg| arg == "--wand-lab") {
        return dev_panel::wand_lab::run_headless();
    }
    if std::env::args().any(|arg| arg == "--factory-lab") {
        return dev_panel::factory_lab::run_headless();
    }
    geology_cache::configure();
    relief_cache::configure();
    // Content tables fail at launch, never mid-game.
    world::adventure::enchanting::validate();
    world::adventure::hedrons::validate();
    #[cfg(feature = "multiplayer")]
    if std::env::args().any(|arg| arg == "--edit-admission-review") {
        return multiplayer::edit_admission_review::run(std::env::var_os("VOXEL_ADMISSION_BASELINE").is_none());
    }
    if std::env::args().any(|arg| arg == "--spillway-playtest") {
        return world::mill::review::run();
    }
    if let Some(root) = std::env::args().find_map(|a| a.strip_prefix("--onboarding-check=").map(std::path::PathBuf::from)) {
        return egui_ui::goals_hud::review::run(&root);
    }
    if let Some(root) = std::env::args().find_map(|a| a.strip_prefix("--city-livelihood-check=").map(std::path::PathBuf::from)) {
        return world::farming::jobs::production::check::run(&root);
    }
    if let Some(root)=std::env::args().find_map(|a|a.strip_prefix("--city-service-check=").map(std::path::PathBuf::from)) {
        return world::farming::jobs::routine::check::services::run(&root);
    }
    if let Some(root)=std::env::args().find_map(|a|a.strip_prefix("--city-group-check=").map(std::path::PathBuf::from)) {
        return world::farming::jobs::groups::check::run(&root);
    }
    if let Some(root)=std::env::args().find_map(|a|a.strip_prefix("--city-quarter-check=").map(std::path::PathBuf::from)) {
        return world::farming::jobs::quarter::check::run(&root);
    }
    #[cfg(feature="multiplayer-sdr")]
    let _steam_runtime=engine_net::steam::RuntimeScope;
    env_logger::init();
    crash_log::install();
    if let Ok(path)=std::env::var("VOXEL_TOOL_ART_SHEET") {
        return egui_ui::icons::tools::export_sheet(std::path::Path::new(&path));
    }
    if let Some(path) = std::env::args().find_map(|arg| arg.strip_prefix("--anchor-studio-pack=").map(str::to_owned)) {
        use std::io::Read;
        use engine_worldgen::anchor_studio::{Design, MAX_PLAN_BYTES, install_city_review};
        let scratch = std::env::var_os("VOXEL_SAVE_DIR").filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Anchor Studio needs a fresh review save folder"))?;
        anyhow::ensure!(parse_benchmark_config().enabled
            && !std::env::args().any(|a| a.starts_with("--multiplayer") || a.starts_with("--headless") || a.starts_with("--ca-window"))
            && std::fs::read_dir(scratch)?.next().is_none(),
            "Anchor Studio city review requires a benchmark and an empty save folder");
        let mut source = String::new();
        std::fs::File::open(path)?.take(MAX_PLAN_BYTES as u64 + 1).read_to_string(&mut source)?;
        install_city_review(Design::parse(&source)?)?;
    }
    // Before any recipe read: the tables are built once on first use.
    world::inventory::datapack::install(&world::storage::save_directory());
    anyhow::ensure!(!std::env::args().any(|a| a.starts_with("--ca-window"))
        || !std::env::args().any(|a| a.starts_with("--headless") || a.starts_with("--multiplayer")),
        "cellular window cannot share another driver");
    #[cfg(feature = "multiplayer")]
    if multiplayer::browse_cli()? || multiplayer::run_headless()? { return Ok(()); }

    if let Some(path) = std::env::args().find_map(|arg| arg.strip_prefix("--capture-menu=").map(str::to_owned)) {
        return pollster::block_on(egui_ui::menu_capture::run(std::path::Path::new(&path)));
    }

    if let Some(path) = std::env::args().find_map(|arg| arg.strip_prefix("--headless=").map(str::to_owned)) {
        std::process::exit(headless::main(&path));
    }
    let mut ca_window = ca_window::Session::parse()?;
    anyhow::ensure!(ca_window.is_some() ||
        !std::env::args().any(|arg| arg.starts_with("--ca-cpu-region") || arg.starts_with("--ca-checkpoint-") || arg.starts_with("--ca-session-") || arg.starts_with("--ca-backend")),
        "CPU region and checkpoint options require --headless=<command-document.ron>"
    );
    let benchmark_config = parse_benchmark_config();
    if let Some(value)=std::env::args().find_map(|arg|arg.strip_prefix("--benchmark-generator-version=").map(str::to_owned)) {
        let version=value.parse::<u32>()?;
        // Freeze the legacy general range: a new default must not admit
        // every dormant or unknown number below it. Keep the former Planet
        // default explicit because it was already reviewable for every kind.
        let reviewable = (1..=engine_voxel::WORKSHOP_SUPPLY_GENERATOR).contains(&version)
            || version == engine_voxel::SHARED_SURFACE_GENERATOR
            || version == engine_voxel::CLEAN_RIVERBED_GENERATOR || version == engine_voxel::CUSTOM_PLANET_VERSION || version == engine_voxel::CUSTOM_PLANET_GENERATOR_VERSION || version == engine_worldgen::geo::GEO_GENERATOR || version == engine_voxel::PLANET_GENERATOR_VERSION || version == engine_worldgen::ground_exposure::OUTCROP_GENERATOR || version == engine_worldgen::basin::VERSION || version == engine_worldgen::TREE_FOLIAGE_VERSION || version == engine_worldgen::surface::MEADOW_GROUND_VERSION || version == engine_worldgen::vistas::FORM_VERSION || version == engine_worldgen::planet_climate::VERSION || version == engine_worldgen::planet_ocean::VERSION;
        // Compare the dormant regional pass with its immediate baseline using
        // one executable, without changing the default or loading real saves.
        let reviewable = reviewable || (engine_worldgen::geo::corridor::VERSION
            ..=engine_worldgen::geo::corridor::LOD_VERSION).contains(&version)
            || version == engine_worldgen::plants::VERSION
            || version == engine_worldgen::landscape::WOODLAND_GENERATOR
            || matches!(benchmark_config.world_type, WorldType::Hills | WorldType::Ridged | WorldType::Planet)
                && version == engine_worldgen::WOODLAND_HABITAT_PREVIEW
            || version == engine_worldgen::dungeon::v2::TREASURE_VERSION
            || version == engine_voxel::DUNGEON_MECHANISMS_GENERATOR
            || version == engine_worldgen::undercrown::v2::props::VERSION
            || version == engine_worldgen::undercrown::v2::life::VERSION
            || matches!(benchmark_config.world_type, WorldType::Hills | WorldType::Ridged)
                && matches!(version, engine_worldgen::geo::MOUNTAIN_EROSION_GENERATOR | engine_worldgen::geo::RAVINE_GENERATOR)
            || benchmark_config.world_type == WorldType::Manyfold && version == engine_worldgen::manyfold::maze::TWISTED
            || benchmark_config.world_type == WorldType::Planet
                && ((engine_voxel::COASTAL_PLANET_GENERATOR..=engine_voxel::PLANET_FREIGHT_DETOUR_GENERATOR).contains(&version)
                    || matches!(version, engine_worldgen::civilization::CHRONICLE_GENERATOR
                        | engine_worldgen::planetary::ESTUARY_OUTLOOK_GENERATOR
                        | engine_worldgen::planetary::ERODED_RELIEF_GENERATOR
                        | engine_worldgen::planetary::REGIONAL_EROSION_GENERATOR
                        | engine_worldgen::planetary::LANDSCAPE_HISTORY_GENERATOR
                        | engine_worldgen::planetary::MAPPED_CANYON_GENERATOR
                        | engine_worldgen::planetary::FAULT_HISTORY_GENERATOR
                        | engine_worldgen::planetary::WEATHERING_HISTORY_GENERATOR
                        | engine_worldgen::planetary::UPLAND_RUNOFF_GENERATOR
                        | engine_worldgen::planetary::JOINTED_CANYON_GENERATOR
                        | engine_worldgen::planetary::FOLDED_SCARP_GENERATOR
                        | engine_worldgen::planetary::BEDROCK_SCARP_GENERATOR
                        | engine_worldgen::planetary::TECTONIC_TRAVERSE_GENERATOR
                        | engine_worldgen::planetary::HEADWATER_RELIEF_GENERATOR
                        | engine_worldgen::planetary::DRAINAGE_RELIEF_GENERATOR
                        | engine_worldgen::planetary::WATERSHED_EROSION_GENERATOR
                        | engine_worldgen::planetary::VOLCANIC_SECTOR_GENERATOR
                        | engine_worldgen::planetary::CONNECTED_WATERSHEDS_GENERATOR
                        | engine_worldgen::planetary::REFINED_DRAINAGE_GENERATOR
                        | engine_worldgen::planetary::RIFTED_MARGIN_GENERATOR
                        | engine_worldgen::planetary::COUPLED_RIFT_GENERATOR
                        | engine_worldgen::planetary::GEOLOGICAL_HISTORY_GENERATOR
                        | engine_worldgen::planetary::EVOLVED_WATERSHEDS_GENERATOR
                        | engine_worldgen::planetary::RESOLVED_HISTORY_GENERATOR
                        | engine_worldgen::planetary::BRANCHING_WATERSHEDS_GENERATOR
                        | engine_worldgen::planetary::LANDSCAPE_CYCLES_GENERATOR
                        | engine_worldgen::planetary::DEEP_TIME_GENERATOR
                        | engine_worldgen::planetary::FRACTAL_COAST_GENERATOR
                        | engine_worldgen::planetary::PROCESS_COAST_GENERATOR
                        | engine_worldgen::planetary::CLEAN_RIVERBED_GENERATOR
                        | engine_worldgen::planetary::GROUND_BREAK_GENERATOR
                        | engine_worldgen::planetary::LIVING_SURFACE_GENERATOR
                        | engine_worldgen::planetary::LOWLAND_RIVER_GENERATOR
                        | engine_worldgen::planetary::RIFT_VALLEY_GENERATOR
                        | engine_worldgen::planetary::DIKE_SWARM_GENERATOR
                        | engine_worldgen::planetary::CARVED_SURFACE_GENERATOR
                        | engine_worldgen::planetary::ROCK_RELIEF_GENERATOR
                        | engine_worldgen::planetary::CONNECTED_RELIEF_GENERATOR
                        | engine_worldgen::planetary::LAYERED_CANYON_GENERATOR
                        | engine_worldgen::planetary::reference_relief::VERSION
                        | engine_worldgen::planetary::SEDIMENT_WEATHERING_GENERATOR
                        | engine_worldgen::planetary::CATCHMENT_EXPLORATION_GENERATOR
                        | engine_worldgen::planetary::VALLEY_FORM_GENERATOR
                        | engine_worldgen::planetary::SILL_EROSION_GENERATOR
                        | engine_worldgen::planetary::COUPLED_SEDIMENT_GENERATOR
                        | engine_worldgen::planetary::DELTA_ISLANDS_GENERATOR
                        | engine_worldgen::planetary::MOBILE_DELTA_GENERATOR
                        | engine_worldgen::planetary::EVOLVING_DELTA_GENERATOR
                        | engine_worldgen::planetary::DELTA_FEEDBACK_GENERATOR
                        | engine_worldgen::planetary::DISTRIBUTARY_GENERATOR))
            || benchmark_config.world_type == WorldType::Nether
                && matches!(version, engine_voxel::HELL_ECOLOGY_GENERATOR | engine_voxel::HELL_RUINS_GENERATOR | engine_voxel::HELL_SURVEYED_RUINS_GENERATOR | engine_voxel::HELL_MIRROR_GENERATOR | engine_voxel::HELL_BIOME_GENERATOR);
        anyhow::ensure!(benchmark_config.enabled && reviewable,"unsupported benchmark generator version {version}");
        BENCHMARK_GENERATOR_VERSION.store(version,std::sync::atomic::Ordering::Relaxed);
        println!("[benchmark] generator_version={version} current_default={}",world::CURRENT_GENERATOR_VERSION);
    }
    if std::env::args().any(|arg| arg == "--horizon-probe-only") {
        let review_world = review_configured_world(benchmark_config.world_type, benchmark_config.voxel_scale, benchmark_config.seed);
        horizon_review::probe_only(review_world.sample_config())?;
        return Ok(());
    }
    // Find reproducible visual-review locations without opening a window or
    // touching saves. Uses the same generator as streamed gameplay chunks.
    if std::env::args().any(|arg| arg == "--terrain-survey") {
        let review_world = review_configured_world(
            benchmark_config.world_type,
            benchmark_config.voxel_scale,
            benchmark_config.seed,
        );
        let cfg = review_world.sample_config();
        let mut locations = std::collections::BTreeMap::new();
        // Turf points: the nearest Grass-surfaced columns per biome, so a
        // review pose shows the biome floor and not a site, bank or cliff.
        let mut turf: std::collections::BTreeMap<&str, Vec<(i32, i32, i32)>> =
            std::collections::BTreeMap::new();
        for z in (-3072..=3072).step_by(96) {
            for x in (-3072..=3072).step_by(96) {
                let (bx, bz) = (cfg.blocks_for_height(x as f32), cfg.blocks_for_height(z as f32));
                let s = world::landscape::sample(&cfg, bx, bz);
                let distance = x * x + z * z;
                let entry = locations
                    .entry(s.biome.label())
                    .or_insert((i32::MAX, x, z, s));
                if distance < entry.0 {
                    *entry = (distance, x, z, s);
                }
                if distance > 150 * 150
                    && engine_worldgen::sample_column(&cfg, bx, bz).surface == world::BlockId::Grass
                {
                    let list = turf.entry(s.biome.label()).or_default();
                    list.push((distance, x, z));
                    list.sort_unstable();
                    list.truncate(3);
                }
            }
        }
        for (biome, (_, x, z, s)) in locations {
            println!(
                "[terrain-survey] {biome}: x={x} z={z} height_m={:.1} temperature={:.2} moisture={:.2}",
                s.height, s.temperature, s.moisture
            );
        }
        for (biome, points) in turf {
            let list: Vec<String> = points.iter().map(|(_, x, z)| format!("({x},{z})")).collect();
            println!("[terrain-survey] {biome} turf: {}", list.join(" "));
        }
        if let Some((centre, forward)) = engine_worldgen::amberroot::anchor(&cfg) {
            println!(
                "[terrain-survey] Amberroot grove: x={:.0} z={:.0} floor_m={:.1} approach=({:.3},{:.3})",
                centre.x, centre.z, centre.y, forward.x, forward.y
            );
        }
        if let Some((door, undercroft)) = engine_worldgen::amberroot::interior_points(&cfg) {
            println!("[terrain-survey] Amberroot hollow door: {door:.1} undercroft floor: {undercroft:.1}");
        }
        for (element, x, z, ground) in world::adventure::hedrons::survey(&cfg, 6) {
            // Pose stands 14 m south of the hedron looking north at it.
            println!(
                "[terrain-survey] {element} hedron: x={x:.1} z={z:.1} ground_m={ground:.1} pose={x:.1},{:.1},6,0,0.05",
                z + 14.0
            );
        }
        return Ok(());
    }
    let low_spec = low_spec::requested() && !benchmark_config.enabled;
    let mut preferences = preferences::PreferenceStore::new(!benchmark_config.enabled && !low_spec);
    if !benchmark_config.enabled {
        keybinds::load();
    }
    if low_spec {
        low_spec::apply(&mut preferences.loaded);
    }
    // The last session ended in a lost graphics device: start gently, and say so.
    let mut quality_notice_pending: Option<String> = None;
    if !benchmark_config.enabled && quality::take_gpu_reset() {
        let p = &mut preferences.loaded;
        p.view = p.view.min(16);
        p.lod = p.lod.clamp(2, (p.view - 1).max(2));
        p.levels = p.levels.min(3);
        p.clouds = false;
        p.cloud_scale = 4;
        p.indirect_lighting = false;
        p.auto_quality = true;
        quality_notice_pending = Some(
            "The graphics driver reset last time, so quality was lowered and clouds are off. Raise it in Settings.".into(),
        );
    }
    if !benchmark_config.enabled {
        preferences.loaded.apply_globals();
    }
    let mut quality_governor = quality::Governor::default();
    let mut preferences_render_applied = false;
    let mut windowed_size = (preferences.loaded.width, preferences.loaded.height);
    let mut combat_audio = audio::Audio::new();
    let mut soundscape = soundscape::Soundscape::default();
    let event_loop = EventLoop::new()?;
    let mut window: Option<Arc<Window>> = None;

    let mesh_workers = thread::available_parallelism()
        .map(|count| (count.get() / 3).clamp(1, 8))
        .unwrap_or(1);

    let mut startup = StartupState::WaitingForWindow;
    let startup_started = Instant::now();
    let playable_probe = terrain_review::option("VOXEL_PLAYABLE_PROBE");
    let mut playable_probe_start = None::<Instant>;
    let mut playable_probe_spawn = false;
    let mut playable_probe_horizon = false;
    let mut egui_ui: Option<EguiUi> = None;
    let mut water_probe = world::water_tools::Probe::default();

    let mut selected_world = preferences.loaded.kind();
    let mut selected_voxel_scale = preferences.loaded.voxel_scale();
    let mut selected_seed = default_random_seed();
    if std::env::args().any(|arg| arg == "--real-terrain") {
        selected_world = WorldType::Planet;
    }
    if benchmark_config.enabled {
        selected_world = benchmark_config.world_type;
        selected_voxel_scale = benchmark_config.voxel_scale;
        selected_seed = benchmark_config.seed;
    }
    let initial_window_size = if benchmark_config.enabled {
        PhysicalSize::new(benchmark_config.width, benchmark_config.height)
    } else {
        PhysicalSize::new(windowed_size.0, windowed_size.1)
    };

    let mut world: Option<World> = None;
    let mut player = Player::new();
    // Invalid until the player first stands on ground; the void rescue falls
    // back to suggested_spawn rather than teleporting to world origin.
    let mut last_safe_player_pos = glam::Vec3::splat(f32::NAN);
    let mut day_night = DayNightCycle::new(DEFAULT_START_TIME_OF_DAY, DEFAULT_DAY_LENGTH_SECONDS);
    let mut cloud_weather = WeatherRuntimeState::new(selected_seed);
    let mut freeze_time = false;
    // Orbit time off the default day pace, with the calendar it was last in step with.
    let mut sky_clock: Option<(f64, f64)> = None;
    let mut input = Input::default();
    let mut view_zoom = comfort::Zoom::default();
    let mut pending_screenshot: Option<std::path::PathBuf> = None;
    // Shown a frame after the request so the notice is not in the image.
    let mut screenshot_notice: Option<String> = None;
    let mut creative = CreativeState::default();
    let mut blueprints = creative::blueprint::BlueprintTool::default();
    let mut edit_history = world::edit_history::EditHistory::new(world::edit_history::LOCAL_CELLS);
    player.selected_block = creative.selected_block();
    let mut render_debug_view = benchmark_config.debug_view;
    let mut atmosphere_mode = AtmosphereMode::default();

    let mut grabbed = false;
    let mut focused = true;
    let mut occluded = false;
    let mut next_background_frame = Instant::now();
    let mut next_retirement_pass = Instant::now();
    let mut next_menu_frame = Instant::now();
    let mut mode = AppMode::WorldSelect;
    let mut world_entry_pending = false;
    let mut world_entry_prompt = false;
    let mut pending_planet: Option<(engine_voxel::WorldSampleConfig, UiAction)> = None;
    let mut saves = world::storage::SaveService::new();
    let mut freight_review = world::farming::planet_freight::review::Run::from_env(benchmark_config.enabled)?;
    let mut jobs_review = world::farming::jobs::review::Run::from_env(benchmark_config.enabled)?;
    let mut hell_gate = world::hell_gate::Travel::default();
    #[cfg(feature = "multiplayer")]
    let mut realm_plan: Option<multiplayer::RealmPlan> = None;
    let mut hall_gate = world::hall_gate::Travel::default();
    // Feet in metres where the player last stood under open sky; never saved.
    let mut library = world::storage::WorldLibrary::new(!benchmark_config.enabled && ca_window.is_none());
    let mut pending_loaded: Option<anyhow::Result<world::storage::SavedSession>> = None;
    let mut next_library_refresh = Instant::now();
    let mut exit_pending = false;
    let mut last = Instant::now();
    let mut outline = Vec::new();
    let mut invert_outline = Vec::new();
    let mut simulation_clock = simulation::SimulationClock::default();
    let mut sky_time = 0.0f32;
    // Eased 0..1 'inside a cosmos chamber', and the last sky event announced.
    let mut cosmos_smooth = 0.0f32;
    let mut last_sky_event: Option<world::mood::SkyEvent> = None;
    let mut last_zone_key: Option<(u32, world::zones::Archetype)> = None;
    let mut star_timer = 20.0f32;
    let mut location_ambience = world::ambience::Presentation::default();
    let mut star_count = 0u32;

    let mut chunk_cache = ChunkCache::new(glam::IVec3::ZERO);
    chunk_cache.reset(glam::IVec3::ZERO);
    let mut dirty_blocks = HashSet::new();
    let mut dirty_chunks = HashSet::new();
    #[cfg(feature = "multiplayer")]
    let mut multiplayer: Option<multiplayer::Session> = None;
    #[cfg(feature = "multiplayer")]
    let mut multiplayer_start_pending = true;
    // Lobby work started from the Online pages; sessions can start and end at runtime.
    #[cfg(feature = "multiplayer")]
    let mut coop = multiplayer::coop::Driver::default();
    #[cfg(feature = "multiplayer")]
    let mut coop_review = multiplayer::coop::review();
    // Set by Host Latest World: host the world the library is opening once it spawns.
    #[cfg(feature = "multiplayer")]
    let mut host_after_load = false;
    #[cfg(feature = "multiplayer-sdr")]
    let mut steam_invites = engine_net::steam::invites::Incoming::start().ok();
    #[cfg(feature = "multiplayer-sdr")]
    let mut startup_invite = std::env::args().find_map(|arg|
        arg.strip_prefix(engine_net::steam::invites::LAUNCH_ARG).map(str::to_owned));
    #[cfg(not(feature = "multiplayer"))]
    let network_mode = false;
    #[cfg(feature = "multiplayer")]
    let mut network_stream_initialized=false;
    #[cfg(feature = "multiplayer")]
    let mut network_ready_reported=false;
    #[cfg(feature = "multiplayer")]
    let mut network_capture=std::env::args().find_map(|a|a.strip_prefix("--multiplayer-capture=").map(std::path::PathBuf::from));
    #[cfg(feature = "multiplayer")]
    let network_capture_delay:f32=std::env::args().find_map(|a|a.strip_prefix("--multiplayer-capture-delay=").and_then(|s|s.parse().ok())).unwrap_or(0.0);
    #[cfg(feature = "multiplayer")]
    let mut network_ready_at:Option<Instant>=None;
    let mut chat_ui = egui_ui::chat::ChatUi::default();
    let mut local_social = social::Social::default();
    #[cfg(not(feature = "multiplayer"))]
    anyhow::ensure!(!std::env::args().any(|arg| arg.starts_with("--multiplayer-")), "build with --features multiplayer to host or join");
    let mut physics_enabled = true;
    let mut fps_counter = FpsCounter::new();
    let mut altitude_state = altitude::TierState::new();
    let mut place_was_down = false;
    let mut break_was_down = false;
    let mut place_repeat_timer = 0.0f32;
    let mut break_repeat_timer = 0.0f32;
    let mut readonly_picker = world::ReadOnlyPicker::default();
    let readonly_pick_review = std::env::var_os("AXIOM_READONLY_PICK_REVIEW").is_some_and(|v| v == "1");
    let mut mining_progress = world::mining::Progress::default();
    let mut next_outpost_view = Instant::now();
    let mut heavy_charge = world::mining::charge::Charge::default();
    let mut pending_tool_action: Option<egui_ui::ToolUseAction> = None;
    let mut perf_tracker = PerfTracker::default();
    let mut perf_snapshot = PerfSnapshot::default();
    let mut next_perf_snapshot = Instant::now();
    let live_profile = std::env::var_os("VOXEL_LIVE_PROFILE").is_some();
    let mut last_live_profile = Instant::now();
    let mut benchmark_hitch_reports = 0u32;
    let mut last_presented_at: Option<Instant> = None;
    let mut redraw_requested_at: Option<Instant> = None;
    let mut present_gap = perf::route::PresentGap::default();
    let mut gpu_retry_at: Option<Instant> = None;
    let mut gpu_retry_simulation_at = None;
    let benchmark_barrier = std::env::var_os("VOXEL_BENCHMARK_BARRIER").map(std::path::PathBuf::from);
    let mut benchmark_barrier_open = benchmark_barrier.is_none();
    let mut sandbox_review = benchmark_config
        .sandbox_review
        .then(sandbox_review::SandboxReview::default);
    let mut wand_review = wand_review::Review::from_env(benchmark_config.enabled);
    let mut latency_probe = latency_probe::Probe::from_env(benchmark_config.enabled);
    let mut workshop_review = workshop_review::Review::from_env(benchmark_config.enabled);
    let mut industrial_review = industrial_review::Review::enabled(benchmark_config.enabled);
    let mut receiver_review = receiver_review::Review::from_env(benchmark_config.enabled);
    let open_block_scene = std::env::args().any(|arg| arg == "--benchmark-water-block");
    let mut water_review = benchmark_config.water_review.then(|| {
        if open_block_scene {
            water_review::WaterReview::open_block()
        } else if std::env::var_os("VOXEL_WATER_SEAM_REVIEW").is_some() {
            water_review::WaterReview::seam_view()
        } else if std::env::var_os("VOXEL_WATER_SURFACE_REVIEW").is_some() {
            water_review::WaterReview::splash_view()
        } else {
            water_review::WaterReview::default()
        }
    });
    let mut benchmark = benchmark_config.enabled.then(|| BenchmarkState {
        config: benchmark_config,
        auto_started: false,
        measuring: false,
        phase_started_at: Instant::now(),
        motion_started_at: Instant::now(),
        motion_anchor_blocks: Vec3::ZERO,
        lod_travel_profile: None,
        lod_travel_worker: None,
        cloud_review_phase: usize::MAX,
        last_capture_key: usize::MAX,
        last_periodic_capture: None,
        last_resize_phase: usize::MAX,
        last_profile_second: 0,
        measured_samples: Vec::new(),
        route_timing: None,
        horizon_world_switches: 0,
        horizon_route_started_at: None,
        scripted_stream_velocity: None,
    });

    let mut pads = gamepad::Gamepads::open();
    let mut draw_scratch = render::DrawScratch::default();
    let mut draw_list = Vec::new();
    let dev_panel = dev_panel::Bridge::open();
    let scripted_route = benchmark_config.enabled && terrain_review::option("VOXEL_HORIZON_ROUTE");
    let route_raw_input = terrain_review::option("VOXEL_HORIZON_ROUTE_RAW_INPUT");
    let quiet_benchmark_input = benchmark_config.enabled && terrain_review::option("VOXEL_BENCHMARK_QUIET_INPUT");
    let benchmark_profile_logs = perf::profile_log::enabled(
        std::env::args().any(|arg| arg == "--city-quarter" || arg == "--city-livelihood"),
        std::env::args().any(|arg| arg == "--benchmark-profile-physics"),
        std::env::var("VOXEL_BENCHMARK_PROFILE_LOG").ok().as_deref(),
    );
    let sprint_benchmark = benchmark_config.enabled
        && terrain_review::option("VOXEL_BENCHMARK_SPRINT");
    let badlands_sprint = sprint_benchmark && terrain_review::option("VOXEL_BENCHMARK_BADLANDS");
    let mut sprint_route = terrain_review::SprintRoute::default();

    #[allow(deprecated)]
    event_loop.run(move |event, elwt| {
        if matches!(&event, Event::WindowEvent { event: WindowEvent::RedrawRequested, .. })
            && let Some(requested) = redraw_requested_at.take()
        {
            present_gap.redraw_dispatch_ms += requested.elapsed().as_secs_f32() * 1000.0;
        }
        let _event_trace = gameplay_trace::EventScope::new(match &event {
            Event::WindowEvent { event: WindowEvent::RedrawRequested, .. } => "event-redraw",
            Event::WindowEvent { event: WindowEvent::Resized(_), .. } => "event-resize",
            Event::WindowEvent { .. } => "event-window",
            Event::DeviceEvent { .. } => "event-device",
            Event::AboutToWait => "event-wait",
            Event::NewEvents(_) => "event-new",
            _ => "event-other",
        });
        // Raw mouse packets can arrive hundreds of times per second. They
        // don't need the redraw/control-flow or multiplayer-mode machinery.
        if let Event::DeviceEvent { event, .. } = &event {
            if benchmark_config.enabled {
                present_gap.device_events += 1;
            }
            if let DeviceEvent::MouseMotion { delta } = event
                && !scripted_route
                && grabbed && focused && matches!(mode, AppMode::Playing)
                && matches!(startup, StartupState::Ready { .. })
            {
                let sensitivity = egui_ui.as_ref().map_or(0.0025, |ui| ui.mouse_sensitivity)
                    * view_zoom.sensitivity_scale();
                player.yaw += delta.0 as f32 * sensitivity;
                let y_sign = if egui_ui.as_ref().is_some_and(|ui| ui.invert_mouse) { -1.0 } else { 1.0 };
                player.pitch = (player.pitch - delta.1 as f32 * sensitivity * y_sign).clamp(-1.54, 1.54);
            }
            return;
        }
        // Follows the session as it starts and ends; reassigned after a switch within a frame.
        #[cfg(feature = "multiplayer")]
        #[allow(unused_assignments)]
        let mut network_mode = multiplayer::coop::Mode::of(multiplayer.as_ref()).network();
        let loading = !matches!(
            startup,
            StartupState::Ready { .. } | StartupState::Failed(_)
        );
        let active_gameplay = matches!(mode, AppMode::Playing);
        elwt.set_control_flow(if (benchmark_config.enabled || (!occluded && focused)) && (loading || active_gameplay) {
            ControlFlow::Poll
        } else {
            ControlFlow::Wait
        });

        match event {
            Event::Resumed => {
                if quiet_benchmark_input || scripted_route && !route_raw_input {
                    elwt.listen_device_events(DeviceEvents::Never);
                }
                if window.is_none() {
                    let created_window = Arc::new(
                        elwt.create_window(
                            WindowAttributes::default()
                                .with_title(if benchmark_config.enabled { "Axiom - UI Review" } else { "Axiom" })
                                .with_resizable(!benchmark_config.enabled || benchmark_config.resize_review)
                                .with_active(!benchmark_config.enabled)
                                .with_inner_size(initial_window_size),
                        )
                        .expect("failed to create window"),
                    );
                    // Windows clamps a decorated desktop-sized window to its
                    // work area. Match a monitor-sized review to the full
                    // monitor so a requested 1440p run really renders 1440p.
                    if benchmark_config.enabled && !benchmark_config.resize_review
                        && let Some(monitor) = created_window.current_monitor()
                            .filter(|m| m.size() == initial_window_size)
                        {
                            created_window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(Some(monitor))));
                        }
                    // Keep both rendered review clients visible. Overlapping,
                    // covered swapchains can be throttled by the compositor.
                    if benchmark_config.enabled
                        && let Ok(side) = std::env::var("VOXEL_REVIEW_WINDOW_SIDE")
                        && let Some(monitor) = created_window.current_monitor()
                    {
                        let origin = monitor.position();
                        let x = if side == "right" {
                            monitor.size().width.saturating_sub(created_window.outer_size().width + 8) as i32
                        } else { 8 };
                        created_window.set_outer_position(winit::dpi::PhysicalPosition::new(origin.x + x, origin.y + 48));
                        created_window.set_window_level(winit::window::WindowLevel::AlwaysOnTop);
                    }
                    window = Some(created_window);
                }

                if matches!(startup, StartupState::WaitingForWindow) {
                    let Some(window_ref) = window.as_ref() else {
                        return;
                    };

                    let bootstrap_started = Instant::now();
                    let bootstrap = pollster::block_on(GpuBootstrap::new(window_ref.clone()));
                    println!(
                        "[startup-timing] gpu_bootstrap ms={:.1}",
                        bootstrap_started.elapsed().as_secs_f64() * 1000.0
                    );
                    match bootstrap {
                        Ok(bootstrap) => {
                            egui_ui = Some(EguiUi::new(
                                window_ref,
                                bootstrap.device(),
                                bootstrap.surface_format(),
                            ));
                            if !benchmark_config.enabled {
                                if let Some(ui)=egui_ui.as_mut(){preferences.loaded.apply_ui(ui);}
                                if preferences.loaded.fullscreen {window_ref.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));}
                            }
                            startup = StartupState::BuildingRenderer {
                                pending: bootstrap.into_pending_init(),
                            };
                            window_ref.request_redraw();
                        }
                        Err(err) => {
                            eprintln!("[startup] {err:#}"); startup = StartupState::Failed(format!("{err:#}"));
                            elwt.exit();
                        }
                    }
                }
            }

            Event::WindowEvent { event, .. } => {
                let Some(window) = window.as_ref() else {
                    return;
                };
                // Benchmarks render while occluded: a fullscreen game in front
                // of the review window must not stall the horizon upload.
                if matches!(event, WindowEvent::RedrawRequested)
                    && ((occluded && !benchmark_config.enabled) || window.inner_size().width == 0 || window.inner_size().height == 0)
                {
                    last = Instant::now();
                    simulation_clock = simulation::SimulationClock::default();
                    return;
                }

                let egui_consumed = egui_ui
                    .as_mut()
                    .map(|ui| ui.on_window_event(window, &event))
                    .unwrap_or(false);
                gameplay_trace::frame_stage("window-ui-done");

                macro_rules! world_entry_press {
                    ($pressed:expr) => {{
                        if world_entry_pending && matches!(mode, AppMode::Playing) {
                            if $pressed && focused && world_entry_prompt {
                                world_entry_pending = false;
                                world_entry_prompt = false;
                                reset_gameplay_input(
                                    &mut input, &mut place_was_down, &mut break_was_down,
                                    &mut place_repeat_timer, &mut break_repeat_timer, &mut pending_tool_action,
                                );
                                capture_mouse(window, &mut grabbed, false, focused);
                                window.request_redraw();
                            }
                            true
                        } else {
                            false
                        }
                    }};
                }

                // Keyboard and pad presses share this dispatch for discrete actions.
                macro_rules! bound_action {
                    ($action:expr, $fresh_press:expr, $escape:expr, $tab:expr) => { 'bound: {
                        if hell_gate.busy() || hall_gate.busy() { break 'bound; }
                        let (action, fresh_press, escape, tab): (Option<BindAction>, bool, bool, bool) =
                            ($action, $fresh_press, $escape, $tab);
                        if fresh_press && action == Some(BindAction::Fullscreen) {
                            let fullscreen = if window.fullscreen().is_some() { None } else {
                                Some(winit::window::Fullscreen::Borderless(None))
                            };
                            window.set_fullscreen(fullscreen);
                            break 'bound;
                        }
                        if fresh_press && action == Some(BindAction::Debug) {
                            if let Some(ui) = egui_ui.as_mut() { ui.toggle_debug(); }
                            break 'bound;
                        }
                        if fresh_press && action == Some(BindAction::ToggleHud) {
                            if let Some(ui) = egui_ui.as_mut() { ui.show_hud = !ui.show_hud; }
                            break 'bound;
                        }
                        if fresh_press && action == Some(BindAction::Screenshot) {
                            pending_screenshot = Some(screenshot::next_path());
                            break 'bound;
                        }
                        if matches!(mode, AppMode::Paused)
                            && !escape { break 'bound; }
                        if matches!(mode,AppMode::WorldSelect) {break 'bound;}
                        match action {
                            _ if escape && fresh_press => {
                                player.jump_tap_timer = 0.0;
                                player.jump_was_down = false;
                                if matches!(mode, AppMode::Playing) && creative.any_ui_open() {
                                    if let Some(w) = world.as_mut() {
                                        #[cfg(feature="multiplayer")]
                                        if let Some(session)=multiplayer.as_mut(){session.queue_inventory(vec![multiplayer::inventory::Command::Return]);}
                                        if !network_mode {w.inventory.return_transient_to_pack();w.sync_from_inventory();}
                                    }
                                    creative.close_panels();
                                    input.clear();
                                    sync_creative_ui_cursor(window, &mut grabbed, focused, &mode, &creative);
                                    break 'bound;
                                }
                                reset_gameplay_input(
                                    &mut input,
                                    &mut place_was_down,
                                    &mut break_was_down,
                                    &mut place_repeat_timer,
                                    &mut break_repeat_timer,
                                    &mut pending_tool_action,
                                );
                                mode = match mode {
                                    AppMode::Playing => {
                                        release_mouse(window, &mut grabbed);
                                        AppMode::Paused
                                    }
                                    AppMode::Paused => {
                                        capture_mouse(window, &mut grabbed, false, focused);
                                        last = Instant::now();
                                        AppMode::Playing
                                    }
                                    AppMode::WorldSelect => AppMode::WorldSelect,
                                };
                            }
                            Some(BindAction::CreativePanel) if fresh_press => {
                                creative.toggle_creative_panel(); input.clear(); player.jump_tap_timer = 0.0;
                                sync_creative_ui_cursor(
                                    window,
                                    &mut grabbed,
                                    focused,
                                    &mode,
                                    &creative,
                                );
                            }
                            Some(BindAction::SwapHands) if fresh_press && grabbed && !creative.any_ui_open() => {
                                if let Some(w) = world.as_mut() {
                                    #[cfg(feature="multiplayer")]
                                    if let Some(session) = multiplayer.as_mut() { session.queue_inventory(vec![multiplayer::inventory::Command::Swap(world::inventory::SlotAddr::Offhand, creative.selected_slot)]); }
                                    if !network_mode {
                                        w.inventory.swap_slot_with_hotbar(world::inventory::SlotAddr::Offhand, creative.selected_slot);
                                        w.sync_from_inventory();
                                    }
                                }
                            }
                            Some(BindAction::Interact) if fresh_press && world::interaction::gameplay_enabled(focused, grabbed, creative.any_ui_open(), egui_ui.as_ref().is_some_and(|ui| ui.ctx.egui_wants_keyboard_input())) => {
                                if world.as_ref().is_some_and(|w| egui_ui::nav_console::focused(w, &player)) {
                                    egui_ui::system_map::toggle(&mut creative, world.as_ref());
                                    input.clear();
                                    sync_creative_ui_cursor(window, &mut grabbed, focused, &mode, &creative);
                                    break 'bound;
                                }
                                #[cfg(feature = "multiplayer")]
                                let realm_host = multiplayer.as_ref().is_some_and(multiplayer::Session::can_cross_realm);
                                #[cfg(not(feature = "multiplayer"))]
                                let realm_host = false;
                                #[cfg(feature = "multiplayer")]
                                if world.as_ref().is_some_and(|w| world::hell_gate::focused(w, &player))
                                    && multiplayer.as_mut().is_some_and(multiplayer::Session::request_gate) {
                                    break 'bound;
                                }
                                if (!network_mode || realm_host) && ca_window.is_none() && !benchmark_config.enabled
                                    && world.as_ref().is_some_and(|w| world::hall_gate::usable(w, &player) || world::hell_gate::focused(w, &player)) {
                                    #[cfg(feature = "multiplayer")]
                                    if let (Some(session), Some(w)) = (multiplayer.as_mut(), world.as_mut()) {
                                        if let Err(error) = session.begin_realm(w, &player) {
                                            w.expedition.say(&error.to_string());
                                            break 'bound;
                                        }
                                        realm_plan = None;
                                    }
                                    if world.as_ref().is_some_and(|w| world::hall_gate::usable(w, &player)) { hall_gate.request(&mut saves); }
                                    else { hell_gate.request(&mut saves); }
                                    input.clear();
                                    player.vel = Vec3::ZERO;
                                } else { input.interact = true; }
                            }
                            Some(BindAction::Lamp) if fresh_press && grabbed && !creative.any_ui_open() => {
                                #[cfg(feature="multiplayer")]
                                if let Some(session)=multiplayer.as_mut(){session.queue_lamp();}
                                if !network_mode && let Some(w) = world.as_mut() {w.expedition.toggle_lamp();}
                            }
                            Some(BindAction::NextSpell) if fresh_press && grabbed && !creative.any_ui_open() => { input.cycle_spell=true; }
                            Some(BindAction::Inventory)
                                if fresh_press && (!tab || !creative.show_palette) => {
                                let open = !creative.show_palette;
                                if !open
                                    && let Some(w) = world.as_mut() {
                                        #[cfg(feature="multiplayer")]
                                        if let Some(session)=multiplayer.as_mut(){session.queue_inventory(vec![multiplayer::inventory::Command::Return]);}
                                        if !network_mode {w.inventory.return_transient_to_pack();w.sync_from_inventory();}
                                    }
                                creative.close_panels(); creative.show_palette = open; input.clear(); player.jump_tap_timer = 0.0;
                                sync_creative_ui_cursor(
                                    window,
                                    &mut grabbed,
                                    focused,
                                    &mode,
                                    &creative,
                                );
                            }
                            Some(BindAction::WandAtelier) if fresh_press && !creative.show_palette => {
                                let open = !creative.show_wand_atelier; creative.close_panels(); creative.show_wand_atelier = open; input.clear(); player.jump_tap_timer = 0.0;
                                sync_creative_ui_cursor(
                                    window,
                                    &mut grabbed,
                                    focused,
                                    &mode,
                                    &creative,
                                );
                            }
                            Some(BindAction::QuickSelect) if fresh_press && !creative.show_palette => {
                                creative.toggle_quick_select(); input.clear(); player.jump_tap_timer = 0.0;
                                sync_creative_ui_cursor(
                                    window,
                                    &mut grabbed,
                                    focused,
                                    &mode,
                                    &creative,
                                );
                            }
                            Some(BindAction::Map) if fresh_press && !creative.show_palette => {
                                // One key for every map: the hub opens the region, planet or
                                // system view that fits the player's height.
                                egui_ui::map_hub::toggle(&mut creative, world.as_ref());
                                input.clear(); player.jump_tap_timer = 0.0;
                                sync_creative_ui_cursor(window, &mut grabbed, focused, &mode, &creative);
                            }
                            Some(BindAction::SystemMap) if fresh_press && !creative.show_palette => {
                                egui_ui::system_map::toggle(&mut creative, world.as_ref()); input.clear(); player.jump_tap_timer = 0.0;
                                sync_creative_ui_cursor(window, &mut grabbed, focused, &mode, &creative);
                            }
                            Some(BindAction::NavTarget) if fresh_press && grabbed && !creative.any_ui_open() => {
                                if let Some(w) = world.as_ref() { space_nav::cycle_target(w, &player); }
                            }
                            Some(BindAction::Waypoint) if fresh_press && grabbed && !creative.any_ui_open() => {
                                if let Some(index) = egui_ui::map::add_waypoint_at_player()
                                    && let Some(ui) = egui_ui.as_mut() {
                                        ui.quality_notice = Some((format!("Waypoint {}", index + 1), 2.0));
                                    }
                            }
                            Some(BindAction::Minimap) if fresh_press => egui_ui::map::toggle_minimap(),
                            Some(BindAction::GoalTrack) if fresh_press => egui_ui::goals_hud::toggle(),
                            Some(bound @ (BindAction::RecipeLookup | BindAction::UsesLookup))
                                if fresh_press && grabbed && !creative.any_ui_open() => {
                                let item = world.as_ref().and_then(|w| {
                                    let target = egui_ui.as_ref().and_then(|ui| ui.target_block)
                                        .filter(|block| *block != BlockId::Air)
                                        .and_then(world::inventory::Item::canonical_block)
                                        .map(world::inventory::Item::Block);
                                    let held = w.inventory.hotbar[creative.selected_slot].map(|s| s.item);
                                    if bound == BindAction::RecipeLookup {
                                        let goal = w.goals.current(&world::goals::GOALS)
                                            .and_then(|goal| goal.condition.target()).map(|(item, _)| item);
                                        goal.or(target).or(held)
                                    } else {
                                        target.or(held)
                                    }
                                });
                                if let (Some(item), Some(ui)) = (item, egui_ui.as_ref()) {
                                    egui_ui::show_recipe(&ui.ctx, item, bound == BindAction::UsesLookup);
                                    creative.close_panels(); creative.show_palette = true; input.clear(); player.jump_tap_timer = 0.0;
                                    sync_creative_ui_cursor(window, &mut grabbed, focused, &mode, &creative);
                                }
                            }
                            Some(BindAction::Records) if fresh_press && !creative.show_palette => {
                                let open = !creative.show_records; creative.close_panels(); creative.show_records = open; input.clear();
                                sync_creative_ui_cursor(window, &mut grabbed, focused, &mode, &creative);
                            }
                            Some(BindAction::Blueprints) if fresh_press && !creative.show_palette => {
                                let open = !creative.show_blueprints; creative.close_panels(); creative.show_blueprints = open; input.clear();
                                sync_creative_ui_cursor(window, &mut grabbed, focused, &mode, &creative);
                            }
                            Some(bound @ (BindAction::BlueprintCorner | BindAction::BlueprintCopy | BindAction::BlueprintCut
                                | BindAction::BlueprintPaste | BindAction::BlueprintRotate | BindAction::BlueprintMirror))
                                if fresh_press && grabbed && !creative.any_ui_open() => blueprints.press(bound),
                            Some(bound @ (BindAction::Undo | BindAction::Redo))
                                if fresh_press && grabbed && !creative.any_ui_open() => {
                                let direction = if bound == BindAction::Undo { world::edit_history::Direction::Undo } else { world::edit_history::Direction::Redo };
                                if let Some(w) = world.as_mut() {
                                    if direction == world::edit_history::Direction::Undo && blueprints.cancel_paste() {
                                        w.expedition.say("Paste Cancelled");
                                    } else if network_mode {
                                        #[cfg(feature="multiplayer")]
                                        if let Some(session) = multiplayer.as_mut() { session.queue_edit_history(direction); }
                                    } else if creative.locked() {
                                        w.expedition.say("Undo: Creative Only");
                                    } else if !edit_history.request(direction) {
                                        w.expedition.say("Edit History Busy");
                                    }
                                }
                            }
                            Some(BindAction::RangeDown | BindAction::RangeUp)
                                if fresh_press && grabbed && !creative.any_ui_open() => {
                                creative.adjust_range(if action == Some(BindAction::RangeUp) { 1 } else { -1 });
                            }
                            Some(BindAction::ToolHand) if fresh_press && !creative.any_ui_open() => {
                                creative.tool = creative::ToolKind::Hand;
                            }
                            Some(BindAction::ToolBrush) if fresh_press && !creative.any_ui_open() => {
                                creative.choose_tool(creative::ToolKind::Brush);
                            }
                            Some(BindAction::ToolBuilders) if fresh_press && !creative.any_ui_open() => {
                                creative.choose_tool(creative::ToolKind::BuildersWand);
                            }
                            Some(BindAction::ToolDestructor) if fresh_press && !creative.any_ui_open() => {
                                creative.choose_tool(creative::ToolKind::DestructorWand);
                            }
                            Some(BindAction::ToolArea) if fresh_press && !creative.any_ui_open() => {
                                creative.choose_tool(creative::ToolKind::AreaTool);
                            }
                            Some(BindAction::ToolExplosive) if fresh_press && !creative.any_ui_open() => {
                                creative.choose_tool(creative::ToolKind::ExplosiveWand);
                            }
                            Some(BindAction::HotbarPrev | BindAction::HotbarNext)
                                if fresh_press && grabbed && !creative.any_ui_open() => {
                                let dir = if action == Some(BindAction::HotbarNext) { 1 } else { -1 };
                                player.selected_block = creative.cycle_hotbar_slot(dir);
                            }
                            Some(bound) if fresh_press => {
                                if let Some(slot) = bound.hotbar_slot() {
                                    player.selected_block = creative.select_slot(slot);
                                    if creative.show_palette { creative.tool = creative::ToolKind::Hand; }
                                }
                            }
                            _ => {}
                        }
                    } };
                }

                if matches!(event, WindowEvent::RedrawRequested) && matches!(startup, StartupState::Ready { .. }) {
                    gameplay_trace::frame_stage("input");
                    let text_editing = egui_ui.as_ref().is_some_and(|ui| ui.ctx.egui_wants_keyboard_input());
                    for (button, pressed) in pads.poll(focused) {
                        if world_entry_press!(pressed) { return; }
                        let binding = keybinds::Binding::Pad(button);
                        if pressed && keybinds::offer(binding) {
                            continue;
                        }
                        if !pressed || (matches!(mode, AppMode::Playing) && grabbed && !creative.any_ui_open()) {
                            input.pad(button, pressed);
                        }
                        if pressed && !text_editing {
                            // Menu is fixed like Escape; B also backs out of open panels.
                            let escape = button == keybinds::PadButton::Start
                                || (button == keybinds::PadButton::East && creative.any_ui_open());
                            bound_action!(keybinds::lookup(binding).map(|(a, _)| a), true, escape, false);
                        }
                    }
                    if grabbed && focused && matches!(mode, AppMode::Playing) {
                        let sensitivity = egui_ui.as_ref().map_or(0.0025, |ui| ui.mouse_sensitivity)
                            * view_zoom.sensitivity_scale();
                        let look = pads.look(sensitivity);
                        let y_sign = if egui_ui.as_ref().is_some_and(|ui| ui.invert_mouse) { -1.0 } else { 1.0 };
                        player.yaw += look.x;
                        player.pitch = (player.pitch + look.y * y_sign).clamp(-1.54, 1.54);
                    }
                }

                gameplay_trace::frame_stage("event-dispatch");
                match event {
                    WindowEvent::CloseRequested => {
                        // A hosted world closes to single-player, then saves like any world.
                        #[cfg(feature = "multiplayer")]
                        if multiplayer::coop::Mode::of(multiplayer.as_ref()) == multiplayer::coop::Mode::Opened {
                            multiplayer::coop::end(&mut multiplayer, world.as_mut(), &player, &day_night, None);
                            network_mode = false;
                        }
                        if ca_window.as_ref().is_some_and(|session| session.loaded.is_some()) {
                            elwt.exit();
                        } else if network_mode {
                            #[cfg(feature = "multiplayer")]
                            if let (Some(session),Some(w))=(multiplayer.as_mut(),world.as_mut()) && session.persistent_host()
                                && let Err(error)=session.save(w,&player,&day_night) {eprintln!("[multiplayer] final save failed: {error:#}");}
                            elwt.exit();
                        }
                        else if !benchmark_config.enabled {
                            mode = AppMode::Paused;
                            release_mouse(window, &mut grabbed);
                            exit_pending = true;
                            if world.is_some() {
                                if let Some(session) = ca_window.as_mut() { session.request(); }
                                else { saves.request(true); }
                            }
                        } else {elwt.exit();}
                    },

                    WindowEvent::Resized(size) => {
                        if window.fullscreen().is_none() && size.width>=640 && size.height>=480 {windowed_size=(size.width,size.height);}
                        match &mut startup {
                        StartupState::BuildingRenderer { pending } => pending.resize(size),
                        StartupState::BuildingChunkStreamer { gpu } => gpu.resize(size),
                        StartupState::BuildingWorldSimulation { gpu, .. } => gpu.resize(size),
                        StartupState::Ready { gpu, .. } => gpu.resize(size),
                        _ => {}
                        }
                    },

                    WindowEvent::Occluded(value) => {
                        occluded = value;
                        last = Instant::now();
                        simulation_clock = simulation::SimulationClock::default();
                    }
                    WindowEvent::Focused(is_focused) => {
                        focused = is_focused;
                        combat_audio.set_active(is_focused && matches!(mode,AppMode::Playing));
                        if !focused && world.is_some() && !benchmark_config.enabled {
                            player.jump_tap_timer = 0.0;
                            player.jump_was_down = false;
                            reset_gameplay_input(
                                &mut input,
                                &mut place_was_down,
                                &mut break_was_down,
                                &mut place_repeat_timer,
                                &mut break_repeat_timer,
                                &mut pending_tool_action,
                            );
                            if !world_entry_pending {
                                mode = AppMode::Paused;
                            }
                            release_mouse(window, &mut grabbed);
                        }
                    }

                    WindowEvent::KeyboardInput { event: key, .. } => {
                        if !matches!(startup, StartupState::Ready { .. }) {
                            if key.state == ElementState::Pressed
                                && matches!(key.physical_key, PhysicalKey::Code(KeyCode::Escape))
                            {
                                elwt.exit();
                            }
                            return;
                        }

                        let pressed = key.state == ElementState::Pressed;
                        let fresh_press = pressed && !key.repeat;
                        if world_entry_press!(fresh_press) { return; }
                        // A pending rebind on the controls screen takes the next press, Escape included.
                        if let PhysicalKey::Code(code) = key.physical_key
                            && fresh_press && keybinds::offer(keybinds::Binding::Key(code)) {
                                return;
                            }
                        // Ctrl chords sit beside the bindings: Ctrl is sprint and Z a tool key.
                        let action = match key.physical_key {
                            PhysicalKey::Code(KeyCode::KeyZ) if input.ctrl_down && !input.shift_down() => Some(BindAction::Undo),
                            PhysicalKey::Code(KeyCode::KeyZ | KeyCode::KeyY) if input.ctrl_down => Some(BindAction::Redo),
                            // JEI's R and U, in play: Alt keeps them clear of Cast and the Wand Atelier.
                            PhysicalKey::Code(KeyCode::KeyR) if input.alt_down => Some(BindAction::RecipeLookup),
                            PhysicalKey::Code(KeyCode::KeyU) if input.alt_down => Some(BindAction::UsesLookup),
                            PhysicalKey::Code(code) => keybinds::action_for_key(code),
                            PhysicalKey::Unidentified(_) => None,
                        };
                        let lookup_chord = input.alt_down
                            && matches!(key.physical_key, PhysicalKey::Code(KeyCode::KeyR | KeyCode::KeyU));
                        // Alt+F6 opens the HUD editor: the mouse is freed and the HUD keeps
                        // drawing, so elements can be dragged in place (hud_layout::editor).
                        if fresh_press && input.alt_down && matches!(mode, AppMode::Playing)
                            && key.physical_key == PhysicalKey::Code(KeyCode::F6) && !creative.any_ui_open() {
                            let on = !egui_ui::hud_layout::editing();
                            egui_ui::hud_layout::set_editing(on);
                            input.clear();
                            if on { release_mouse(window, &mut grabbed); } else { capture_mouse(window, &mut grabbed, false, focused); }
                            return;
                        }
                        // Releases always reach gameplay; presses belong to the active surface.
                        if let PhysicalKey::Code(code) = key.physical_key
                            && (!pressed || (matches!(mode, AppMode::Playing) && grabbed && !egui_consumed && !lookup_chord)) {
                                input.keyboard(code, pressed);
                            }
                        let text_editing = egui_ui.as_ref().is_some_and(|ui| ui.ctx.egui_wants_keyboard_input());
                        if text_editing && !matches!(key.physical_key, PhysicalKey::Code(KeyCode::Escape)) {
                            return;
                        }
                        // The chat field handles its own Enter and Escape.
                        if chat_ui.is_open() {
                            return;
                        }
                        if fresh_press && matches!(mode, AppMode::Playing) && grabbed && !creative.any_ui_open()
                            && matches!(key.physical_key, PhysicalKey::Code(KeyCode::KeyT | KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Slash))
                        {
                            chat_ui.open(if key.physical_key == PhysicalKey::Code(KeyCode::Slash) { "/" } else { "" });
                            input.clear();
                            release_mouse(window, &mut grabbed);
                            return;
                        }
                        // Escape is fixed: it is never offered as a binding.
                        let escape = key.physical_key == PhysicalKey::Code(KeyCode::Escape);
                        let tab = key.physical_key == PhysicalKey::Code(KeyCode::Tab);
                        bound_action!(action, fresh_press, escape, tab);
                    }

                    WindowEvent::MouseInput { state, button, .. } => {
                        if !matches!(startup, StartupState::Ready { .. }) {
                            return;
                        }

                        let pressed = state == ElementState::Pressed;
                        if world_entry_press!(pressed) { return; }
                        if pressed && keybinds::offer(keybinds::Binding::Mouse(button)) {
                            return;
                        }
                        // Releases always reach gameplay so a held action cannot stick.
                        if !pressed {
                            input.mouse(button, false);
                            return;
                        }
                        if egui_consumed {
                            return;
                        }
                        if matches!(mode, AppMode::Playing) && grabbed && !creative.any_ui_open() {
                            input.mouse(button, true);
                        }
                    }

                    WindowEvent::MouseWheel { delta, .. } => {
                        if !matches!(startup, StartupState::Ready { .. }) {
                            return;
                        }
                        if egui_consumed {
                            return;
                        }
                        let delta = match delta {
                            MouseScrollDelta::LineDelta(_, y) => y,
                            MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0,
                        };
                        if !matches!(mode,AppMode::WorldSelect) {input.wheel += delta;}
                    }

                    WindowEvent::RedrawRequested => match &mut startup {
                        StartupState::Failed(err) => {
                            eprintln!("Startup failed: {err}");
                        }

                        StartupState::BuildingRenderer { pending } => {
                            let Some(egui_ui) = egui_ui.as_mut() else {
                                return;
                            };

                            let progress = pending.progress();
                            let loading_ui = LoadingUiState {
                                title: progress.detail.to_string(),
                                progress_01: Some(progress.progress_01),
                            };

                            egui_ui.begin_frame(window.as_ref());
                            egui_ui.build_loading_ui(&loading_ui);

                            match pending.render_loading_frame(
                                [0.06, 0.07, 0.09],
                                |device, queue, encoder, target_view, width, height| {
                                    egui_ui.end_frame(
                                        window.as_ref(),
                                        device,
                                        queue,
                                        encoder,
                                        target_view,
                                        width,
                                        height,
                                    );
                                },
                            ) {
                                Ok(()) => {}
                                Err(render::SurfaceFrameError::Lost | render::SurfaceFrameError::Outdated) => {
                                    egui_ui.end_frame_no_paint(window.as_ref());
                                    pending.resize(pending.size());
                                }
                                Err(render::SurfaceFrameError::Timeout)
                                | Err(render::SurfaceFrameError::Other) => {
                                    egui_ui.end_frame_no_paint(window.as_ref());
                                }
                            }

                            match pending.step() {
                                Ok(Some(gpu)) => {
                                    startup = StartupState::BuildingChunkStreamer { gpu };
                                }
                                Ok(None) => {
                                    window.request_redraw();
                                }
                                Err(err) => {
                                    eprintln!("[startup] {err:#}"); startup = StartupState::Failed(format!("{err:#}"));
                                    elwt.exit();
                                }
                            }
                        }

                        StartupState::BuildingChunkStreamer { .. } => {
                            let Some(egui_ui) = egui_ui.as_mut() else {
                                return;
                            };

                            let mut gpu = match std::mem::replace(
                                &mut startup,
                                StartupState::WaitingForWindow,
                            ) {
                                StartupState::BuildingChunkStreamer { gpu } => gpu,
                                other => {
                                    startup = other;
                                    return;
                                }
                            };

                            egui_ui.begin_frame(window.as_ref());
                            egui_ui.build_loading_ui(&LoadingUiState {
                                title: "Starting".to_string(),
                                progress_01: Some(0.88),
                            });

                            let render_result = gpu.render_loading_frame(
                                [0.06, 0.07, 0.09],
                                |device, queue, encoder, target_view, width, height| {
                                    egui_ui.end_frame(
                                        window.as_ref(),
                                        device,
                                        queue,
                                        encoder,
                                        target_view,
                                        width,
                                        height,
                                    );
                                },
                            );

                            if gpu.device_usable() && matches!(&render_result,
                                Err(render::SurfaceFrameError::Timeout | render::SurfaceFrameError::Other
                                    | render::SurfaceFrameError::Lost | render::SurfaceFrameError::Outdated)) {
                                egui_ui.end_frame_no_paint(window.as_ref());
                                if matches!(&render_result, Err(render::SurfaceFrameError::Lost | render::SurfaceFrameError::Outdated)) {
                                    gpu.resize(window.inner_size());
                                }
                                startup = StartupState::BuildingChunkStreamer { gpu };
                                window.request_redraw();
                                return;
                            }
                            let render_result = render_result.map_err(anyhow::Error::from).and_then(|()| ChunkStreamer::new(&gpu.device, mesh_workers));

                            match render_result {
                                Ok(chunk_streamer) => {
                                    startup = StartupState::BuildingWorldSimulation {
                                        gpu,
                                        chunk_streamer,
                                    };
                                    window.request_redraw();
                                }
                                Err(err) => {
                                    eprintln!("[startup] {err:#}"); startup = StartupState::Failed(format!(
                                        "Renderer or mesh-worker startup failed: {err:#}"
                                    ));
                                    elwt.exit();
                                }
                            }
                        }

                        StartupState::BuildingWorldSimulation { .. } => {
                            let Some(egui_ui) = egui_ui.as_mut() else {
                                return;
                            };

                            let (mut gpu, chunk_streamer) = match std::mem::replace(
                                &mut startup,
                                StartupState::WaitingForWindow,
                            ) {
                                StartupState::BuildingWorldSimulation {
                                    gpu,
                                    chunk_streamer,
                                } => (gpu, chunk_streamer),
                                other => {
                                    startup = other;
                                    return;
                                }
                            };

                            egui_ui.begin_frame(window.as_ref());
                            egui_ui.build_loading_ui(&LoadingUiState {
                                title: "Starting".to_string(),
                                progress_01: Some(0.96),
                            });

                            let render_result = gpu.render_loading_frame(
                                [0.06, 0.07, 0.09],
                                |device, queue, encoder, target_view, width, height| {
                                    egui_ui.end_frame(
                                        window.as_ref(),
                                        device,
                                        queue,
                                        encoder,
                                        target_view,
                                        width,
                                        height,
                                    );
                                },
                            );

                            if gpu.device_usable() && matches!(&render_result,
                                Err(render::SurfaceFrameError::Timeout | render::SurfaceFrameError::Other
                                    | render::SurfaceFrameError::Lost | render::SurfaceFrameError::Outdated)) {
                                egui_ui.end_frame_no_paint(window.as_ref());
                                if matches!(&render_result, Err(render::SurfaceFrameError::Lost | render::SurfaceFrameError::Outdated)) {
                                    gpu.resize(window.inner_size());
                                }
                                startup = StartupState::BuildingWorldSimulation { gpu, chunk_streamer };
                                window.request_redraw();
                                return;
                            }
                            let mut fluid_sim = WorldSimulation::new();
                            fluid_sim.reset_for_world(selected_voxel_scale);

                            match render_result {
                                Ok(()) => {
                                    println!(
                                        "[startup-timing] ready total_s={:.2}",
                                        startup_started.elapsed().as_secs_f64()
                                    );
                                    startup = StartupState::Ready {
                                        gpu,
                                        chunk_streamer,
                                        fluid_sim,
                                    };
                                    window.request_redraw();
                                }
                                Err(err) => {
                                    eprintln!("[startup] {err:#}"); startup = StartupState::Failed(format!(
                                        "Renderer or mesh-worker startup failed: {err:#}"
                                    ));
                                    elwt.exit();
                                }
                            }
                        }

                        StartupState::Ready {
                            gpu,
                            chunk_streamer,
                            fluid_sim,
                        } => {
                            let Some(egui_ui) = egui_ui.as_mut() else {
                                return;
                            };

                            if let Some(retry_at) = gpu_retry_at {
                                let now = Instant::now();
                                if now < retry_at && gpu.device_usable() {
                                    return;
                                }
                                gpu_retry_at = None;
                                if benchmark_config.enabled {
                                    present_gap.retry_late_ms += now.saturating_duration_since(retry_at).as_secs_f32() * 1000.0;
                                }
                                // Keep short retries cheap, but let a prolonged GPU
                                // stall service gameplay at the fixed-step cadence.
                                if gpu_retry_simulation_at.is_some_and(|deadline| now < deadline)
                                    && gpu.device_usable()
                                    && !gpu.ready_for_frame()
                                    && gpu.device_usable()
                                {
                                    let elapsed = now.elapsed().as_secs_f32() * 1000.0;
                                    if benchmark_config.enabled {
                                        present_gap.count += 1;
                                        present_gap.wait_ms += elapsed;
                                        present_gap.redraw_ms += elapsed;
                                    }
                                    gpu_retry_at = Some(Instant::now() + Duration::from_millis(1));
                                    return;
                                }
                                gpu_retry_simulation_at = None;
                            }

                            // Benchmark worlds use the same asynchronous geology preparation
                            // as the world picker, so the real evolving planet stays visible.
                            let preparing_planet = pending_planet.as_ref().map(|(cfg, _)| *cfg)
                                .or_else(|| benchmark.as_ref().filter(|b| !b.auto_started && world.is_none()
                                    && b.config.world_type == WorldType::Planet).map(|b| engine_voxel::WorldSampleConfig {
                                        generator_version: new_planet_generator(false), seed: b.config.seed,
                                        world_type: WorldType::Planet, voxel_scale: b.config.voxel_scale, planet: None,
                                    }));
                            if !exit_pending && let Some(cfg) = preparing_planet
                                && engine_worldgen::planetary::drift_active(&cfg) {
                                let entry = engine_worldgen::geo::entry_for(&cfg);
                                if !entry.ready() {
                                    engine_worldgen::geo::prewarm_for(cfg);
                                    egui_ui.begin_frame(window.as_ref());
                                    egui_ui.build_world_loading_ui(&loading_report::preparing(&cfg, &entry));
                                    if benchmark_config.enabled && let Some(path) = terrain_review::generation_capture() {
                                        gpu.request_capture(path);
                                    }
                                    let result = gpu.render_loading_frame(
                                        [0.025, 0.035, 0.05],
                                        |device, queue, encoder, target_view, width, height| {
                                            egui_ui.end_frame(window.as_ref(), device, queue, encoder, target_view, width, height);
                                        },
                                    );
                                    if let Err(error) = result {
                                        egui_ui.end_frame_no_paint(window.as_ref());
                                        match error {
                                            render::SurfaceFrameError::Lost | render::SurfaceFrameError::Outdated => gpu.resize(window.inner_size()),
                                            render::SurfaceFrameError::Timeout => {
                                                gpu_retry_at = Some(Instant::now() + Duration::from_millis(1));
                                            }
                                            _ => {}
                                        }
                                    }
                                    return;
                                }
                            }

                            // Cold graphics initialization can outlast the server's handshake
                            // deadline. Start connecting only once we can service the session.
                            #[cfg(feature = "multiplayer")]
                            if multiplayer_start_pending {
                                multiplayer_start_pending = false;
                                match multiplayer::Session::start() {
                                    Ok(Some((session, connected_world, connected_player, connected_day))) => {
                                        selected_world = connected_world.world_type;
                                        selected_voxel_scale = connected_world.voxel_scale;
                                        selected_seed = connected_world.seed;
                                        world = Some(connected_world);
                                        player = connected_player;
                                        day_night = connected_day;
                                        cloud_weather = WeatherRuntimeState::new(selected_seed);
                                        mode = AppMode::Playing;
                                        multiplayer = Some(session);
                                        network_mode = true;
                                        if let Some(review) = benchmark.as_mut() {
                                            gpu.set_vsync(terrain_review::option("VOXEL_BENCHMARK_NORMAL_PRESENT") && preferences.loaded.vsync);
                                            review.auto_started = true;
                                            review.phase_started_at = Instant::now();
                                            quality::set_auto_enabled(false);
                                            if let Some(view_distance_chunks) = review.config.view_distance_chunks {
                                                set_render_distance_settings(render::RenderDistanceSettings {
                                                    view_distance_chunks,
                                                    ..render_distance_settings()
                                                });
                                            }
                                        }
                                    }
                                    Ok(None) => {}
                                    Err(err) => { eprintln!("[multiplayer] startup failed: {err:#}"); elwt.exit(); return; }
                                }
                            }
                            if !preferences_render_applied {
                                if !benchmark_config.enabled && !low_spec::active() && render::integrated_gpu() {
                                    low_spec::apply_integrated(&mut preferences.loaded, !preferences.from_file);
                                    preferences.loaded.apply_globals();
                                }
                                if !benchmark_config.enabled {
                                    gpu.set_vsync(preferences.loaded.vsync);
                                    gpu.fxaa_enabled=preferences.loaded.fxaa;
                                    atmosphere_mode=preferences.loaded.atmosphere();
                                }
                                preferences_render_applied=true;
                            }
                            #[cfg(feature = "multiplayer")]
                            if network_mode && !network_stream_initialized && let Some(w)=world.as_mut() {
                                let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                chunk_cache.reset_with_snapshot(center,&w.snapshot_near(center,render_distance_settings().view_distance_chunks));
                                chunk_streamer.reset();
                                chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                fluid_sim.reset_for_world(w.voxel_scale);
                                network_stream_initialized=true;
                            }
                            #[cfg(feature = "multiplayer")]
                            if ca_window.is_none() && !exit_pending && !hell_gate.busy() && !hall_gate.busy() {
                                #[cfg(feature = "multiplayer-sdr")]
                                if let Some(code) = startup_invite.take()
                                    .or_else(|| steam_invites.as_mut().and_then(|incoming| incoming.poll(Instant::now())))
                                    && multiplayer.is_none() && !coop.busy()
                                {
                                    coop.join(&code);
                                    egui_ui.show_friends();
                                    if matches!(mode, AppMode::Playing) {
                                        mode = AppMode::Paused;
                                        release_mouse(window, &mut grabbed);
                                    }
                                }
                                match coop.poll(world.as_mut(), !saves.busy(), saves.failed) {
                                    Some(multiplayer::coop::Event::Hosted(mut session)) => {
                                        session.exchange_simulation(fluid_sim);
                                        if let Some(code) = session.code() {
                                            egui_ui.quality_notice = Some((format!("Session Code {code}"), 8.0));
                                        }
                                        multiplayer = Some(session);
                                        network_ready_reported = false;
                                        network_ready_at = None;
                                        chat_ui = egui_ui::chat::ChatUi::default();
                                    }
                                    Some(multiplayer::coop::Event::SaveBeforeJoin) => {
                                        coop.return_to = world.as_ref().map(|w| w.regions.view.session_name().to_owned());
                                        mode = AppMode::Paused;
                                        release_mouse(window, &mut grabbed);
                                        saves.request(true);
                                    }
                                    Some(multiplayer::coop::Event::Joined(joined)) => {
                                        let (session, joined_world, joined_player, joined_day) = *joined;
                                        // Installed by the world-list load path below, in this frame.
                                        pending_loaded = Some(Ok(world::storage::SavedSession {
                                            world: joined_world,
                                            player: joined_player,
                                            day_night: joined_day,
                                            recovery_notice: Some("Joined".into()),
                                        }));
                                        multiplayer = Some(session);
                                        mode = AppMode::WorldSelect;
                                        network_stream_initialized = true;
                                        network_ready_reported = false;
                                        network_ready_at = None;
                                        chat_ui = egui_ui::chat::ChatUi::default();
                                    }
                                    None => {}
                                }
                                network_mode = multiplayer::coop::Mode::of(multiplayer.as_ref()).network();
                                egui_ui.coop.view = coop.view(multiplayer.as_ref(), world.is_some());
                            }
                            if let Some(mut restored) = ca_window.as_mut().and_then(|session| session.loaded.take()) {
                                selected_world = restored.saved.world.world_type;
                                let old_scale = selected_voxel_scale.meters();
                                selected_voxel_scale = restored.saved.world.voxel_scale;
                                selected_seed = restored.saved.world.seed;
                                set_render_distance_settings(render_distance_settings().rescale_blocks(old_scale, selected_voxel_scale.meters()));
                                fluid_sim.reset_for_world(selected_voxel_scale);
                                fluid_sim.player_tick = engine_core::time::TickId(restored.player_tick);
                                restored.saved.world.publish_ca_view(restored.region.published.clone(), &mut dirty_blocks, &mut dirty_chunks);
                                restored.region.publish_reservations(&mut restored.saved.world);
                                creative.select_slot(restored.region.tool_slot);
                                creative.tool = creative::ToolKind::Hand;
                                fluid_sim.cpu_region = Some(restored.region);
                                player = restored.saved.player;
                                day_night = restored.saved.day_night;
                                input = restored.input;
                                let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                chunk_cache.reset_with_snapshot(center, &restored.saved.world.snapshot_near(center, render_distance_settings().view_distance_chunks));
                                chunk_streamer.reset();
                                chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                world = Some(restored.saved.world);
                                cloud_weather.reset(selected_seed);
                                physics_enabled = true;
                                mode = AppMode::Playing;
                                last = Instant::now();
                                simulation_clock = simulation::SimulationClock::default();
                                capture_mouse(window, &mut grabbed, false, focused);
                                egui_ui.persistence_status = ca_window.as_ref().and_then(|s| s.status.clone());
                            }
                            // A world-list load queued before play started must not replace the live world.
                            if !matches!(mode, AppMode::WorldSelect) && pending_loaded.is_some() {
                                eprintln!("[library] dropped stale world load outside the world list");
                                pending_loaded = None;
                            }
                            if !exit_pending && ca_window.is_none()
                                && let Some(result)=pending_loaded.take() {
                                    match result {
                                        Ok(mut saved) => {
                                            selected_world = saved.world.world_type;
                                            let old_scale=selected_voxel_scale.meters();
                                            selected_voxel_scale = saved.world.voxel_scale;
                                            set_render_distance_settings(render_distance_settings().rescale_blocks(old_scale,selected_voxel_scale.meters()));
                                            selected_seed = saved.world.seed;
                                            let center = chunk_coord_from_world_block(saved.player.pos.floor().as_ivec3());
                                            let snapshot = saved.world.snapshot_near(center, render_distance_settings().view_distance_chunks);
                                            chunk_streamer.reset();
                                            chunk_cache.reset_with_snapshot(center, &snapshot);
                                            chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                            fluid_sim.reset_for_world(selected_voxel_scale);

                                            player = saved.player;
                                            day_night = saved.day_night;
                                            creative.cancel_edits();
                                            world = Some(saved.world);
                                                                                cloud_weather.reset(selected_seed);
                                            gpu.invalidate_cloud_history();
                                            dirty_blocks.clear();
                                            dirty_chunks.clear();
                                            reset_gameplay_input(&mut input, &mut place_was_down, &mut break_was_down,
                                                &mut place_repeat_timer, &mut break_repeat_timer, &mut pending_tool_action);
                                            physics_enabled = true;
                                            freeze_time = false;
                                            mode = AppMode::Playing;
                                            capture_mouse(window, &mut grabbed, false, focused);
                                            last = Instant::now();
                                            simulation_clock = simulation::SimulationClock::default();
                                            egui_ui.persistence_status = Some(saved.recovery_notice.unwrap_or_else(||"Saved world restored".into()));
                                            saves.status = egui_ui.persistence_status.clone();
                                        }
                                        Err(error)=>{egui_ui.persistence_status=Some(format!("Cannot open world: {error:#}"));saves.status=egui_ui.persistence_status.clone();}
                                    }
                                }
                            let frame_start = Instant::now();
                            gameplay_trace::frame_stage("frame-start");
                            let now = Instant::now();
                            let raw_frame_dt = (now - last).as_secs_f32();
                            let display_frame_dt = raw_frame_dt.min(0.25);
                            last = now;
                            view_zoom.update(input.zoom && matches!(mode, AppMode::Playing), display_frame_dt);
                            if let Some(text) = screenshot_notice.take() {
                                egui_ui.quality_notice = Some((text, 3.0));
                            }
                            if let Some(path) = pending_screenshot.take() {
                                screenshot_notice = Some(format!(
                                    "Screenshot {}",
                                    path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                                ));
                                gpu.request_capture(path);
                            }
                            if !gpu.horizon_ready() && world.as_ref().is_some_and(|w| gpu.horizon_budget_exhausted(w)) {
                                let message = "Far terrain cannot fit its admitted publication budget";
                                if benchmark_config.enabled {
                                    eprintln!("[benchmark] FAIL horizon_terminal_budget=true");
                                    render::flush_review_captures();
                                    elwt.exit();
                                    return;
                                }
                                // Keep the loaded session intact, but leave the unfinishable
                                // loading transition with an honest, visible library error.
                                egui_ui.persistence_status = Some(message.into());
                                mode = AppMode::WorldSelect;
                                release_mouse(window, &mut grabbed);
                            }
                            let spawn_progress = chunk_cache.spawn_progress();
                            // Give the first distant terrain a short head start once
                            // nearby chunks are ready. Foliage fills in during play.
                            // Benchmarks retain the full-detail loading barrier.
                            let horizon_pending = world.is_some() && chunk_cache.spawn_ready
                                && !gpu.horizon_ready() && matches!(mode, AppMode::Playing);
                            let horizon_hold = quality::hold_for_horizon(horizon_pending);
                            // The six-face survey draws the loading globe; it does
                            // not provide collision, spawn terrain or the horizon.
                            // Keep the full survey wait only for benchmark parity.
                            let benchmark_loading = benchmark_config.enabled && !playable_probe;
                            let planet_survey_faces = if benchmark_loading && matches!(mode, AppMode::Playing) {
                                let center = (player.pos.x.floor() as i32, player.pos.z.floor() as i32);
                                world.as_ref().map(|w| w.sample_config()).filter(|cfg| cfg.is_planet())
                                    .and_then(|cfg| egui_ui::world_loader::sphere::request(cfg, center))
                                    .map(|survey| survey.faces())
                            } else { None };
                            let planet_survey_pending = planet_survey_faces.is_some_and(|faces| faces < 6);
                            let loading_world = world.is_some()
                                && (!chunk_cache.spawn_ready || horizon_hold || planet_survey_pending
                                    || (benchmark_loading && !gpu.horizon_ready()))
                                && matches!(mode, AppMode::Playing);
                            if world.is_none() || network_mode || benchmark_config.enabled || playable_probe || ca_window.is_some() {
                                world_entry_pending = false;
                                world_entry_prompt = false;
                            } else if matches!(mode, AppMode::Playing) && !chunk_cache.spawn_ready {
                                world_entry_pending = true;
                                world_entry_prompt = false;
                            }
                            let continue_ready = world_entry_pending && !loading_world
                                && matches!(mode, AppMode::Playing);
                            let spawning = loading_world || continue_ready;
                            if let Some(started) = playable_probe_start {
                                if chunk_cache.spawn_ready && !playable_probe_spawn {
                                    playable_probe_spawn = true;
                                    println!("[playable-probe] spawn_meshes_ms={:.1} horizon_stage={}", started.elapsed().as_secs_f64() * 1000.0, gpu.horizon_work_stage());
                                }
                                if gpu.horizon_ready() && !playable_probe_horizon {
                                    playable_probe_horizon = true;
                                    println!("[playable-probe] horizon_ms={:.1}", started.elapsed().as_secs_f64() * 1000.0);
                                }
                            }
                            #[cfg(feature = "multiplayer")]
                            if network_mode && !spawning && !network_ready_reported && multiplayer.as_ref().is_some_and(|s|s.ready()) {
                                network_ready_reported=true;
                                eprintln!("[multiplayer] world_ready terrain={}/{} horizon={}",spawn_progress.0,spawn_progress.1,gpu.horizon_ready());
                                network_ready_at=Some(Instant::now());
                            }
                            #[cfg(feature = "multiplayer")]
                            match coop_review.take() {
                                Some(multiplayer::coop::Review::Join(code)) if world.is_none() => coop.join(&code),
                                Some(multiplayer::coop::Review::Open(name)) if multiplayer.is_none() => {
                                    match world.as_ref() {
                                        Some(w) if !spawning && !coop.busy() => coop.open(w, String::new(), false, w.rules.mode.creative_tools()),
                                        Some(_) => coop_review = Some(multiplayer::coop::Review::Open(name)),
                                        None => {
                                            if let Some(entry) = egui_ui.library_entries.iter().find(|e| e.name == name || e.id == name) {
                                                library.open(entry.id.clone());
                                            }
                                            coop_review = Some(multiplayer::coop::Review::Open(name));
                                        }
                                    }
                                }
                                other => coop_review = other,
                            }
                            // Host Latest World: open the session once the loaded world has spawned;
                            // drop the request if the load failed or never started.
                            #[cfg(feature = "multiplayer")]
                            if host_after_load {
                                match world.as_ref() {
                                    None if !library.loading() && pending_loaded.is_none() => host_after_load = false,
                                    Some(_) if multiplayer.is_some() => host_after_load = false,
                                    Some(w) if !spawning && !coop.busy() => {
                                        coop.open(w, String::new(), false, w.rules.mode.creative_tools());
                                        egui_ui.show_friends();
                                        if matches!(mode, AppMode::Playing) {
                                            mode = AppMode::Paused;
                                            release_mouse(window, &mut grabbed);
                                        }
                                        host_after_load = false;
                                    }
                                    _ => {}
                                }
                            }
                            // `--multiplayer-capture-delay=S` waits so chat and other players can appear first.
                            #[cfg(feature = "multiplayer")]
                            if network_ready_at.is_some_and(|at|at.elapsed().as_secs_f32()>=network_capture_delay)
                                && let Some(path)=network_capture.take() {gpu.request_capture(path);}
                            let lead_before = simulation_clock.lead_seconds();
                            let simulation_steps = simulation_clock.advance(
                                raw_frame_dt * world.as_ref().map_or(1.0, world::farming::jobs::production::scene::speed),
                                matches!(mode, AppMode::Playing) && !spawning && !hell_gate.busy() && !hall_gate.busy(),
                            );
                            let mut gameplay_dt = simulation_steps as f32 * simulation::STEP_SECONDS;
                            let mut presented_dt = simulation_clock.presented_seconds(lead_before, simulation_steps);

                            gpu.begin_frame_stats();
                            chunk_streamer.begin_frame_stats();

                            if let Some(benchmark_state) = benchmark.as_mut()
                                && !benchmark_state.auto_started && world.is_none() {
                                    if playable_probe { playable_probe_start = Some(Instant::now()); }
                                    // Final validation can use normal presentation preferences;
                                    // diagnostics default uncapped.
                                    gpu.set_vsync(terrain_review::option("VOXEL_BENCHMARK_NORMAL_PRESENT")
                                        && preferences.loaded.vsync);
                                    render::set_cloud_adaptive_enabled(benchmark_state.config.adaptive_clouds);
                                    // World quality stays fixed; cloud adaptation is opt-in.
                                    quality::set_auto_enabled(false);
                                    let (cpu,_)=render::world_residency_budgets_mib();
                                    let gpu_budget=render::gpu_residency_limit_mib();
                                    render::set_world_residency_budgets_mib(benchmark_state.config.cpu_mib.unwrap_or(cpu),benchmark_state.config.gpu_mib.unwrap_or(gpu_budget));
                                    if let Some(view_distance_chunks) = benchmark_state.config.view_distance_chunks {
                                        let current = render_distance_settings();
                                        set_render_distance_settings(render::RenderDistanceSettings {
                                            view_distance_chunks,
                                            ..current
                                        });
                                    }
                                    selected_world = benchmark_state.config.world_type;
                                    selected_voxel_scale = benchmark_state.config.voxel_scale;
                                    selected_seed = benchmark_state.config.seed;
                                    creative.cancel_edits();
                                    start_world(
                                        selected_world,
                                        selected_voxel_scale,
                                        selected_seed,
                                        None,
                                        &mut world,
                                        &mut player,
                                        &mut day_night,
                                        &mut chunk_cache,
                                        chunk_streamer,
                                        fluid_sim,
                                        &mut dirty_blocks,
                                        &mut dirty_chunks,
                                        &mut mode,
                                        window,
                                        &mut grabbed,
                                        focused && !benchmark_config.enabled,
                                        &mut last,
                                        &mut last_safe_player_pos,
                                    );
                                    release_mouse(window, &mut grabbed);
                                    if let (Some(review),Some(w))=(receiver_review.as_mut(),world.as_mut()) {
                                        review.install(w,&mut player,&mut day_night);
                                        freeze_time=true;
                                        let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        chunk_cache.reset_with_snapshot(center,&w.snapshot_near(center,4));
                                        chunk_streamer.reset();chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                    }
                                    if let (Some(review),Some(w))=(wand_review.as_mut(),world.as_mut()) {
                                        review.install(w,&mut player,&mut creative);
                                        let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        chunk_cache.reset_with_snapshot(center,&w.snapshot_near(center,4));
                                        chunk_streamer.reset();chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                    }
                                    if let (Some(review),Some(w))=(sandbox_review.as_mut(),world.as_mut()) {
                                        review.install(w,&mut player);
                                        let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        let snapshot=w.snapshot_near(center,4);chunk_cache.reset_with_snapshot(center,&snapshot);chunk_streamer.reset();chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                    }
                                    if let (Some(review), Some(w)) = (water_review.as_mut(), world.as_mut()) {
                                        review.install(w, &mut player);
                                        let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        let snapshot = w.snapshot_near(center, 4);
                                        chunk_cache.reset_with_snapshot(center, &snapshot);
                                        chunk_streamer.reset();
                                        chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                    }
                                    if let Some(w) = world.as_mut() && planet::review::install_walk(w, &mut player) {
                                        let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        chunk_cache.reset_with_snapshot(center, &w.snapshot_near(center, 4));
                                        chunk_streamer.reset();
                                        chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                    }
                                    if std::env::var_os("VOXEL_REVIEW_ENEMIES").is_some()
                                        && let Some(w) = world.as_mut() {
                                            world::adventure::install_enemy_review(w, &mut player);
                                            let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                            let snapshot=w.snapshot_near(center,4);
                                            chunk_cache.reset_with_snapshot(center,&snapshot);
                                            chunk_streamer.reset();
                                            chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                        }
                                    if benchmark_state.config.ossuary_review && let Some(w)=world.as_mut() {
                                        world::adventure::install_ossuary_review(w,&mut player);
                                        let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        let snapshot=w.snapshot_near(center,4);
                                        chunk_cache.reset_with_snapshot(center,&snapshot);
                                        chunk_streamer.reset();chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                    }
                                    if benchmark_state.config.tollkeeper_review
                                        && let Some(w)=world.as_mut() {
                                            world::adventure::install_tollkeeper_review(w,&mut player);
                                            let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                            let snapshot=w.snapshot_near(center,4);
                                            chunk_cache.reset_with_snapshot(center,&snapshot);
                                            chunk_streamer.reset();chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                        }
                                    if let (Some((variant, difficulty)), Some(w)) = (benchmark_state.config.hart_review, world.as_mut()) {
                                        world::adventure::install_hart_review(w, &mut player, variant, difficulty);
                                        let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        let snapshot = w.snapshot_near(center, 4);
                                        chunk_cache.reset_with_snapshot(center, &snapshot);
                                        chunk_streamer.reset();
                                        chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                    }
                                    if benchmark_state.config.saved_review {
                                        if std::env::var_os("VOXEL_SAVE_DIR").is_none() {
                                            eprintln!("[benchmark] Saved-scene review requires VOXEL_SAVE_DIR pointing to a disposable copy");
                                            elwt.exit(); return;
                                        }
                                        match world::storage::load_review_checkpoint() {
                                            Ok(mut saved) => {
                                                selected_world = saved.world.world_type;
                                                selected_voxel_scale = saved.world.voxel_scale;
                                                selected_seed = saved.world.seed;
                                                player = saved.player;
                                                day_night = saved.day_night;
                                                if let Some(time) = benchmark_state.config.review_time_of_day { day_night.set_time_of_day(time); }
                                                benchmark_state.config.world_type = selected_world;
                                                benchmark_state.config.voxel_scale = selected_voxel_scale;
                                                benchmark_state.config.seed = selected_seed;
                                                let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                                let snapshot = saved.world.snapshot_near(center, render_distance_settings().view_distance_chunks);
                                                chunk_cache.reset_with_snapshot(center, &snapshot);
                                                chunk_streamer.reset();
                                                chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                                fluid_sim.reset_for_world(selected_voxel_scale);
                                                world = Some(saved.world);
                                                println!("[saved-scene] generator={} seed={} position={:?} time={:.6}", world.as_ref().unwrap().generator_version, selected_seed, player.pos, day_night.time_of_day());
                                            }
                                            Err(error) => { eprintln!("[benchmark] Cannot load copied scene: {error:#}"); elwt.exit(); return; }
                                        }
                                    }
                                    if benchmark_state.config.reference_review
                                        && let Some(w)=world.as_mut() {
                                            reference_environment::install(w);
                                            reference_environment::camera(&mut player, selected_voxel_scale.meters());
                                            let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                            let snapshot=w.snapshot_near(center,8);
                                            chunk_cache.reset_with_snapshot(center,&snapshot);
                                            chunk_streamer.reset();
                                            chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                        }
                                    if benchmark_state.config.torch_review
                                        && let Some(w) = world.as_mut() {
                                            torch_review::install(w);
                                            torch_review::camera(&mut player, selected_voxel_scale.meters());
                                            let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                            let snapshot = w.snapshot_near(center, 4);
                                            chunk_cache.reset_with_snapshot(center, &snapshot);
                                            chunk_streamer.reset();
                                            chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                        }
                                    if let Some(preset) = benchmark_state.config.weather_preset {
                                        day_night.set_weather_preset(preset);
                                    }
                                    cloud_weather.reset(selected_seed);
                                    if let (Some(probe),Some(w))=(latency_probe.as_mut(),world.as_mut()) {
                                        probe.install(w,&mut player,&mut creative);
                                        let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        chunk_cache.reset_with_snapshot(center,&w.snapshot_near(center,4));
                                        chunk_streamer.reset();chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                    }

                                    physics_enabled = benchmark_state.config.profile_physics;
                                    if let (Some(review), Some(w)) = (industrial_review.as_mut(), world.as_mut()) {
                                        review.install(w, &mut player);
                                        let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        chunk_cache.reset_with_snapshot(center, &w.snapshot_near(center, 4));
                                        chunk_streamer.reset();
                                        chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                    }
                                    if workshop_review.is_some() {
                                        if let Some(w) = world.as_mut() {
                                            use world::rules::{GameMode, Rules};
                                            w.set_rules(Rules::new_world(GameMode::Survival));
                                            w.adventure.set_difficulty(world::adventure::difficulty::Difficulty::Normal);
                                            println!("[workshop-loop] mode=Survival difficulty=Normal");
                                        }
                                        physics_enabled = true;
                                    }
                                    if industrial_review.is_some() { physics_enabled = true; }
                                    freeze_time = benchmark_state.config.saved_review || receiver_review.is_some();
                                    if benchmark_state.config.motion == BenchmarkMotionMode::LodTravel {
                                        day_night.set_time_of_day(benchmark_state.config.review_time_of_day.unwrap_or(0.5));
                                        freeze_time = true;
                                    }
                                    if wand_review.is_some() {day_night.set_time_of_day(0.5);freeze_time=true;}
                                    if benchmark_state.config.capture_frames && !benchmark_state.config.capture_hud { egui_ui.show_hud = false; }
                                    if benchmark_state.config.sandbox_review {
                                        egui_ui.show_hud = false;
                                        day_night.set_time_of_day(0.5);
                                        freeze_time = true;
                                    }
                                    if badlands_sprint && let Some(w) = world.as_mut() {
                                        if !terrain_review::place_badlands_sprint(w, &mut player) {
                                            eprintln!("[badlands-sprint] no dry Badlands start found");
                                            elwt.exit();
                                            return;
                                        }
                                        let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        let snapshot = w.snapshot_near(center, render_distance_settings().view_distance_chunks);
                                        chunk_cache.reset_with_snapshot(center, &snapshot);
                                        chunk_streamer.reset();
                                        chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                        last_safe_player_pos = player.pos;
                                    }
                                    benchmark_state.auto_started = true;
                                    benchmark_state.measuring = false;
                                    benchmark_state.phase_started_at = Instant::now();
                                    benchmark_state.motion_started_at = Instant::now();
                                    benchmark_state.motion_anchor_blocks = player.pos;
                                    if benchmark_state.config.motion == BenchmarkMotionMode::LodTravel
                                        && let Some(w) = world.as_ref()
                                    {
                                        let cfg = w.sample_config();
                                        let anchor = benchmark_state.motion_anchor_blocks;
                                        benchmark_state.lod_travel_worker = Some(std::thread::spawn(move || lod_travel_profile(cfg, anchor)));
                                    }
                                    benchmark_state.measured_samples.clear();
                                    if terrain_review::option("VOXEL_ROUTE_TIMING") || terrain_review::option("VOXEL_HORIZON_ROUTE")
                                        || terrain_review::option("VOXEL_RENDER_TIMING") {
                                        let root = std::env::var_os("VOXEL_CAPTURE_DIR")
                                            .map(std::path::PathBuf::from)
                                            .unwrap_or_else(|| "target/benchmark-frames".into());
                                        match perf::route::RouteTiming::new(root) {
                                            Ok(timing) => benchmark_state.route_timing = Some(timing),
                                            Err(error) => eprintln!("[route-timing] cannot start diagnostics: {error}"),
                                        }
                                    }
                                    println!(
                                        "[benchmark] starting world={:?} voxel_scale={:?} seed={} weather={:?} motion={} warmup_s={:.1} duration_s={:.1}",
                                        benchmark_state.config.world_type,
                                        benchmark_state.config.voxel_scale,
                                        benchmark_state.config.seed,
                                        benchmark_state.config.weather_preset,
                                        benchmark_state.config.motion.label(),
                                        benchmark_state.config.warmup_seconds,
                                        benchmark_state.config.duration_seconds
                                    );
                                }

                            // Opt-in review uses the actual application reset path,
                            // once during generation and again during queue upload.
                            // It never opens an existing player save.
                            if terrain_review::option("VOXEL_HORIZON_WORLD_SWITCH")
                                && let Some(review)=benchmark.as_mut().filter(|b|b.auto_started) {
                                    let stage=gpu.horizon_work_stage();
                                    let switch=(review.horizon_world_switches==0 && stage=="build"
                                        && review.motion_started_at.elapsed().as_secs_f32()>0.25)
                                        || (review.horizon_world_switches==1 && stage=="upload");
                                    if switch {
                                        let old=selected_seed;
                                        selected_seed=review.config.seed.wrapping_add(u32::from(review.horizon_world_switches)+1);
                                        println!("[horizon-world-switch] pending={stage} old_seed={old} new_seed={selected_seed} old_published={:?}",gpu.horizon_published_source());
                                        creative.cancel_edits();
                                        start_world(selected_world,selected_voxel_scale,selected_seed,None,&mut world,&mut player,&mut day_night,&mut chunk_cache,chunk_streamer,fluid_sim,&mut dirty_blocks,&mut dirty_chunks,&mut mode,window,&mut grabbed,false,&mut last,&mut last_safe_player_pos);
                                        release_mouse(window,&mut grabbed);
                                        if let Some(preset)=review.config.weather_preset { day_night.set_weather_preset(preset); }
                                        cloud_weather.reset(selected_seed);
                                        review.horizon_world_switches+=1;
                                        review.measuring=false;
                                        review.phase_started_at=Instant::now();
                                        review.measured_samples.clear();
                                    } else if review.horizon_world_switches==2 && gpu.horizon_ready() {
                                        let published=gpu.horizon_published_source();
                                        assert_eq!(published.map(|p|p.seed),Some(selected_seed),"obsolete world published after review switch");
                                        println!("[horizon-world-switch] complete final_seed={selected_seed} published={published:?}");
                                        review.horizon_world_switches=3;
                                    }
                                }
                            if terrain_review::option("VOXEL_HORIZON_ROUTE") && gpu.horizon_ready() && chunk_cache.spawn_ready
                                && let Some(review)=benchmark.as_mut().filter(|b|b.auto_started&&b.horizon_route_started_at.is_none()) {
                                    review.horizon_route_started_at=Some(Instant::now());
                                    println!("[horizon-route] clock_started_after_minimum_coverage wall_seconds={:.3}",review.motion_started_at.elapsed().as_secs_f32());
                                }
                            creative.sync_world_scale(world.as_ref().map_or(selected_voxel_scale.meters(),|w|w.voxel_scale.meters()));
                            let pre_ui_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
                            let ui_start = Instant::now();
                            if let Some(review) = benchmark.as_mut().filter(|b|
                                b.auto_started && matches!(b.config.motion, BenchmarkMotionMode::CloudReview | BenchmarkMotionMode::CloudReviewFar))
                            {
                                let wall_elapsed = review.motion_started_at.elapsed().as_secs_f32()
                                    + review.config.review_start_seconds;
                                let far_review = review.config.motion == BenchmarkMotionMode::CloudReviewFar;
                                let elapsed = wall_elapsed + if far_review { 90.0 } else { 0.0 };
                                // The far review teleports to a new hillside at startup. Hold its
                                // first weather phase while those visible chunks stream in.
                                let phase_elapsed = if far_review {
                                    90.0 + (wall_elapsed - 24.0).max(0.0)
                                } else {
                                    elapsed
                                };
                                let phase = (phase_elapsed / 15.0) as usize;
                                let preset_index = if phase < 5 { phase } else if phase == 5 { 1 } else { phase.saturating_sub(6) % 5 };
                                let presets = [WeatherPreset::Clear, WeatherPreset::PartlyCloudy,
                                    WeatherPreset::Overcast, WeatherPreset::Stormy, WeatherPreset::WinterSky];
                                if phase != review.cloud_review_phase {
                                    let preset = review.config.weather_preset.unwrap_or(presets[preset_index]);
                                    if review.cloud_review_phase == usize::MAX || review.config.weather_preset.is_none() {
                                        day_night.set_weather_preset(preset);
                                        gpu.invalidate_cloud_history();
                                    }
                                    review.cloud_review_phase = phase;
                                    let location = if phase < 5 { "origin" } else if phase == 5 { "travel" } else { "16 km" };
                                    window.set_title(&format!("Cloud review: {preset:?} | {location}"));
                                    println!("[cloud-review] phase={phase} preset={preset:?} location={location} elapsed={elapsed:.1} camera_m={:?} mouse_grabbed={grabbed}", player.pos * world.as_ref().map_or(1.0, |w| w.voxel_scale.meters()));
                                }
                                let sunset = phase >= 11;
                                let start_time = review.config.review_time_of_day
                                    .unwrap_or(if sunset { 0.7475 } else { 0.5 });
                                let progress = (review.motion_started_at.elapsed().as_secs_f32()
                                    / (review.config.duration_seconds + review.config.warmup_seconds).max(0.001)).clamp(0.0, 1.0);
                                let review_time = start_time + (review.config.review_time_end.unwrap_or(start_time) - start_time) * progress;
                                day_night.set_time_of_day(review_time);
                                freeze_time = true;
                                let shot_time = phase_elapsed % 15.0;
                                let terrain_missing = chunk_cache.visible.iter()
                                    .filter(|key| !chunk_cache.gpu_meshes.contains_key(&key.coord))
                                    .count();
                                let terrain_ready = chunk_cache.fully_expanded && terrain_missing == 0;
                                // Emit the first available frame after each trigger. Narrow
                                // time windows lost samples during terrain upload hitches.
                                let next_sample = if review.last_capture_key != usize::MAX
                                    && review.last_capture_key / 4 == phase {
                                    review.last_capture_key % 4 + 1
                                } else { 0 };
                                let capture_sample = if next_sample < 4
                                    && shot_time >= [4.0, 4.2, 12.0, 12.2][next_sample] {
                                    Some(next_sample)
                                } else { None };
                                if let Some(sample) = capture_sample.filter(|_| review.config.capture_frames && terrain_ready) {
                                    let key = phase * 4 + sample;
                                    if key != review.last_capture_key {
                                        let preset = review.config.weather_preset.unwrap_or(presets[preset_index]);
                                        let clock = if sunset { "sunset" } else { "noon" };
                                        let terrain = world.as_ref().unwrap().sample_config();
                                        let ground_m = world::terrain_height(&terrain, player.pos.x.floor() as i32,
                                            player.pos.z.floor() as i32) as f32 * terrain.voxel_scale.meters();
                                        println!("[cloud-review-capture] phase={phase} preset={preset:?} time={clock} solar_time={review_time:.5} camera_m={:?} ground_m={ground_m:.2} terrain_ready={terrain_ready} mouse_grabbed={grabbed}", player.pos * terrain.voxel_scale.meters());
                                        let capture_root = std::env::var_os("VOXEL_CAPTURE_DIR")
                                            .map(std::path::PathBuf::from)
                                            .unwrap_or_else(|| "target/cloud-captures".into());
                                        gpu.request_capture(capture_root.join(format!(
                                            "{}/{:?}/{:?}-{phase:02}-{preset:?}-{clock}-{sample}.png", if review.config.horizon_sweep { "horizon-review" } else { "hills-detail" }, review.config.debug_view, review.config.world_type)));
                                        review.last_capture_key = key;
                                    }
                                }
                            }
                            if let Some(review) = benchmark.as_mut().filter(|b| b.auto_started
                                && b.config.terrain_camera.is_some()
                                && !b.config.horizon_sweep
                                && matches!(b.config.motion, BenchmarkMotionMode::Static))
                            {
                                // Fixed-pose review: cosmos chambers, mazes, seasons, sky events.
                                if let Some(t) = review.config.review_time_of_day {
                                    day_night.set_time_of_day(t);
                                    freeze_time = true;
                                }
                                if let Some(day) = review.config.review_day {
                                    day_night.set_day_index(day);
                                }
                                // VOXEL_REVIEW_CYCLONE=orbit|eyewall: the strongest daylit tropical cyclone of the seed.
                                if let (Some(w), Ok(mode)) = (world.as_ref(), std::env::var("VOXEL_REVIEW_CYCLONE")) {
                                    static SHOT: std::sync::OnceLock<Option<world::CycloneShot>> = std::sync::OnceLock::new();
                                    let shot = SHOT.get_or_init(|| {
                                        let shot = world::cyclone_review(day_night.weather_seed(), w.sample_config(), &mode, 120.0);
                                        println!("[cyclone-review] mode={mode} shot={shot:?}");
                                        shot
                                    });
                                    if let Some(shot) = shot {
                                        day_night.set_day_index(shot.day_index);
                                        day_night.set_time_of_day(shot.time_of_day);
                                        freeze_time = true;
                                        review.config.terrain_camera = Some(shot.camera);
                                    }
                                }
                                if let (Some(w), Some(tier)) = (world.as_mut(), std::env::var("VOXEL_REVIEW_WAND").ok().and_then(|v| v.parse::<u32>().ok())) {
                                    w.adventure.wand = tier.min(3);
                                    if let Some(spell) = std::env::var("VOXEL_REVIEW_WAND_SPELL").ok()
                                        .and_then(|v| v.parse::<u32>().ok())
                                        .filter(|&id| world::adventure::spell_defs::def(id).is_some()) {
                                        w.adventure.loadout.set_slot(0, spell);
                                        w.adventure.loadout.cursor = 0;
                                        w.adventure.last_cast = spell;
                                    }
                                    // Review the real selected inventory item. Merely setting
                                    // adventure.wand is undone by normal hotbar synchronization.
                                    if tier > 0 && !matches!(w.inventory.hotbar[creative.selected_slot],
                                        Some(world::inventory::ItemStack { item: world::inventory::Item::Wand { .. }, .. })) {
                                        let record=world::adventure::wand_items::WandRecord {
                                            tier:tier.min(3), loadout:w.adventure.loadout,
                                        };
                                        if let Some(item)=w.adventure.mint_wand(record) {
                                            w.inventory.hotbar[creative.selected_slot]=Some(world::inventory::ItemStack::new(item,1));
                                        }
                                    }
                                }
                                // VOXEL_REVIEW_CRYSTAL=<metres ahead>: one crystal slab in front
                                // of the fixed camera, for judging gem glints in a capture.
                                static CRYSTAL_PLACED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                                if let (Some(w), Some(metres)) = (world.as_mut(), std::env::var("VOXEL_REVIEW_CRYSTAL").ok().and_then(|v| v.parse::<f32>().ok()))
                                    && !CRYSTAL_PLACED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                                        let scale = w.voxel_scale.meters();
                                        let eye = player.eye(w.voxel_scale);
                                        let fwd = player.flat_forward();
                                        let side = glam::Vec3::new(-fwd.z, 0.0, fwd.x);
                                        let centre = eye + fwd * (metres / scale);
                                        let (half_w, below, above) = ((2.5 / scale) as i32, (2.0 / scale) as i32, (2.5 / scale) as i32);
                                        let mut edits = Vec::new();
                                        for u in -half_w..=half_w {
                                            for dy in -below..=above {
                                                for depth in 0..2 {
                                                    let p = centre + side * u as f32 + fwd * depth as f32 + glam::Vec3::Y * dy as f32;
                                                    edits.push((p.floor().as_ivec3(), BlockId::Crystal));
                                                }
                                            }
                                        }
                                        w.set_many(&edits);
                                    }
                                // VOXEL_REVIEW_TERRAIN=<metres ahead>: a terrain-material test bed
                                // (yaw 0, looking -z). Left to right: stone stairs, a flush
                                // gravel strip, grass-over-dirt steps with exposed banks, a
                                // cobblestone wall and a dug dirt pit.
                                static TERRAIN_PLACED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                                if let (Some(w), Some(metres)) = (world.as_mut(), std::env::var("VOXEL_REVIEW_TERRAIN").ok().and_then(|v| v.parse::<f32>().ok()))
                                    && !TERRAIN_PLACED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                                        let scale = w.voxel_scale.meters();
                                        let eye = player.eye(w.voxel_scale);
                                        let cx = eye.x.floor() as i32;
                                        let cz = (eye.z - metres / scale).floor() as i32;
                                        let h = w.terrain_height_at(cx, cz);
                                        eprintln!("[terrain-review] fixture at x={cx} z={cz} ground={h} voxel={scale}m");
                                        let mut edits = Vec::new();
                                        for x in -9..=9 {
                                            for z in -4..=4 {
                                                let p = |y: i32| glam::IVec3::new(cx + x, h + y, cz + z);
                                                for y in -3..=5 {
                                                    edits.push((p(y), if y <= -1 { BlockId::Dirt } else { BlockId::Air }));
                                                }
                                                // Stairs rise away from the camera (toward -z).
                                                let step = (4 - z).clamp(0, 8) / 2;
                                                match x {
                                                    -9..=-6 => {
                                                        for y in 0..=step { edits.push((p(y), BlockId::Stone)); }
                                                    }
                                                    -5..=-3 => {
                                                        edits.push((p(0), BlockId::Gravel));
                                                        edits.push((p(-1), BlockId::Gravel));
                                                    }
                                                    -2..=3 => {
                                                        for y in 0..step { edits.push((p(y), BlockId::Dirt)); }
                                                        edits.push((p(step), BlockId::Grass));
                                                    }
                                                    4..=5 => {
                                                        for y in 0..=3 { edits.push((p(y), BlockId::Cobblestone)); }
                                                    }
                                                    _ => {
                                                        if z.abs() <= 2 {
                                                            edits.push((p(0), BlockId::Air));
                                                            edits.push((p(-1), BlockId::Air));
                                                        } else {
                                                            edits.push((p(0), BlockId::Grass));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        w.set_many(&edits);
                                    }
                                // VOXEL_REVIEW_FLUIDS=<metres ahead>: a fluids-and-powders test bed
                                // (yaw 0, looking -z). Left to right: a sand-bed pool that deepens
                                // from 1 to 4 voxels away from the camera, with boulders on the bed;
                                // sand, gravel, ash and dirt mounds; a burning log under smoke; steam.
                                static FLUIDS_PLACED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                                if let (Some(w), Some(metres)) = (world.as_mut(), std::env::var("VOXEL_REVIEW_FLUIDS").ok().and_then(|v| v.parse::<f32>().ok()))
                                    && !FLUIDS_PLACED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                                        let scale = w.voxel_scale.meters();
                                        let eye = player.eye(w.voxel_scale);
                                        let cx = eye.x.floor() as i32;
                                        let cz = (eye.z - metres / scale).floor() as i32;
                                        let h = w.terrain_height_at(cx, cz);
                                        let mut edits = Vec::new();
                                        for x in -14..=14 {
                                            for z in -5..=5 {
                                                let p = |y: i32| glam::IVec3::new(cx + x, h + y, cz + z);
                                                for y in -5..=7 {
                                                    edits.push((p(y), if y <= 0 { BlockId::Grass } else { BlockId::Air }));
                                                }
                                                let mound = 2 - (z.abs() + 1) / 3;
                                                let pile = match x {
                                                    -4..=-3 => Some(BlockId::Sand),
                                                    -1..=0 => Some(BlockId::Gravel),
                                                    2..=3 => Some(BlockId::Ash),
                                                    5..=6 => Some(BlockId::Dirt),
                                                    _ => None,
                                                };
                                                if let Some(block) = pile {
                                                    for y in 1..=mound { edits.push((p(y), block)); }
                                                }
                                                if (-14..=-6).contains(&x) && z.abs() <= 4 {
                                                    let depth = (4 - z).clamp(2, 8) / 2;
                                                    edits.push((p(-depth), BlockId::Sand));
                                                    for y in (-depth + 1)..=0 { edits.push((p(y), BlockId::Water)); }
                                                    if (x == -11 && z == 1) || (x == -8 && z == -2) {
                                                        edits.push((p(-depth + 1), BlockId::Stone));
                                                    }
                                                }
                                                if (9..=10).contains(&x) && z.abs() <= 1 {
                                                    edits.push((p(1), BlockId::Log));
                                                    edits.push((p(2), BlockId::Fire));
                                                    if z == 0 {
                                                        for y in 3..=6 { edits.push((p(y), BlockId::Smoke)); }
                                                    }
                                                }
                                                if x == 13 && z.abs() <= 1 {
                                                    for y in 1..=4 { edits.push((p(y), BlockId::Steam)); }
                                                }
                                            }
                                        }
                                        w.set_many(&edits);
                                    }
                                // VOXEL_REVIEW_WATER=<metres ahead>: a flowing-water test bed
                                // (yaw 0). Partial levels are written directly because the
                                // fixed-pose review does not step the water simulation.
                                static WATER_PLACED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                                if let (Some(w), Some(metres)) = (world.as_mut(), std::env::var("VOXEL_REVIEW_WATER").ok().and_then(|v| v.parse::<f32>().ok()))
                                    && !WATER_PLACED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                                        let scale = w.voxel_scale.meters();
                                        let eye = player.eye(w.voxel_scale);
                                        let cx = eye.x.floor() as i32;
                                        let cz = (eye.z - metres / scale).floor() as i32;
                                        let floor_y = w.terrain_height_at(cx, cz) + 1;
                                        let mut edits = Vec::new();
                                        let mut partial = Vec::new();
                                        for x in -7..=7 {
                                            for z in -4..=4 {
                                                let p = |y: i32| glam::IVec3::new(cx + x, floor_y + y, cz + z);
                                                for y in 0..=4 {
                                                    edits.push((p(y), BlockId::Air));
                                                }
                                                edits.push((p(0), BlockId::StoneBricks));
                                                match z {
                                                    // Pool, then a ramp of falling levels.
                                                    -4..=-3 => {
                                                        edits.push((p(1), BlockId::Water));
                                                        if x < 0 { edits.push((p(2), BlockId::Water)); }
                                                        else { partial.push((p(1), (7 - x).clamp(1, 7) as u8)); }
                                                    }
                                                    // Step: a two-high column beside a shallow cell.
                                                    -1..=0 => {
                                                        if x <= 2 { edits.push((p(1), BlockId::Water)); edits.push((p(2), BlockId::Water)); }
                                                        else if x <= 5 { edits.push((p(1), BlockId::Water)); partial.push((p(1), 3)); }
                                                    }
                                                    // Shore: a shallow film against a solid bank.
                                                    2..=4 => {
                                                        if x < -1 { edits.push((p(1), BlockId::Water)); }
                                                        else if x < 3 { edits.push((p(1), BlockId::Water)); partial.push((p(1), (1 + (x + 1) * 2).clamp(1, 7) as u8)); }
                                                        else { edits.push((p(1), BlockId::StoneBricks)); }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        w.set_many(&edits);
                                        for (p, v) in partial { w.set_water_volume(p, v); }
                                    }
                                if let Ok(spec) = std::env::var("VOXEL_REVIEW_STYLE") {
                                    let n: Vec<u8> = spec.split(',').filter_map(|v| v.trim().parse::<u8>().ok()).collect();
                                    if n.len() == 6 {
                                        wand_style::preview(wand_style::WandStyle { shaft: n[0], head: n[1], trail: n[2], grip: n[3], glow: n[4], runes: n[5] });
                                    }
                                }
                                let missing = chunk_cache.visible.iter()
                                    .filter(|key| !chunk_cache.gpu_meshes.contains_key(&key.coord))
                                    .count();
                                if review.config.capture_frames && review.last_capture_key == usize::MAX
                                    && review.measuring
                                    && review.phase_started_at.elapsed().as_secs_f32()
                                        >= (review.config.duration_seconds - 1.0).max(0.0)
                                    && gpu.horizon_ready() && gpu.planet_detail_ready()
                                    && chunk_cache.fully_expanded && missing == 0
                                {
                                    let root = std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from)
                                        .unwrap_or_else(|| "target/camera-review".into());
                                    gpu.request_capture(root.join("camera.png"));
                                    println!("[camera-review] captured camera_m={:?}", player.pos * world.as_ref().map_or(1.0, |w| w.voxel_scale.meters()));
                                    review.last_capture_key = 0;
                                }
                            }
                            if let Some(review) = benchmark.as_mut().filter(|b| b.auto_started && b.config.torch_review) {
                                let elapsed = review.motion_started_at.elapsed().as_secs_f32();
                                let mode = torch_review::mode();
                                let workbench_factory = dev_panel.is_some() && mode == "factory";
                                let phase = if mode == "survival" {
                                    (elapsed / 3.0) as usize
                                } else {
                                    (elapsed / 12.0) as usize
                                };
                                torch_review::motion_camera(&mut player, selected_voxel_scale.meters(), elapsed);
                                day_night.set_time_of_day(if matches!(mode, "survival" | "ui") { 0.35 } else if mode == "factory" || mode == "station" || mode == "wildlife" || (mode == "minerals" && phase == 0) { 0.5 } else if phase == 1 || !mode.is_empty() { 0.0 } else { 0.5 });
                                if mode == "factory" && !workbench_factory { egui_ui.show_hud = (elapsed / 3.0) as usize % 2 == 1; }
                                freeze_time = true;
                                if mode == "combustion" {
                                    if let Some(w) = world.as_mut() {
                                        torch_review::combustion_phase(w, ((elapsed / 4.0) as usize).min(2));
                                    }
                                } else if mode == "survival" || (mode == "ui" && elapsed < 2.0) {
                                    if let Some(w) = world.as_mut() {
                                        torch_review::apply_survival_phase(w, &mut player, &mut creative, &egui_ui.ctx, if mode == "ui" { 0 } else { phase });
                                    }
                                } else if mode == "motion" {
                                    creative.show_palette = false;
                                    creative.assign_hotbar_slot(0, BlockId::Stone);
                                } else if !workbench_factory && (!matches!(mode, "interactive" | "drag" | "ui") || elapsed < 2.0) {
                                    creative.show_palette = matches!(mode, "interactive" | "drag") || (phase >= 2 && mode != "minerals");
                                    // VOXEL_REVIEW_PANEL=<page> picks the panel to capture (7 Production, 3 Machines).
                                    let review_page = std::env::var("VOXEL_REVIEW_PANEL").ok().and_then(|v| v.parse::<u32>().ok());
                                    if review_page.is_some() { creative.show_palette = true; }
                                    if let Ok(scale) = std::env::var("VOXEL_REVIEW_UI_SCALE").unwrap_or_default().parse::<f32>() { egui_ui.ui_scale = scale.clamp(0.75,2.5); }
                                    egui_ui.ctx.data_mut(|d| d.insert_temp(egui::Id::new("inventory_page"), review_page.unwrap_or(if mode.is_empty() && phase == 3 { 2u32 } else { 4u32 })));
                                }
                                let capture_phase = if mode == "combustion" { ((elapsed / 4.0) as usize).min(2) } else if mode.is_empty() { phase } else { (elapsed / 3.0) as usize };
                                let capture_ready = if mode == "combustion" {
                                    elapsed % 4.0 >= 2.5
                                } else {
                                    mode.is_empty() && elapsed % 12.0 >= 9.0 || (mode == "survival" && elapsed % 3.0 >= 2.2) || (!mode.is_empty() && mode != "survival")
                                };
                                if !workbench_factory && capture_ready && review.last_capture_key != capture_phase {
                                    let root = std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from)
                                        .unwrap_or_else(|| "target/torch-review".into());
                                    gpu.request_capture(root.join(format!("{:?}-{capture_phase:03}.png", selected_voxel_scale)));
                                    if mode == "station" {
                                        if let Some(w) = world.as_ref() {
                                            for e in w.adventure.enemies.iter().filter(|e| e.slot >= 1000) {
                                                println!("[station-review] actor kind={} slot={} pos={:?} health={}", e.kind, e.slot, e.pos, e.health);
                                            }
                                        }
                                    }
                                    if mode == "motion" || mode == "combustion" {
                                        let snapshot = world.as_mut().unwrap().snapshot();
                                        let (lights, extinguished) = collect_nearby_torches(world.as_ref().unwrap(), &snapshot, &chunk_cache, player.pos, selected_voxel_scale.meters());
                                        if mode == "combustion" {
                                            println!("[combustion-review] phase={capture_phase} extinguished={extinguished:?}");
                                        }
                                        println!("[torch-review] t={elapsed:.2} camera_m={:?} lights={:?}", player.pos * selected_voxel_scale.meters(), lights.iter().map(|t| (t.position, t.strength)).collect::<Vec<_>>());
                                        for (coord, chunk) in snapshot.chunks.iter().filter(|(_, c)| !c.torches.is_empty()) {
                                            println!("[torch-review-lod] chunk={coord:?} torches={} resident_lod={:?}", chunk.torches.len(), chunk_cache.gpu_meshes.get(coord).map(|m| m.lod));
                                        }
                                    }
                                    review.last_capture_key = capture_phase;
                                }
                            }
                            if let Some(review)=benchmark.as_mut().filter(|b| b.auto_started && b.config.reference_review) {
                                let elapsed=review.motion_started_at.elapsed().as_secs_f32();
                                let phase=std::env::var("VOXEL_REFERENCE_PHASE").ok().and_then(|v| v.parse::<usize>().ok()).filter(|v|*v<4).unwrap_or(((elapsed / 16.0) as usize).min(3));
                                reference_environment::camera(&mut player,selected_voxel_scale.meters());
                                day_night.set_time_of_day([0.5,0.73,0.0,0.5][phase]); freeze_time=true;
                                day_night.set_weather_preset(if phase==3 {WeatherPreset::Stormy} else {WeatherPreset::Clear});
                                egui_ui.show_hud=false;
                                creative.show_material_editor=std::env::var_os("VOXEL_REVIEW_EDITOR").is_some();
                                if elapsed%16.0>=13.0 && review.last_capture_key!=phase {
                                    let root=std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(||"target/reference-review".into());
                                    gpu.request_capture(root.join(format!("{:?}-{}.png",selected_voxel_scale,["noon","sunset","night","rain"][phase])));
                                    println!("[reference-review] {:?} {} camera_m={:?}",selected_voxel_scale,["noon","sunset","night","rain"][phase],player.pos*selected_voxel_scale.meters());
                                    review.last_capture_key=phase;
                                }
                            }
                            if let Some(w)=world.as_ref(){material_studio::bind_world(w.regions.view.session_name());}
                            if let Some(review) = benchmark.as_mut().filter(|b| b.auto_started && b.config.saved_review && b.config.capture_frames)
                                && review.last_capture_key == usize::MAX && review.motion_started_at.elapsed().as_secs_f32()
                                    >= review.config.warmup_seconds + review.config.duration_seconds - 1.0 {
                                    let root = std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| "target/saved-scene-review".into());
                                    gpu.request_capture(root.join("saved-world.png"));
                                    review.last_capture_key = 0;
                                }
                            egui_ui.begin_frame(window.as_ref());
                            if let Some(world) = world.as_ref() {
                                let cfg = world.sample_config();
                                egui_ui.elevation_m = player.pos.y * world.voxel_scale.meters();
                                egui_ui.biome_label = world.adventure.location
                                    .reading_at(cfg, player.pos * cfg.voxel_meters())
                                    .map_or("Wilderness", |reading| reading.label);
                                // Danger tier for the F3 readout; only sampled while it shows.
                                if egui_ui.show_debug_overlay() {
                                    let cell = player.pos.floor().as_ivec3();
                                    let biome = engine_worldgen::landscape::sample(&cfg, cell.x, cell.z).biome;
                                    egui_ui.danger = world.adventure.danger_at(player.pos * cfg.voxel_meters(), Some(biome));
                                }
                            }
                            let mut time_ui = TimeUiState {
                                time_of_day: day_night.time_of_day(),
                                day_length_seconds: day_night.day_length_seconds(),
                                freeze_time,
                                cloud_controls: day_night.cloud_controls(),
                            };
                            let mut physics_ui = PhysicsUiState {
                                enabled: physics_enabled,
                                stats: fluid_sim.stats(),
                                voxel_scale: world
                                    .as_ref()
                                    .map(|world| world.voxel_scale)
                                    .unwrap_or(selected_voxel_scale),
                            };
                            let previous_render_settings = render_distance_settings();
                            let mut render_ui = RenderUiState {
                                fxaa_enabled: gpu.fxaa_enabled,
                                vsync: gpu.vsync(),
                                settings: previous_render_settings,
                                debug_view: render_debug_view,
                                atmosphere_mode,
                            };
                            egui_ui.region_status = world.as_ref().map(World::region_status);
                            egui_ui.manyfold_hint = world.as_ref().and_then(|w| manyfold::hint(w, &player));
                            egui_ui.world_difficulty = world.as_ref().map_or_else(Default::default, |w| w.adventure.difficulty);
                            egui_ui.world_rules = world.as_ref().map_or_else(Default::default, |w| w.rules);
                            creative.set_locked(!egui_ui.world_rules.mode.creative_tools());
                            if let Some(w) = world.as_mut().filter(|w| w.ca_view.is_none() && !network_mode) {
                                let mut drops = std::mem::take(&mut w.inventory.dropped);
                                let scale = w.voxel_scale.meters();
                                for drop in &mut drops {
                                    let next = drop.pos - glam::Vec3::Y * (gameplay_dt * 2.5).min(0.1);
                                    if w.get(((next - glam::Vec3::Y * 0.12) / scale).floor().as_ivec3()).is_empty() { drop.pos = next; }
                                }
                                w.inventory.dropped = drops;
                            }
                            if let Some(w) = world.as_mut() {
                                world::goals::update(w, gameplay_dt, world::bed::is_night(day_night.time_of_day()));
                                world::goals::visit(w, player.pos * w.voxel_scale.meters());
                                world::chests::step(w, creative.block_page, player.pos, gameplay_dt);
                            }
                            egui_ui.expedition = world.as_ref().map(|w|w.expedition.clone());
                            if !hell_gate.busy() && !hall_gate.busy() && let Some(w) = world.as_mut() {
                                hell_gate.ambience(w, &player);
                                if !network_mode && ca_window.is_none() && !benchmark_config.enabled
                                    && hell_gate.crossing(w, &mut player) {
                                    hell_gate.request(&mut saves);
                                    input.clear();
                                    player.vel = Vec3::ZERO;
                                }
                            }
                            let mut outposts = std::mem::take(&mut egui_ui.factory.outposts);
                            let production_open = creative.show_palette && egui_ui.ctx.data(|d|
                                d.get_temp::<u32>(egui::Id::new("inventory_page")) == Some(7));
                            let logistics_open = creative.show_palette && egui_ui.ctx.data(|d|
                                d.get_temp::<u32>(egui::Id::new("inventory_page")) == Some(egui_ui::workshop::LOGISTICS));
                            egui_ui.factory = world.as_ref().map(|w|world::factory::view(w,player.pos,production_open,logistics_open)).unwrap_or_default();
                            if production_open && let Some(w) = world.as_mut() {
                                let now = Instant::now();
                                if now >= next_outpost_view {
                                    let mut bases = std::mem::take(&mut w.factory.bases);
                                    outposts = world::factory::overview::outposts(w, player.pos, &mut bases);
                                    w.factory.bases = bases;
                                    next_outpost_view = now + std::time::Duration::from_millis(250);
                                }
                                egui_ui.factory.outposts = outposts;
                            } else {
                                next_outpost_view = Instant::now();
                            }
                            if creative.show_palette
                                && egui_ui.factory.workshop.as_ref().is_some_and(|p| !p.delivery)
                                && egui_ui.ctx.data(|d| matches!(
                                    d.get_temp::<u32>(egui::Id::new("inventory_page")), Some(0 | 7 | 9)
                                ))
                                && let Some(w) = world.as_ref()
                            {
                                egui_ui.factory.iron_line = Some(world::factory::overview::workshop_iron(w, player.pos));
                            }
                            // The ledger includes a day of pass predictions. Do not
                            // build its presentation while playing or on other pages.
                            let orbital_open = creative.show_palette && egui_ui.ctx.data(|d|
                                d.get_temp::<u32>(egui::Id::new("inventory_page")) == Some(8));
                            egui_ui.lift = world.as_ref().map(|w| world::orbital::express::view(w, &player, &day_night)).unwrap_or_default();
                            if orbital_open {
                                egui_ui.orbital = world.as_ref().map(|w| {
                                    world::orbital::view(w, &day_night, &player)
                                }).unwrap_or_default();
                            } else {
                                egui_ui.orbital.receiver = world.as_ref().map(|w| {
                                    world::orbital::receiver::View {
                                        position: world::orbital::receiver::position(w),
                                        cargo_ready: world::orbital::receiver::cargo_ready(w, &day_night),
                                        delivered_iron: w.orbital.dynamics.delivered_iron,
                                        ..Default::default()
                                    }
                                }).unwrap_or_default();
                            }
                            egui_ui.adventure = world
                                .as_ref()
                                .map(|w| {
                                    let origin = player.pos * w.voxel_scale.meters();
                                    let mut view = w.adventure.view();
                                    view.hell_biome = engine_worldgen::nether::biome(&w.sample_config(), origin.x, origin.z)
                                        .map(engine_worldgen::nether::Biome::name);
                                    view.workshop_available = w.generator_version >= 48 && w.world_type != WorldType::Superflat;
                                    view.quest = egui_ui::quest_log::sync(w, origin, creative.selected_slot);
                                    #[cfg(feature = "multiplayer")]
                                    let gate_locked = network_mode && !multiplayer.as_ref().is_some_and(multiplayer::Session::can_cross_realm);
                                    #[cfg(not(feature = "multiplayer"))]
                                    let gate_locked = network_mode;
                                    if let Some(prompt) = world::adventure::chronicle::prompt(w, &player) {
                                        view.undercrown_focus = Some(prompt);
                                    } else if let Some(prompt) = world::hall_gate::prompt(w, &player, gate_locked) {
                                        view.undercrown_focus = Some(prompt);
                                    } else if let Some(prompt) = world::hell_gate::prompt(w, &player, false) {
                                        view.undercrown_focus = Some(prompt);
                                    } else if world::orbital::receiver::focused(w, &player) {
                                        view.undercrown_focus = Some(world::interaction::Prompt {
                                            title: "Meridian ground receiver", action: "F  Open service ledger",
                                        });
                                    } else if let Some(prompt) = world::orbital::express::prompt(w, &player) {
                                        view.undercrown_focus = Some(prompt);
                                    } else if let Some(prompt) = world::portals::gates::focus(w, &player) {
                                        view.undercrown_focus = Some(prompt);
                                    } else if let Some(prompt) = world::progression::study::prompt(w, &player) {
                                        view.undercrown_focus = Some(prompt);
                                    }
                                    if let Some(prompt) = world::interaction::prompt(w, &player, creative.selected_slot) {
                                        view.relic_focus = Some(prompt);
                                        view.undercrown_focus = None;
                                    }
                                    view
                                })
                                .unwrap_or_default();
                            if let Some(w) = world.as_mut() {
                                world::containers::flush(w);
                                if !creative.show_palette { w.inventory.container = None; }
                                if w.inventory.container.as_ref().is_some_and(|(id, _)| {
                                    w.caches.get(*id).is_none_or(|c| !c.cells.iter().any(|p| {
                                        w.get(*p) != BlockId::Air && (p.as_vec3() - player.pos).length() * w.voxel_scale.meters() <= 6.0
                                    }))
                                }) { w.inventory.container = None; creative.show_palette = false; }
                                egui_ui.survival = w.inventory.clone();
                                #[cfg(feature="multiplayer")]
                                if let Some(session)=multiplayer.as_ref(){egui_ui.survival=session.inventory_view(w,player.pos);}
                                egui_ui.survival_eat_timer = w.inventory.eat_timer;
                                egui_ui.survival_armor_points = w.inventory.armor_points();
                                egui_ui::progression_hud::publish(&egui_ui.ctx, w.progression.view(&w.inventory));
                                egui_ui::goals_hud::publish_world(&egui_ui.ctx, w);
                                let mut records = w.stats.view(w.progression.level());
                                let held = w.inventory.hotbar.get(creative.selected_slot).copied().flatten().map(|s| s.item);
                                records.clock = (held == Some(world::stats::CLOCK)).then(|| day_night.time_of_day());
                                egui_ui::records::publish(&egui_ui.ctx, records);
                                egui_ui.ctx.data_mut(|d| d.insert_temp(egui::Id::new("threshold_research"), w.progression.lore));
                                egui_ui::publish_study(&egui_ui.ctx, world::progression::study::view(w, &player));
                            }
                            egui_ui.region_error = fluid_sim.authority_error.clone()
                                .or_else(|| world.as_ref().and_then(|w|w.regions.view.error()))
                                .or_else(||chunk_streamer.upload_error().map(str::to_owned));
                            #[cfg(feature="multiplayer")]
                            if network_mode {multiplayer::inventory::begin();}
                            egui_ui.target_water = None;
                            if let Some(w) = world.as_ref() {
                                let liquid = world::water_tools::target(w, &player)
                                    .map(|(p, _)| (p, w.get(p)))
                                    .filter(|&(_, block)| matches!(block, BlockId::Water | BlockId::Lava | BlockId::SulfuricAcid));
                                let wet = liquid.filter(|&(_, block)| block == BlockId::Water).map(|(p, _)| p);
                                egui_ui.target_water = water_probe.update(w, wet);
                                if let Some((p, block)) = liquid {
                                    egui_ui.target_block = Some(block);
                                    egui_ui.target_volume_ul = wet.map(|_| w.water_microlitres(p));
                                    egui_ui.target_machine = None;
                                    egui_ui.target_environment = None;
                                    egui_ui.target_cell_temp_c = None;
                                    egui_ui.target_harvest = None;
                                }
                            }
                            // The HUD editor closes when play pauses or the mouse is captured again.
                            if egui_ui::hud_layout::editing() && (grabbed || !matches!(mode, AppMode::Playing)) {
                                egui_ui::hud_layout::set_editing(false);
                            }
                            let mut action = egui_ui.build_ui(
                                if spawning { UiMode::Loading } else { ui_mode(&mode) },
                                &mut selected_world,
                                &mut selected_voxel_scale,
                                &mut selected_seed,
                                fps_counter.fps_rounded(),
                                display_frame_dt,
                                &mut player,
                                &mut time_ui,
                                &mut physics_ui,
                                &mut render_ui,
                                &perf_snapshot,
                                &mut creative,
                                pending_tool_action.take(),
                                if network_mode { None } else { world.as_mut() },
                            );
                            if !spawning {
                                if matches!(mode, AppMode::Playing) && let Some(w) = world.as_mut() {
                                    world::farming::jobs::quarter::panel::draw(&egui_ui.ctx, w, &mut player);
                                    world::farming::jobs::production::scene::panel(&egui_ui.ctx, w);
                                }
                                egui_ui::blueprints::draw(&egui_ui.ctx, &mut creative.show_blueprints, &mut blueprints, edit_history.depth(),
                                    world.as_ref().is_some_and(|w| w.sample_config().civilization_enabled()));
                                egui_ui::records::draw(&egui_ui.ctx, &mut creative.show_records);
                                if egui_ui.show_hud {
                                    egui_ui::records::hud(&egui_ui.ctx);
                                    if matches!(mode, AppMode::Playing) && !creative.any_ui_open() {
                                        egui_ui::goals_hud::tracker(&egui_ui.ctx);
                                    }
                                }
                            }
                            #[cfg(feature="multiplayer")]
                            if network_mode {
                                let commands=multiplayer::inventory::finish();
                                if let Some(session)=multiplayer.as_mut(){session.queue_inventory(commands);}
                            }
                            #[cfg(feature="multiplayer")]
                            if let (Some(session), Some(w)) = (multiplayer.as_mut(), world.as_ref())
                                && !spawning && matches!(mode, AppMode::Playing | AppMode::Paused)
                            {
                                if !matches!(mode, AppMode::Playing) && chat_ui.is_open() {
                                    chat_ui.close();
                                }
                                if egui_ui.show_hud {
                                    let eye = player.eye(w.voxel_scale);
                                    let size = window.inner_size();
                                    let aspect = size.width.max(1) as f32 / size.height.max(1) as f32;
                                    let view_proj = glam::camera::rh::proj::directx::perspective(
                                        view_zoom.lens(egui_ui.fov_degrees, player.movement.fov_kick_degrees()).to_radians(),
                                        aspect,
                                        0.05,
                                        4096.0,
                                    ) * glam::camera::rh::view::look_to_mat4(eye, player.forward_dir(), player.up_dir());
                                    egui_ui::chat::name_tags(&egui_ui.ctx, view_proj, eye, w.voxel_scale.meters(), &session.name_tags(w.voxel_scale.meters()));
                                }
                                if egui_ui::chat::draw(&egui_ui.ctx, &mut chat_ui, &mut session.social) && matches!(mode, AppMode::Playing) {
                                    capture_mouse(window, &mut grabbed, false, true);
                                }
                            }
                            if !network_mode && !spawning && matches!(mode, AppMode::Playing | AppMode::Paused) {
                                if !matches!(mode, AppMode::Playing) { chat_ui.close(); }
                                if egui_ui::chat::draw(&egui_ui.ctx, &mut chat_ui, &mut local_social) && matches!(mode, AppMode::Playing) {
                                    capture_mouse(window, &mut grabbed, false, true);
                                }
                                if let Some(line) = local_social.take_outbox().into_iter().next() {
                                    action = UiAction::LocalChat(line);
                                }
                            }
                            if !network_mode && let Some(w) = world.as_mut() {
                                w.inventory = egui_ui.survival.clone();
                                world::containers::flush(w);
                                w.sync_from_inventory();
                            }
                            if let Some(w) = world.as_mut() {
                                let was_open = creative.block_page.is_some() || creative.trade_page.is_some();
                                let nav_was_open = creative.show_system_map;
                                if benchmark_config.enabled && !spawning && std::env::var_os("VOXEL_REVIEW_NAVIGATION").is_some() {
                                    egui_ui::system_map::review_frame(&egui_ui.ctx, &mut creative, w, &player);
                                    egui_ui.show_hud = true;
                                }
                                if benchmark_config.enabled && !spawning && std::env::var_os("VOXEL_REVIEW_SCANNER").is_some()
                                    && !egui_ui.ctx.data_mut(|d| d.get_temp::<bool>(egui::Id::new("scanner_native_prepared")).unwrap_or(false)) {
                                    creative.close_panels();
                                    creative.block_page = Some(world::scanner::review::prepare(w, &player));
                                    egui_ui.show_hud = true;
                                    egui_ui.ctx.data_mut(|d| d.insert_temp(egui::Id::new("scanner_native_prepared"), true));
                                }
                                let eye = player.eye(w.voxel_scale);
                                let view_proj = glam::camera::rh::proj::directx::perspective(
                                    view_zoom.lens(egui_ui.fov_degrees, 0.0).to_radians(),
                                    gpu.aspect_ratio(),
                                    0.05,
                                    4096.0,
                                ) * glam::camera::rh::view::look_to_mat4(eye, player.forward_dir(), player.up_dir());
                                egui_ui::block_page::draw(&egui_ui.ctx, &mut creative.block_page, w, eye, view_proj, network_mode);
                                if !spawning && matches!(mode, AppMode::Playing) {
                                    // VOXEL_REVIEW_NAV=<body>[,map]: benchmark captures hide the HUD; show the instruments, target a body, open the map.
                                    let nav_review = benchmark_config.enabled && space_nav::review(&mut creative.show_system_map);
                                    if (egui_ui.show_hud || nav_review) && !creative.any_ui_open() {
                                        egui_ui::nav_console::draw(&egui_ui.ctx, w, &player, view_proj);
                                        egui_ui::flight_hud::draw(&egui_ui.ctx, w, &player, view_zoom.lens(egui_ui.fov_degrees, 0.0));
                                    }
                                    egui_ui::system_map::draw(&egui_ui.ctx, &mut creative.show_system_map, w, &player);
                                    egui_ui::map_hub::frame(&egui_ui.ctx, &mut creative, w, &player);
                                    if nav_was_open && !creative.show_system_map {
                                        sync_creative_ui_cursor(window, &mut grabbed, focused, &mode, &creative);
                                    }
                                }
                                let _trades = egui_ui::trade_page::draw(&egui_ui.ctx, &mut creative.trade_page, w,
                                    eye * w.voxel_scale.meters(), network_mode);
                                #[cfg(feature = "multiplayer")]
                                if let Some(session) = multiplayer.as_mut() {
                                    for (villager, action) in _trades { session.queue_resident(villager, action); }
                                }
                                #[cfg(feature="multiplayer")]
                                if let Some(session) = multiplayer.as_mut()
                                    && let Some((anchor, action)) = egui_ui.ctx.data_mut(|d| {
                                        let id = egui::Id::new("scanner_request");
                                        let request = d.get_temp::<(IVec3, world::scanner::commands::Action)>(id);
                                        d.remove::<(IVec3, world::scanner::commands::Action)>(id);
                                        request
                                    }) {
                                    session.queue_scanner(anchor, action);
                                }
                                #[cfg(feature="multiplayer")]
                                if was_open && creative.block_page.is_none() && let Some(session) = multiplayer.as_mut() {
                                    session.queue_inventory(vec![multiplayer::inventory::Command::Return]);
                                }
                                let open = creative.block_page.is_some() || creative.trade_page.is_some();
                                if (open && grabbed) || (was_open && !open) {
                                    sync_creative_ui_cursor(window, &mut grabbed, focused, &mode, &creative);
                                }
                            }
                            if ca_window.is_some() {
                                creative.tool = creative::ToolKind::Hand;
                            }
                            if let Some(c) = world.as_ref().map(|w| w.sample_config()).filter(engine_worldgen::geo::active) {
                                engine_worldgen::geo::prewarm_for(c);
                            }
                            if spawning && let Some(w) = world.as_ref() {
                                egui_ui.build_world_loading_ui(&loading_report::build(&loading_report::Inputs {
                                    world: w, gpu: &gpu, spawn: spawn_progress,
                                    spawn_ready: chunk_cache.spawn_ready, player_voxel: player.pos, ready: continue_ready,
                                }));
                            }
                            let mut cpu_stats = CpuFrameStats::default();
                            cpu_stats.ui_ms = ui_start.elapsed().as_secs_f32() * 1000.0;
                            player.selected_block = creative.selected_block();
                            #[cfg(feature="multiplayer")]
                            let clock_owner=multiplayer.as_ref().is_none_or(|s|s.is_host());
                            #[cfg(not(feature="multiplayer"))]
                            let clock_owner=true;
                            if clock_owner {
                                day_night.set_time_of_day(time_ui.time_of_day);
                                day_night.set_day_length_seconds(time_ui.day_length_seconds);
                                day_night.set_cloud_controls(time_ui.cloud_controls);
                            }
                            freeze_time = time_ui.freeze_time || receiver_review.is_some();
                            physics_enabled = physics_ui.enabled;
                            let previous_debug_view = render_debug_view;
                            let previous_atmosphere_mode = atmosphere_mode;
                            render_debug_view = render_ui.debug_view;
                            gpu.set_vsync(render_ui.vsync);
                            gpu.fxaa_enabled = render_ui.fxaa_enabled;
                            atmosphere_mode = render_ui.atmosphere_mode;
                            fluid_sim.set_enabled(physics_enabled);
                            physics_enabled = fluid_sim.enabled();

                            if render_debug_view != previous_debug_view
                                || atmosphere_mode != previous_atmosphere_mode
                            {
                                gpu.invalidate_cloud_history();
                            }

                            if render_ui.settings != previous_render_settings {
                                set_render_distance_settings(render_ui.settings);
                                if let Some(world_ref) = world.as_mut() {
                                    let snapshot = world_ref.snapshot();
                                    chunk_cache
                                        .sync_visible_to_render_settings_with_snapshot(&snapshot);
                                    chunk_cache.retain_visible();
                                    chunk_streamer.retain_visible(&chunk_cache.visible_keys);
                                    chunk_streamer.queue_needed_visible(
                                        &chunk_cache,
                                        chunk_cache.center_chunk,
                                    );
                                    chunk_streamer.set_view(
                                        player.precise_eye(world_ref.voxel_scale), player.forward_dir(),
                                        player.up_dir(), egui_ui.fov_degrees, gpu.aspect_ratio(),
                                    );
                                    chunk_streamer.enqueue_missing(
                                        gpu,
                                        &snapshot,
                                        world_ref.revision(),
                                        &mut chunk_cache,
                                    );
                                chunk_streamer.upload_finished(
                                        gpu,
                                        &snapshot,
                                        &mut chunk_cache,
                                        world_ref.revision(),
                                        12,
                                    );
                                } else {
                                    chunk_cache.sync_visible_to_render_settings();
                                    chunk_cache.retain_visible();
                                    chunk_streamer.retain_visible(&chunk_cache.visible_keys);
                                }
                            }

                            let prepared_planet = pending_planet.take();
                            let resume_planet = prepared_planet.is_some();
                            let action = prepared_planet.map_or(action, |(_, pending)| pending);
                            #[cfg(feature="multiplayer")]
                            let action=if let Some(session)=multiplayer.as_mut(){session.route_ui(action)}else{action};
                            let host_weather=clock_owner && matches!(action,UiAction::SetWeatherPreset(_) | UiAction::RandomizeWeather);
                            #[cfg(feature="multiplayer")]
                            let allowed=multiplayer::coop::Mode::of(multiplayer.as_ref()).allows(&action,host_weather);
                            #[cfg(not(feature="multiplayer"))]
                            let allowed=!network_mode || host_weather;
                            let action = if !allowed {
                                if let Some(w)=world.as_mut(){w.expedition.say("Host Only");}
                                UiAction::None
                            } else { action };
                            // Leaving a joined session is the world list; that arm ends the session.
                            let action = match action {
                                UiAction::Coop(egui_ui::coop_panel::Request::Leave) => UiAction::BackToWorldSelect,
                                other => other,
                            };
                            let action = if ca_window.is_some()
                                && !matches!(action, UiAction::None | UiAction::Resume | UiAction::Quit | UiAction::SaveWorld) {
                                if let Some(w) = world.as_mut() { w.expedition.say("Cellular session: save or quit"); }
                                UiAction::None
                            } else { action };
                            let action = if exit_pending { UiAction::None } else if let UiAction::LocalChat(ref line) = action {
                                if !network_mode && let Some(w) = world.as_mut() {
                                    local_commands::execute(line, &mut local_social, w, &mut player, &mut day_night)
                                } else { UiAction::None }
                            } else { action };
                            let action = if let Some(bridge) = &dev_panel {
                                bridge.poll(action, world.as_mut().filter(|_| {
                                    !network_mode && !spawning && !exit_pending && ca_window.is_none()
                                        && !hell_gate.busy() && !hall_gate.busy()
                                        && matches!(mode, AppMode::Playing | AppMode::Paused)
                                }), &mut player, &mut day_night)
                            } else { action };
                            if let Some(path) = dev_panel::engine::take_capture() { gpu.request_capture(path); }
                            // A realm layout edit in the dev tools: rebuild every resident chunk from worldgen.
                            if dev_panel::gilded_layout::take_regen() && let Some(w) = world.as_mut() {
                                let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                let snapshot = w.snapshot_near(center, render_distance_settings().view_distance_chunks);
                                chunk_cache.reset_with_snapshot(center, &snapshot);
                                chunk_streamer.reset();
                                chunk_streamer.queue_needed_visible(&chunk_cache, center);
                            }
                            if let UiAction::StartWorld { world_type: WorldType::Planet, voxel_scale, seed, planet, .. } = &action {
                                let generator_version = new_planet_generator(planet.is_some());
                                let cfg = engine_voxel::WorldSampleConfig {
                                    generator_version, seed: *seed, world_type: WorldType::Planet,
                                    voxel_scale: *voxel_scale, planet: *planet,
                                };
                                if engine_worldgen::planetary::drift_active(&cfg) && !resume_planet {
                                    egui_ui::world_loader::reset_drift(&egui_ui.ctx, loading_report::world_key(&cfg));
                                    if !engine_worldgen::geo::entry_for(&cfg).ready() {
                                        engine_worldgen::geo::prewarm_for(cfg);
                                        pending_planet = Some((cfg, action));
                                        egui_ui.end_frame_no_paint(window.as_ref());
                                        window.request_redraw();
                                        return;
                                    }
                                }
                            }
                            match if exit_pending {UiAction::None} else {action} {
                                UiAction::LocalChat(_) => {}
                                UiAction::None => {}
                                UiAction::Study(lesson) => {
                                    if let Some(w) = world.as_mut()
                                        && world::progression::study::buy(w, &player, lesson)
                                    {
                                        saves.request(true);
                                    }
                                }
                                UiAction::StudyMaterials(lesson, research) => {
                                    if let Some(w) = world.as_mut()
                                        && world::progression::study::buy_materials(w, &player, lesson, research)
                                    {
                                        saves.request(true);
                                    }
                                }
                                UiAction::DropInventory(slot) => {
                                    if let Some(w) = world.as_mut() {
                                        if w.inventory.dropped.len() < 1024 {
                                            let stack = w.inventory.cursor.take().or_else(|| w.inventory.hotbar.get_mut(slot).and_then(Option::take));
                                            if let Some(stack) = stack {
                                                let scale = w.voxel_scale.meters();
                                                let origin = player.eye(w.voxel_scale);
                                                let dir = player.forward_dir();
                                                // A drag-out throw lands to the side of the
                                                // aim ray, so the stack visibly leaves the
                                                // hand instead of appearing under the feet.
                                                let toss = (dir + player.right_dir() * 0.45).normalize_or_zero();
                                                let mut pos = origin * scale;
                                                for i in 1..=8 {
                                                    let candidate = origin * scale + toss * (i as f32 * 0.2);
                                                    if !w.get((candidate / scale).floor().as_ivec3()).is_empty() { break; }
                                                    pos = candidate;
                                                }
                                                if std::env::var_os("VOXEL_TOOL_REVIEW").is_some() { eprintln!("[tool-review] dropped {:?} count={} at={:?}",stack.item,stack.count,pos); }
                                                w.inventory.dropped.push(world::inventory::DroppedStack { pos, stack });
                                                w.inventory.pickup_cooldown = 0.6;
                                                w.sync_from_inventory();
                                                w.expedition.say("Stack dropped");
                                            }
                                        } else { w.expedition.say("Retrieve some dropped stacks first."); }
                                    }
                                }
                                UiAction::SelectCreativeItem(item,count,slot) => {
                                    if let Some(w) = world.as_mut().filter(|_| creative.catalogue_open()) {
                                        if let Err(error) = creative::catalogue::select(w,item,count,slot) {
                                            w.expedition.say(error.to_string());
                                        }
                                    }
                                }
                                UiAction::SelectCreativeRow(items) => {
                                    if let Some(w) = world.as_mut().filter(|_| creative.catalogue_open()) {
                                        if let Err(error) = creative::catalogue::select_row(w,&items) {
                                            w.expedition.say(error.to_string());
                                        }
                                    }
                                }
                                UiAction::GiveItem(item,count) => {
                                    if let Some(w) = world.as_mut().filter(|_| creative.catalogue_open()) {
                                        let limit = w.inventory.stack_limit(item);
                                        let stack = world::inventory::ItemStack::new(
                                            item,
                                            count.clamp(1, limit.max(1)),
                                        );
                                        let (inserted, remaining) = w.inventory.try_insert_pack_first(stack);
                                        w.sync_from_inventory();
                                        if let world::inventory::Item::Weapon(tier) = item
                                            && inserted > 0 {
                                                let _ = w.adventure.select_weapon(tier as u32);
                                            }
                                        if inserted > 0 {
                                            w.expedition.say(format!(
                                                "Creative catalogue: {} ×{} added to your pack.",
                                                item.name(),
                                                inserted
                                            ));
                                        }
                                        if remaining > 0 {
                                            w.expedition.say("Pack full. Clear a slot and take the item again.");
                                        }
                                    }
                                }
                                UiAction::Factory(action) => {
                                    if let Some(world) = world.as_mut()
                                        && let Some(block) = world::factory::action(world,player.pos,action) {
                                            creative.assign_hotbar_slot(creative.selected_slot,block);
                                            creative.tool = creative::ToolKind::Hand;
                                            player.selected_block = block;
                                        }
                                }
                                UiAction::WorkshopMap(target) => {
                                    if let Some(world) = world.as_ref() {
                                        egui_ui::map::show_workshop(&mut creative, world.sample_config(), target);
                                        input.clear();
                                    }
                                }
                                UiAction::Chip(action) => {
                                    if let Some(w) = world.as_mut() {
                                        world::factory::chips::chip_action(w, player.pos, action);
                                    }
                                }
                                UiAction::RequestOrbital(cell, channel) => {
                                    if let Some(w) = world.as_mut() {
                                        let message = world::orbital::radio::request(w, &day_night,
                                            &player, cell, channel);
                                        w.expedition.say(message);
                                        saves.request(true);
                                    }
                                }
                                UiAction::SystemMap => {
                                    egui_ui::system_map::toggle(&mut creative, world.as_ref());
                                    input.clear();
                                    sync_creative_ui_cursor(window, &mut grabbed, focused, &mode, &creative);
                                }
                                UiAction::OrbitalRadio(action) => {
                                    if let Some(w) = world.as_mut() {
                                        for cell in world::orbital::radio::action(w, &day_night, &player, action) {
                                            dirty_blocks.insert(cell);
                                            mark_chunk_and_neighbors_dirty(&mut dirty_chunks, cell);
                                        }
                                        saves.request(true);
                                    }
                                }
                                UiAction::OrbitalReceiver(action) => {
                                    if let Some(w) = world.as_mut() {
                                        let changed = world::orbital::receiver::action(w, &day_night, &player, action);
                                        for cell in changed {
                                            dirty_blocks.insert(cell);
                                            mark_chunk_and_neighbors_dirty(&mut dirty_chunks, cell);
                                        }
                                        saves.request(true);
                                    }
                                }
                                UiAction::SaveWorld => {
                                    #[cfg(feature = "multiplayer")]
                                    let local = multiplayer::coop::Mode::of(multiplayer.as_ref()).local_saves();
                                    #[cfg(not(feature = "multiplayer"))]
                                    let local = !network_mode;
                                    // A building host writes its building save; a hosted world writes
                                    // its friends' profiles here and its checkpoint below.
                                    if network_mode {
                                        #[cfg(feature = "multiplayer")]
                                        if let (Some(session),Some(w))=(multiplayer.as_mut(),world.as_mut()) {
                                            match session.save(w,&player,&day_night) {
                                                Ok(())=>if !local {w.expedition.say("World saved")},
                                                Err(error)=>w.expedition.say(error.to_string()),
                                            }
                                        }
                                    }
                                    if !local {}
                                    else if let Some(session) = ca_window.as_mut() { session.request(); }
                                    else {saves.request(true);}
                                }
                                UiAction::Coop(request) => {
                                    #[cfg(feature = "multiplayer")]
                                    {
                                        use egui_ui::coop_panel::Request;
                                        let coop_mode = multiplayer::coop::Mode::of(multiplayer.as_ref());
                                        match coop.request(request, world.as_ref(), coop_mode) {
                                            Some(Request::Join { code }) => {
                                                coop.join(&code);
                                            }
                                            Some(Request::Close) => {
                                                if multiplayer::coop::end(&mut multiplayer, world.as_mut(), &player, &day_night, Some(&mut *fluid_sim))
                                                    == Some(multiplayer::coop::Mode::Opened) {
                                                    saves.request(true);
                                                    if let Some(w) = world.as_mut() { w.expedition.say("Closed to Friends"); }
                                                }
                                                network_mode = false;
                                            }
                                            Some(Request::AcceptAnswer(answer)) => {
                                                if let Some(session) = multiplayer.as_ref() {
                                                    coop.status = match session.accept_punch_answer(&answer) {
                                                        Ok(()) => Some("Reply Accepted".into()),
                                                        Err(error) => Some(error.to_string()),
                                                    };
                                                }
                                            }
                                            #[cfg(feature = "multiplayer-sdr")]
                                            Some(Request::InviteFriends(code)) => {
                                                coop.status = engine_net::steam::invites::open_overlay(&code)
                                                    .err().map(|error| error.to_string());
                                            }
                                            _ => {}
                                        }
                                    }
                                    #[cfg(not(feature = "multiplayer"))]
                                    let _ = request;
                                }
                                UiAction::OpenWorld(id)=>{
                                    #[cfg(feature = "multiplayer")]
                                    { host_after_load = false; }
                                    library.open(id);
                                }
                                UiAction::HostWorld(id)=>{
                                    #[cfg(feature = "multiplayer")]
                                    { host_after_load = world.is_none() && !library.loading(); }
                                    library.open(id);
                                }
                                UiAction::Respawn=>if let Some(w)=world.as_mut(){world::adventure::death::respawn(w,&mut player);},
                                UiAction::SetRules(rules)=>if let Some(w)=world.as_mut(){w.set_rules(rules);},
                                UiAction::SetDifficulty(level)=>{
                                    if let Some(w)=world.as_mut(){w.adventure.set_difficulty(level);w.expedition.say(format!("Difficulty: {}",level.label()));}
                                }
                                UiAction::RefreshWorlds=>library.refresh(),
                                UiAction::DeleteAllWorlds=>library.delete_all(),
                                UiAction::RenameWorld {id,name}=>library.rename(id,name),
                                UiAction::FavoriteWorld {id,favorite}=>library.favorite(id,favorite),
                                UiAction::StartWorld {
                                    name,
                                    reference,
                                    world_type,
                                    voxel_scale,
                                    seed,
                                    planet,
                                    difficulty,
                                    mode: game_mode,
                                    systems: world_systems,
                                } => {
                                    reset_gameplay_input(
                                        &mut input,
                                        &mut place_was_down,
                                        &mut break_was_down,
                                        &mut place_repeat_timer,
                                        &mut break_repeat_timer,
                                        &mut pending_tool_action,
                                    );
                                    selected_world = match world_type {
                                        WorldType::Hills | WorldType::Ridged => WorldType::Planet,
                                        kind => kind,
                                    };
                                    selected_voxel_scale = voxel_scale;
                                    selected_seed = seed;
                                    creative.cancel_edits();
                                    start_world(
                                        selected_world,
                                        selected_voxel_scale,
                                        selected_seed,
                                        planet,
                                        &mut world,
                                        &mut player,
                                        &mut day_night,
                                        &mut chunk_cache,
                                        chunk_streamer,
                                        fluid_sim,
                                        &mut dirty_blocks,
                                        &mut dirty_chunks,
                                        &mut mode,
                                        window,
                                        &mut grabbed,
                                        focused,
                                        &mut last,
                                        &mut last_safe_player_pos,
                                    );
                                    if let Some(w)=world.as_mut(){w.adventure.set_difficulty(difficulty);}
                                    if let Some(w)=world.as_mut(){
                                        let rules=world::rules::Rules{systems:world_systems,..world::rules::Rules::new_world(game_mode)};
                                        w.set_rules(rules);
                                    }
                                    if reference
                                        && let Some(w)=world.as_mut() {
                                            reference_environment::install(w);
                                            reference_environment::camera(&mut player,selected_voxel_scale.meters());
                                            let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                            let snapshot=w.snapshot_near(center,8);
                                            chunk_cache.reset_with_snapshot(center,&snapshot);
                                            chunk_streamer.reset();
                                            chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                        }
                                    if let Some(w)=world.as_ref(){library.rename(w.regions.view.session_name().to_owned(),name);}
                                    cloud_weather.reset(selected_seed);
                                    physics_enabled = !reference;
                                    freeze_time = reference;
                                }
                                UiAction::SetWeatherPreset(preset) => {
                                    day_night.set_weather_preset(preset);
                                    gpu.invalidate_cloud_history();
                                }
                                UiAction::RandomizeWeather => {
                                    day_night.randomize_weather();
                                    gpu.invalidate_cloud_history();
                                }
                                UiAction::Resume => {
                                    player.jump_tap_timer = 0.0;
                                    player.jump_was_down = false;
                                    reset_gameplay_input(
                                        &mut input,
                                        &mut place_was_down,
                                        &mut break_was_down,
                                        &mut place_repeat_timer,
                                        &mut break_repeat_timer,
                                        &mut pending_tool_action,
                                    );
                                    mode = AppMode::Playing;
                                    capture_mouse(window, &mut grabbed, false, focused);
                                    last = Instant::now();
                                }
                                UiAction::BackToWorldSelect => {
                                    #[cfg(feature = "multiplayer")]
                                    {
                                        let ended = multiplayer::coop::end(&mut multiplayer, world.as_mut(), &player, &day_night, Some(&mut *fluid_sim));
                                        network_mode = false;
                                        // A joiner goes back to the world it left, if it came from one.
                                        if ended == Some(multiplayer::coop::Mode::Joined) && let Some(id) = coop.return_to.take() {
                                            library.open(id);
                                        }
                                    }
                                    reset_gameplay_input(
                                        &mut input,
                                        &mut place_was_down,
                                        &mut break_was_down,
                                        &mut place_repeat_timer,
                                        &mut break_repeat_timer,
                                        &mut pending_tool_action,
                                    );
                                    mode = AppMode::WorldSelect;
                                    egui_ui.show_title_screen();
                                    release_mouse(window, &mut grabbed);
                                    physics_enabled = false;
                                    fluid_sim.set_enabled(false);
                                    fluid_sim.reset_for_world(selected_voxel_scale);
                                    library.refresh();
                                    creative.cancel_edits();
                                    world = None;
                                    chunk_cache.reset(glam::IVec3::ZERO);
                                    chunk_streamer.reset();
                                    dirty_blocks.clear();
                                    dirty_chunks.clear();
                                }
                                UiAction::Quit => {
                                    #[cfg(feature = "multiplayer")]
                                    if multiplayer::coop::Mode::of(multiplayer.as_ref()) == multiplayer::coop::Mode::Opened {
                                        multiplayer::coop::end(&mut multiplayer, world.as_mut(), &player, &day_night, Some(&mut *fluid_sim));
                                        network_mode = false;
                                    }
                                    if network_mode {
                                        #[cfg(feature = "multiplayer")]
                                        if let (Some(session),Some(w))=(multiplayer.as_mut(),world.as_mut()) && session.persistent_host()
                                            && let Err(error)=session.save(w,&player,&day_night) {eprintln!("[multiplayer] final save failed: {error:#}");}
                                        elwt.exit();
                                    }
                                    else if !benchmark_config.enabled {
                                        mode = AppMode::Paused;
                                        release_mouse(window, &mut grabbed);
                                        exit_pending = true;
                                        if world.is_some() {
                                            if let Some(session) = ca_window.as_mut() { session.request(); }
                                            else { saves.request(true); }
                                        }
                                    } else {elwt.exit();}
                                }
                            }

                            if let Some(w) = world.as_mut().filter(|_| ca_window.is_some()) {
                                simulation::tools::select(w, &mut player, creative.selected_slot);
                            } else { player.selected_block = creative.selected_block(); }

                            if benchmark_config.enabled && !egui_ui::nav_console::manual_review() && !world::adventure::enemy_review_manual() && wand_review.as_ref().is_none_or(|r| r.automatic) {
                                if grabbed { release_mouse(window, &mut grabbed); }
                            } else if world.as_ref().is_some_and(|w| w.adventure.is_dead()) {
                                if grabbed { release_mouse(window, &mut grabbed); }
                            } else { sync_creative_ui_cursor(
                                window,
                                &mut grabbed,
                                focused,
                                &mode,
                                &creative,
                            ); }

                            combat_audio.set_active(matches!(mode,AppMode::Playing) && focused && !spawning);
                            draw_list.clear();
                            let mut visible_missing = (0, 0);
                            if spawning || creative.any_ui_open() || !matches!(mode, AppMode::Playing) {
                                outline.clear();
                                invert_outline.clear();
                            }
                            let mut nearby_torch_count = 0usize;
                            let render_world =
                                !matches!(mode, AppMode::WorldSelect) && world.is_some();
                            let paused_render = !render_world;

                            if (matches!(mode, AppMode::Playing) || network_mode) && !hell_gate.busy() && !hall_gate.busy()
                                && let Some(world) = world.as_mut()
                            {
                                let gameplay_start = Instant::now();
                                let mut frame_gameplay_trace = gameplay_trace::Trace::new();
                                creative.set_locked(!world.rules.mode.creative_tools());
                                creative.update_feedback(gameplay_dt);

                                let benchmark_camera_active = benchmark_motion_active(benchmark.as_ref());
                                let mut active_input = if benchmark_camera_active {
                                    Input::default()
                                } else {
                                    input.clone()
                                };
                                if creative.any_ui_open() || spawning || !matches!(mode, AppMode::Playing) || (!grabbed && !benchmark_camera_active) {
                                    active_input.clear();
                                }

                                if let Some(probe)=latency_probe.as_mut() {
                                    probe.input(world,&mut player,&mut creative,&mut active_input,&mut dirty_blocks,&mut dirty_chunks,!spawning);
                                }
                                if !creative.any_ui_open() && grabbed && !spawning {
                                    let dir = input.wheel.trunc() as i32;
                                    if dir != 0 {
                                        if active_input.ctrl_down {
                                            creative.adjust_range(dir);
                                        } else if active_input.alt_down {
                                            creative.adjust_cursor_size(dir);
                                        } else {
                                            player.selected_block = creative.cycle_hotbar_slot(-dir);
                                        }
                                        input.wheel -= dir as f32;
                                    }
                                } else {
                                    input.wheel = 0.0;
                                }

                                #[cfg(feature = "multiplayer")]
                                if let Some(session) = multiplayer.as_mut() {
                                    if session.ready() && !spawning
                                        && world::interaction::gameplay_enabled(focused, grabbed, creative.any_ui_open(), egui_ui.ctx.egui_wants_keyboard_input())
                                        && (active_input.interact || (active_input.place_pressed && !active_input.sneak
                                            && creative.tool == creative::ToolKind::Hand))
                                        && let Some(villager) = world::travel::trade_target(world, &player)
                                    {
                                        creative.close_panels();
                                        creative.trade_page = Some(villager);
                                        active_input.clear();
                                        input.clear();
                                        release_mouse(window, &mut grabbed);
                                    }
                                    if !spawning && world::interaction::gameplay_enabled(focused, grabbed, creative.any_ui_open(), egui_ui.ctx.egui_wants_keyboard_input())
                                        && (active_input.interact || (active_input.place_pressed && !active_input.sneak))
                                        && let Some((cell, _)) = world::raycast_block(world, player.eye(world.voxel_scale), player.forward_dir(), 6.0 / world.voxel_scale.meters())
                                        && world.get(cell).chest_tier().is_some() {
                                        creative.close_panels(); creative.block_page = Some(cell);
                                        active_input.clear(); input.clear();
                                        release_mouse(window, &mut grabbed);
                                    }
                                    let read_threshold_guide = session.ready() && !spawning && active_input.interact
                                        && world::interaction::gameplay_enabled(focused, grabbed, creative.any_ui_open(), egui_ui.ctx.egui_wants_keyboard_input())
                                        && world::progression::study::guide_focused(world, &player);
                                    session.configure(&creative);
                                    session.hold_body(spawning);
                                    session.rendered_review_input(world, &mut player, &mut active_input);
                                    gameplay_trace::frame_stage("network");
                                    session.update(world, &mut player, &mut day_night, &active_input, raw_frame_dt, &mut dirty_blocks, &mut dirty_chunks);
                                    gameplay_trace::frame_stage("network-done");
                                    if read_threshold_guide {
                                        creative.close_panels();
                                        creative.show_palette = true;
                                        egui_ui::open_magic_guide(&egui_ui.ctx);
                                        release_mouse(window, &mut grabbed);
                                    }
                                    outline.clear();
                                    invert_outline.clear();
                                    let can_act = matches!(mode, AppMode::Playing) && grabbed && !spawning && !creative.any_ui_open();
                                    let mining = session.mining_feedback(world, &player, &active_input, can_act, raw_frame_dt);
                                    egui_ui.target_block = mining.target.map(|(cell, _)| world.get(cell));
                                    egui_ui.tool_feedback.break_progress = mining.progress;
                                    egui_ui.tool_feedback.break_requires = mining.requires;
                                    if egui_ui.show_hud && can_act && let Some((cell, _)) = mining.target {
                                        let expand = 0.003 / world.voxel_scale.meters();
                                        outline.extend(build_outline_vertices(cell, [0.82, 0.90, 0.94, 0.85], expand));
                                        if let Some(progress) = mining.progress {
                                            mesh::mining::append_block(world, cell, progress, expand * 1.5, &mut outline);
                                        }
                                    }
                                    let digging = mining.target.filter(|_| mining.working && mining.progress.is_some());
                                    player.viewmodel.mine_timing = mining.timing;
                                    soundscape.dig(&mut combat_audio, digging, mining.timing, world, player.eye(world.voxel_scale) * world.voxel_scale.meters(), player.forward_dir());
                                    if session.take_world_reset() {
                                        let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        chunk_streamer.reset();
                                        if let Some(warmup) = session.take_portal_warmup() {
                                            chunk_cache.reset_with_snapshot(center, &warmup.snapshot);
                                            chunk_streamer.install_portal_warmup(&mut chunk_cache, warmup);
                                        } else {
                                            chunk_cache.reset_with_snapshot(center, &world.snapshot_near(center, render_distance_settings().view_distance_chunks));
                                        }
                                        chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                        fluid_sim.reset_for_world(world.voxel_scale);
                                        gpu.invalidate_cloud_history();
                                        cloud_weather.reset(world.seed);
                                        creative.cancel_edits();
                                        creative.close_panels();
                                        dirty_blocks.clear();dirty_chunks.clear();
                                    }
                                    if session.failed() { mode = AppMode::Paused; }
                                    input.break_pressed = false; input.place_pressed = false; input.cast_pressed=false;input.cycle_spell=false;input.interact=false;input.dash_pressed=false;
                                }
                                let mut simulation_budget = None;
                                let mut edit_frame_started = None;
                                let gameplay_deadline=Instant::now()+std::time::Duration::from_millis(8);
                                frame_gameplay_trace.mark("gameplay-frame-setup");
                                for step_index in 0..if network_mode { 0 } else { simulation_steps } {
                                let mut step_trace = gameplay_trace::Trace::new();
                                if step_index>0 && Instant::now()>=gameplay_deadline {
                                    simulation_clock.defer_steps(simulation_steps-step_index);
                                    gameplay_dt=step_index as f32*simulation::STEP_SECONDS;
                                    presented_dt=simulation_clock.presented_seconds(lead_before,step_index);
                                    break;
                                }
                                if !fluid_sim.prepare_step() {
                                    simulation_clock.defer_steps(simulation_steps - step_index);
                                    gameplay_dt = step_index as f32 * simulation::STEP_SECONDS;
                                    presented_dt = simulation_clock.presented_seconds(lead_before, step_index);
                                    break;
                                }
                                let gameplay_dt = simulation::STEP_SECONDS;
                                outline.clear();
                                invert_outline.clear();
                                if creative.any_ui_open() || (!grabbed && !benchmark_camera_active && latency_probe.is_none()) { active_input.clear(); }
                                if sprint_benchmark && !spawning
                                    && benchmark.as_ref().is_some_and(|run| run.measuring)
                                {
                                    active_input.forward = true;
                                    active_input.sprint = true;
                                    active_input.jump = player.vel.xz().length_squared() < 1.0;
                                    if badlands_sprint {
                                        sprint_route.steer(&player, &mut active_input, gameplay_dt);
                                    }
                                }
                                if ca_window.is_some() {
                                    let target = simulation::tools::step(fluid_sim, world, &mut player, &mut active_input,
                                        creative.selected_slot);
                                    if let Some(region) = fluid_sim.cpu_region.as_ref() { creative.select_slot(region.tool_slot); }
                                    if let Some(at) = target {
                                        outline.extend(build_outline_vertices(at, [1.0, 0.68, 0.22, 0.85], 0.003));
                                    }
                                    egui_ui.target_block = target.map(|at| world.get(at));
                                    input.place_pressed = false;
                                    input.break_pressed = false;
                                    input.pick_block = false;
                                    input.cast_pressed = false;
                                    input.dash_pressed = false;
                                    input.cycle_spell = false;
                                    simulation::fixed::finish(fluid_sim, world, &player, &mut day_night,
                                        &mut dirty_blocks, &mut dirty_chunks, simulation::Budget::UNBOUNDED, false, !freeze_time);
                                    for sound in std::mem::take(&mut world.adventure.sounds) {
                                        egui_ui::subtitles::note(&sound, player.eye(world.voxel_scale) * world.voxel_scale.meters(), player.forward_dir());
                                        crate::soundscape::play_event(&mut combat_audio, &world, sound, player.eye(world.voxel_scale) * world.voxel_scale.meters(), player.forward_dir());
                                    }
                                    continue;
                                }
                                if benchmark_camera_active
                                    && let Some(benchmark_state) = benchmark.as_mut() {
                                        apply_benchmark_camera_motion(
                                            benchmark_state,
                                            &mut player,
                                            world.sample_config(),
                                        );
                                        if std::env::var_os("VOXEL_REVIEW_NAVIGATION").is_some() {
                                            for cell in egui_ui::nav_console::prepare_review(world, &player) {
                                                dirty_blocks.insert(cell);
                                                mark_chunk_and_neighbors_dirty(&mut dirty_chunks, cell);
                                            }
                                        }
                                        terrain_review::frame(&mut player, world, benchmark_state.motion_started_at.elapsed().as_secs_f32(), &mut dirty_blocks, &mut dirty_chunks);
                                        benchmark_state.scripted_stream_velocity = horizon_review::frame(&mut player, world, benchmark_state.horizon_route_started_at.map_or(0.0,|t|t.elapsed().as_secs_f32()), &mut dirty_blocks, &mut dirty_chunks);
                                    }
                                step_trace.mark("gameplay-step-camera");
                                if let Some(review) = freight_review.as_mut() {
                                    match review.frame(world, &player, &day_night, &mut dirty_blocks, &mut dirty_chunks, &mut saves) {
                                        Ok(false) => {}
                                        Ok(true) => { elwt.exit(); return; }
                                        Err(error) => {
                                            eprintln!("[freight-review] failed: {error:#}");
                                            elwt.exit();
                                            return;
                                        }
                                    }
                                }
                                if let Some(review) = jobs_review.as_mut() {
                                    match review.frame(world, &player, &day_night, &mut saves) {
                                        Ok(false) => {}
                                        Ok(true) => { elwt.exit(); return; }
                                        Err(error) => {
                                            eprintln!("[jobs-review] failed: {error:#}");
                                            elwt.exit();
                                            return;
                                        }
                                    }
                                }
                                let (mut place_edit, mut break_edit, mut can_act) = (false, false, false);
                                step_trace.mark("gameplay-step-reviews");
                                player.glide.wind = player::glide::weather_wind(&world, &day_night, player.pos);
                                let melee_consumed = simulation::fixed::gameplay(
                                    world, &mut player, &mut active_input, !benchmark_camera_active,
                                    |hook, world, player, active_input| {
                                        match hook {
                                            simulation::fixed::Hook::BeforePlayer => {
                                                // Keep review movement inside the measured, visible
                                                // phase instead of walking behind the loading screen.
                                                if benchmark.as_ref().is_none_or(|b| b.measuring) {
                                                    planet::review::input(world, player, active_input);
                                                } else if planet::review::walking() || planet::review::enabled() {
                                                    active_input.clear();
                                                }
                                                if !benchmark_camera_active {
                                        world::adventure::enemy_review_input(world,player,active_input);
                                        if benchmark.as_ref().is_some_and(|b|b.config.ossuary_review) {world::adventure::ossuary_review_input(world,player,active_input);}
                                        if let Some(review) = workshop_review.as_mut() {
                                            review.input(world, player, active_input);
                                        }
                                        if benchmark.as_ref().is_some_and(|b|b.config.tollkeeper_review) {
                                            world::adventure::tollkeeper_review_input(world,player,active_input);
                                        }
                                        if let Some(review)=sandbox_review.as_mut(){review.input(player,active_input,simulation::STEP_SECONDS);}
                                        if let Some(review) = water_review.as_mut() {
                                            review.input(player, active_input, world, simulation::STEP_SECONDS);
                                        }
                                                }
                                                simulation::fixed::Actions::default()
                                            }
                                            simulation::fixed::Hook::BeforeAdventure => {
                                                if !benchmark_camera_active {
                                        if let Some(review)=sandbox_review.as_mut(){review.observe(player);}
                                        if let Some(review) = water_review.as_mut()
                                            && let Some(shot) = review.observe(player, world.voxel_scale.meters()) {
                                                let root = std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from)
                                                    .unwrap_or_else(|| "target/water-review".into());
                                                gpu.request_capture(root.join(format!("{shot}.png")));
                                            }
                                                }
                                if player.on_ground
                                    && player.pos.y
                                        > world.sample_config().min_world_y() as f32 + 8.0
                                {
                                    last_safe_player_pos = player.pos;
                                }
                                rescue_from_void(world, player, last_safe_player_pos);
                                if let Some(review)=wand_review.as_mut() {
                                    review.input(world,player,&mut creative,active_input,&mut dirty_blocks,&mut dirty_chunks);
                                }
                                let edit_repeat_interval = creative.edit_repeat_interval_seconds();
                                if active_input.place_pressed { place_was_down = false; }
                                if active_input.break_pressed { break_was_down = false; }
                                place_edit = consume_tool_action(
                                    (active_input.place_block || active_input.place_pressed) && !player.traversal.sling,
                                    &mut place_was_down,
                                    &mut place_repeat_timer,
                                    edit_repeat_interval,
                                    gameplay_dt,
                                );
                                break_edit = consume_tool_action(
                                    active_input.break_block || active_input.break_pressed,
                                    &mut break_was_down,
                                    &mut break_repeat_timer,
                                    if creative.tool==creative::ToolKind::ExplosiveWand {f32::INFINITY}else{edit_repeat_interval},
                                    gameplay_dt,
                                );

                                let hart_review_cast = benchmark.as_ref().is_some_and(|b| b.config.hart_review.is_some()) && world::adventure::hart_review_cast(world, player);
                                let tollkeeper_review_cast=benchmark.as_ref().is_some_and(|b|b.config.tollkeeper_review) && world::adventure::tollkeeper_review_cast(world,player);
                                let ossuary_review_cast=benchmark.as_ref().is_some_and(|b|b.config.ossuary_review) && world::adventure::ossuary_review_cast(world);
                                let ossuary_review_auto=benchmark.as_ref().is_some_and(|b|b.config.ossuary_review) && world::adventure::ossuary_review_automatic();
                                let enemy_review_cast=world::adventure::enemy_review_casting(world);
                                can_act=(grabbed || workshop_review.is_some() || world::adventure::enemy_review_automated() || hart_review_cast || tollkeeper_review_cast || ossuary_review_auto || wand_review.as_ref().is_some_and(|r|r.automatic) || latency_probe.is_some()) && !creative.any_ui_open() && !spawning
                                    && world.rules.mode.interacts();
                                if !can_act {world.adventure.cancel_cast_input();}
                                combat_audio.set_active(matches!(mode,AppMode::Playing) && (focused || workshop_review.is_some() || wand_review.as_ref().is_some_and(|r|r.automatic)) && !spawning);
                                world.adventure.hand_slot = creative.selected_slot;
                                                simulation::fixed::Actions {
                                                    cast: (active_input.cast_spell || active_input.cast_pressed || enemy_review_cast || hart_review_cast || tollkeeper_review_cast || ossuary_review_cast) && can_act,
                                                    cycle: active_input.cycle_spell && can_act,
                                                    melee: break_edit && can_act && creative.tool == creative::ToolKind::Hand,
                                                }
                                            }
                                        }
                                    },
                                );
                                step_trace.mark("gameplay-and-input");
                                if melee_consumed { break_edit = false; }
                                if let Some(review) = industrial_review.as_mut() {
                                    if review.frame(world, &mut player, &mut day_night, &mut saves,
                                        &mut dirty_blocks, &mut dirty_chunks) {
                                        fluid_sim.reset_for_world(world.voxel_scale);
                                        chunk_streamer.reset();
                                        let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        chunk_cache.reset_with_snapshot(center, &world.snapshot_near(center, 4));
                                        chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                    }
                                    if let Some(path) = review.capture() { gpu.request_capture(path); }
                                }
                                if let Some(review) = workshop_review.as_mut() {
                                    review.after(world, &mut player, &mut creative, &mut day_night, &mut saves,
                                        &mut dirty_blocks, &mut dirty_chunks);
                                    if review.take_reloaded() {
                                        fluid_sim.reset_for_world(world.voxel_scale);
                                        chunk_streamer.reset();
                                        let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        chunk_cache.reset_with_snapshot(center, &world.snapshot_near(center, 4));
                                        chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                    }
                                    if let Some(path) = review.capture() { gpu.request_capture(path); }
                                }
                                input.cast_pressed=false;
                                if let Some(path) = planet::review::capture(world, &player) { gpu.request_capture(path); }
                                input.place_pressed=false;
                                input.break_pressed=false;
                                world::adventure::enemy_review_checkpoint(world,&mut player,&mut day_night,&mut saves);
                                input.cycle_spell=false;
                                if benchmark.as_ref().is_some_and(|b| b.config.hart_review.is_some()) {
                                    world::adventure::observe_hart_review(world);
                                }
                                if benchmark.as_ref().is_some_and(|b|b.config.ossuary_review) {
                                    world::adventure::observe_ossuary_review(world);
                                    world::adventure::ossuary_review_checkpoint(world,&mut player,&mut day_night,&mut saves);
                                }
                                if benchmark.as_ref().is_some_and(|b|b.config.tollkeeper_review) {world::adventure::observe_tollkeeper_review(world);}
                                if benchmark.as_ref().is_some_and(|b|b.config.tollkeeper_review) {
                                    world::adventure::tollkeeper_review_checkpoint(world,&mut player,&mut day_night,&mut saves);
                                }
                                let eye = player.eye(world.voxel_scale);
                                let forward = player.forward_dir();
                                let tool_hud = egui_ui.show_hud && !creative.any_ui_open()
                                    && (!quiet_benchmark_input || !benchmark_camera_active || benchmark_config.capture_hud);
                                let pick_block = input.pick_block && grabbed && !creative.any_ui_open();
                                let eye_precise = player.precise_eye(world.voxel_scale);
                                let ray_origin = eye_precise + forward.as_dvec3() * 0.0005;
                                // Opt-in inspection and middle-click only. Other actions retain
                                // their original path. Pending clicks are consumed, never queued.
                                let review_action = place_edit || break_edit || (input.pick_block && !pick_block)
                                    || active_input.interact || active_input.place_block || active_input.break_block
                                    || active_input.place_pressed || active_input.break_pressed || active_input.alt_down;
                                let review_pick = picking_review_gate::enabled(readonly_pick_review,
                                    world::ReadOnlyPicker::supports(world), tool_hud, pick_block, review_action);
                                let review_result = review_pick.then(|| readonly_picker.poll(world, ray_origin, forward,
                                    creative.reach_distance_blocks(world.voxel_scale)));
                                let (hit, review_material, pick_resolved) = if let Some(result) = &review_result {
                                    match result {
                                        world::Readiness::Ready(Some(h)) => (Some((h.cell,h.previous)),Some(h.block),true),
                                        world::Readiness::Ready(None) => (None,None,true),
                                        world::Readiness::Pending | world::Readiness::Failed(_) => (None,None,false),
                                    }
                                } else {
                                    let hit=(tool_hud || can_act || pick_block).then(|| world::raycast_block_precise(
                                        world,ray_origin,forward,creative.reach_distance_blocks(world.voxel_scale))).flatten();
                                    (hit,None,true)
                                };
                                step_trace.mark("tool-ray");
                                if input.pick_block {
                                    let target = if let Some(result) = &review_result {
                                        use picking_review_gate::ClickDecision;
                                        match picking_review_gate::consume_click(&mut input.pick_block, result) {
                                            ClickDecision::Pick(block) => Some(block),
                                            ClickDecision::Pending => { world.expedition.say("Terrain Loading"); None },
                                            ClickDecision::Failed => { world.expedition.say("Terrain Unavailable"); None },
                                            ClickDecision::Idle | ClickDecision::Miss => None,
                                        }
                                    } else {
                                        hit.filter(|_| grabbed && !creative.any_ui_open()).map(|(cell, _)| world.get(cell))
                                    };
                                    if grabbed && !creative.any_ui_open()
                                        && let Some(target) = target {
                                            let survival_hand = creative.tool == creative::ToolKind::Hand
                                                && !creative.show_panel;
                                            if survival_hand {
                                                if let Some(canon) = world::inventory::Item::canonical_block(target) {
                                                    let item = world::inventory::Item::Block(canon);
                                                    if let Some(slot) = world.inventory.pick_into_hotbar(item, creative.selected_slot) {
                                                        creative.select_slot(slot);
                                                        player.selected_block = canon;
                                                    }
                                                }
                                            } else {
                                                player.selected_block = creative.pick_block(target);
                                            }
                                        }
                                    input.pick_block = false;
                                }
                                let inspected = hit.filter(|_| tool_hud).map(|(cell, _)| (cell, review_material.unwrap_or_else(|| world.get(cell))));
                                egui_ui.target_block = inspected.map(|(_, block)| block);
                                egui_ui.target_environment = inspected.and_then(|(cell, _)|
                                    world::environment::inspect::at(world, cell));
                                egui_ui.environment_lag = world.environment.lag_seconds();
                                egui_ui.target_machine =
                                    inspected.and_then(|(cell, _)| world::factory::probe(world, cell));
                                egui_ui.target_cell_temp_c = inspected.and_then(|(cell, block)| {
                                    let t = world.sandbox.cell_for(cell, block).temperature;
                                    t.is_finite().then_some(t)
                                });
                                egui_ui.target_ambient_c = inspected.map(|(cell, block)| world.sandbox.climate.ambient_for(cell, block));
                                // Jade's harvest line: survival only, solid cells with a hardness.
                                egui_ui.target_harvest = inspected
                                    .filter(|_| world.rules.mode == world::rules::GameMode::Survival)
                                    .filter(|(_, block)| {
                                        let physics = &block.material().physics;
                                        physics.matter_state == material::MatterState::Solid && physics.hardness > 0.0
                                    })
                                    .map(|(_, block)| {
                                        let held = world.inventory.hotbar[creative.selected_slot].map(|s| s.item);
                                        let broken = world::mining::survival_break(block, world::mining::tool_for_world(world, held));
                                        egui_ui::look_at::Harvest {
                                            tool: world::recipe_index::harvest_tool(block),
                                            tier: broken.required_tier,
                                            seconds: broken.seconds,
                                            drops: broken.drops,
                                        }
                                    });
                                egui_ui.target_volume_ul = inspected.map(|(cell, block)| {
                                    if block == BlockId::Water {
                                        world.water_microlitres(cell)
                                    } else {
                                        engine_core::environment::cell_volume(world.voxel_scale)
                                    }
                                });
                                // A bounded second probe distinguishes empty sky from an unreachable face.
                                let sight = if tool_hud && pick_resolved {
                                    hit.or_else(|| world::raycast_loaded_block_precise(
                                        world, ray_origin, forward, 24.0 / world.voxel_scale.meters(),
                                        |p| world.resident_block(p).or_else(|| {
                                            chunk_cache.observation_source(chunk_coord_from_world_block(p))
                                                .map(|source| source.get_world(p))
                                        }),
                                    ))
                                } else {
                                    None
                                };
                                egui_ui.tool_feedback.distance_m = sight.map(|(cell, previous)| {
                                    let normal = previous - cell;
                                    (0..3).find(|&axis| normal[axis] != 0 && forward[axis].abs() > 1e-6)
                                        .map(|axis| {
                                            let face = cell[axis] as f64 + if normal[axis] > 0 { 1.0 } else { 0.0 };
                                            (((face - eye_precise[axis]) / forward[axis] as f64).max(0.0)
                                                * world.voxel_scale.meters() as f64) as f32
                                        }).unwrap_or(0.0)
                                });
                                egui_ui.tool_feedback.out_of_range = hit.is_none() && sight.is_some();
                                egui_ui.tool_feedback.air_placement = pick_resolved && hit.is_none() && active_input.alt_down
                                    && creative.tool == creative::ToolKind::Hand;
                                step_trace.mark("tool-inspection");
                                // Block use takes priority over either hand, except while sneaking.
                                // A held torch lights TNT.
                                if place_edit && can_act && !active_input.sneak && creative.tool == creative::ToolKind::Hand
                                    && let Some((cell, _)) = hit
                                    && (world.inventory.hotbar[creative.selected_slot].map(|s| s.item)
                                        == Some(world::inventory::Item::Block(BlockId::Torch))
                                        || creative.selected_block() == BlockId::Torch)
                                    && world::block_data::tnt::ignite(world, cell) {
                                    place_edit = false;
                                }
                                let use_target = place_edit && can_act && !active_input.sneak
                                    && creative.tool == creative::ToolKind::Hand
                                    && world::interaction::secondary_targets_world(world, &player);
                                let interact_pressed = active_input.interact || use_target;
                                if interact_pressed {
                                    input.interact = false;
                                    active_input.interact = false;
                                    if can_act && world::interaction::gameplay_enabled(focused, grabbed, creative.any_ui_open(), egui_ui.ctx.egui_wants_keyboard_input()) {
                                        use world::interaction::Effect;
                                        let effect = world::interaction::activate(world, &player, creative.selected_slot,
                                            active_input.sneak, &mut dirty_blocks, &mut dirty_chunks);
                                        if effect != Effect::None || !use_target { break_edit = false; place_edit = false; }
                                        match effect {
                                            Effect::Inventory(page) => {
                                                creative.close_panels(); creative.show_palette = true;
                                                egui_ui.ctx.data_mut(|d| d.insert_temp(egui::Id::new("inventory_page"), page));
                                            }
                                            Effect::ThresholdGuide => {
                                                creative.close_panels();
                                                creative.show_palette = true;
                                                egui_ui::open_magic_guide(&egui_ui.ctx);
                                            }
                                            Effect::Trade(id) => {
                                                creative.close_panels(); creative.trade_page = Some(id);
                                            }
                                            Effect::Block(cell) => {
                                                let target = world.get(cell);
                                                if world::block_data::opens_page(target) {
                                                    creative.close_panels(); creative.block_page = Some(cell);
                                                } else if target == BlockId::Bed {
                                                    let feet = (cell.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * world.voxel_scale.meters();
                                                    let local = planet::local_time(world.world_type, feet.as_dvec3(), &day_night);
                                                    let slept = world::bed::sleep(&mut day_night, &mut world.adventure.respawn_point, feet, local);
                                                    if slept.is_ok() { world.goals.placed(world::BlockId::Bed); world.goals.note(world::goals::Event::Slept); }
                                                    world.expedition.say(match slept {
                                                        Ok(true) => "Slept through the night. Morning came the next day.",
                                                        Ok(false) => "Slept until dawn.",
                                                        Err(_) => "Beds take sleepers after sunset.",
                                                    });
                                                } else {
                                                    world::expedition::interact(world, cell, player.pos, &mut dirty_blocks, &mut dirty_chunks);
                                                    let page = if world::factory::Kind::from_block(target) == Some(world::factory::Kind::Storage)
                                                        || target == BlockId::Workbench || world::containers::is_cache(target) { Some(2u32) }
                                                        else if world::factory::Kind::from_block(target).is_some() { Some(3u32) } else { None };
                                                    if let Some(page) = page {
                                                        creative.close_panels(); creative.show_palette = true;
                                                        egui_ui.ctx.data_mut(|d| d.insert_temp(egui::Id::new("inventory_page"), page));
                                                    }
                                                }
                                            }
                                            Effect::None | Effect::Taken => {}
                                        }
                                        if creative.any_ui_open() { input.clear(); active_input.clear(); release_mouse(window, &mut grabbed); }
                                    }
                                }
                                if place_edit && can_act
                                    && matches!(world.inventory.hotbar[creative.selected_slot].map(|s|s.item), Some(world::inventory::Item::Bucket(_))) {
                                    if let Some((hit, before)) = world::water_tools::target(world, &player) {
                                        let cell = world::water_tools::use_cell(world, hit, before, active_input.sneak);
                                        if fluid_sim.prepare_tool_edit(world, cell) {
                                            world::water_tools::use_bucket(world, &player, creative.selected_slot,
                                                active_input.sneak, &mut dirty_blocks, &mut dirty_chunks);
                                        }
                                    } else {
                                        // No water in reach: lava scoop or pour.
                                        world::water_tools::use_bucket(world, &player, creative.selected_slot,
                                            active_input.sneak, &mut dirty_blocks, &mut dirty_chunks);
                                    }
                                    place_edit = false;
                                }
                                // Warm the aimed-at source before a click or survival swing
                                // completes. This uses the single bounded edit-source lane.
                                if can_act && creative.tool == creative::ToolKind::Hand
                                    && let Some((cell, _)) = hit {
                                    fluid_sim.prefetch_tool_edit(world, cell);
                                }
                                // Bore upgrades share the asynchronous, frame-budgeted edit path.
                                if break_edit && can_act && active_input.alt_down && creative.tool == creative::ToolKind::Hand {
                                    if creative.pending_cells()==0
                                        && let Some((cell,_))=hit
                                            && (cell.as_vec3()-eye).length()*world.voxel_scale.meters()<=6.0 {
                                                let targets=world::expedition::bore_targets(world,cell,player.pos,simulation::Budget::wall(std::time::Duration::from_millis(2)));
                                                creative.queue_edit(world,creative::BrushMode::Erase,BlockId::Air,targets);
                                            }
                                    break_edit=false;
                                }
                                let raycast_target = if !pick_resolved { creative::RaycastTarget {hit:None,place:None} } else { hit.map_or_else(
                                    || creative::RaycastTarget {
                                        hit: None,
                                        place: Some(
                                            (eye + forward
                                                * creative
                                                    .reach_distance_blocks(world.voxel_scale))
                                            .floor()
                                            .as_ivec3(),
                                        ),
                                    },
                                    |(hit_cell, place_cell)| creative::RaycastTarget {
                                        hit: Some(hit_cell),
                                        place: Some(place_cell),
                                    },
                                ) };

                                let mut preview = if pick_resolved && (tool_hud || can_act) { creative::get_preview(
                                    &creative,
                                    world,
                                    raycast_target,
                                    active_input.place_block,
                                    active_input.break_block,
                                    active_input.alt_down,
                                    false,
                                ) } else { creative::PreviewResult::default() };
                                step_trace.mark("tool-preview");
                                egui_ui.tool_feedback.material_budget = None;
                                if creative.tool == creative::ToolKind::Hand {
                                    let available = world.inventory.hotbar[creative.selected_slot]
                                        .filter(|s| s.item.placeable_block().is_some())
                                        .map_or(0, |s| world.inventory.count_item(s.item));
                                    if active_input.alt_down || active_input.place_block {
                                        if let Some((_, targets)) = creative::edit_blocks(&creative, world, &player, raycast_target, true, false, active_input.alt_down) {
                                            preview.total_cells = targets.len();
                                        }
                                        egui_ui.tool_feedback.material_budget = Some(available);
                                        if (available as usize) < preview.total_cells { preview.color = [1.0, 0.68, 0.22, 0.9]; }
                                    }
                                }
                                egui_ui.tool_feedback.total_cells = preview.total_cells;
                                egui_ui.tool_feedback.visible_cells = preview.total_cells;

                                // The explosive wand uses the normal break press, but edits
                                // a bounded spherical blast. Dirty cells wake the local
                                // sediment and water simulation on this same frame.
                                if creative.tool == creative::ToolKind::ExplosiveWand
                                    && !creative.locked()
                                    && world.rules.mode.creative_tools()
                                    && break_edit
                                    && can_act
                                    && creative.blast_ready()
                                    && let Some(center) = raycast_target.hit {
                                        pending_tool_action = Some(egui_ui::ToolUseAction::Break);
                                        let center_blocks = center.as_vec3() + glam::Vec3::splat(0.5);
                                        let scale = world.voxel_scale.meters();
                                        let radius_m = creative.radius_cells(creative.explosion_radius,world.voxel_scale) as f32 * scale;
                                        let normal=raycast_target.place.map_or(-forward,|p|(p-center).as_vec3());
                                        let color=world.get(center).color(center);
                                        world.adventure.blast_feedback(center_blocks * scale, radius_m,normal,color);
                                        world.sandbox.apply_radial_impulse(center_blocks * scale, radius_m * 1.6, 9.0, scale);
                                        world::factory::industry::rupture(world, center_blocks * scale, radius_m);
                                        let away = player.pos + glam::Vec3::Y * (0.8/scale) - center_blocks;
                                        let distance = away.length() * scale;
                                        if distance < radius_m {
                                            let falloff = 1.0 - distance / radius_m;
                                            let direction = (away.normalize_or_zero()
                                                + glam::Vec3::Y * 0.55)
                                                .normalize_or_zero();
                                            player.vel += direction
                                                * world::speed_blocks(
                                                    world.voxel_scale,
                                                    18.0 * falloff,
                                                );
                                        }
                                        let targets=creative::blast_targets(&creative,world,raycast_target);
                                        creative.queue_blast(world,targets,Some(preview.clone()));
                                    }

                                let lines = if tool_hud {
                                    preview.segments.iter().flat_map(|segment| segment.iter().map(|&p| {
                                        let p = p + (eye-p).normalize_or_zero() * (0.003/world.voxel_scale.meters());
                                        mesh::LineVertex {pos:p.to_array(),color:preview.color}
                                    })).collect()
                                } else {
                                    Vec::new()
                                };
                                if preview.invert_blend {
                                    invert_outline = lines;
                                } else {
                                    outline = lines;
                                }
                                if tool_hud
                                    && egui_ui.tool_feedback.out_of_range
                                    && !egui_ui.tool_feedback.air_placement
                                    && let Some((cell, _)) = sight {
                                        outline.extend(build_outline_vertices(cell, [1.0, 0.68, 0.22, 0.85], 0.003));
                                    }

                                if let Some(center) = preview.center_block.filter(|_| tool_hud) {
                                    let center_lines =
                                        build_outline_vertices(center, preview.color, 0.003/world.voxel_scale.meters());
                                    outline.extend(center_lines);
                                }
                                if tool_hud {
                                    blueprints.overlay(creative.tool, eye, 0.003/world.voxel_scale.meters(), &mut outline);
                                    let scale = world.voxel_scale.meters();
                                    for (cell, color) in world::farming::jobs::markers(world) {
                                        if cell.as_vec3().distance_squared(eye) * scale * scale < 32.0 * 32.0 {
                                            outline.extend(build_outline_vertices(cell, color, 0.008 / scale));
                                        }
                                    }
                                }
                                // Pastes share the tool edit path, so they wait for residency like a brush stroke.
                                // Pastes and undo are player edits: their chunks take the remesh fast lane.
                                let mut edit_chunks = HashSet::new();
                                let mut sink = world::blueprint::Sink {
                                    prepare: &mut |w: &mut World, p| fluid_sim.prepare_tool_edit(w, p),
                                    blocks: &mut dirty_blocks, chunks: &mut edit_chunks,
                                };
                                blueprints.frame(world, &mut creative, raycast_target, network_mode, &mut edit_history, &mut sink,
                                    simulation::Budget::wall(std::time::Duration::from_millis(3)));
                                if let Some(message) = edit_history.step(world, &mut sink, simulation::Budget::wall(std::time::Duration::from_millis(3)), usize::MAX) {
                                    world.expedition.say(message);
                                }
                                if !edit_chunks.is_empty() {
                                    dirty_chunks.extend(edit_chunks.iter().copied());
                                    chunk_streamer.note_player_edits(edit_chunks, world.revision());
                                }

                                if place_edit && can_act && creative.tool == creative::ToolKind::Hand
                                    && world.inventory.hotbar.get(creative.selected_slot).copied().flatten().map(|s| s.item) == Some(world::stats::MAP) {
                                    egui_ui::map::toggle_panel(&mut creative, Some(world.sample_config()));
                                    input.clear();
                                    place_edit = false;
                                    break_edit = false;
                                    release_mouse(window, &mut grabbed);
                                }
                                if place_edit && can_act && creative.tool == creative::ToolKind::Hand
                                    && let Some((cell, _)) = hit
                                    && active_input.sneak
                                    && world.inventory.hotbar.get(creative.selected_slot).is_some_and(Option::is_none)
                                    && world::farming::jobs::designate(world, cell)
                                {
                                    place_edit = false;
                                }
                                if place_edit && can_act && creative.tool == creative::ToolKind::Hand
                                    && let Some((cell, _)) = hit
                                    && world::farming::planet_freight::collect(world, cell)
                                {
                                    place_edit = false;
                                }
                                if place_edit && can_act && creative.tool == creative::ToolKind::Hand {
                                    match world::travel::interact(world, eye * world.voxel_scale.meters(), forward, creative.selected_slot) {
                                        world::travel::Use::None => {}
                                        world::travel::Use::Taken => place_edit = false,
                                        world::travel::Use::OpenTrade(id) => {
                                            creative.close_panels();
                                            creative.trade_page = Some(id);
                                            input.clear();
                                            place_edit = false;
                                            break_edit = false;
                                            release_mouse(window, &mut grabbed);
                                        }
                                    }
                                }
                                if place_edit && can_act && !active_input.sneak
                                    && creative.tool == creative::ToolKind::Hand
                                    && world::farming::interact(world,hit,eye*world.voxel_scale.meters(),forward,creative.selected_slot,&mut dirty_blocks,&mut dirty_chunks) {
                                    place_edit=false;
                                }
                                let survival_hand = creative.tool == creative::ToolKind::Hand
                                    && !creative.show_panel;
                                if survival_hand {
                                    match world.inventory.hotbar[creative.selected_slot].map(|s| s.item) {
                                        Some(world::inventory::Item::PowerKit(k)) => player.selected_block = k.kind.block(),
                                        Some(world::inventory::Item::Block(b)) => {
                                            player.selected_block = b;
                                        }
                                        Some(world::inventory::Item::Weapon(tier)) => {
                                            let _ = world.adventure.select_weapon(tier as u32);
                                        }
                                        _ => {}
                                    }
                                    if place_edit && can_act {
                                        match world.inventory.hotbar[creative.selected_slot].map(|s| s.item) {
                                            Some(world::inventory::Item::Food(food)) => {
                                                let _ = world.eat_food(food, Some(creative.selected_slot));
                                                place_edit = false;
                                            }
                                            Some(world::inventory::Item::Block(_) | world::inventory::Item::PowerKit(_)) => {}
                                            _ => {
                                                let ready = hit.is_none_or(|(_, p)| fluid_sim.prepare_tool_edit(world, p));
                                                if ready && use_offhand(world, &player, creative.selected_slot, hit, &mut dirty_blocks, &mut dirty_chunks) {
                                                    pending_tool_action = Some(egui_ui::ToolUseAction::Place);
                                                    if let Some((_, p)) = hit {
                                                        let mut changed = HashSet::new();
                                                        mark_chunk_and_neighbors_dirty(&mut changed, p);
                                                        chunk_streamer.note_player_edits(changed, world.revision());
                                                    }
                                                }
                                                place_edit = false;
                                            }
                                        }
                                    }
                                }
                                // Adventure keeps use and combat but never edits blocks.
                                let edits_blocks = world.rules.mode.edits_blocks();
                                if !edits_blocks { place_edit = false; }
                                // Creative worlds keep the instant break; Survival mines over time.
                                let mining_tool = (survival_hand && creative.locked() && edits_blocks).then(|| world::mining::tool_for_world(
                                    world, world.inventory.hotbar[creative.selected_slot].map(|s| s.item),
                                ));
                                egui_ui.tool_feedback.break_progress = None;
                                egui_ui.tool_feedback.break_requires = None;
                                if let Some(tool) = mining_tool.filter(|_| !active_input.alt_down) {
                                    let target = hit.map(|(cell, _)| (cell, review_material.unwrap_or_else(|| world.get(cell))));
                                    let held = (active_input.break_block || active_input.break_pressed)
                                        && can_act && !melee_consumed;
                                    let heavy = tool.heavy && target.is_some_and(|(p, _)| world::mining::crater::can_crack(world, p, tool));
                                    if heavy && can_act && !melee_consumed
                                        && let Some((cell, material)) = target
                                    {
                                        if let Some(fraction) = heavy_charge.track((cell, material), held, gameplay_dt) {
                                            let mut impact_chunks = HashSet::new();
                                            let result = world::mining::crater::crack(world, cell, world::mining::charge::radius_m(fraction), tool, &mut dirty_blocks, &mut impact_chunks);
                                            if result.mined > 0 {
                                                pending_tool_action = Some(egui_ui::ToolUseAction::Break);
                                                soundscape.block(&mut combat_audio, audio::Contact::Break, material, cell, world, eye * world.voxel_scale.meters(), forward);
                                                dirty_chunks.extend(impact_chunks.iter().copied());
                                                chunk_streamer.note_player_edits(impact_chunks, world.revision());
                                            }
                                        }
                                        egui_ui.tool_feedback.break_progress = heavy_charge.display();
                                        if let Some(progress) = heavy_charge.display() {
                                            mesh::mining::append_block(world, cell, progress, 0.0045 / world.voxel_scale.meters(), &mut outline);
                                        }
                                    } else {
                                        heavy_charge = Default::default();
                                    }
                                    let mined = target.map(|(_, block)| world::mining::survival_break(block, tool));
                                    break_edit = mining_progress.tick(target, held && !heavy, mined.map_or(0.0, |m| m.seconds), gameplay_dt);
                                    // Breaking stone carries: listening hunters turn toward it.
                                    if break_edit { world.adventure.make_noise(0.6); }
                                    if !heavy { egui_ui.tool_feedback.break_progress = mining_progress.fraction(); }
                                    player.viewmodel.mine_timing = mining_progress.timing();
                                    egui_ui.tool_feedback.break_requires = mined.filter(|m| !m.drops).map(|m| m.required_tier);
                                    if let (Some(progress), Some((cell, _))) = (mining_progress.fraction(), target) {
                                        for cell in preview.blocks.iter().copied().chain(std::iter::once(cell)).collect::<HashSet<_>>() {
                                            mesh::mining::append_block(world, cell, progress, 0.0045 / world.voxel_scale.meters(), &mut outline);
                                        }
                                    }
                                    let digging = held && mining_progress.fraction().is_some();
                                    soundscape.dig(&mut combat_audio, target.filter(|_| digging), mining_progress.timing(), world, eye * world.voxel_scale.meters(), forward);
                                } else {
                                    heavy_charge = Default::default();
                                    mining_progress = world::mining::Progress::default();
                                    player.viewmodel.mine_timing = None;
                                }
                                if egui_ui.show_hud {
                                    mesh::mining::append_fractures(world, eye, &mut outline);
                                }
                                if !edits_blocks { break_edit = false; }

                                if let Some((edit_mode, targets)) = creative::edit_blocks(
                                    &creative,
                                    world,
                                    &player,
                                    raycast_target,
                                    place_edit,
                                    break_edit,
                                    active_input.alt_down,
                                ) {

                                    let active_block = if survival_hand {
                                        match world.inventory.hotbar[creative.selected_slot].map(|s| s.item) {
                                            Some(world::inventory::Item::PowerKit(k)) => k.kind.block(),
                                            Some(world::inventory::Item::Block(b)) => b,
                                            _ => BlockId::Air,
                                        }
                                    } else {
                                        creative.selected_block()
                                    };
                                    let placement_block = if active_block == BlockId::Air {
                                        None
                                    } else {
                                        hit.and_then(|(cell, prev)| {
                                            resolve_placement_block(active_block, prev - cell,
                                                player.look_axis(), player.up_dir())
                                        })
                                        .or(Some(active_block))
                                    };
                                    if edit_mode == creative::BrushMode::Erase { creative.queue_edit(world,edit_mode,BlockId::Air,targets); } else if let Some(block)=placement_block {creative.queue_edit(world,edit_mode,block,targets);}
                                }
                                step_trace.mark("tool-targeting-preview");
                                // A committed stroke is applied over bounded frames; preview
                                // sampling never truncates edits or turns a sphere into a slice.
                                while let Some(cell) = world.adventure.dirty_voxels.pop() {
                                    dirty_blocks.insert(cell);
                                    mark_chunk_and_neighbors_dirty(&mut dirty_chunks, cell);
                                }
                                if can_act {
                                    if creative.pending_cells()==0
                                        && let Some((point,radius))=world.adventure.take_terrain_blast() {
                                            world::factory::industry::rupture(world,point,radius);
                                            let cells=creative::spell_blast_targets(world.voxel_scale.meters(),point,radius);
                                            creative.queue_spell_blast(world,cells,point,radius);
                                        }
                                    let edit_started=*edit_frame_started.get_or_insert_with(Instant::now);
                                    let explosive_edit=creative.pending_explosive();
                                    let spell_blast=creative.pending_spell_blast();
                                    let mut emitted_tool_burst = false;
                                    let mut player_edit_chunks = HashSet::new();
                                    let mut unloadable_cells = 0usize;
                                    let cfg=world.sample_config();
                                    let (mut body_min,mut body_max)=player.bounds(world.voxel_scale);
                                    let mut lifted=false;
                                    for _ in 0..if explosive_edit {8192}else{1024} {
                                        if edit_started.elapsed()>=std::time::Duration::from_millis(if explosive_edit {4}else{2}) {break;}
                                        let mut cell_trace=gameplay_trace::Trace::new();
                                        let Some(next)=creative.next_edit_position(cfg) else {break;};
                                        // One unreadable cell must not wedge the stroke:
                                        // the queue front used to retry forever and every
                                        // later click was refused behind it.
                                        if !fluid_sim.prepare_tool_edit(world,next) {
                                            if fluid_sim.tool_edit_waiting() { break; }
                                            creative.next_edit(cfg);
                                            unloadable_cells += 1;
                                            if std::env::var_os("VOXEL_TOOL_REVIEW").is_some() {
                                                eprintln!("[tool-review] skipped unloadable cell {next:?} status={:?}", fluid_sim.status);
                                            }
                                            continue;
                                        }
                                        if creative.pending_mode()==Some(creative::BrushMode::Erase) && world.get(next)==BlockId::Air {
                                            creative.next_edit(cfg);continue;
                                        }
                                        cell_trace.mark("tool-residency");
                                        let Some((edit_mode,cell,queued_block))=creative.next_edit(cfg) else {break;};
                                        if edit_mode==creative::BrushMode::Place && creative::placement_traps_body(queued_block,cell,body_min,body_max){
                                            let old=world.get(cell);
                                            let Some(lift)=creative::pillar_lift(world,&player,cell).filter(|_| !lifted&&(old.is_empty()||old.is_liquid())) else {continue;};
                                            creative::apply_pillar_lift(&mut player,lift,world.voxel_scale.meters());
                                            lifted=true;
                                            (body_min,body_max)=player.bounds(world.voxel_scale);
                                        }
                                        let placement_block=Some(queued_block);
                                        let previous_block = world.get(cell);
                                        match edit_mode {
                                            creative::BrushMode::Erase => {
                                                if world.get(cell) == BlockId::Air {
                                                    continue;
                                                }
                                                let changed = if spell_blast {creative.spell_blast_cell(world,cell,&mut dirty_blocks,&mut player_edit_chunks)}
                                                else if explosive_edit {world::expedition::blast_harvest(world,cell,&mut dirty_blocks,&mut player_edit_chunks)}
                                                else {world::mining::harvest(world,cell,mining_tool,&mut dirty_blocks,&mut player_edit_chunks,
                                                    (!creative.locked()).then_some(&mut edit_history), forward)};
                                                // A refused/deferred break has no mesh changes. Completed
                                                // survival progress retries without rebuilding intact terrain.
                                                if !changed { continue; }
                                                // Recorded after the break, so undo puts torches back before their support.
                                                if !creative.locked() && explosive_edit && !spell_blast { edit_history.record(world, cell, previous_block, world.get(cell)); }
                                                cleanup_detached_torches(
                                                    world,
                                                    cell,
                                                    &mut dirty_blocks,
                                                    &mut player_edit_chunks,
                                                    (!creative.locked() && !spell_blast).then_some(&mut edit_history),
                                                );
                                            }
                                            creative::BrushMode::Place => {
                                                let Some(place_block) = placement_block else {
                                                    continue;
                                                };
                                                let held = if survival_hand { world.inventory.hotbar[creative.selected_slot].map(|s|s.item) } else { None };
                                                if world::fittings::place(world,cell,place_block,&player,survival_hand.then_some(creative.selected_slot),&mut dirty_blocks,&mut player_edit_chunks) { continue; }
                                                if world::factory::place(world,cell,place_block,&player,held,&mut dirty_blocks,&mut player_edit_chunks) {
                                                    continue;
                                                }
                                                if (place_block.is_torch() || engine_content::thin::is_climbable(place_block))
                                                    && !is_torch_supported(world, cell, place_block)
                                                {
                                                    continue;
                                                }
                                                if !world.get(cell).is_empty() && !world.get(cell).is_liquid() {
                                                    continue;
                                                }
                                                if world::effects::place(world, cell, previous_block, place_block,
                                                    survival_hand.then_some(creative.selected_slot),
                                                    &mut dirty_blocks, &mut player_edit_chunks).is_err() {
                                                    world.expedition.say("Placement blocked");
                                                    continue;
                                                }
                                                if engine_content::shaped::text_limits(place_block).is_some() {
                                                    creative.block_page = Some(cell);
                                                }
                                            }
                                        }
                                        cell_trace.mark("tool-material-commit");
                                        if world.get(cell) != previous_block {
                                            if edit_mode == creative::BrushMode::Place && !creative.locked() { edit_history.record(world, cell, previous_block, world.get(cell)); }
                                            mark_chunk_and_neighbors_dirty(&mut player_edit_chunks,cell);
                                            if let Some(probe)=latency_probe.as_mut() {probe.after_edit(world,cell,previous_block);}
                                        }
                                        if !emitted_tool_burst && world.get(cell) != previous_block {
                                            pending_tool_action = Some(match edit_mode { creative::BrushMode::Place => egui_ui::ToolUseAction::Place, creative::BrushMode::Erase => egui_ui::ToolUseAction::Break });
                                            let scale = world.voxel_scale.meters();
                                            let center = (cell.as_vec3() + Vec3::splat(0.5)) * scale;
                                            let placing = edit_mode == creative::BrushMode::Place;
                                            let color = if placing { queued_block.color(cell) } else { previous_block.color(cell) };
                                            if world.adventure.tool_debris.len() < 24 {
                                                world.adventure.tool_debris.push(tool_effects::Burst {
                                                    pos: center + (eye * scale - center).normalize_or_zero() * scale * 0.55,
                                                    color, age: 0.0, placing, scale,
                                                });
                                            }
                                            let contact = if placing { audio::Contact::Place } else { audio::Contact::Break };
                                            soundscape.block(&mut combat_audio, contact, if placing { queued_block } else { previous_block }, cell, world, eye * scale, forward);
                                            if std::env::var_os("VOXEL_TOOL_REVIEW").is_some() { eprintln!("[tool-review] committed tool={:?} mode={:?} cell={:?} before={:?} after={:?}",creative.tool,edit_mode,cell,previous_block,world.get(cell)); }
                                            emitted_tool_burst = true;
                                            cell_trace.mark("tool-audio-feedback");
                                        }
                                                                        }
                                    // A stroke is queued whole, so an empty queue closes one undo step.
                                    if creative.pending_cells() == 0 { edit_history.seal(); }
                                    if explosive_edit && !spell_blast {world.sync_from_inventory();}
                                    if !player_edit_chunks.is_empty() {
                                        dirty_chunks.extend(player_edit_chunks.iter().copied());
                                        chunk_streamer.note_player_edits(player_edit_chunks,world.revision());
                                    }
                                    if unloadable_cells > 0 && world.expedition.message_seconds <= 0.0 {
                                        world.expedition.say("Edit skipped: chunk unavailable");
                                    }
                                }
                                step_trace.mark("gameplay-and-tools");
                                if let Some(review)=sandbox_review.as_ref(){fluid_sim.set_enabled(review.running());}
                                simulation::fixed::finish(
                                    fluid_sim, world, &player, &mut day_night,
                                    &mut dirty_blocks, &mut dirty_chunks,
                                    // One wall budget for the whole frame. A fresh one per step let
                                    // up to 16 catch-up steps each spend it, so a busy water front
                                    // lengthened the frame, which granted more steps next frame.
                                    *simulation_budget.get_or_insert_with(|| simulation::WorldSimulation::live_budget(world)),
                                    false, !freeze_time,
                                );
                                planet::influence::sync_hell_anchor(world, &mut player);
                                step_trace.mark("finish");
                                if let Some(probe)=latency_probe.as_mut() {probe.after_sim(world);}
                                if let Some(review)=wand_review.as_mut()
                                    && let Some(shot)=review.observe(world,&player,&creative,&preview) {
                                        let root=std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(||"target/wand-review".into());
                                        gpu.request_capture(root.join(shot));
                                    }
                                // Drain after all producers, including tools. A tool blast used
                                // to enqueue after the drain and lose its sound on the next tick.
                                for sound in std::mem::take(&mut world.adventure.sounds) {
                                    egui_ui::subtitles::note(&sound,player.eye(world.voxel_scale)*world.voxel_scale.meters(),player.forward_dir());
                                    crate::soundscape::play_event(&mut combat_audio, &world, sound, player.eye(world.voxel_scale)*world.voxel_scale.meters(), player.forward_dir());
                                }
                                step_trace.mark("gameplay-sound-events");
                                soundscape.tick(&mut combat_audio, world, &player, &day_night, gameplay_dt);
                                step_trace.mark("gameplay-soundscape");
                                {
                                    let (speed, load) = world::orbital::express::drive_cue(&world.orbital);
                                    combat_audio.set_drive(speed, load);
                                }
                                #[cfg(feature="multiplayer")]
                                if let Some(session)=multiplayer.as_mut() {session.play_contacts(&mut combat_audio,world,&player);}
                                if let Some(review)=sandbox_review.as_mut(){
                                    let (reloaded,shot)=review.tick(world,&mut player,&day_night,fluid_sim,&mut dirty_blocks,&mut dirty_chunks);
                                    if reloaded {chunk_streamer.reset();let c=chunk_coord_from_world_block(player.pos.floor().as_ivec3());chunk_cache.reset_with_snapshot(c,&world.snapshot_near(c,4));chunk_streamer.queue_needed_visible(&chunk_cache,c);}
                                    if let Some(shot)=shot {let root=std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(||"target/sandbox-review".into());gpu.request_capture(root.join(format!("{shot}.png")));}
                                }
                                step_trace.mark("gameplay-step-tail");
                                }
                                frame_gameplay_trace.mark("gameplay-fixed-steps");
                                // Receiver commands and captures also run during
                                // render-only reviews with zero simulation steps.
                                if let Some(review)=receiver_review.as_mut() {
                                    let (reloaded,shot)=review.frame(world,&mut player,&mut day_night,&mut saves,&mut creative,&mut dirty_blocks,&mut dirty_chunks);
                                    if reloaded {chunk_streamer.reset();let c=chunk_coord_from_world_block(player.pos.floor().as_ivec3());chunk_cache.reset_with_snapshot(c,&world.snapshot_near(c,4));chunk_streamer.queue_needed_visible(&chunk_cache,c);}
                                    if let Some(shot)=shot&& let Some(root)=std::env::var_os("VOXEL_CAPTURE_DIR") {gpu.request_capture(std::path::PathBuf::from(root).join(shot));}
                                }
                                let eye = player.eye(world.voxel_scale);
                                let forward = player.forward_dir();
                                frame_gameplay_trace.mark("gameplay-frame-tail");
                                cpu_stats.gameplay_ms =
                                    gameplay_start.elapsed().as_secs_f32() * 1000.0;

                                // A congested GPU must not receive another frame's
                                // uploads on every presentation retry. Keep dirty
                                // terrain queued and continue simulating above.
                                if gpu.device_usable() {
                                    let wait_started = benchmark_config.enabled.then(Instant::now);
                                    if !gpu.ready_for_frame() && gpu.device_usable() {
                                        let wait_ms = wait_started.map_or(0.0, |start| start.elapsed().as_secs_f32() * 1000.0);
                                        egui_ui.end_frame_no_paint(window.as_ref());
                                        if wait_started.is_some() {
                                            present_gap.count += 1;
                                            present_gap.wait_ms += wait_ms;
                                            present_gap.redraw_ms += frame_start.elapsed().as_secs_f32() * 1000.0;
                                        }
                                        let now = Instant::now();
                                        gpu_retry_at = Some(now + Duration::from_millis(1));
                                        gpu_retry_simulation_at = Some(now + Duration::from_secs_f32(simulation::STEP_SECONDS));
                                        return;
                                    }
                                }

                                let center_chunk =
                                    chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                let snapshot_start = Instant::now();
                                gameplay_trace::frame_stage("stream");
                                let precise_eye_m = player.precise_eye(world.voxel_scale) * world.voxel_scale.meters() as f64;
                                let altitude_m = altitude::altitude_m(precise_eye_m, altitude::planet_world(world));
                                let resident_radius = altitude::resident_radius_chunks(
                                    altitude::residency_altitude_m(world, &player, altitude_m),
                                    render_distance_settings().view_distance_chunks,
                                );
                                let terrain_streaming = resident_radius > 0;
                                world.set_visual_region_interest(&if terrain_streaming { chunk_cache.edit_proxy_source_coords() } else { chunk_cache.space_source_coords() });
                                if terrain_streaming { chunk_cache.set_horizon_pins(gpu.horizon_retention_columns()); }
                                // Frames without a fixed step still publish edits made by input handlers.
                                world.publish_changes(&mut dirty_blocks, &mut dirty_chunks);
                                // Edits inside the near window already invalidate the near snapshot, so
                                // only dirt outside it needs the full (whole-world clone) snapshot.
                                let view_chunks = render_distance_settings().view_distance_chunks;
                                let dirt_in_near_window = dirty_chunks.iter().all(|c| {
                                    c.x.abs_diff(center_chunk.x).max(c.z.abs_diff(center_chunk.z)) <= (view_chunks + 1) as u32
                                });
                                let snapshot = if !terrain_streaming || chunk_cache.has_horizon_pins() || !dirt_in_near_window { world.stream_regions(center_chunk, resident_radius); world.snapshot() } else { world.snapshot_near(center_chunk,resident_radius) };
                                cpu_stats.snapshot_ms =
                                    snapshot_start.elapsed().as_secs_f32() * 1000.0;
                                let tier = altitude_state.update(altitude_m);
                                gpu.set_tier(tier);
                                let orbit = tier == altitude::Tier::Orbit;
                                chunk_cache.freeze(false);
                                let streaming_start = Instant::now();
                                let interest_change = chunk_cache.update_streaming_center(
                                    center_chunk,
                                    &snapshot,
                                    resident_radius,
                                    benchmark.as_ref().and_then(|state| state.scripted_stream_velocity).unwrap_or(player.vel),
                                );
                                let (mut stream_invalidate_ms, mut stream_proxies_ms, mut stream_expand_ms,
                                    mut stream_dirty_ms, mut stream_quality_ms, mut stream_enqueue_ms) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
                                if !terrain_streaming {
                                    if interest_change == render::StreamInterestChange::Bounds { chunk_streamer.reset(); }
                                    chunk_streamer.stream_space(gpu, &snapshot, &mut chunk_cache, world.revision());
                                } else {
                                    if interest_change == render::StreamInterestChange::Bounds {
                                        chunk_cache.retain_visible();
                                        if chunk_cache.has_horizon_pins() { chunk_streamer.retain_visible(&chunk_cache.visible_keys); } else { chunk_streamer.retain_within_range(center_chunk); }
                                    }
                                    if interest_change != render::StreamInterestChange::None {
                                        chunk_streamer.queue_needed_visible(&chunk_cache, center_chunk);
                                    }

                                    chunk_streamer.invalidate_changed_sources(&snapshot,&mut chunk_cache);
                                    stream_invalidate_ms=streaming_start.elapsed().as_secs_f32()*1000.0;
                                    let proxy_keys=chunk_cache.update_edit_proxies(&snapshot,&dirty_chunks,world.revision(),gpu.horizon_published_revision());
                                    if let Some(revision)=gpu.horizon_published_revision() {
                                        let baked=gpu.horizon_baked_columns(&chunk_cache.edit_proxy_columns());
                                        chunk_cache.retire_edit_proxies(&baked,revision);
                                    }
                                    if !proxy_keys.is_empty() { chunk_streamer.push_pending(proxy_keys,chunk_cache.center_chunk); }
                                    stream_proxies_ms=streaming_start.elapsed().as_secs_f32()*1000.0;
                                    let new_keys = if orbit { Vec::new() } else { chunk_cache
                                        .expand_visible(&snapshot, chunk_streamer.expansion_pressure()) };
                                    if !new_keys.is_empty() {
                                        chunk_streamer.push_pending(new_keys, chunk_cache.center_chunk);

                                    }

                                    stream_expand_ms=streaming_start.elapsed().as_secs_f32()*1000.0;
                                    if !dirty_chunks.is_empty() {
                                        chunk_streamer.rebuild_dirty(
                                            &snapshot,
                                            &mut chunk_cache,
                                            &dirty_blocks,
                                            &dirty_chunks,
                                            world.revision(),
                                        );
                                        dirty_blocks.clear();
                                        dirty_chunks.clear();
                                    }


                                    stream_dirty_ms=streaming_start.elapsed().as_secs_f32()*1000.0;
                                    stream_quality_ms=stream_dirty_ms;
                                    if !orbit {
                                    let quality_eye=player.precise_eye(world.voxel_scale).as_vec3()*world.voxel_scale.meters();
                                    let wanted_quality=gpu.horizon_quality_columns(snapshot.cfg,quality_eye,player.forward_dir(),egui_ui.fov_degrees);
                                    let previous_quality=chunk_cache.quality_columns();
                                    let quality_parent_valid=chunk_cache.quality_parent_validation_needed()
                                        && gpu.horizon_quality_parent_valid(&snapshot,quality_eye,&previous_quality);
                                    let quality_keys=chunk_cache.set_quality_columns(&snapshot,&wanted_quality,quality_parent_valid,chunk_streamer);
                                    if !quality_keys.is_empty(){chunk_streamer.push_pending(quality_keys,chunk_cache.center_chunk);}
                                    stream_quality_ms=streaming_start.elapsed().as_secs_f32()*1000.0;
                                    chunk_streamer.set_view(
                                        player.precise_eye(world.voxel_scale), player.forward_dir(),
                                        player.up_dir(), egui_ui.fov_degrees, gpu.aspect_ratio(),
                                    );
                                    chunk_streamer.enqueue_missing(
                                        gpu,
                                        &snapshot,
                                        world.revision(),
                                        &mut chunk_cache,
                                    );
                                    }
                                    stream_enqueue_ms=streaming_start.elapsed().as_secs_f32()*1000.0;
                                    for (coord,mask) in chunk_cache.take_simulation_seeds() {
                                        if world.ca_view.is_none() { fluid_sim.seed(world,player.pos.floor().as_ivec3(),coord,mask); }
                                    }
                                    let resident_upload_start = Instant::now();
                                    gameplay_trace::frame_stage("upload");
                                    // Let small LOD meshes use spare upload capacity.
                                    // Time, geometry and in-flight limits still apply.
                                    chunk_streamer.upload_finished(
                                        gpu,
                                        &snapshot,
                                        &mut chunk_cache,
                                        world.revision(),
                                        8,
                                    );
                                    if let Some(probe)=latency_probe.as_mut() {
                                        probe.after_upload(|coord| chunk_cache.published_revision(coord));
                                        if let Some(coord)=probe.stalled_chunk(){eprintln!("[player-mesh-wait] {}",chunk_streamer.stalled_edit_debug(&chunk_cache,coord));}
                                    }
                                    cpu_stats.resident_upload_wall_ms =
                                        resident_upload_start.elapsed().as_secs_f32() * 1000.0;
                                } // terrain_streaming
                                gameplay_trace::frame_stage("upload-done");
                                cpu_stats.streaming_ms =
                                    streaming_start.elapsed().as_secs_f32() * 1000.0;
                                for (stage, ms) in [("stream.invalidate", stream_invalidate_ms), ("stream.proxies", stream_proxies_ms-stream_invalidate_ms),
                                    ("stream.expand", stream_expand_ms-stream_proxies_ms), ("stream.dirty", stream_dirty_ms-stream_expand_ms),
                                    ("stream.quality", stream_quality_ms-stream_dirty_ms), ("stream.enqueue", stream_enqueue_ms-stream_quality_ms),
                                    ("stream.upload", cpu_stats.streaming_ms-stream_enqueue_ms)] {
                                    gameplay_trace::note(stage, f64::from(ms));
                                }
                                if cpu_stats.streaming_ms>20.0 && std::env::var_os("VOXEL_STREAM_TIMINGS").is_some() {
                                    eprintln!("[stream-stages] invalidate={:.2} proxies={:.2} expand={:.2} dirty={:.2} quality={:.2} enqueue={:.2} upload={:.2}",
                                        stream_invalidate_ms,stream_proxies_ms-stream_invalidate_ms,stream_expand_ms-stream_proxies_ms,
                                        stream_dirty_ms-stream_expand_ms,stream_quality_ms-stream_dirty_ms,stream_enqueue_ms-stream_quality_ms,
                                        cpu_stats.streaming_ms-stream_enqueue_ms);
                                }


                                let world_scale = world.voxel_scale.meters();
                                let render_settings = render_distance_settings();
                                let far_plane_voxels = (render_settings.view_distance_chunks
                                    as f32
                                    * world::CHUNK_SIZE as f32
                                    * 1.20)
                                    .max(640.0);
                                let render_eye = eye * world_scale;
                                let horizon_start = Instant::now();
                                let backdrop_coverage = chunk_cache.backdrop_coverage_meters(
                                    render_eye,
                                    render_settings.view_distance_chunks,
                                    world_scale,
                                );
                                let (resident_coverage, strict_radius) = chunk_cache.resident_coverage_and_radius(render_eye, render_settings.view_distance_chunks, world_scale);
                                gpu.update_horizon(world, render_eye, strict_radius, resident_coverage, chunk_cache.desired_resident_coverage(render_eye,render_settings.view_distance_chunks,world_scale),&chunk_cache,egui_ui.fov_degrees,!benchmark_loading);
                                cpu_stats.horizon_ms = horizon_start.elapsed().as_secs_f32() * 1000.0;
                                let cull_view = glam::camera::rh::view::look_to_mat4(eye, forward, player.up_dir());
                                // Rendering includes true orbital distances. The GPU
                                // converts this conventional infinite projection to
                                // reverse-Z; terrain culling keeps its finite budget.
                                let proj = glam::camera::rh::proj::directx::perspective_infinite(
                                    view_zoom.lens(egui_ui.fov_degrees, player.movement.fov_kick_degrees()).to_radians(),
                                    gpu.aspect_ratio(),
                                    0.05 * world_scale,
                                );
                                let globe_start = Instant::now();
                                let globe_fov = view_zoom.lens(egui_ui.fov_degrees, 0.0).to_radians();
                                gpu.update_globe(world, &day_night, precise_eye_m, forward, proj, gpu.focal_px(globe_fov));
                                cpu_stats.globe_ms = globe_start.elapsed().as_secs_f32() * 1000.0;
                                cpu_stats.globe_upload_bytes = gpu.globe_stats().upload_bytes as f32;
                                cpu_stats.globe_wanted = gpu.globe_stats().wanted as f32;
                                cpu_stats.globe_bake_ms = gpu.globe_stats().bake_ms;
                                cpu_stats.orbital_cloud_bake_ms = gpu.globe_stats().cloud_bake_ms;
                                cpu_stats.orbital_cloud_pending = gpu.globe_stats().cloud_pending as f32;
                                cpu_stats.orbital_cloud_resolution = gpu.globe_stats().cloud_resolution as f32;
                                let cull_proj = glam::camera::rh::proj::directx::perspective(
                                    view_zoom.lens(egui_ui.fov_degrees, player.movement.fov_kick_degrees()).to_radians(),
                                    gpu.aspect_ratio(),
                                    0.05,
                                    far_plane_voxels,
                                );
                                let culling_start = Instant::now();
                                build_draw_list_with_coverage(&chunk_cache, cull_proj * cull_view, Some(eye), &resident_coverage, &mut draw_scratch, &mut draw_list);
                                cpu_stats.culling_ms =
                                    culling_start.elapsed().as_secs_f32() * 1000.0;
                                if benchmark.is_some() { visible_missing = chunk_cache.missing_in_view(cull_proj * cull_view); }
                                let held_torch =
                                    held_torch_light(world, &player);
                                let torch_start = Instant::now();
                                let (mut nearby_torches, extinguished) =
                                    collect_nearby_torches(world, &snapshot, &chunk_cache, player.pos, world_scale);
                                gpu.upload_extinguished_torches(extinguished);
                                append_fire_lights(world, player.pos, world_scale, &mut nearby_torches);
                                world::hell_gate::built::append_lights(world, player.pos * world_scale, &mut nearby_torches);
                                for blast in world.adventure.blasts.iter().filter(|b| b.age<0.28).take(4) {
                                    if nearby_torches.len()>=MAX_TORCH_LIGHTS {nearby_torches.pop();}
                                    nearby_torches.insert(0,TorchLight {
                                        position:blast.pos+blast.normal*0.3,
                                        radius:blast.radius*2.0+2.0,color:[1.0,0.64,0.25],
                                        strength:5.0*(1.0-blast.age/0.28).powi(2),
                                        moving:true,
                                    });
                                }
                                for (pos, radius, color, strength) in world.adventure.spell_lights(player.eye(world.voxel_scale) * world_scale) {
                                    if nearby_torches.len() >= MAX_TORCH_LIGHTS {
                                        nearby_torches.pop();
                                    }
                                    nearby_torches.insert(
                                        0,
                                        TorchLight {
                                            position: pos,
                                            radius,
                                            color,
                                            strength,
                                            moving: true,
                                        },
                                    );
                                }
                                if let Some((pos, color, strength)) = mesh::models::wand_tip_light(world, &player, world_scale) {
                                    if nearby_torches.len() >= MAX_TORCH_LIGHTS {
                                        nearby_torches.pop();
                                    }
                                    nearby_torches.insert(0, TorchLight { position: pos, radius: 6.5, color, strength, moving: true });
                                }
                                cpu_stats.torch_ms =
                                    torch_start.elapsed().as_secs_f32() * 1000.0;
                                push_cast_light(
                                    world,
                                    &player,
                                    forward,
                                    world_scale,
                                    &mut nearby_torches,
                                );

                                if matches!(mode, AppMode::Playing) && !creative.any_ui_open() && !spawning {
                                    egui_ui::map::perceive(world, &player, &chunk_cache, &nearby_torches, held_torch, day_night.lighting().daylight > 0.25,
                                        view_zoom.lens(egui_ui.fov_degrees, player.movement.fov_kick_degrees()).to_radians(), gpu.aspect_ratio());
                                }
                                nearby_torch_count = nearby_torches.len();

                                let weather_start = Instant::now();
                                let day_speed = DEFAULT_DAY_LENGTH_SECONDS
                                    / day_night.day_length_seconds().max(1.0);
                                let sky_motion_rate = 0.55 + day_speed * 0.65;
                                    sky_time = benchmark.as_ref().and_then(|b| b.config.review_sky_time)
                                        .unwrap_or(sky_time + presented_dt * sky_motion_rate);
                                let abs_player_world_xz_m: DVec2 = (player.pos.xz() * world_scale).as_dvec2();

                                if let Some(upload_manifest) = cloud_weather.advance(
                                    gameplay_dt,
                                    abs_player_world_xz_m.as_vec2(),
                                    &day_night.cloud_controls(),
                                    day_night.weather_seed(),
                                    sky_time,
                                    world::synoptic::days(&day_night),
                                    world.sample_config(),
                                ) {
                                    gpu.upload_cloud_weather(&cloud_weather.field, &upload_manifest);
                                }
                                cpu_stats.weather_ms =
                                    weather_start.elapsed().as_secs_f32() * 1000.0;
                                let uniform_start = Instant::now();
                                let eye_m = (player.precise_eye(world.voxel_scale) * world_scale as f64).as_vec3();
                                world::ambience::refresh(world, &player);
                                let environment = world.adventure.location.reading_at(world.sample_config(), eye_m);
                                let cosmos_target = environment.map_or(0.0, |reading| reading.cosmos);
                                cosmos_smooth += (cosmos_target - cosmos_smooth)
                                    * (1.0 - (-gameplay_dt.max(0.0) * 3.5).exp());
                                let planet_tick = planet::tick_of(&day_night);
                                let planet_eye = player.precise_eye(world.voxel_scale) * world_scale as f64;
                                // Weather over the player: sky dimming, wetness and rain share it.
                                let local_clouds = world::synoptic::controls_at(day_night.cloud_controls(), Some(world.sample_config()), abs_player_world_xz_m, world::synoptic::days(&day_night), day_night.weather_seed());
                                let mut scene_lighting = if world.world_type == WorldType::Planet {
                                    day_night.lighting_under(planet::solar_time(planet_eye, planet_tick), local_clouds)
                                } else { day_night.lighting_under(day_night.time_of_day(), local_clouds) };
                                if world.sample_config().is_planet() { planet::lighting(&mut scene_lighting, planet_eye, planet_tick); }
                                if world.world_type == WorldType::Nether { world::portals::realm::lighting(&mut scene_lighting, world.sample_config().is_hell()); world::portals::realm::hell_weather(&mut scene_lighting, world.sample_config(), world.adventure.time, eye_m); }
                                if !world.sample_config().is_hell() && !scene_lighting.hell
                                    && let Some(reading) = environment {
                                    world::sky_air::apply_region(&mut scene_lighting, reading.regional_air[0], reading.regional_air[1]);
                                }
                                scene_lighting.mood[0] = if cosmos_smooth > 0.002 { cosmos_smooth } else { 0.0 };
                                // Living air and the sky-event grade only exist above ground.
                                let ground_m = environment.map_or(eye_m.y, |reading| reading.ground_m);
                                let underground = eye_m.y < ground_m - 3.0;
                                scene_lighting.cave_darkness = ((ground_m-eye_m.y-3.0)/9.0).clamp(0.0,1.0) * (1.0-cosmos_smooth);
                                // The Lightless is closed rock: never any sky, wherever the column top is.
                                if let Some(kind) = world.world_type.realm() { world::portals::realm::realm_lighting(kind, eye_m, world.seed, &mut scene_lighting); if !kind.has_sky() { scene_lighting.cave_darkness = 1.0; } }
                                location_ambience.apply(world, &mut scene_lighting, player.underwater,
                                    local_clouds.cumulonimbus, day_night.sky_event(), gameplay_dt);
                                if !underground && world.adventure.hell_ashfall_at(world, player.pos * world.voxel_scale.meters()) {
                                    scene_lighting.ambience[3] = 0.86;
                                    scene_lighting.biome_ambience[3] = 0.70;
                                }
                                if underground {
                                    scene_lighting.mood_tint = [1.0, 1.0, 1.0, 0.0];
                                }
                                if cosmos_smooth > 0.5 {
                                    // Underground there is no sky to announce.
                                    scene_lighting.mood[1] = 0.0;
                                    scene_lighting.mood[2] = 0.0;
                                    scene_lighting.mood[3] = 0.0;
                                }
                                let sky_event = day_night.sky_event().filter(|e| e.intensity > 0.35).map(|e| e.kind);
                                if sky_event != last_sky_event {
                                    if let Some(kind) = sky_event {
                                        egui_ui.announce_sky(kind.name(), kind.announcement());
                                    }
                                    last_sky_event = sky_event;
                                }
                                // A meteor shower leaves real stars on the ground to go and find.
                                if world.ca_view.is_none() && sky_event == Some(world::mood::SkyEvent::MeteorShower)
                                    && day_night.sky_event().is_some_and(|e| e.intensity > 0.5)
                                    && cosmos_smooth < 0.05
                                {
                                    star_timer -= gameplay_dt.max(0.0);
                                    if star_timer <= 0.0 {
                                        star_timer = 38.0 + 30.0 * ((star_count * 7 + 3) % 10) as f32 / 10.0;
                                        let (dx, dz) = world::mood::star_landing(day_night.weather_seed(), star_count);
                                        star_count = star_count.wrapping_add(1);
                                        let bx = (player.pos.x + dx / world_scale).floor() as i32;
                                        let bz = (player.pos.z + dz / world_scale).floor() as i32;
                                        let ground = world.terrain_height_at(bx, bz);
                                        let sea = world.sample_config().water_level();
                                        if ground > sea {
                                            // A scorched dimple with a crystal in it.
                                            let mut edits = vec![(glam::IVec3::new(bx, ground + 1, bz), BlockId::Crystal)];
                                            for (ox, oz) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                                                edits.push((glam::IVec3::new(bx + ox, ground, bz + oz), BlockId::Basalt));
                                            }
                                            world.set_many(&edits);
                                            egui_ui.announce_sky("Fallen star", world::mood::compass_line(dx, dz));
                                        }
                                    }
                                }
                                gpu.upload_lighting(
                                    &scene_lighting,
                                    &local_clouds,
                                    world::wind_at(&day_night, Some(world.sample_config()), abs_player_world_xz_m),
                                    sky_time,
                                    held_torch,
                                    &nearby_torches,
                                    Some(&cloud_weather.field),
                                    render_debug_view,
                                    atmosphere_mode,
                                );
                                gpu.upload_torch_blockage(&chunk_cache, eye, world_scale);
                                let render_eye = player.precise_eye(world.voxel_scale) * world_scale as f64 + world::adventure::ground_shake(((day_night.time_of_day() as f64 * f64::from(day_night.day_length_seconds())) % 1000.0) as f32).as_dvec3();
                                gpu.camera_medium = camera_medium(world, render_eye);
                                gpu.water_level_m = world.sample_config().water_level() as f32 * world.voxel_scale.meters();
                                gpu.set_camera_up(player.up_dir());
                                gpu.upload_camera(render_eye, world_scale, forward, proj, world.seed, world.generator_version, ((day_night.day_index() as f64 + day_night.time_of_day() as f64) * 480.0 * 120.0) as u64, backdrop_coverage);
                                gpu.set_portals(&crate::manyfold::maze_portal::shown(world), world.portals.revision,
                                    world::portals::realm::gates(world, player.pos * world_scale));
                                cpu_stats.uniform_ms =
                                    uniform_start.elapsed().as_secs_f32() * 1000.0;
                            } else if render_world {
                                let world = world.as_mut().unwrap();

                                let day_speed = DEFAULT_DAY_LENGTH_SECONDS
                                    / day_night.day_length_seconds().max(1.0);
                                let sky_motion_rate = 0.55 + day_speed * 0.65;
                                sky_time = benchmark.as_ref().and_then(|b| b.config.review_sky_time)
                                    .unwrap_or(sky_time + presented_dt * sky_motion_rate);
                                let precise_eye_m = player.precise_eye(world.voxel_scale) * world.voxel_scale.meters() as f64;
                                let tier = altitude_state.update(altitude::altitude_m(precise_eye_m, altitude::planet_world(world)));
                                gpu.set_tier(tier);
                                let orbit = tier == altitude::Tier::Orbit;
                                chunk_cache.freeze(false);
                                let streaming_start = Instant::now();
                                let center_chunk = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                let resident_radius = altitude::resident_radius_chunks(
                                    altitude::residency_altitude_m(world, &player, precise_eye_m.y),
                                    render_distance_settings().view_distance_chunks,
                                );
                                let terrain_streaming = resident_radius > 0;
                                world.set_visual_region_interest(&if terrain_streaming { chunk_cache.edit_proxy_source_coords() } else { chunk_cache.space_source_coords() });
                                if terrain_streaming { chunk_cache.set_horizon_pins(gpu.horizon_retention_columns()); }
                                let snapshot = if !terrain_streaming || chunk_cache.has_horizon_pins() { world.stream_regions(center_chunk, resident_radius); world.snapshot() } else { world.snapshot_near(center_chunk, resident_radius) };
                                if !terrain_streaming {
                                    if chunk_cache.update_center_chunk_with_snapshot_radius(center_chunk, &snapshot, 0) { chunk_streamer.reset(); }
                                    chunk_streamer.stream_space(gpu, &snapshot, &mut chunk_cache, world.revision());
                                } else {
                                    chunk_streamer.invalidate_changed_sources(&snapshot,&mut chunk_cache);
                                    let proxy_keys=chunk_cache.update_edit_proxies(&snapshot,&dirty_chunks,world.revision(),gpu.horizon_published_revision());
                                    if !proxy_keys.is_empty() { chunk_streamer.push_pending(proxy_keys,chunk_cache.center_chunk); }
                                    if !orbit {
                                        chunk_streamer.set_view(
                                            player.precise_eye(world.voxel_scale), player.forward_dir(),
                                            player.up_dir(), egui_ui.fov_degrees, gpu.aspect_ratio(),
                                        );
                                        chunk_streamer.enqueue_missing(gpu, &snapshot, world.revision(), &mut chunk_cache);
                                    }
                                    for (coord,mask) in chunk_cache.take_simulation_seeds() {fluid_sim.seed(world,player.pos.floor().as_ivec3(),coord,mask);}
                                    chunk_streamer.upload_finished(
                                        gpu,
                                        &snapshot,
                                        &mut chunk_cache,
                                        world.revision(),
                                        3,
                                    );
                                } // terrain_streaming
                                cpu_stats.streaming_ms =
                                    streaming_start.elapsed().as_secs_f32() * 1000.0;

                                let eye = player.eye(world.voxel_scale);
                                let forward = player.forward_dir();
                                let snapshot_start = Instant::now();
                                let snapshot = world.snapshot();
                                cpu_stats.snapshot_ms =
                                    snapshot_start.elapsed().as_secs_f32() * 1000.0;
                                let world_scale = world.voxel_scale.meters();
                                let render_settings = render_distance_settings();
                                let far_plane_voxels = (render_settings.view_distance_chunks
                                    as f32
                                    * world::CHUNK_SIZE as f32
                                    * 1.20)
                                    .max(640.0);
                                let render_eye = eye * world_scale;
                                let horizon_start = Instant::now();
                                let backdrop_coverage = chunk_cache.backdrop_coverage_meters(
                                    render_eye,
                                    render_settings.view_distance_chunks,
                                    world_scale,
                                );
                                let (resident_coverage, strict_radius) = chunk_cache.resident_coverage_and_radius(render_eye, render_settings.view_distance_chunks, world_scale);
                                gpu.update_horizon(world, render_eye, strict_radius, resident_coverage, chunk_cache.desired_resident_coverage(render_eye,render_settings.view_distance_chunks,world_scale),&chunk_cache,egui_ui.fov_degrees,!benchmark_loading);
                                cpu_stats.horizon_ms = horizon_start.elapsed().as_secs_f32() * 1000.0;
                                let cull_view = glam::camera::rh::view::look_to_mat4(eye, forward, player.up_dir());
                                let proj = glam::camera::rh::proj::directx::perspective_infinite(
                                    view_zoom.lens(egui_ui.fov_degrees, player.movement.fov_kick_degrees()).to_radians(),
                                    gpu.aspect_ratio(),
                                    0.05 * world_scale,
                                );
                                let globe_start = Instant::now();
                                let globe_fov = view_zoom.lens(egui_ui.fov_degrees, 0.0).to_radians();
                                gpu.update_globe(world, &day_night, precise_eye_m, forward, proj, gpu.focal_px(globe_fov));
                                cpu_stats.globe_ms = globe_start.elapsed().as_secs_f32() * 1000.0;
                                cpu_stats.globe_upload_bytes = gpu.globe_stats().upload_bytes as f32;
                                cpu_stats.globe_wanted = gpu.globe_stats().wanted as f32;
                                cpu_stats.globe_bake_ms = gpu.globe_stats().bake_ms;
                                cpu_stats.orbital_cloud_bake_ms = gpu.globe_stats().cloud_bake_ms;
                                cpu_stats.orbital_cloud_pending = gpu.globe_stats().cloud_pending as f32;
                                cpu_stats.orbital_cloud_resolution = gpu.globe_stats().cloud_resolution as f32;
                                let cull_proj = glam::camera::rh::proj::directx::perspective(
                                    view_zoom.lens(egui_ui.fov_degrees, player.movement.fov_kick_degrees()).to_radians(),
                                    gpu.aspect_ratio(),
                                    0.05,
                                    far_plane_voxels,
                                );
                                let culling_start = Instant::now();
                                build_draw_list_with_coverage(&chunk_cache, cull_proj * cull_view, Some(eye), &resident_coverage, &mut draw_scratch, &mut draw_list);
                                cpu_stats.culling_ms =
                                    culling_start.elapsed().as_secs_f32() * 1000.0;
                                if benchmark.is_some() { visible_missing = chunk_cache.missing_in_view(cull_proj * cull_view); }
                                let held_torch =
                                    held_torch_light(world, &player);
                                let torch_start = Instant::now();
                                let (mut nearby_torches, extinguished) =
                                    collect_nearby_torches(world, &snapshot, &chunk_cache, player.pos, world_scale);
                                gpu.upload_extinguished_torches(extinguished);
                                append_fire_lights(world, player.pos, world_scale, &mut nearby_torches);
                                world::hell_gate::built::append_lights(world, player.pos * world_scale, &mut nearby_torches);
                                for (pos, radius, color, strength) in world.adventure.spell_lights(player.eye(world.voxel_scale) * world_scale) {
                                    if nearby_torches.len() >= MAX_TORCH_LIGHTS {
                                        nearby_torches.pop();
                                    }
                                    nearby_torches.insert(
                                        0,
                                        TorchLight {
                                            position: pos,
                                            radius,
                                            color,
                                            strength,
                                            moving: true,
                                        },
                                    );
                                }
                                if let Some((pos, color, strength)) = mesh::models::wand_tip_light(world, &player, world_scale) {
                                    if nearby_torches.len() >= MAX_TORCH_LIGHTS {
                                        nearby_torches.pop();
                                    }
                                    nearby_torches.insert(0, TorchLight { position: pos, radius: 6.5, color, strength, moving: true });
                                }
                                cpu_stats.torch_ms =
                                    torch_start.elapsed().as_secs_f32() * 1000.0;
                                push_cast_light(
                                    world,
                                    &player,
                                    forward,
                                    world_scale,
                                    &mut nearby_torches,
                                );

                                if matches!(mode, AppMode::Playing) && !creative.any_ui_open() && !spawning {
                                    egui_ui::map::perceive(world, &player, &chunk_cache, &nearby_torches, held_torch, day_night.lighting().daylight > 0.25,
                                        view_zoom.lens(egui_ui.fov_degrees, player.movement.fov_kick_degrees()).to_radians(), gpu.aspect_ratio());
                                }
                                nearby_torch_count = nearby_torches.len();
                                let weather_start = Instant::now();
                                let abs_player_world_xz_m: DVec2 = (player.pos.xz() * world_scale).as_dvec2();
                                if let Some(upload_manifest) = cloud_weather.advance(
                                    0.0,
                                    abs_player_world_xz_m.as_vec2(),
                                    &day_night.cloud_controls(),
                                    day_night.weather_seed(),
                                    sky_time,
                                    world::synoptic::days(&day_night),
                                    world.sample_config(),
                                ) {
                                    gpu.upload_cloud_weather(&cloud_weather.field, &upload_manifest);
                                }
                                cpu_stats.weather_ms =
                                    weather_start.elapsed().as_secs_f32() * 1000.0;
                                let uniform_start = Instant::now();
                                let eye_m = (player.precise_eye(world.voxel_scale) * world_scale as f64).as_vec3();
                                let cosmos_target = world::cosmos_at(&world.sample_config(), eye_m);
                                cosmos_smooth += (cosmos_target - cosmos_smooth)
                                    * (1.0 - (-gameplay_dt.max(0.0) * 3.5).exp());
                                let planet_tick = planet::tick_of(&day_night);
                                let planet_eye = player.precise_eye(world.voxel_scale) * world_scale as f64;
                                // Weather over the player: sky dimming, wetness and rain share it.
                                let local_clouds = world::synoptic::controls_at(day_night.cloud_controls(), Some(world.sample_config()), abs_player_world_xz_m, world::synoptic::days(&day_night), day_night.weather_seed());
                                let mut scene_lighting = if world.world_type == WorldType::Planet {
                                    day_night.lighting_under(planet::solar_time(planet_eye, planet_tick), local_clouds)
                                } else { day_night.lighting_under(day_night.time_of_day(), local_clouds) };
                                if world.sample_config().is_planet() { planet::lighting(&mut scene_lighting, planet_eye, planet_tick); }
                                if world.world_type == WorldType::Nether { world::portals::realm::lighting(&mut scene_lighting, world.sample_config().is_hell()); world::portals::realm::hell_weather(&mut scene_lighting, world.sample_config(), world.adventure.time, eye_m); }
                                world::sky_air::apply_region_at(&mut scene_lighting, &world.sample_config(), eye_m);
                                scene_lighting.mood[0] = if cosmos_smooth > 0.002 { cosmos_smooth } else { 0.0 };
                                // Living air and the sky-event grade only exist above ground.
                                let ground_m = world.terrain_height_at(
                                    (eye_m.x / world_scale).floor() as i32,
                                    (eye_m.z / world_scale).floor() as i32,
                                ) as f32 * world_scale;
                                let underground = eye_m.y < ground_m - 3.0;
                                scene_lighting.cave_darkness = ((ground_m-eye_m.y-3.0)/9.0).clamp(0.0,1.0) * (1.0-cosmos_smooth);
                                // The Lightless is closed rock: never any sky, wherever the column top is.
                                if let Some(kind) = world.world_type.realm() { world::portals::realm::realm_lighting(kind, eye_m, world.seed, &mut scene_lighting); if !kind.has_sky() { scene_lighting.cave_darkness = 1.0; } }
                                world::ambience::refresh(world, &player);
                                location_ambience.apply(world, &mut scene_lighting, player.underwater,
                                    local_clouds.cumulonimbus, day_night.sky_event(), gameplay_dt);
                                if !underground && world.adventure.hell_ashfall_at(world, player.pos * world.voxel_scale.meters()) {
                                    scene_lighting.ambience[3] = 0.86;
                                    scene_lighting.biome_ambience[3] = 0.70;
                                }
                                if underground {
                                    scene_lighting.mood_tint = [1.0, 1.0, 1.0, 0.0];
                                }
                                if cosmos_smooth > 0.5 {
                                    // Underground there is no sky to announce.
                                    scene_lighting.mood[1] = 0.0;
                                    scene_lighting.mood[2] = 0.0;
                                    scene_lighting.mood[3] = 0.0;
                                }
                                let sky_event = day_night.sky_event().filter(|e| e.intensity > 0.35).map(|e| e.kind);
                                if sky_event != last_sky_event {
                                    if let Some(kind) = sky_event {
                                        egui_ui.announce_sky(kind.name(), kind.announcement());
                                    }
                                    last_sky_event = sky_event;
                                }
                                // A meteor shower leaves real stars on the ground to go and find.
                                if world.ca_view.is_none() && sky_event == Some(world::mood::SkyEvent::MeteorShower)
                                    && day_night.sky_event().is_some_and(|e| e.intensity > 0.5)
                                    && cosmos_smooth < 0.05
                                {
                                    star_timer -= gameplay_dt.max(0.0);
                                    if star_timer <= 0.0 {
                                        star_timer = 38.0 + 30.0 * ((star_count * 7 + 3) % 10) as f32 / 10.0;
                                        let (dx, dz) = world::mood::star_landing(day_night.weather_seed(), star_count);
                                        star_count = star_count.wrapping_add(1);
                                        let bx = (player.pos.x + dx / world_scale).floor() as i32;
                                        let bz = (player.pos.z + dz / world_scale).floor() as i32;
                                        let ground = world.terrain_height_at(bx, bz);
                                        let sea = world.sample_config().water_level();
                                        if ground > sea {
                                            // A scorched dimple with a crystal in it.
                                            let mut edits = vec![(glam::IVec3::new(bx, ground + 1, bz), BlockId::Crystal)];
                                            for (ox, oz) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                                                edits.push((glam::IVec3::new(bx + ox, ground, bz + oz), BlockId::Basalt));
                                            }
                                            world.set_many(&edits);
                                            egui_ui.announce_sky("Fallen star", world::mood::compass_line(dx, dz));
                                        }
                                    }
                                }
                                gpu.upload_lighting(
                                    &scene_lighting,
                                    &local_clouds,
                                    world::wind_at(&day_night, Some(world.sample_config()), abs_player_world_xz_m),
                                    sky_time,
                                    held_torch,
                                    &nearby_torches,
                                    Some(&cloud_weather.field),
                                    render_debug_view,
                                    atmosphere_mode,
                                );
                                gpu.upload_torch_blockage(&chunk_cache, eye, world_scale);
                                let render_eye = player.precise_eye(world.voxel_scale) * world_scale as f64 + world::adventure::ground_shake(((day_night.time_of_day() as f64 * f64::from(day_night.day_length_seconds())) % 1000.0) as f32).as_dvec3();
                                gpu.camera_medium = camera_medium(world, render_eye);
                                gpu.water_level_m = world.sample_config().water_level() as f32 * world.voxel_scale.meters();
                                gpu.set_camera_up(player.up_dir());
                                gpu.upload_camera(render_eye, world_scale, forward, proj, world.seed, world.generator_version, ((day_night.day_index() as f64 + day_night.time_of_day() as f64) * 480.0 * 120.0) as u64, backdrop_coverage);
                                gpu.set_portals(&crate::manyfold::maze_portal::shown(world), world.portals.revision,
                                    world::portals::realm::gates(world, player.pos * world_scale));
                                cpu_stats.uniform_ms =
                                    uniform_start.elapsed().as_secs_f32() * 1000.0;
                            } else {
                                let uniform_start = Instant::now();
                                gpu.upload_lighting(
                                    &day_night.lighting(),
                                    &day_night.cloud_controls(),
                                    world::wind_at(&day_night, None, glam::DVec2::ZERO),
                                    sky_time,
                                    None,
                                    &[],
                                    None,
                                    render_debug_view,
                                    atmosphere_mode,
                                );
                                gpu.camera_medium = render::CameraMedium::Air;
                                gpu.set_camera_up(Vec3::Y);
                                gpu.upload_camera(glam::DVec3::ZERO, 1.0, Vec3::NEG_Z, Mat4::IDENTITY, 0, 0, 0, -1.0);
                                cpu_stats.uniform_ms =
                                    uniform_start.elapsed().as_secs_f32() * 1000.0;
                            }

                            let render_start = Instant::now();
                            let orbital_pixel_angle = 2.0 * (view_zoom.fov_degrees(egui_ui.fov_degrees).to_radians() * 0.5).tan()
                                / window.inner_size().height.max(1) as f32;
                            // The day clock moves in whole simulation steps; stations cross
                            // the sky at tens of degrees per second, so those hops read as
                            // jumps. Orbits are drawn at the clock plus the banked remainder.
                            // Automatic quality: step settings down when frames stay slow.
                            if let Some(text) = quality_notice_pending.take() {
                                egui_ui.quality_notice = Some((text, 14.0));
                            }
                            if benchmark_config.enabled || spawning || world.is_none() || !focused {
                                quality_governor.reset_trend();
                            } else if let Some(mut step) = quality_governor.gpu_frame(
                                gpu.last_render_stats(),
                                display_frame_dt,
                                matches!(mode, AppMode::Playing),
                            ) {
                                let mut view = render_distance_settings();
                                let before = (view, render::horizon_range_m());
                                // A step that changes nothing (already at that setting) is
                                // skipped at once, so the player never waits for a no-op.
                                while !step.apply(&mut view) {
                                    match quality_governor.force_next() {
                                        Some(next) => step = next,
                                        None => break,
                                    }
                                }
                                set_render_distance_settings(view);
                                crate::quality::record_cut(
                                    (before.0, render_distance_settings()),
                                    (before.1, render::horizon_range_m()),
                                );
                                egui_ui.quality_notice = Some((step.notice().to_string(), 8.0));
                            }
                            let mut sky_day = day_night;
                            // Orbits run at a fixed pace of their own whenever the day is not
                            // running at its default speed (long day, frozen time); otherwise a
                            // slow or stopped day parks every station and skyhook in the sky.
                            let orbital_review = benchmark.as_ref().filter(|b| b.auto_started)
                                .and_then(|b| b.config.review_orbital_time.map(|start| {
                                    let progress = if b.measuring {
                                        (b.phase_started_at.elapsed().as_secs_f64()
                                            / b.config.duration_seconds.max(0.001) as f64).clamp(0.0, 1.0)
                                    } else { 0.0 };
                                    start + (b.config.review_orbital_time_end.unwrap_or(start) - start) * progress
                                }));
                            let default_pace = !freeze_time
                                && (day_night.day_length_seconds() - DEFAULT_DAY_LENGTH_SECONDS).abs() < 0.5;
                            if let Some(days) = orbital_review {
                                sky_day.set_day_index(days.floor() as u32);
                                sky_day.set_time_of_day(days.fract() as f32);
                            } else if receiver_review.is_some() {
                                // Explicit fixture commands own the calendar. Do not
                                // run an independent render clock past a recorded pose.
                                sky_clock = None;
                            } else if let Some(seconds)=world.as_ref().and_then(|w|w.orbital.dynamics.time_seconds()) {
                                // Rendering samples the saved physics clock, including
                                // its banked fixed-step remainder, rather than inventing
                                // a second orbit clock when the solar day is frozen.
                                let lead=if matches!(mode,AppMode::Playing) {simulation_clock.lead_seconds() as f64} else {0.0};
                                let days=(seconds+lead*world::orbital::megastructures::DAY_SECONDS/DEFAULT_DAY_LENGTH_SECONDS as f64)
                                    /world::orbital::megastructures::DAY_SECONDS;
                                sky_day.set_day_index(days.floor() as u32);
                                sky_day.set_time_of_day(days.fract() as f32);
                                sky_clock=None;
                            } else if default_pace {
                                sky_clock = None;
                                if matches!(mode, AppMode::Playing) {
                                    sky_day.advance(simulation_clock.lead_seconds());
                                }
                            } else {
                                let calendar = world::orbital::skyhook::clock(&day_night);
                                let (seconds, last_calendar) = sky_clock.get_or_insert((calendar, calendar));
                                // A jump in the calendar (time slider, loaded save, review
                                // harness) asks to be somewhere else in the day: follow it,
                                // or the sky would show a different time than the sun.
                                if (calendar - *last_calendar).abs() > 600.0 {
                                    *seconds = calendar;
                                }
                                *last_calendar = calendar;
                                if matches!(mode, AppMode::Playing) {
                                    *seconds += display_frame_dt as f64
                                        * (world::orbital::megastructures::DAY_SECONDS / DEFAULT_DAY_LENGTH_SECONDS as f64);
                                }
                                let days = *seconds / world::orbital::megastructures::DAY_SECONDS;
                                sky_day.set_day_index(days.floor() as u32);
                                sky_day.set_time_of_day(days.fract() as f32);
                            }
                            let mut scene_trace = gameplay_trace::Trace::new();
                            let scene=world.as_ref().map(|w|mesh::models::build(w,&player,&sky_day,orbital_pixel_angle,&chunk_cache)).unwrap_or_default();
                            scene_trace.mark("scene-models");
                            #[cfg(feature = "multiplayer")]
                            let scene={let mut scene=scene;if let (Some(session),Some(w))=(multiplayer.as_ref(),world.as_ref()){session.append_players(&mut scene,w.voxel_scale.meters());}scene};
                            gpu.upload_scene(&scene);
                            mesh::models::recycle(scene);
                            scene_trace.mark("scene-upload");
                            let zone = world.as_ref().and_then(|w| {
                                // player.pos is in voxel units; zones work in metres.
                                let cfg = w.sample_config();
                                let metres = player.pos * cfg.voxel_meters();
                                let surface = w.adventure.location.reading_at(cfg, metres)?.ground_m;
                                world::zones::sample(cfg.seed, metres.x, metres.z, surface - metres.y)
                            });
                            let (magic_intensity, magic_color) = world
                                .as_ref()
                                .map(|w| w.adventure.screen_magic())
                                .unwrap_or((0.0, [0.0, 0.0, 0.0]));
                            gpu.set_screen_magic(
                                magic_intensity,
                                world::zones::tint_spell(magic_color, zone.as_ref()),
                            );
                            gpu.set_zone_grade(world::zones::grade(zone.as_ref()));
                            gpu.set_motion_boost(player.movement.boost());
                            gpu.set_camera_up(player.up_dir());
                            gpu.set_camera_roll(player.movement.camera_roll());
                            if let Some(w) = world.as_mut() {
                                w.adventure.zone_fx = world::zones::effects(zone.as_ref());
                            }
                            let zone_key = zone
                                .as_ref()
                                .filter(|z| z.depth_weight > 0.85)
                                .map(|z| (z.layer.index, z.layer.archetype));
                            if zone_key != last_zone_key {
                                if let Some((_, archetype)) = zone_key {
                                    egui_ui.announce_sky(archetype.name(), archetype.entry_line());
                                }
                                last_zone_key = zone_key;
                            }
                            // Cold starts/world switches/teleports may have no
                            // valid destination cover yet. Keep streaming while
                            // gameplay waits behind the explicit transition.
                            if let Some(state) = benchmark.as_mut()
                                && state.lod_travel_worker.as_ref().is_some_and(|worker| worker.is_finished())
                            {
                                match state.lod_travel_worker.take().unwrap().join() {
                                    Ok(profile) => state.lod_travel_profile = Some(profile),
                                    Err(_) => {
                                        eprintln!("[benchmark] flight terrain profile worker failed");
                                        elwt.exit();
                                        return;
                                    }
                                }
                            }
                            let preparing_horizon = (world.is_some() && !gpu.horizon_ready())
                                || benchmark.as_ref().is_some_and(|state| state.lod_travel_worker.is_some());
                            // Route footage uses one monotonic clock, including
                            // real loading frames, rather than restarting PNG
                            // names whenever measurement warmup restarts.
                            if (terrain_review::option("VOXEL_HORIZON_ROUTE") || terrain_review::option("VOXEL_HORIZON_WORLD_SWITCH"))
                                && let Some(state) = benchmark.as_mut()
                                    && state.config.capture_frames {
                                        let shot=(state.motion_started_at.elapsed().as_secs_f32()/state.config.capture_interval) as usize;
                                        if state.last_periodic_capture != Some(shot) {
                                            state.last_periodic_capture=Some(shot);
                                            let root=std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| "target/benchmark-frames".into());
                                            gpu.request_capture(root.join(format!("bench-{shot:03}.png")));
                                        }
                                    }
                            // Only a fresh world waits for its first far field; a
                            // rebuild during play keeps gameplay on screen.
                            // An opt-in native capture of the completed survey, using
                            // this world's real recipe and the ordinary loading pass.
                            static ATLAS_CAPTURED: std::sync::atomic::AtomicBool =
                                std::sync::atomic::AtomicBool::new(false);
                            let atlas_capture = benchmark_loading
                                && terrain_review::option("VOXEL_CAPTURE_LOADING_ATLAS")
                                && !ATLAS_CAPTURED.load(std::sync::atomic::Ordering::Relaxed)
                                // Use readiness from before the first UI pass,
                                // so a survey that finishes mid-frame cannot
                                // capture the preceding empty globe area.
                                && planet_survey_faces == Some(6);
                            let loader_visible = spawning || atlas_capture || preparing_horizon && !gpu.horizon_loaded_this_world()
                                && (benchmark_loading || !quality::horizon_wait_expired());
                            if loader_visible && !spawning && let Some(w) = world.as_ref() {
                                egui_ui.build_world_loading_ui(&loading_report::build(&loading_report::Inputs {
                                    world: w, gpu: &gpu, spawn: spawn_progress,
                                    spawn_ready: chunk_cache.spawn_ready, player_voxel: player.pos, ready: continue_ready,
                                }));
                            }
                            if atlas_capture {
                                let root = std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from)
                                    .unwrap_or_else(|| "target/benchmark-frames".into());
                                gpu.request_capture(root.join("loading-atlas.png"));
                            }
                            if loader_visible && benchmark_config.enabled
                                && let Some(path) = terrain_review::generation_capture() {
                                gpu.request_capture(path);
                            }
                            let show_hud = egui_ui.show_hud;
                            let overlay = |device: &wgpu::Device, queue: &wgpu::Queue,
                                encoder: &mut wgpu::CommandEncoder, target_view: &wgpu::TextureView,
                                width: u32, height: u32| {
                                egui_ui.end_frame(window.as_ref(), device, queue, encoder,
                                    target_view, width, height);
                            };
                            // The opaque title/browser needs only its UI pass,
                            // without the world's render setup and postprocessing.
                            gameplay_trace::frame_stage("render");
                            let review_playable = !loader_visible && !spawning && matches!(mode, AppMode::Playing);
                            if review_playable && let Some(path) = workshop_review.as_mut()
                                .and_then(workshop_review::Review::prepare_playable_capture) {
                                gpu.request_acceptance_capture(path);
                            }
                            let render_result = if loader_visible || matches!(mode, AppMode::WorldSelect) {
                                gpu.render_loading_frame([0.025, 0.035, 0.05], overlay)
                            } else {
                                let mut claim_outline = Vec::new();
                                if matches!(mode, AppMode::Playing) && show_hud
                                    && !spawning && !creative.any_ui_open()
                                    && let Some(w) = world.as_ref()
                                {
                                    world::farming::base::overlay(w, player.eye(w.voxel_scale), &mut claim_outline);
                                }
                                let curved = world.as_ref().filter(|w| w.world_type == WorldType::Manyfold)
                                    .and_then(|w| {
                                        let curve = engine_worldgen::manyfold::shell_curve(w.generator_version)?;
                                        let scale = w.voxel_scale.meters();
                                        let project = |lines: &[mesh::LineVertex]| lines.iter().copied().map(|mut line| {
                                            line.pos = (curve.project(Vec3::from_array(line.pos) * scale) / scale).to_array();
                                            line
                                        }).collect::<Vec<_>>();
                                        Some((project(&outline), project(&invert_outline), project(&claim_outline)))
                                    });
                                let (outline, invert_outline, claim_outline) = curved.as_ref()
                                    .map_or((outline.as_slice(), invert_outline.as_slice(), claim_outline.as_slice()),
                                        |(a, b, c)| (a.as_slice(), b.as_slice(), c.as_slice()));
                                gpu.render_cached_with_overlay(
                                &chunk_cache,
                                &draw_list,
                                outline,
                                invert_outline,
                                claim_outline,
                                &[],
                                paused_render,
                                overlay,
                            ) };
                            if atlas_capture && render_result.is_ok() {
                                ATLAS_CAPTURED.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                            if continue_ready && render_result.is_ok() {
                                world_entry_prompt = true;
                            }
                            if render_result.is_err() && gpu.device_usable() {
                                gpu.flush_pending_frame_work();
                            }
                            cpu_stats.render_ms = render_start.elapsed().as_secs_f32() * 1000.0;
                            gameplay_trace::frame_stage("render-done");
                            let redraw_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
                            cpu_stats.total_ms = redraw_ms;
                            if render_result.is_ok() {
                                if !loader_visible && !spawning && playable_probe_spawn && matches!(mode, AppMode::Playing)
                                    && let Some(started) = playable_probe_start {
                                    println!("[playable-probe] first_playable_ms={:.1} process_ms={:.1} horizon_ready={} horizon_stage={}",
                                        started.elapsed().as_secs_f64() * 1000.0, startup_started.elapsed().as_secs_f64() * 1000.0,
                                        gpu.horizon_ready(), gpu.horizon_work_stage());
                                    elwt.exit();
                                    return;
                                }
                                // Frame cadence includes input/event-loop delays and
                                // failed presentation attempts, not just CPU redraw work.
                                let presented_at = Instant::now();
                                let previous = last_presented_at.replace(presented_at).unwrap_or(frame_start);
                                cpu_stats.total_ms = presented_at.duration_since(previous).as_secs_f32() * 1000.0;
                                if !matches!(mode, AppMode::WorldSelect) {
                                    fps_counter.tick(cpu_stats.total_ms / 1000.0);
                                }
                            }
                            if let Some(state) = benchmark.as_mut()
                                && let Some(timing) = state.route_timing.as_mut() {
                                    let pose_scale = world.as_ref().map_or(selected_voxel_scale, |w| w.voxel_scale);
                                    timing.record(state.motion_started_at.elapsed().as_secs_f64(),
                                        !preparing_horizon, !preparing_horizon && state.measuring, physics_enabled,
                                        render_result.is_ok(), player.eye(pose_scale) * pose_scale.meters(),
                                        player.yaw, player.pitch, cpu_stats,
                                        [gpu.size().width, gpu.size().height], redraw_ms,
                                        if render_result.is_ok() { gpu.last_render_stats().surface_acquire_ms } else { 0.0 },
                                        present_gap);
                                }
                            if render_result.is_ok() {
                                present_gap = perf::route::PresentGap::default();
                            }
                            gameplay_trace::frame_stage("timing-done");
                            if !gpu.device_usable() {
                                eprintln!("[renderer-fatal] GPU device lost; stopped before resource recreation/publication");
                                quality::record_gpu_reset(gpu.adapter_summary(), render::device_loss_report().unwrap_or("device lost while rendering"), benchmark.is_none());
                                elwt.exit();return;
                            }
                            if render_result.is_err() && !matches!(render_result, Err(render::SurfaceFrameError::Timeout))
                                && benchmark.as_ref().is_some_and(|b|b.auto_started) {
                                eprintln!("[benchmark] failed native rendering: {:?}",render_result.as_ref().err());
                                let _=gpu.device.poll(wgpu::PollType::Poll);
                                elwt.exit();return;
                            }
                            if render_result.is_ok() && let Some(probe)=latency_probe.as_mut() {
                                probe.after_present();
                                if probe.finished() {render::flush_review_captures();elwt.exit();return;}
                            }
                            let workshop_finished = workshop_review.as_mut()
                                .is_some_and(|review| review.after_present(review_playable, render_result.is_ok()));
                            if workshop_finished || !preparing_horizon && !spawning
                                && industrial_review.as_ref().is_some_and(industrial_review::Review::done) {
                                let _ = gpu.device.poll(wgpu::PollType::Poll);
                                render::flush_review_captures();
                                elwt.exit();
                                return;
                            }
                            match render_result {
                                Ok(()) => {
                                    let render_stats = gpu.last_render_stats();
                                    let sample = PerfSample {
                                        cpu: cpu_stats,
                                        render: render_stats,
                                        chunk: chunk_streamer.last_stats(),
                                        weather: gpu.last_weather_upload_stats(),
                                        counters: FrameCounters {
                                            visible_chunks: chunk_cache.visible.len(),
                                            visible_meshes: chunk_cache.gpu_meshes.len(),
                                            draw_chunks: render_stats.terrain_draw_calls,
                                            terrain_triangles: render_stats.terrain_indices / 3,
                                            nearby_torches: nearby_torch_count,
                                            visible_missing: visible_missing.0,
                                            near_missing: visible_missing.1,
                                        },
                                    };
                                    if let Some(review)=benchmark.as_mut() && benchmark_profile_logs {
                                        let elapsed=review.motion_started_at.elapsed().as_secs_f64();
                                        // Bound diagnostics so a slow run cannot amplify its own
                                        // stalls with unbounded logging. Normal play is unaffected.
                                        if sample.cpu.total_ms > 50.0 && benchmark_hitch_reports < 24 {
                                            benchmark_hitch_reports += 1;
                                            println!("[frame-hitch] seconds={elapsed:.2} total_ms={:.2} ui={:.2} gameplay={:.2} snapshot={:.2} stream={:.2} cull={:.2} torch={:.2} weather={:.2} uniforms={:.2} render={:.2} acquire={:.2} submit={:.2} shadow_draws={} terrain_draws={} horizon={:.2} resident_upload_wall_ms={:.2} globe={:.2} redraw={redraw_ms:.2} pre_ui={pre_ui_ms:.2}",
                                                sample.cpu.total_ms, sample.cpu.ui_ms, sample.cpu.gameplay_ms,
                                                sample.cpu.snapshot_ms, sample.cpu.streaming_ms, sample.cpu.culling_ms,
                                                sample.cpu.torch_ms, sample.cpu.weather_ms, sample.cpu.uniform_ms,
                                                sample.cpu.render_ms, sample.render.surface_acquire_ms, sample.render.submit_ms,
                                                sample.render.shadow_draw_calls, sample.render.terrain_draw_calls, sample.cpu.horizon_ms, sample.cpu.resident_upload_wall_ms, sample.cpu.globe_ms);
                                            println!("[render-hitch] seconds={elapsed:.2} upload={:.2} shadow={:.2} terrain={:.2} cloud={:.2} ui={:.2} overlay={:.2} present={:.2}",
                                                sample.render.stream_upload_ms, sample.render.shadow_encode_ms,
                                                sample.render.terrain_encode_ms, sample.render.cloud_encode_ms,
                                                sample.render.ui_encode_ms, sample.render.overlay_encode_ms, sample.render.present_ms);
                                        }
                                        let second=elapsed as usize;
                                        if second>review.last_profile_second {
                                            review.last_profile_second=second;
                                            let missing=chunk_cache.visible.iter().filter(|k|!chunk_cache.gpu_meshes.contains_key(&k.coord)).count();
                                            println!("[stream-profile] seconds={elapsed:.2} expanded={} visible={} missing={} pending={} generating={} meshing={} ready={} deferred={} cpu_mib={:.2} gpu_mib={:.2} mesh_mib={:.2} cpu_evictions={} gpu_evictions={} horizon={} physics={} gpu_limit_mib={} gpu_auto={}",
                                                chunk_cache.fully_expanded,chunk_cache.visible.len(),missing,sample.chunk.pending_schedule,sample.chunk.generating,sample.chunk.meshing,
                                                sample.chunk.ready_chunks+sample.chunk.ready_meshes,sample.chunk.budget_deferred,sample.chunk.cpu_resident_bytes as f64/1048576.0,sample.chunk.gpu_resident_bytes as f64/1048576.0,
                                                sample.chunk.queued_mesh_bytes as f64/1048576.0,sample.chunk.cpu_evictions,sample.chunk.gpu_evictions,gpu.horizon_ready(),physics_enabled,
                                                sample.chunk.gpu_budget_bytes/1048576,render::gpu_residency_limit_mib()==0);
                                            let physics = fluid_sim.stats();
                                            chunk_cache.trace_gpu_memory();
                                            println!("[simulation-profile] active={} dormant={} inflight={} pending={} stepped={} updates={} source_mib={:.2} source_failed={} writeback_pressure={} recovering={}",
                                                physics.active_chunks, fluid_sim.dormant_chunks(), physics.inflight_chunks, physics.pending_updates,
                                                physics.simulated_chunks_last_step, physics.applied_updates_last_step,
                                                physics.source_bytes as f64/1048576.0, physics.source_failed,
                                                physics.writeback_pressure, physics.recovering);
                                            if review.config.profile_physics && let Some(w)=world.as_ref() {
                                                river_review::observe(w);
                                                let env=&w.environment;
                                                println!("[environment-profile] tick={} target={} lag_s={} serviced={} blocked={} patches={} pending={} sediment={} fractions={} reserved_bytes={} sky_chunks={} first_event={:?} sky_cursor={:?}",
                                                    env.tick,env.target_tick,env.lag_seconds(),env.serviced,env.blocked,
                                                    env.patches.len(),env.due.len(),env.sediment.len(),env.water_deficit.len(),env.reserved_bytes(),
                                                    env.sky.len(),env.due.first(),env.due.first().and_then(|(_,p)|env.light.get(p)).map(|q|(q.next_y,q.shade)));
                                            }
                                        }
                                    }
                                    perf_tracker.push(sample);
                                    let now = Instant::now();
                                    if now >= next_perf_snapshot {
                                        // The UI snapshot sorts and copies the whole history.
                                        // Benchmark samples and rolling history still update every frame.
                                        perf_snapshot = perf_tracker.snapshot(
                                            gpu.adapter_summary(),
                                            [gpu.size().width, gpu.size().height],
                                            render_distance_settings(),
                                        );
                                        next_perf_snapshot = now + Duration::from_millis(200);
                                        if dev_panel.is_some() { dev_panel::engine::publish_perf(&perf_snapshot); }
                                    }
                                    gameplay_trace::frame_stage("perf-done");
                                    if live_profile && benchmark.is_none() && last_live_profile.elapsed().as_secs_f32() >= 1.0 {
                                        last_live_profile = Instant::now();
                                        let missing = chunk_cache.visible.iter().filter(|k| !chunk_cache.gpu_meshes.contains_key(&k.coord)).count();
                                        let physics = fluid_sim.stats();
                                        println!("[live-profile] fps={:.2} p99_ms={:.3} cpu_game_ms={:.3} cpu_stream_ms={:.3} cpu_uniform_ms={:.3} gpu_ms={:.3} terrain_ms={:.3} clouds_ms={:.3} visible={} missing={} physics={} active={} updates={} gpu_mib={:.2} cpu_horizon_ms={:.3}",
                                            perf_snapshot.fps_avg, perf_snapshot.frame_ms_p99, perf_snapshot.cpu_avg.gameplay_ms,
                                            perf_snapshot.cpu_avg.streaming_ms, perf_snapshot.cpu_avg.uniform_ms,
                                            perf_snapshot.render_avg.gpu_pass_total_ms, perf_snapshot.render_avg.gpu_terrain_ms,
                                            perf_snapshot.render_avg.gpu_cloud_raymarch_ms, chunk_cache.visible.len(), missing,
                                            physics_enabled, physics.active_chunks, physics.applied_updates_last_step,
                                            sample.chunk.gpu_resident_bytes as f64/1048576.0, perf_snapshot.cpu_avg.horizon_ms);
                                    }

                                    if let Some(benchmark_state) = benchmark.as_mut() {
                                        if !benchmark_barrier_open {
                                            benchmark_barrier_open = benchmark_barrier.as_ref().is_some_and(|path| path.exists());
                                        }
                                        // Loading frames have no world passes and cannot be
                                        // reported as world performance or used for warmup.
                                        let fixed_terrain_review = benchmark_state.config.terrain_camera.is_some()
                                            && matches!(benchmark_state.config.motion, BenchmarkMotionMode::Static);
                                        let ready_for_timing = !preparing_horizon && benchmark_barrier_open
                                            && (!fixed_terrain_review || gpu.planet_detail_ready());
                                        if !ready_for_timing {
                                            benchmark_state.measuring = false;
                                            benchmark_state.phase_started_at = Instant::now();
                                            benchmark_state.measured_samples.clear();
                                            if benchmark_state.motion_started_at.elapsed().as_secs_f32()
                                                > benchmark_state.config.warmup_seconds + benchmark_state.config.duration_seconds + 120.0 {
                                                eprintln!("[benchmark] failed: distant terrain never became ready; no world timing result");
                                                elwt.exit();
                                            }
                                        }
                                        if ready_for_timing && benchmark_state.auto_started && !benchmark_state.measuring
                                            && benchmark_state.phase_started_at.elapsed().as_secs_f32()
                                                >= benchmark_state.config.warmup_seconds
                                            {
                                                benchmark_state.measuring = true;
                                                benchmark_state.phase_started_at = Instant::now();
                                                benchmark_state.measured_samples.clear();
                                                println!("[benchmark] warmup complete eye_voxels=({:.1}, {:.1}, {:.1}) yaw={:.2} pitch={:.2}", player.pos.x, player.pos.y, player.pos.z, player.yaw, player.pitch);
                                                world::adventure::arm_enemy_review();
                                            }

                                        if benchmark_state.measuring {
                                            benchmark_state.measured_samples.push(sample);
                                            if benchmark_state.config.resize_review {
                                                let phase = (benchmark_state.phase_started_at.elapsed().as_secs() / 8).min(3) as usize;
                                                if phase != benchmark_state.last_resize_phase {
                                                    benchmark_state.last_resize_phase = phase;
                                                    let sizes = [(benchmark_state.config.width, benchmark_state.config.height), (1279, 719), (1537, 865), (benchmark_state.config.width, benchmark_state.config.height)];
                                                    let (width, height) = sizes[phase];
                                                    if let Some(size) = window.request_inner_size(winit::dpi::PhysicalSize::new(width, height)) { gpu.resize(size); }
                                                    println!("[resize-review] phase={phase} requested={width}x{height}");
                                                }
                                            }
                                            // `--benchmark-capture-frames`: one PNG every 5 s of measuring,
                                            // into VOXEL_CAPTURE_DIR (default target/benchmark-frames).
                                            if benchmark_state.config.capture_frames && !terrain_review::option("VOXEL_HORIZON_ROUTE") {
                                                let shot = (benchmark_state.phase_started_at.elapsed().as_secs_f32() / benchmark_state.config.capture_interval) as usize;
                                                player::viewmodel::clips::REVIEW_FRAME.store(shot as u32, std::sync::atomic::Ordering::Relaxed);
                                                let root = std::env::var_os("VOXEL_CAPTURE_DIR")
                                                    .map(std::path::PathBuf::from)
                                                    .unwrap_or_else(|| "target/benchmark-frames".into());
                                                let path = root.join(format!("bench-{shot:03}.png"));
                                                if perf::capture_gate::needed(benchmark_state.last_periodic_capture, shot, || path.exists()) {
                                                    benchmark_state.last_periodic_capture = Some(shot);
                                                    gpu.request_capture(path);
                                                }
                                            }
                                            if (if terrain_review::option("VOXEL_HORIZON_ROUTE") { benchmark_state.horizon_route_started_at.map_or(0.0,|t|t.elapsed().as_secs_f32()) } else { benchmark_state.phase_started_at.elapsed().as_secs_f32() })
                                                >= benchmark_state.config.duration_seconds
                                            {
                                                let summary = snapshot_from_samples(
                                                    &benchmark_state.measured_samples,
                                                    gpu.adapter_summary(),
                                                    [gpu.size().width, gpu.size().height],
                                                    render_distance_settings(),
                                                );
                                                if let Some(timing) = benchmark_state.route_timing.as_mut() {
                                                    timing.finish();
                                                }
                                                log_benchmark_summary(&summary, &benchmark_state.config, gpu.vsync());
                                                if std::env::var_os("VOXEL_NIGHT_ROOF_STATS").is_some()
                                                    && let Some(world) = world.as_ref()
                                                {
                                                    world.adventure.report_night_roof_stats();
                                                }
                                                let globe = gpu.globe_stats();
                                                let upload_kb = summary.history.iter().map(|s| f64::from(s.cpu.globe_upload_bytes)).sum::<f64>() / 1024.0;
                                                println!("[benchmark] globe resident={} drawn={} wanted={} bake_ms={:.3} upload_kb={upload_kb:.1}", globe.resident, globe.drawn, globe.wanted, summary.cpu_avg.globe_bake_ms);
                                                println!("[benchmark] orbital_cloud bake_ms={:.3} pending={:.1} resolution={:.0} night_lights={}", summary.cpu_avg.orbital_cloud_bake_ms, summary.cpu_avg.orbital_cloud_pending, summary.cpu_avg.orbital_cloud_resolution, globe.night_lights);
                                                elwt.exit();
                                            }
                                        }
                                    }
                                }
                                Err(render::SurfaceFrameError::Lost | render::SurfaceFrameError::Outdated) => {
                                    egui_ui.end_frame_no_paint(window.as_ref());
                                    gpu.resize(gpu.size())
                                }
                                Err(render::SurfaceFrameError::Timeout) => {
                                    egui_ui.end_frame_no_paint(window.as_ref());
                                    gpu_retry_at = Some(Instant::now() + Duration::from_millis(1));
                                }
                                Err(render::SurfaceFrameError::Other) => {
                                    egui_ui.end_frame_no_paint(window.as_ref());
                                }
                            }
                        }

                        StartupState::WaitingForWindow => {}
                    },

                    _ => {}
                }
            }

            Event::AboutToWait => {
                if !benchmark_config.enabled
                    && (occluded || window.as_ref().is_some_and(|w| w.inner_size().width == 0 || w.inner_size().height == 0))
                    && Instant::now() >= next_retirement_pass
                {
                    if let StartupState::Ready { gpu, .. } = &mut startup {
                        gameplay_trace::frame_stage("idle-retirement");
                        gpu.flush_pending_frame_work();
                        chunk_cache.maintain_retired_meshes(gpu);
                        gameplay_trace::frame_stage("idle-retirement-done");
                    }
                    next_retirement_pass = Instant::now() + Duration::from_millis(100);
                }
                // A world hosted for friends keeps its own checkpoints and preferences.
                #[cfg(feature = "multiplayer")]
                let local_saves = multiplayer::coop::Mode::of(multiplayer.as_ref()).local_saves();
                #[cfg(not(feature = "multiplayer"))]
                let local_saves = !network_mode;
                if !benchmark_config.enabled {
                    if let Some(loaded)=library.poll(){pending_loaded=Some(loaded);}
                    if matches!(mode,AppMode::WorldSelect) && !exit_pending && Instant::now()>=next_library_refresh {
                        if !library.busy(){library.refresh_quietly();}
                        next_library_refresh=Instant::now()+Duration::from_secs(5);
                    }
                    if let Some(ui)=egui_ui.as_mut() {
                        if ui.library_revision!=library.revision {
                            ui.library_entries=std::sync::Arc::clone(&library.entries);
                            ui.library_revision=library.revision;
                        }
                        ui.library_busy=library.busy_visible();ui.library_loading=library.loading();
                        ui.library_status=library.status.clone();
                        if preferences_render_applied && local_saves && ca_window.is_none()
                            && let StartupState::Ready {gpu,..}=&startup {
                            preferences.tick(preferences::Preferences::capture(ui,gpu.vsync(),gpu.fxaa_enabled,atmosphere_mode,selected_world,selected_voxel_scale,
                                    windowed_size.0,windowed_size.1,window.as_ref().is_some_and(|w|w.fullscreen().is_some())),exit_pending);
                            }
                        ui.preferences_status=preferences.status.clone();
                    }
                }
                if !benchmark_config.enabled {
                    if exit_pending {
                        mode = AppMode::Paused;
                        input = Input::default();
                    }
                    // Maintenance also runs while minimized and while waiting for
                    // a final checkpoint; neither capture nor sync blocks the UI.
                    if let Some(w) = world.as_mut()
                        && ((occluded && !benchmark_config.enabled) || window.as_ref().is_some_and(|win|win.inner_size().width==0 || win.inner_size().height==0)) {
                            #[cfg(feature = "multiplayer")]
                            if !hell_gate.busy() && !hall_gate.busy() && let Some(session)=multiplayer.as_mut() {
                                let now=Instant::now();let elapsed=(now-last).as_secs_f32();last=now;
                                session.update(w,&mut player,&mut day_night,&Input::default(),elapsed,&mut dirty_blocks,&mut dirty_chunks);
                            }
                            let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                            let radius = altitude::resident_radius_chunks(
                                altitude::residency_altitude_m(w, &player, player.precise_pos.y * w.voxel_scale.meters() as f64),
                                render_distance_settings().view_distance_chunks);
                            w.stream_regions(center, radius);
                        }
                    // Review worlds are disposable and must not replace the
                    // player's autosave or introduce background save cost.
                    if let (Some(session), Some(w), StartupState::Ready { fluid_sim, .. }) =
                        (ca_window.as_mut(), world.as_mut(), &mut startup) {
                        if let Some(region) = fluid_sim.cpu_region.as_mut() { region.tool_slot = creative.selected_slot; }
                        session.tick(w, &player, &day_night, fluid_sim, &input);
                    }
                    if !benchmark_config.enabled && local_saves && ca_window.is_none() && !hell_gate.saving_destination() && !hall_gate.saving_destination() {
                        saves.tick(world.as_mut(), &player, &day_night);
                    }
                    if (hell_gate.busy() || hall_gate.busy()) && let Some(source) = world.as_mut() {
                        let prepare = |_destination: &mut World, _host: &Player, _arrival: Vec3| -> anyhow::Result<()> {
                            #[cfg(feature = "multiplayer")]
                            if let Some(session) = multiplayer.as_ref() {
                                realm_plan = Some(session.prepare_realm(_destination, _host, _arrival, &day_night)?);
                            }
                            Ok(())
                        };
                        let polled = if hell_gate.busy() { hell_gate.poll(source, &mut player, &day_night, &saves, prepare) } else { hall_gate.poll(source, &mut player, &day_night, &saves, prepare) };
                        match polled {
                            Ok(Some(mut destination)) => {
                                // A shaft back up from the Lightless is not the buried gate home.
                                let from_realm = source.world_type.realm().is_some();
                                #[cfg(feature = "multiplayer")]
                                if let (Some(session), Some(plan)) = (multiplayer.as_mut(), realm_plan.take()) {
                                    session.commit_realm(plan);
                                    destination.network_host = true;
                                }
                                destination.adventure.cancel_cast_input();
                                selected_world = destination.world_type;
                                if world::progression::thresholds::Milestone::arrival(&mut destination.progression, selected_world) {
                                    saves.request(true);
                                }
                                selected_seed = destination.seed;
                                selected_voxel_scale = destination.voxel_scale;
                                let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                if let StartupState::Ready { gpu, chunk_streamer, fluid_sim } = &mut startup {
                                    chunk_streamer.reset();
                                    chunk_cache.reset_with_snapshot(center, &destination.snapshot_near(center, render_distance_settings().view_distance_chunks));
                                    chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                    fluid_sim.reset_for_world(selected_voxel_scale);
                                    gpu.invalidate_cloud_history();
                                }
                                destination.expedition.say(match selected_world {
                                    WorldType::Nether => "The Nether",
                                    WorldType::Hall => "Hall of the Crimson King",
                                    WorldType::Manyfold => "Manyfold",
                                    WorldType::Realm(kind) => kind.title(),
                                    _ => "Returned",
                                });
                                if selected_world == WorldType::Hills && !from_realm && world::adventure::chronicle::grant(&mut destination, 13) {
                                    saves.request(true);
                                }
                                destination.adventure.sounds.push(audio::Event { cue: audio::Cue::Rift, pos: None, gain: 0.6, pitch: 0.75 });
                                world = Some(destination);
                                creative.cancel_edits();
                                creative.close_panels();
                                dirty_blocks.clear();
                                dirty_chunks.clear();
                                reset_gameplay_input(&mut input, &mut place_was_down, &mut break_was_down,
                                    &mut place_repeat_timer, &mut break_repeat_timer, &mut pending_tool_action);
                                cloud_weather.reset(selected_seed);
                                simulation_clock = simulation::SimulationClock::default();
                                last = Instant::now();
                                saves.status = Some("Crossing complete".into());
                            }
                            Ok(None) => { saves.status = Some("Crossing…".into()); }
                            Err(error) => {
                                let message = format!("Cannot cross: {error:#}");
                                #[cfg(feature = "multiplayer")]
                                { realm_plan = None; }
                                source.expedition.say(&message);
                                saves.status = Some(message);
                            }
                        }
                    }
                    if let Some(ui) = egui_ui.as_mut()
                        && let Some(status) = ca_window.as_ref().and_then(|s| s.status.as_ref()).or(saves.status.as_ref()) {ui.persistence_status=Some(status.clone());}
                    if exit_pending {
                        if ca_window.as_ref().map_or(saves.failed, |s| s.failed) {exit_pending=false;}
                        else if !hell_gate.busy() && !hall_gate.busy() && !ca_window.as_ref().map_or(saves.busy(), |s| s.busy()) && !preferences.busy() && !library.busy() {elwt.exit();return;}
                    }
                }
                if let Some(window) = window.as_ref() {
                    if (occluded && !benchmark_config.enabled) || window.inner_size().width == 0 || window.inner_size().height == 0 {
                        elwt.set_control_flow(ControlFlow::WaitUntil(Instant::now()+Duration::from_millis(100)));
                    } else if let Some(retry_at) = gpu_retry_at.filter(|&retry_at| Instant::now() < retry_at) {
                        elwt.set_control_flow(ControlFlow::WaitUntil(retry_at));
                    } else if !focused && !benchmark_config.enabled && Instant::now() < next_background_frame {
                        elwt.set_control_flow(ControlFlow::WaitUntil(next_background_frame));
                    } else if !benchmark_config.enabled && matches!(mode, AppMode::WorldSelect)
                        && Instant::now() < next_menu_frame {
                        elwt.set_control_flow(ControlFlow::WaitUntil(next_menu_frame));
                    } else {
                        if benchmark_config.enabled {
                            redraw_requested_at.get_or_insert_with(Instant::now);
                        }
                        window.request_redraw();
                        next_background_frame = Instant::now() + Duration::from_millis(if network_mode { 30 } else { 100 });
                        next_menu_frame = Instant::now() + Duration::from_secs_f64(1.0 / 60.0);
                    }
                }
            }

            _ => {}
        }
    })?;

    Ok(())
}
