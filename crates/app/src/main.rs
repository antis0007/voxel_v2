mod dungeon_review;
mod workshop_review;
use anyhow::Result;
use glam::{DVec2, IVec3, Mat4, Vec3, Vec3Swizzles};
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use winit::{
    dpi::PhysicalSize,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowAttributes},
};

use engine_audio as audio;
mod crash_log;
mod altitude;
mod comfort;
mod creative;
mod gamepad;
mod keybinds;
use keybinds::Action as BindAction;
mod screenshot;
mod egui_ui;
use engine_core::fast_hash;
mod fluid;
mod headless;
use engine_content as material;
mod material_studio;
mod mesh;
mod perf;
mod player;
mod preferences;
mod quality;
mod reference_environment;
mod render;
mod sandbox_review;
mod receiver_review;
mod shader_sources;
mod simulation;
mod soundscape;
mod tool_effects;
mod torch_review;
mod terrain_review;
mod horizon_review;
mod low_spec;
#[cfg(test)]
mod repo_layout;
#[cfg(test)]
mod shader_contract_tests;
mod wand_review;
mod viewmodel_review;
mod wand_appearance;
mod wand_style;
mod water_review;
mod world;

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
    RenderDebugView, TorchLight, build_draw_list, render_distance_settings,
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
}

impl BenchmarkMotionMode {
    fn label(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Orbit => "orbit",
            Self::CloudReview => "cloud-review",
            Self::CloudReviewFar => "cloud-review-far",
            Self::Ascent => "ascent",
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
            world_type: WorldType::Hills,
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
    cloud_review_phase: usize,
    last_capture_key: usize,
    last_periodic_capture: Option<usize>,
    last_resize_phase: usize,
    last_profile_second: usize,
    measured_samples: Vec<PerfSample>,
    route_timing: Option<perf::route::RouteTiming>,
    horizon_world_switches: u8,
    horizon_route_started_at: Option<Instant>,
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
fn review_configured_world(kind:WorldType,scale:VoxelScalePreset,seed:u32)->World {
    let mut world=World::with_seed(kind,scale,seed);
    let version=BENCHMARK_GENERATOR_VERSION.load(std::sync::atomic::Ordering::Relaxed);
    if version!=0 { world.generator_version=version; }
    world
}

fn parse_benchmark_config() -> BenchmarkConfig {
    let mut config = BenchmarkConfig::default();

    for arg in std::env::args().skip(1) {
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
        if arg == "--benchmark-physics" {
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
                "ridged" => WorldType::Ridged,
                _ => WorldType::Hills,
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
                _ => BenchmarkMotionMode::Static,
            };
        }
    }

    config
}

/// Height in metres of the water surface above a submerged eye (eye in voxel units).
fn water_surface_above_m(world: &World, eye: Vec3) -> f32 {
    let mut cell = eye.floor().as_ivec3();
    for _ in 0..256 {
        let above = cell + glam::IVec3::Y;
        if world.water_volume(above) == 0 {
            break;
        }
        cell = above;
    }
    let fill = world.water_fill(cell).clamp(0.0, 1.0);
    (cell.y as f32 + fill) * world.voxel_scale.meters()
}

fn benchmark_motion_active(benchmark: Option<&BenchmarkState>) -> bool {
    benchmark.is_some_and(|state| {
        state.auto_started
            && !state.config.tollkeeper_review
            && !state.config.ossuary_review
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

fn apply_benchmark_camera_motion(
    state: &BenchmarkState,
    player: &mut Player,
    terrain: world::WorldSampleConfig,
) {
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

const TORCH_LIGHT_RADIUS: f32 = 18.0;
// Warm lamplight keeps pale stone, timber and brass distinguishable; the
// visible flame retains its hotter core and orange edge in the material pass.
const TORCH_LIGHT_COLOR: [f32; 3] = [1.00, 0.64, 0.36];
const TORCH_LIGHT_STRENGTH: f32 = 4.40;
// Amber gills: a small warm pool under each clump, well below a torch.
const AMBER_LIGHT_RADIUS: f32 = 9.0;
const AMBER_LIGHT_COLOR: [f32; 3] = [1.00, 0.60, 0.26];
const AMBER_LIGHT_STRENGTH: f32 = 1.70;

fn held_torch_light(
    player: &Player,
    voxel_scale: VoxelScalePreset,
    world_scale: f32,
    torch: bool,
    lumen_lantern: bool,
) -> Option<TorchLight> {
    if !torch && !lumen_lantern {
        return None;
    }

    let eye = player.eye(voxel_scale);
    let forward = player.forward_dir();
    let right = player.right_dir();
    let inv_world_scale = world_scale.recip().max(1e-6);
    // At the drawn flame, so the light and the held torch agree.
    let flame = mesh::models::held_item::torch_flame_offset();
    Some(TorchLight {
        position: (eye
            + (forward * flame.z + right * flame.x + Vec3::Y * flame.y) * inv_world_scale)
            * world_scale,
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
    snapshot: &world::WorldSnapshot,
    cache: &ChunkCache,
    player_pos: Vec3,
    world_scale: f32,
) -> Vec<TorchLight> {
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
    let chunk_radius =
        ((max_radius_blocks.ceil() as i32) + world::CHUNK_SIZE - 1) / world::CHUNK_SIZE;
    // Edited chunks sit in the snapshot; generated terrain lives only in the render
    // cache. Both feed the light list, so dedup by cell.
    let mut sources: Vec<(IVec3, BlockId)> = Vec::new();
    for (coord, chunk) in snapshot.chunks.iter() {
        if (*coord - center_chunk).abs().max_element() > chunk_radius {
            continue;
        }
        sources.extend(chunk.torches.iter().map(|&pos| (pos, snapshot.get(pos))));
    }
    sources.extend(cache.torch_cells_within(center_chunk, chunk_radius));
    sources.sort_unstable_by_key(|(pos, _)| (pos.x, pos.y, pos.z));
    sources.dedup_by_key(|(pos, _)| *pos);
    // Amber gills grow in clumps: one light per 4 m cell, chosen from the
    // cell's own positions so the pick does not hop as the player moves.
    // `sources` is sorted by position, so the first gill seen in a cell wins.
    let cell = (4.0 / world_scale.max(1e-6)).max(1.0) as i32;
    let mut amber_cells = HashSet::new();
    sources.retain(|&(pos, block)| {
        block != BlockId::AmberCap || amber_cells.insert(pos.div_euclid(IVec3::splat(cell)))
    });
    let mut torches: Vec<(f32, IVec3, BlockId)> = Vec::new();
    for (pos, block) in sources {
        let dist_sq = (pos.as_vec3() + Vec3::splat(0.5) - player_pos).length_squared();
        if dist_sq <= max_radius_sq {
            torches.push((dist_sq, pos, block));
        }
    }
    torches.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| (a.1.x, a.1.y, a.1.z).cmp(&(b.1.x, b.1.y, b.1.z)))
    });
    // Fade out at the budget boundary as well as the distance boundary. Sources
    // exchanging the final slot therefore enter with zero intensity.
    let fade_end = torches
        .get(render::torch_light_budget())
        .map_or(max_distance, |t| t.0.sqrt() * world_scale)
        .min(max_distance);
    torches.truncate(render::torch_light_budget());
    torches
        .into_iter()
        .map(|(distance_sq, pos, block)| {
            let amber = block == BlockId::AmberCap;
            let light_offset = match block {
                BlockId::Torch => Vec3::new(0.50, 0.82, 0.50),
                BlockId::TorchWallNorth => Vec3::new(0.50, 0.74, 0.56),
                BlockId::TorchWallSouth => Vec3::new(0.50, 0.74, 0.44),
                BlockId::TorchWallWest => Vec3::new(0.56, 0.74, 0.50),
                BlockId::TorchWallEast => Vec3::new(0.44, 0.74, 0.50),
                _ => Vec3::new(0.50, 0.82, 0.50),
            };
            TorchLight {
                // Gills glow from their own voxel centre at every voxel scale;
                // the torch anchor and model offsets are metre-sized torch geometry.
                position: if amber {
                    (pos.as_vec3() + Vec3::splat(0.5)) * world_scale
                } else {
                    block.torch_point_meters(pos.as_vec3(), light_offset, world_scale)
                },
                radius: if amber { AMBER_LIGHT_RADIUS } else { TORCH_LIGHT_RADIUS },
                color: if amber { AMBER_LIGHT_COLOR } else { TORCH_LIGHT_COLOR },
                strength: {
                    let distance = distance_sq.sqrt() * world_scale;
                    let fade_start = (fade_end * 0.8).max(fade_end - 24.0);
                    let t = ((fade_end - distance) / (fade_end - fade_start).max(0.001))
                        .clamp(0.0, 1.0);
                    let peak = if amber { AMBER_LIGHT_STRENGTH } else { TORCH_LIGHT_STRENGTH };
                    peak * t * t * (3.0 - 2.0 * t)
                },
                moving: false,
            }
        })
        .collect()
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

fn resolve_placement_block(selected: BlockId, support_normal: glam::IVec3, forward: glam::IVec3) -> Option<BlockId> {
    if engine_content::shaped::is_shaped(selected) {
        engine_content::shaped::placed(selected, support_normal, forward)
    } else if engine_content::thin::is_climbable(selected) {
        engine_content::thin::attach(selected, support_normal)
    } else if selected.is_torch() {
        BlockId::torch_from_attachment(support_normal)
    } else {
        Some(selected)
    }
}

fn is_torch_supported(world: &World, pos: glam::IVec3, block: BlockId) -> bool {
    let Some(offset) = block
        .torch_support_offset()
        .or_else(|| engine_content::thin::support_offset(block))
    else {
        return true;
    };
    world.get(pos + offset).is_solid()
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
    // A fresh adventure starts with only the starter spells found.
    new_world.adventure.begin_journey();
    *player = Player::new();
    day_night.reset(DEFAULT_START_TIME_OF_DAY);
    day_night.set_day_length_seconds(DEFAULT_DAY_LENGTH_SECONDS);
    player.pos = new_world.suggested_spawn();
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
    if new_world.adventure.wand > 0 {
        new_world.expedition.say(
            "Lumen Twig in hand. Hold R to cast, T cycles spells, and E → Gear arranges the deck.",
        );
    }
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
            writeln!(output, "frame,total_ms,gameplay_ms,stream_ms,render_cpu_ms,gpu_supported,gpu_pass_ms,cloud_ms,cloud_composite_ms,terrain_ms,render_frame_id,render_config_id,gpu_sample_count,gpu_frame_id,gpu_config_id,acquire_ms,submit_ms,present_ms,gpu_shadow_ms,terrain_draws,terrain_triangles,shadow_draws,chunk_upload_bytes,worker_pending,cpu_resident_bytes,gpu_resident_bytes,horizon_ms,visible_missing,near_missing,globe_ms,globe_upload_bytes,globe_wanted")?;
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
                writeln!(output, ",{:.4},{:.4},{:.4},{:.4},{},{},{},{},{},{},{},{:.4},{},{},{:.4},{},{}",
                    gpu.surface_acquire_ms, gpu.submit_ms, gpu.present_ms, gpu.gpu_shadow_ms,
                    gpu.terrain_draw_calls, sample.counters.terrain_triangles, gpu.shadow_draw_calls,
                    uploaded_bytes, chunk.generating + chunk.meshing + chunk.pending_schedule,
                    chunk.cpu_resident_bytes, chunk.gpu_resident_bytes, cpu.horizon_ms,
                    sample.counters.visible_missing, sample.counters.near_missing,
                    cpu.globe_ms, cpu.globe_upload_bytes as usize, cpu.globe_wanted as usize)?;
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
            "[benchmark] gpu_avg_ms passes={:.3} shadow={:.3} terrain={:.3} indirect={:.3} raymarch={:.3} temporal={:.3} sky={:.3} composite={:.3} outline={:.3} ui={:.3}",
            snapshot.render_avg.gpu_pass_total_ms,
            snapshot.render_avg.gpu_shadow_ms,
            snapshot.render_avg.gpu_terrain_ms,
            snapshot.render_avg.gpu_indirect_ms,
            snapshot.render_avg.gpu_cloud_raymarch_ms,
            snapshot.render_avg.gpu_cloud_temporal_ms,
            snapshot.render_avg.gpu_sky_ms,
            snapshot.render_avg.gpu_cloud_composite_ms,
            snapshot.render_avg.gpu_outline_ms,
            snapshot.render_avg.gpu_ui_ms
        );
    }
    println!(
        "[benchmark] latest_scene visible_chunks={} gpu_meshes={} draw_chunks={} terrain_tris={} torches={} shadow_draws={} terrain_draws={} weather_rects={} weather_bytes={}",
        snapshot.counters_latest.visible_chunks,
        snapshot.counters_latest.visible_meshes,
        snapshot.counters_latest.draw_chunks,
        snapshot.counters_latest.terrain_triangles,
        snapshot.counters_latest.nearby_torches,
        snapshot.render_latest.shadow_draw_calls,
        snapshot.render_latest.terrain_draw_calls,
        snapshot.weather_latest.dirty_rects,
        snapshot.weather_latest.uploaded_bytes
    );
    println!(
        "[benchmark] latest_stream gen={} sched={} mesh_dispatch={} upload={} req={} gpu_gen={} cpu_mesh={} ready_chunk={} ready_mesh={} pending={}",
        snapshot.chunk_latest.terrain_generated,
        snapshot.chunk_latest.terrain_scheduled,
        snapshot.chunk_latest.mesh_dispatched,
        snapshot.chunk_latest.meshes_uploaded,
        snapshot.chunk_latest.requested,
        snapshot.chunk_latest.generating,
        snapshot.chunk_latest.meshing,
        snapshot.chunk_latest.ready_chunks,
        snapshot.chunk_latest.ready_meshes,
        snapshot.chunk_latest.pending_schedule
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

fn main() -> Result<()> {
    env_logger::init();
    crash_log::install();
    // Before any recipe read: the tables are built once on first use.
    world::inventory::datapack::install(&world::storage::save_directory());

    if let Some(path) = std::env::args().find_map(|arg| arg.strip_prefix("--capture-menu=").map(str::to_owned)) {
        return pollster::block_on(egui_ui::menu_capture::run(std::path::Path::new(&path)));
    }

    if let Some(path) = std::env::args().find_map(|arg| arg.strip_prefix("--headless=").map(str::to_owned)) {
        std::process::exit(headless::main(&path));
    }
    anyhow::ensure!(
        !std::env::args().any(|arg| arg.starts_with("--ca-cpu-region")),
        "--ca-cpu-region=x,y,z currently requires --headless=<command-document.ron>"
    );
    let benchmark_config = parse_benchmark_config();
    if let Some(value)=std::env::args().find_map(|arg|arg.strip_prefix("--benchmark-generator-version=").map(str::to_owned)) {
        let version=value.parse::<u32>()?;
        // The dormant planet solve is reviewable before it becomes the default.
        let reviewable = (1..=world::CURRENT_GENERATOR_VERSION).contains(&version) || version == engine_worldgen::geo::GEO_GENERATOR;
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
        for z in (-3072..=3072).step_by(192) {
            for x in (-3072..=3072).step_by(192) {
                let s = world::landscape::sample(
                    &cfg,
                    cfg.blocks_for_height(x as f32),
                    cfg.blocks_for_height(z as f32),
                );
                let distance = x * x + z * z;
                let entry = locations
                    .entry(s.biome.label())
                    .or_insert((i32::MAX, x, z, s));
                if distance < entry.0 {
                    *entry = (distance, x, z, s);
                }
            }
        }
        for (biome, (_, x, z, s)) in locations {
            println!(
                "[terrain-survey] {biome}: x={x} z={z} height_m={:.1} temperature={:.2} moisture={:.2}",
                s.height, s.temperature, s.moisture
            );
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
        .map(|count| count.get().saturating_sub(2).clamp(1, 8))
        .unwrap_or(1);

    let mut startup = StartupState::WaitingForWindow;
    let startup_started = Instant::now();
    let mut egui_ui: Option<EguiUi> = None;

    let mut selected_world = preferences.loaded.kind();
    let mut selected_voxel_scale = preferences.loaded.voxel_scale();
    let mut selected_seed = default_random_seed();
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
    let mut next_menu_frame = Instant::now();
    let mut mode = AppMode::WorldSelect;
    let mut saves = world::storage::SaveService::new();
    let mut library = world::storage::WorldLibrary::new(!benchmark_config.enabled);
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
    let mut ambience_smooth = [0.0f32; 4];
    let mut star_count = 0u32;

    let mut chunk_cache = ChunkCache::new(glam::IVec3::ZERO);
    chunk_cache.reset(glam::IVec3::ZERO);
    let mut dirty_blocks = HashSet::new();
    let mut dirty_chunks = HashSet::new();
    let mut physics_enabled = true;
    let mut fps_counter = FpsCounter::new();
    let mut altitude_state = altitude::TierState::new();
    let mut place_was_down = false;
    let mut break_was_down = false;
    let mut place_repeat_timer = 0.0f32;
    let mut break_repeat_timer = 0.0f32;
    let mut mining_progress = world::mining::Progress::default();
    let mut pending_tool_action: Option<egui_ui::ToolUseAction> = None;
    let mut perf_tracker = PerfTracker::default();
    let mut perf_snapshot = PerfSnapshot::default();
    let live_profile = std::env::var_os("VOXEL_LIVE_PROFILE").is_some();
    let mut last_live_profile = Instant::now();
    let mut benchmark_hitch_reports = 0u32;
    let mut sandbox_review = benchmark_config
        .sandbox_review
        .then(sandbox_review::SandboxReview::default);
    let mut wand_review = wand_review::Review::from_env(benchmark_config.enabled);
    let mut workshop_review = workshop_review::Review::from_env(benchmark_config.enabled);
    let mut receiver_review = receiver_review::Review::from_env(benchmark_config.enabled);
    let open_block_scene = std::env::args().any(|arg| arg == "--benchmark-water-block");
    let mut water_review = benchmark_config.water_review.then(|| {
        if open_block_scene {
            water_review::WaterReview::open_block()
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
        cloud_review_phase: usize::MAX,
        last_capture_key: usize::MAX,
        last_periodic_capture: None,
        last_resize_phase: usize::MAX,
        last_profile_second: 0,
        measured_samples: Vec::new(),
        route_timing: None,
        horizon_world_switches: 0,
        horizon_route_started_at: None,
    });

    let mut pads = gamepad::Gamepads::open();

    #[allow(deprecated)]
    event_loop.run(move |event, elwt| {
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
                if window.is_none() {
                    let created_window = Arc::new(
                        elwt.create_window(
                            WindowAttributes::default()
                                .with_title(if benchmark_config.enabled { "Voxel Workshop — automated review" } else { "Voxel Workshop" })
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

                // Keyboard and pad presses share this dispatch for discrete actions.
                macro_rules! bound_action {
                    ($action:expr, $fresh_press:expr, $escape:expr, $tab:expr) => { 'bound: {
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
                                        w.inventory.return_transient_to_pack();
                                        w.sync_from_inventory();
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
                            Some(BindAction::Interact) if fresh_press && grabbed && !creative.any_ui_open() => {
                                input.interact = true;
                            }
                            Some(BindAction::Lamp) if fresh_press && grabbed && !creative.any_ui_open() => {
                                if let Some(w) = world.as_mut() {
                                    w.expedition.toggle_lamp();
                                }
                            }
                            Some(BindAction::NextSpell) if fresh_press && grabbed && !creative.any_ui_open() => { input.cycle_spell=true; }
                            Some(BindAction::Inventory)
                                if fresh_press && (!tab || !creative.show_palette) => {
                                let open = !creative.show_palette;
                                if !open
                                    && let Some(w) = world.as_mut() {
                                        w.inventory.return_transient_to_pack();
                                        w.sync_from_inventory();
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
                                egui_ui::map::toggle_panel(&mut creative); input.clear(); player.jump_tap_timer = 0.0;
                                sync_creative_ui_cursor(window, &mut grabbed, focused, &mode, &creative);
                            }
                            Some(BindAction::Waypoint) if fresh_press && grabbed && !creative.any_ui_open() => {
                                if let Some(index) = egui_ui::map::add_waypoint_at_player()
                                    && let Some(ui) = egui_ui.as_mut() {
                                        ui.quality_notice = Some((format!("Waypoint {}", index + 1), 2.0));
                                    }
                            }
                            Some(BindAction::Minimap) if fresh_press => egui_ui::map::toggle_minimap(),
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
                    let text_editing = egui_ui.as_ref().is_some_and(|ui| ui.ctx.wants_keyboard_input());
                    for (button, pressed) in pads.poll(focused) {
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

                match event {
                    WindowEvent::CloseRequested => {
                        if !benchmark_config.enabled {
                            mode = AppMode::Paused;
                            release_mouse(window, &mut grabbed);
                            exit_pending = true;
                            if world.is_some(){saves.request(true);}
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
                            mode = AppMode::Paused;
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
                        // A pending rebind on the controls screen takes the next press, Escape included.
                        if let PhysicalKey::Code(code) = key.physical_key
                            && fresh_press && keybinds::offer(keybinds::Binding::Key(code)) {
                                return;
                            }
                        // Ctrl chords sit beside the bindings: Ctrl is sprint and Z a tool key.
                        let action = match key.physical_key {
                            PhysicalKey::Code(KeyCode::KeyZ) if input.ctrl_down && !input.shift_down() => Some(BindAction::Undo),
                            PhysicalKey::Code(KeyCode::KeyZ | KeyCode::KeyY) if input.ctrl_down => Some(BindAction::Redo),
                            PhysicalKey::Code(code) => keybinds::action_for_key(code),
                            PhysicalKey::Unidentified(_) => None,
                        };
                        // Releases always reach gameplay; presses belong to the active surface.
                        if let PhysicalKey::Code(code) = key.physical_key
                            && (!pressed || (matches!(mode, AppMode::Playing) && grabbed && !egui_consumed)) {
                                input.keyboard(code, pressed);
                            }
                        let text_editing = egui_ui.as_ref().is_some_and(|ui| ui.ctx.wants_keyboard_input());
                        if text_editing && !matches!(key.physical_key, PhysicalKey::Code(KeyCode::Escape)) {
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
                                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                    egui_ui.end_frame_no_paint(window.as_ref());
                                    pending.resize(pending.size());
                                }
                                Err(wgpu::SurfaceError::OutOfMemory) => {
                                    egui_ui.end_frame_no_paint(window.as_ref());
                                    eprintln!("[exit] surface out of memory; closing");
                                    elwt.exit();
                                    return;
                                }
                                Err(wgpu::SurfaceError::Timeout)
                                | Err(wgpu::SurfaceError::Other) => {
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
                            if !exit_pending
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
                            let now = Instant::now();
                            let raw_frame_dt = (now - last).as_secs_f32();
                            let display_frame_dt = raw_frame_dt.min(0.25);
                            last = now;
                            if !matches!(mode, AppMode::WorldSelect) {
                                fps_counter.tick(display_frame_dt);
                            }
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
                            let spawn_progress = chunk_cache.spawn_progress();
                            // Distant coverage is part of the loading transition too.
                            // Advancing physics behind that screen can change the
                            // snapshot being built and force another cold publication.
                            // The wait is capped (a 4-core laptop can take 20 s); later
                            // builds run beside physics as in play, reusing the cached base.
                            // Benchmarks keep the full wait so captures stay comparable.
                            let horizon_pending = world.is_some() && chunk_cache.spawn_ready
                                && !gpu.horizon_ready() && matches!(mode, AppMode::Playing);
                            let horizon_hold = quality::hold_for_horizon(horizon_pending);
                            let spawning = world.is_some()
                                && (!chunk_cache.spawn_ready || horizon_hold
                                    || (benchmark_config.enabled && !gpu.horizon_ready()))
                                && matches!(mode, AppMode::Playing);
                            let simulation_steps = simulation_clock.advance(
                                raw_frame_dt,
                                matches!(mode, AppMode::Playing) && !spawning,
                            );
                            let gameplay_dt = simulation_steps as f32 * simulation::STEP_SECONDS;

                            gpu.begin_frame_stats();
                            chunk_streamer.begin_frame_stats();

                            if let Some(benchmark_state) = benchmark.as_mut()
                                && !benchmark_state.auto_started && world.is_none() {
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
                                    physics_enabled = benchmark_state.config.profile_physics;
                                    if workshop_review.is_some() { physics_enabled = true; }
                                    freeze_time = benchmark_state.config.saved_review;
                                    if wand_review.is_some() {day_night.set_time_of_day(0.5);freeze_time=true;}
                                    if benchmark_state.config.capture_frames && !benchmark_state.config.capture_hud { egui_ui.show_hud = false; }
                                    if benchmark_state.config.sandbox_review {
                                        egui_ui.show_hud = false;
                                        day_night.set_time_of_day(0.5);
                                        freeze_time = true;
                                    }
                                    benchmark_state.auto_started = true;
                                    benchmark_state.measuring = false;
                                    benchmark_state.phase_started_at = Instant::now();
                                    benchmark_state.motion_started_at = Instant::now();
                                    benchmark_state.motion_anchor_blocks = player.pos;
                                    benchmark_state.measured_samples.clear();
                                    if terrain_review::option("VOXEL_HORIZON_ROUTE")
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
                                        start_world(selected_world,selected_voxel_scale,selected_seed,&mut world,&mut player,&mut day_night,&mut chunk_cache,chunk_streamer,fluid_sim,&mut dirty_blocks,&mut dirty_chunks,&mut mode,window,&mut grabbed,false,&mut last,&mut last_safe_player_pos);
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
                                if let (Some(w), Some(tier)) = (world.as_mut(), std::env::var("VOXEL_REVIEW_WAND").ok().and_then(|v| v.parse::<u32>().ok())) {
                                    w.adventure.wand = tier.min(3);
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
                                    && gpu.horizon_ready() && chunk_cache.fully_expanded && missing == 0
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
                                let phase = if mode == "survival" {
                                    (elapsed / 3.0) as usize
                                } else {
                                    (elapsed / 12.0) as usize
                                };
                                torch_review::motion_camera(&mut player, selected_voxel_scale.meters(), elapsed);
                                day_night.set_time_of_day(if mode == "survival" { 0.35 } else if mode == "factory" || mode == "wildlife" || (mode == "minerals" && phase == 0) { 0.5 } else if phase == 1 || !mode.is_empty() { 0.0 } else { 0.5 });
                                if mode == "factory" { egui_ui.show_hud = (elapsed / 3.0) as usize % 2 == 1; }
                                freeze_time = true;
                                if mode == "survival" {
                                    if let Some(w) = world.as_mut() {
                                        torch_review::apply_survival_phase(w, &mut player, &mut creative, &egui_ui.ctx, phase);
                                    }
                                } else if mode == "motion" {
                                    creative.show_palette = false;
                                    creative.assign_hotbar_slot(0, BlockId::Stone);
                                } else if !matches!(mode, "interactive" | "drag") || elapsed < 2.0 {
                                    creative.show_palette = matches!(mode, "interactive" | "drag") || (phase >= 2 && mode != "minerals");
                                    // VOXEL_REVIEW_PANEL=<page> picks the panel to capture (7 Production, 3 Machines).
                                    let review_page = std::env::var("VOXEL_REVIEW_PANEL").ok().and_then(|v| v.parse::<u32>().ok());
                                    if review_page.is_some() { creative.show_palette = true; }
                                    if let Ok(scale) = std::env::var("VOXEL_REVIEW_UI_SCALE").unwrap_or_default().parse::<f32>() { egui_ui.ui_scale = scale.clamp(0.75,2.5); }
                                    egui_ui.ctx.data_mut(|d| d.insert_temp(egui::Id::new("inventory_page"), review_page.unwrap_or(if mode.is_empty() && phase == 3 { 2u32 } else { 4u32 })));
                                }
                                let capture_phase = if mode.is_empty() { phase } else { (elapsed / 3.0) as usize };
                                if (mode.is_empty() && elapsed % 12.0 >= 9.0 || (mode == "survival" && elapsed % 3.0 >= 2.2) || (!mode.is_empty() && mode != "survival")) && review.last_capture_key != capture_phase {
                                    let root = std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from)
                                        .unwrap_or_else(|| "target/torch-review".into());
                                    gpu.request_capture(root.join(format!("{:?}-{capture_phase:03}.png", selected_voxel_scale)));
                                    if mode == "motion" {
                                        let snapshot = world.as_mut().unwrap().snapshot();
                                        let lights = collect_nearby_torches(&snapshot, &chunk_cache, player.pos, selected_voxel_scale.meters());
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
                                egui_ui.biome_label = if cfg.generator_version < 2 { "Legacy landscape" }
                                    else if cfg.world_type == WorldType::Superflat { "Building plains" }
                                    else { world::landscape::sample(&cfg, player.pos.x.floor() as i32, player.pos.z.floor() as i32).biome.label() };
                                if let Some(location)=world::sites::location(&cfg,player.pos) {egui_ui.biome_label=location;}
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
                            egui_ui.world_rules = world.as_ref().map_or_else(Default::default, |w| w.rules);
                            creative.set_locked(!egui_ui.world_rules.mode.creative_tools());
                            if let Some(w) = world.as_mut() {
                                let mut drops = std::mem::take(&mut w.inventory.dropped);
                                let scale = w.voxel_scale.meters();
                                for drop in &mut drops {
                                    let next = drop.pos - glam::Vec3::Y * (gameplay_dt * 2.5).min(0.1);
                                    if w.get(((next - glam::Vec3::Y * 0.12) / scale).floor().as_ivec3()).is_empty() { drop.pos = next; }
                                }
                                w.inventory.dropped = drops;
                            }
                            egui_ui.expedition = world.as_ref().map(|w|w.expedition.clone());
                            egui_ui.factory = world.as_ref().map(|w|world::factory::view(w,player.pos)).unwrap_or_default();
                            // The ledger includes a day of pass predictions. Do not
                            // build its presentation while playing or on other pages.
                            let orbital_open = creative.show_palette && egui_ui.ctx.data(|d|
                                d.get_temp::<u32>(egui::Id::new("inventory_page")) == Some(8));
                            if orbital_open {
                                egui_ui.orbital = world.as_ref().map(|w| {
                                    world::orbital::view(w, &day_night, &player)
                                }).unwrap_or_default();
                            }
                            egui_ui.adventure = world
                                .as_ref()
                                .map(|w| {
                                    let origin = player.pos * w.voxel_scale.meters();
                                    let mut view = w.adventure.view();
                                    view.workshop_available = w.generator_version >= 48 && w.world_type != WorldType::Superflat;
                                    view.quest =
                                        w.adventure.quest_marker(&w.sample_config(), origin);
                                    if world::orbital::receiver::focused(w, &player) {
                                        view.undercrown_focus = Some(world::interaction::Prompt {
                                            title: "Meridian ground receiver", action: "F  Open service ledger",
                                        });
                                    }
                                    view
                                })
                                .unwrap_or_default();
                            if let Some(w) = world.as_ref() {
                                egui_ui.survival = w.inventory.clone();
                                egui_ui.survival_eat_timer = w.inventory.eat_timer;
                                egui_ui.survival_armor_points = w.inventory.armor_points();
                                egui_ui::progression_hud::publish(&egui_ui.ctx, w.progression.view(&w.inventory));
                            }
                            egui_ui.region_error = world.as_ref().and_then(|w|w.regions.view.error()).or_else(||chunk_streamer.upload_error().map(str::to_owned));
                            let action = egui_ui.build_ui(
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
                            );
                            egui_ui::blueprints::draw(&egui_ui.ctx, &mut creative.show_blueprints, &mut blueprints, edit_history.depth());
                            if let Some(w) = world.as_mut() {
                                w.inventory = egui_ui.survival.clone();
                                w.sync_from_inventory();
                                let was_open = creative.block_page.is_some() || creative.trade_page.is_some();
                                let eye = player.eye(w.voxel_scale);
                                let view_proj = Mat4::perspective_rh(
                                    view_zoom.lens(egui_ui.fov_degrees, 0.0).to_radians(),
                                    gpu.aspect_ratio(),
                                    0.05,
                                    4096.0,
                                ) * Mat4::look_to_rh(eye, player.forward_dir(), Vec3::Y);
                                egui_ui::block_page::draw(&egui_ui.ctx, &mut creative.block_page, w, eye, view_proj);
                                egui_ui::trade_page::draw(&egui_ui.ctx, &mut creative.trade_page, w, eye * w.voxel_scale.meters());
                                let open = creative.block_page.is_some() || creative.trade_page.is_some();
                                if (open && grabbed) || (was_open && !open) {
                                    sync_creative_ui_cursor(window, &mut grabbed, focused, &mode, &creative);
                                }
                            }
                            let solving = world.as_ref().map(|w| w.sample_config()).filter(engine_worldgen::geo::active).map(|c| {
                                engine_worldgen::geo::prewarm(c.seed);
                                engine_worldgen::geo::entry(c.seed)
                            }).filter(|e| !e.ready());
                            if let Some(entry) = solving.filter(|_| spawning) {
                                egui_ui.build_planet_loading_ui(&entry);
                            } else if spawning {
                                egui_ui.build_loading_ui(&LoadingUiState {
                                    title: "Loading world".into(),
                                    progress_01: (spawn_progress.1 > 0).then(|| spawn_progress.0 as f32 / spawn_progress.1 as f32),
                                });
                            }
                            let mut cpu_stats = CpuFrameStats::default();
                            cpu_stats.ui_ms = ui_start.elapsed().as_secs_f32() * 1000.0;
                            player.selected_block = creative.selected_block();
                            day_night.set_time_of_day(time_ui.time_of_day);
                            day_night.set_day_length_seconds(time_ui.day_length_seconds);
                            day_night.set_cloud_controls(time_ui.cloud_controls);
                            freeze_time = time_ui.freeze_time;
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

                            match if exit_pending {UiAction::None} else {action} {
                                UiAction::None => {}
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
                                                w.sync_from_inventory();
                                                w.expedition.say("Stack thrown aside. Aim nearby and press F to retrieve.");
                                            }
                                        } else { w.expedition.say("Retrieve some dropped stacks first."); }
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
                                UiAction::SaveWorld => { saves.request(true); }
                                UiAction::OpenWorld(id)=>library.open(id),
                                UiAction::SetRules(rules)=>if let Some(w)=world.as_mut(){w.set_rules(rules);},
                                UiAction::RefreshWorlds=>library.refresh(),
                                UiAction::RenameWorld {id,name}=>library.rename(id,name),
                                UiAction::FavoriteWorld {id,favorite}=>library.favorite(id,favorite),
                                UiAction::StartWorld {
                                    name,
                                    reference,
                                    world_type,
                                    voxel_scale,
                                    seed,
                                    mode: game_mode,
                                } => {
                                    reset_gameplay_input(
                                        &mut input,
                                        &mut place_was_down,
                                        &mut break_was_down,
                                        &mut place_repeat_timer,
                                        &mut break_repeat_timer,
                                        &mut pending_tool_action,
                                    );
                                    selected_world = world_type;
                                    selected_voxel_scale = voxel_scale;
                                    selected_seed = seed;
                                    creative.cancel_edits();
                                    start_world(
                                        selected_world,
                                        selected_voxel_scale,
                                        selected_seed,
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
                                    if let Some(w)=world.as_mut(){w.set_rules(world::rules::Rules::new_world(game_mode));}
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
                                    if !benchmark_config.enabled {
                                        mode = AppMode::Paused;
                                        release_mouse(window, &mut grabbed);
                                        exit_pending = true;
                                        if world.is_some(){saves.request(true);}
                                    } else {elwt.exit();}
                                }
                            }

                            player.selected_block = creative.selected_block();

                            if benchmark_config.enabled && !world::adventure::enemy_review_manual() && wand_review.as_ref().is_none_or(|r| r.automatic) {
                                if grabbed { release_mouse(window, &mut grabbed); }
                            } else { sync_creative_ui_cursor(
                                window,
                                &mut grabbed,
                                focused,
                                &mode,
                                &creative,
                            ); }

                            combat_audio.set_active(matches!(mode,AppMode::Playing) && focused && !spawning);
                            let mut draw_list = Vec::new();
                            let mut visible_missing = (0, 0);
                            if spawning || creative.any_ui_open() || !matches!(mode, AppMode::Playing) {
                                outline.clear();
                                invert_outline.clear();
                            }
                            let mut nearby_torch_count = 0usize;
                            let render_world =
                                !matches!(mode, AppMode::WorldSelect) && world.is_some();
                            let paused_render = !render_world;

                            if matches!(mode, AppMode::Playing)
                                && let Some(world) = world.as_mut()
                            {
                                let gameplay_start = Instant::now();
                                creative.update_feedback(gameplay_dt);

                                let benchmark_camera_active = benchmark_motion_active(benchmark.as_ref());
                                let mut active_input = if benchmark_camera_active {
                                    Input::default()
                                } else {
                                    input.clone()
                                };
                                if creative.any_ui_open() || spawning || (!grabbed && !benchmark_camera_active) {
                                    active_input.clear();
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

                                let mut simulation_budget = None;
                                for _ in 0..simulation_steps {
                                if !fluid_sim.prepare_step() { break; }
                                let gameplay_dt = simulation::STEP_SECONDS;
                                outline.clear();
                                invert_outline.clear();
                                if creative.any_ui_open() || (!grabbed && !benchmark_camera_active) { active_input.clear(); }
                                if benchmark_camera_active
                                    && let Some(benchmark_state) = benchmark.as_ref() {
                                        apply_benchmark_camera_motion(
                                            benchmark_state,
                                            &mut player,
                                            world.sample_config(),
                                        );
                                        terrain_review::frame(&mut player, world, benchmark_state.motion_started_at.elapsed().as_secs_f32(), &mut dirty_blocks, &mut dirty_chunks);
                                        horizon_review::frame(&mut player, world, benchmark_state.horizon_route_started_at.map_or(0.0,|t|t.elapsed().as_secs_f32()), &mut dirty_blocks, &mut dirty_chunks);
                                    }
                                let (mut place_edit, mut break_edit, mut can_act) = (false, false, false);
                                let melee_consumed = simulation::fixed::gameplay(
                                    world, &mut player, &mut active_input, !benchmark_camera_active,
                                    |hook, world, player, active_input| {
                                        match hook {
                                            simulation::fixed::Hook::BeforePlayer => {
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
                                            review.input(player, active_input, world.voxel_scale.meters(), simulation::STEP_SECONDS);
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
                                    active_input.place_block || active_input.place_pressed,
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
                                can_act=(grabbed || workshop_review.is_some() || world::adventure::enemy_review_automated() || hart_review_cast || tollkeeper_review_cast || ossuary_review_auto || wand_review.as_ref().is_some_and(|r|r.automatic)) && !creative.any_ui_open() && !spawning
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
                                if melee_consumed { break_edit = false; }
                                if let Some(review) = workshop_review.as_mut() {
                                    review.after(world, &mut player, &mut creative, &mut day_night, &mut saves,
                                        &mut dirty_blocks, &mut dirty_chunks);
                                    if review.reloaded {
                                        review.reloaded = false;
                                        fluid_sim.reset_for_world(world.voxel_scale);
                                        chunk_streamer.reset();
                                        let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                        chunk_cache.reset_with_snapshot(center, &world.snapshot_near(center, 4));
                                        chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                    }
                                    if let Some(path) = review.capture() { gpu.request_capture(path); }
                                }
                                input.cast_pressed=false;
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
                                let hit = world::raycast_block_precise(
                                    world,
                                    player.precise_eye(world.voxel_scale) + forward.as_dvec3() * 0.0005,
                                    forward,
                                    creative.reach_distance_blocks(world.voxel_scale),
                                );
                                if input.pick_block {
                                    if grabbed && !creative.any_ui_open()
                                        && let Some((cell, _)) = hit {
                                            let target = world.get(cell);
                                            let survival_hand = creative.tool == creative::ToolKind::Hand
                                                && !player.flying
                                                && !creative.show_panel;
                                            if survival_hand {
                                                if let Some(canon) = world::inventory::Item::canonical_block(target) {
                                                    let item = world::inventory::Item::Block(canon);
                                                    if let Some(slot) = world
                                                        .inventory
                                                        .hotbar
                                                        .iter()
                                                        .position(|s| s.is_some_and(|st| st.item == item))
                                                    {
                                                        creative.select_slot(slot);
                                                        player.selected_block = canon;
                                                    } else if let Some(pack_idx) = world
                                                        .inventory
                                                        .pack
                                                        .iter()
                                                        .position(|s| s.is_some_and(|st| st.item == item))
                                                    {
                                                        world.inventory.swap_slot_with_hotbar(
                                                            world::inventory::SlotAddr::Pack(pack_idx),
                                                            creative.selected_slot,
                                                        );
                                                        player.selected_block = canon;
                                                    }
                                                }
                                            } else {
                                                player.selected_block = creative.pick_block(target);
                                            }
                                        }
                                    input.pick_block = false;
                                }
                                egui_ui.target_block = hit.map(|(cell, _)| world.get(cell));
                                egui_ui.target_environment = hit.and_then(|(cell, _)|
                                    world::environment::inspect::at(world, cell));
                                egui_ui.environment_lag = world.environment.lag_seconds();
                                egui_ui.target_machine =
                                    hit.and_then(|(cell, _)| world::factory::probe(world, cell));
                                egui_ui.target_cell_temp_c = hit.and_then(|(cell, _)| {
                                    let t = world.sandbox.cell_for(cell, world.get(cell)).temperature;
                                    t.is_finite().then_some(t)
                                });
                                egui_ui.target_volume_ul = hit.map(|(cell, _)| {
                                    if world.get(cell) == BlockId::Water {
                                        world.water_microlitres(cell)
                                    } else {
                                        engine_core::environment::cell_volume(world.voxel_scale)
                                    }
                                });
                                // A bounded second probe distinguishes empty sky from an unreachable face.
                                let sight = hit.or_else(|| world::raycast_block_precise(
                                    world, player.precise_eye(world.voxel_scale) + forward.as_dvec3() * 0.0005,
                                    forward, 24.0 / world.voxel_scale.meters(),
                                ));
                                egui_ui.tool_feedback.distance_m = sight.map(|(cell, previous)| {
                                    let normal = previous - cell;
                                    let origin = player.precise_eye(world.voxel_scale);
                                    (0..3).find(|&axis| normal[axis] != 0 && forward[axis].abs() > 1e-6)
                                        .map(|axis| {
                                            let face = cell[axis] as f64 + if normal[axis] > 0 { 1.0 } else { 0.0 };
                                            (((face - origin[axis]) / forward[axis] as f64).max(0.0)
                                                * world.voxel_scale.meters() as f64) as f32
                                        }).unwrap_or(0.0)
                                });
                                egui_ui.tool_feedback.out_of_range = hit.is_none() && sight.is_some();
                                egui_ui.tool_feedback.air_placement = hit.is_none() && active_input.alt_down
                                    && creative.tool == creative::ToolKind::Hand;
                                // Right-click opens a written block or chest; a held torch lights TNT.
                                if place_edit && can_act && !active_input.sneak && creative.tool == creative::ToolKind::Hand
                                    && let Some((cell, _)) = hit {
                                    let target = world.get(cell);
                                    let torch = world.inventory.hotbar[creative.selected_slot].map(|s| s.item)
                                        == Some(world::inventory::Item::Block(BlockId::Torch))
                                        || creative.selected_block() == BlockId::Torch;
                                    if world::block_data::opens_page(target) {
                                        creative.close_panels();
                                        creative.block_page = Some(cell);
                                        input.clear();
                                        place_edit = false;
                                        break_edit = false;
                                        release_mouse(window, &mut grabbed);
                                    } else if target == BlockId::Tnt && torch && world::block_data::tnt::ignite(world, cell) {
                                        place_edit = false;
                                    }
                                }
                                if input.interact {
                                    if grabbed && !creative.any_ui_open() && !spawning {
                                        if active_input.sneak
                                            && let Some((p, side)) = hit.and_then(|(cell, front)| world::factory::sides::side_toward(world, cell, front))
                                            && world::factory::action(world, player.pos, world::factory::Action::ToggleSide(p, side)).is_none() {
                                            break_edit=false;place_edit=false;
                                        } else if hit.is_some_and(|(cell,_)| world::fittings::interact(world,cell,&player,&mut dirty_blocks,&mut dirty_chunks)) {
                                            break_edit=false;place_edit=false;
                                        } else if world.inventory.retrieve_nearby(player.pos * world.voxel_scale.meters()) {
                                            world.sync_from_inventory();
                                            break_edit=false;place_edit=false;
                                        } else if world::orbital::receiver::focused(world, &player) {
                                            creative.show_palette = true;
                                            egui_ui.ctx.data_mut(|d| d.insert_temp(egui::Id::new("inventory_page"), 8u32));
                                            break_edit=false;place_edit=false;
                                        } else if world::adventure::reliquaries::interact(world,&player)
                                            || hit.is_some_and(|(cell, _)| world::environment::inspect::clear(world, cell))
                                            || hit.is_some_and(|(cell, _)| world::environment::cultivation::compost(world, cell, creative.selected_slot)) {
                                            break_edit=false;place_edit=false;
                                        } else {
                                            let outcome = world::undercrown::interact(world, &player);
                                            if outcome.used {
                                                for &cell in &outcome.changed {
                                                    dirty_blocks.insert(cell);
                                                    mark_chunk_and_neighbors_dirty(&mut dirty_chunks, cell);
                                                }
                                                if let Some(message) = outcome.message {
                                                    world.expedition.say(message);
                                                }
                                                break_edit=false;place_edit=false;
                                            } else if let Some((cell,_))=hit {
                                            if (cell.as_vec3()-eye).length()*world.voxel_scale.meters()<=6.0 {
                                                let target = world.get(cell);
                                                if world::block_data::opens_page(target) {
                                                    creative.close_panels();
                                                    creative.block_page = Some(cell);
                                                    input.clear();
                                                    release_mouse(window, &mut grabbed);
                                                } else if target == BlockId::Bed {
                                                    let feet = (cell.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * world.voxel_scale.meters();
                                                    let message = match world::bed::sleep(&mut day_night, &mut world.adventure.respawn_point, feet) {
                                                        Ok(true) => "Slept through the night. Morning came the next day.",
                                                        Ok(false) => "Slept until dawn.",
                                                        Err(_) => "Beds take sleepers after sunset.",
                                                    };
                                                    world.expedition.say(message);
                                                } else {
                                                    world::expedition::interact(world,cell,player.pos,&mut dirty_blocks,&mut dirty_chunks);
                                                }
                                                let inventory_page = if world::factory::Kind::from_block(target).is_some() {
                                                    Some(3u32)
                                                } else if target == BlockId::Workbench { Some(0u32) } else { None };
                                                if let Some(page) = inventory_page {
                                                    egui_ui.ctx.data_mut(|d| d.insert_temp(egui::Id::new("inventory_page"), page));
                                                    creative.close_panels();
                                                    creative.show_palette = true;
                                                    input.clear();
                                                    place_edit = false;
                                                    break_edit = false;
                                                    release_mouse(window, &mut grabbed);
                                                }
                                            } else {
                                                world.expedition.say("Move within 6 metres to use or collect this.");
                                            }
                                        }
                                        }
                                    }
                                    input.interact=false;
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
                                let raycast_target = hit.map_or_else(
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
                                );

                                let mut preview = creative::get_preview(
                                    &creative,
                                    world,
                                    raycast_target,
                                    active_input.place_block,
                                    active_input.break_block,
                                    active_input.alt_down,
                                    false,
                                );
                                egui_ui.tool_feedback.material_budget = None;
                                if creative.tool == creative::ToolKind::Hand && !player.flying {
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

                                let lines = if egui_ui.show_hud && !creative.any_ui_open() {
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
                                if egui_ui.show_hud && !creative.any_ui_open()
                                    && egui_ui.tool_feedback.out_of_range
                                    && !egui_ui.tool_feedback.air_placement
                                    && let Some((cell, _)) = sight {
                                        outline.extend(build_outline_vertices(cell, [1.0, 0.68, 0.22, 0.85], 0.003));
                                    }

                                if let Some(center) = preview.center_block.filter(|_|egui_ui.show_hud && !creative.any_ui_open()) {
                                    let center_lines =
                                        build_outline_vertices(center, preview.color, 0.003/world.voxel_scale.meters());
                                    outline.extend(center_lines);
                                }
                                if egui_ui.show_hud && !creative.any_ui_open() {
                                    blueprints.overlay(creative.tool, eye, 0.003/world.voxel_scale.meters(), &mut outline);
                                }
                                // Pastes share the tool edit path, so they wait for residency like a brush stroke.
                                let mut sink = world::blueprint::Sink {
                                    prepare: &mut |w: &mut World, p| fluid_sim.prepare_tool_edit(w, p),
                                    blocks: &mut dirty_blocks, chunks: &mut dirty_chunks,
                                };
                                blueprints.frame(world, &mut creative, raycast_target, false, &mut edit_history, &mut sink,
                                    simulation::Budget::wall(std::time::Duration::from_millis(3)));
                                if let Some(message) = edit_history.step(world, &mut sink, simulation::Budget::wall(std::time::Duration::from_millis(3)), usize::MAX) {
                                    world.expedition.say(message);
                                }

                                if place_edit && can_act && !active_input.sneak
                                    && creative.tool == creative::ToolKind::Hand
                                    && hit.is_some_and(|(cell,_)|world::fittings::interact(world,cell,&player,&mut dirty_blocks,&mut dirty_chunks)) {
                                    place_edit=false;
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
                                if place_edit && can_act
                                    && creative.tool == creative::ToolKind::Hand
                                    && world::farming::interact(world,hit,eye*world.voxel_scale.meters(),forward,creative.selected_slot,&mut dirty_blocks,&mut dirty_chunks) {
                                    place_edit=false;
                                }
                                let survival_hand = creative.tool == creative::ToolKind::Hand
                                    && !player.flying
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
                                                place_edit = false;
                                            }
                                        }
                                    }
                                }
                                // Adventure keeps use and combat but never edits blocks.
                                let edits_blocks = world.rules.mode.edits_blocks();
                                if !edits_blocks { place_edit = false; }
                                // Creative worlds keep the instant break; Survival mines over time.
                                let mining_tool = (survival_hand && creative.locked() && edits_blocks).then(|| world::mining::tool_for(
                                    world.inventory.hotbar[creative.selected_slot].map(|s| s.item),
                                    world.expedition.tool_tier,
                                ));
                                egui_ui.tool_feedback.break_progress = None;
                                egui_ui.tool_feedback.break_requires = None;
                                if let Some(tool) = mining_tool.filter(|_| !active_input.alt_down) {
                                    let target = hit.map(|(cell, _)| (cell, world.get(cell)));
                                    let held = (active_input.break_block || active_input.break_pressed)
                                        && can_act && !melee_consumed;
                                    let mined = target.map(|(_, block)| world::mining::survival_break(block, tool));
                                    break_edit = mining_progress.tick(target, held, mined.map_or(0.0, |m| m.seconds), gameplay_dt);
                                    egui_ui.tool_feedback.break_progress = mining_progress.fraction();
                                    egui_ui.tool_feedback.break_requires = mined.filter(|m| !m.drops).map(|m| m.required_tier);
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
                                                engine_content::shaped::horizontal_axis(player.forward_dir()))
                                        })
                                        .or(Some(active_block))
                                    };
                                    if let Some(block)=placement_block {creative.queue_edit(world,edit_mode,block,targets);}
                                }
                                // A committed stroke is applied over bounded frames; preview
                                // sampling never truncates edits or turns a sphere into a slice.
                                while let Some(cell) = world.adventure.dirty_voxels.pop() {
                                    dirty_blocks.insert(cell);
                                    mark_chunk_and_neighbors_dirty(&mut dirty_chunks, cell);
                                }
                                if can_act {
                                    if creative.pending_cells()==0
                                        && let Some((point,radius))=world.adventure.take_terrain_blast() {
                                            let cells=creative::spell_blast_targets(world.voxel_scale.meters(),point,radius);
                                            creative.queue_spell_blast(world,cells);
                                        }
                                    let edit_started=Instant::now();
                                    let explosive_edit=creative.pending_explosive();
                                    let spell_blast=creative.pending_spell_blast();
                                    let mut emitted_tool_burst = false;
                                    let cfg=world.sample_config();
                                    let (body_min,body_max)=world::player_aabb(player.pos,world.voxel_scale,player.scale,player.crouch_t);
                                    for _ in 0..if explosive_edit {8192}else{1024} {
                                        if edit_started.elapsed()>=std::time::Duration::from_millis(if explosive_edit {4}else{2}) {break;}
                                        let Some(next)=creative.next_edit_position(cfg) else {break;};
                                        if creative.pending_mode()==Some(creative::BrushMode::Erase) && world.get(next)==BlockId::Air {
                                            creative.next_edit(cfg);continue;
                                        }
                                        if !fluid_sim.prepare_tool_edit(world,next) {break;}
                                        let Some((edit_mode,cell,queued_block))=creative.next_edit(cfg) else {break;};
                                        if edit_mode==creative::BrushMode::Place && creative::block_overlaps_aabb(cell,body_min,body_max){continue;}
                                        let placement_block=Some(queued_block);
                                        let previous_block = world.get(cell);
                                        match edit_mode {
                                            creative::BrushMode::Erase => {
                                                if world.get(cell) == BlockId::Air {
                                                    continue;
                                                }
                                                if spell_blast {world::expedition::spell_blast(world,cell,&mut dirty_blocks,&mut dirty_chunks);}
                                                else if explosive_edit {world::expedition::blast_harvest(world,cell,&mut dirty_blocks,&mut dirty_chunks);}
                                                else {world::mining::harvest(world,cell,mining_tool,&mut dirty_blocks,&mut dirty_chunks);}
                                                dirty_blocks.insert(cell);
                                                mark_chunk_and_neighbors_dirty(
                                                    &mut dirty_chunks,
                                                    cell,
                                                );
                                                // Recorded after the break, so undo puts torches back before their support.
                                                if !creative.locked() && !spell_blast { edit_history.record(world, cell, previous_block, world.get(cell)); }
                                                cleanup_detached_torches(
                                                    world,
                                                    cell,
                                                    &mut dirty_blocks,
                                                    &mut dirty_chunks,
                                                    (!creative.locked() && !spell_blast).then_some(&mut edit_history),
                                                );
                                            }
                                            creative::BrushMode::Place => {
                                                let Some(place_block) = placement_block else {
                                                    continue;
                                                };
                                                let held = if survival_hand { world.inventory.hotbar[creative.selected_slot].map(|s|s.item) } else { None };
                                                if world::fittings::place(world,cell,place_block,&player,survival_hand.then_some(creative.selected_slot),&mut dirty_blocks,&mut dirty_chunks) { continue; }
                                                if world::factory::place(world,cell,place_block,&player,held,&mut dirty_blocks,&mut dirty_chunks) {
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
                                                    &mut dirty_blocks, &mut dirty_chunks).is_err() {
                                                    world.expedition.say("Placement blocked");
                                                    continue;
                                                }
                                                if engine_content::shaped::text_limits(place_block).is_some() {
                                                    creative.block_page = Some(cell);
                                                }
                                            }
                                        }
                                        if edit_mode == creative::BrushMode::Place && world.get(cell) != previous_block && !creative.locked() {
                                            edit_history.record(world, cell, previous_block, world.get(cell));
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
                                            if std::env::var_os("VOXEL_TOOL_REVIEW").is_some() { eprintln!("[tool-review] committed tool={:?} mode={:?} cell={:?} before={:?} after={:?}",creative.tool,edit_mode,cell,previous_block,world.get(cell)); }
                                            emitted_tool_burst = true;
                                        }
                                                                        }
                                    // A stroke is queued whole, so an empty queue closes one undo step.
                                    if creative.pending_cells() == 0 { edit_history.seal(); }
                                    if explosive_edit && !spell_blast {world.sync_from_inventory();}
                                }
                                if let Some(review)=sandbox_review.as_ref(){fluid_sim.set_enabled(review.running());}
                                simulation::fixed::finish(
                                    fluid_sim, world, &player, &mut day_night,
                                    &mut dirty_blocks, &mut dirty_chunks,
                                    *simulation_budget.get_or_insert_with(|| simulation::WorldSimulation::live_budget(world)),
                                    false, !freeze_time,
                                );
                                if let Some(review)=wand_review.as_mut()
                                    && let Some(shot)=review.observe(world,&player,&creative,&preview) {
                                        let root=std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(||"target/wand-review".into());
                                        gpu.request_capture(root.join(shot));
                                    }
                                // Drain after all producers, including tools. A tool blast used
                                // to enqueue after the drain and lose its sound on the next tick.
                                for sound in std::mem::take(&mut world.adventure.sounds) {
                                    egui_ui::subtitles::note(&sound,player.eye(world.voxel_scale)*world.voxel_scale.meters(),player.forward_dir());
                                    combat_audio.play(sound,player.eye(world.voxel_scale)*world.voxel_scale.meters(),player.forward_dir());
                                }
                                soundscape.tick(&mut combat_audio, world, &player, &day_night, display_frame_dt);
                                if let Some(review)=sandbox_review.as_mut(){
                                    let (reloaded,shot)=review.tick(world,&mut player,&day_night,fluid_sim,&mut dirty_blocks,&mut dirty_chunks);
                                    if reloaded {chunk_streamer.reset();let c=chunk_coord_from_world_block(player.pos.floor().as_ivec3());chunk_cache.reset_with_snapshot(c,&world.snapshot_near(c,4));chunk_streamer.queue_needed_visible(&chunk_cache,c);}
                                    if let Some(shot)=shot {let root=std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(||"target/sandbox-review".into());gpu.request_capture(root.join(format!("{shot}.png")));}
                                }
                                if let Some(review)=receiver_review.as_mut() {
                                    let (reloaded,shot)=review.frame(world,&mut player,&mut day_night,&mut saves,&mut creative,&mut dirty_blocks,&mut dirty_chunks);
                                    if reloaded {chunk_streamer.reset();let c=chunk_coord_from_world_block(player.pos.floor().as_ivec3());chunk_cache.reset_with_snapshot(c,&world.snapshot_near(c,4));chunk_streamer.queue_needed_visible(&chunk_cache,c);}
                                    if let Some(shot)=shot&& let Some(root)=std::env::var_os("VOXEL_CAPTURE_DIR") {gpu.request_capture(std::path::PathBuf::from(root).join(shot));}
                                }
                                }
                                let eye = player.eye(world.voxel_scale);
                                let forward = player.forward_dir();
                                cpu_stats.gameplay_ms =
                                    gameplay_start.elapsed().as_secs_f32() * 1000.0;

                                let center_chunk =
                                    chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                let snapshot_start = Instant::now();
                                world.set_visual_region_interest(&chunk_cache.edit_proxy_source_coords());
                                chunk_cache.set_horizon_pins(gpu.horizon_retention_columns());
                                // Edits inside the near window already invalidate the near snapshot, so
                                // only dirt outside it needs the full (whole-world clone) snapshot.
                                let view_chunks = render_distance_settings().view_distance_chunks;
                                let dirt_in_near_window = dirty_chunks.iter().all(|c| {
                                    c.x.abs_diff(center_chunk.x).max(c.z.abs_diff(center_chunk.z)) <= (view_chunks + 1) as u32
                                });
                                let snapshot = if chunk_cache.has_horizon_pins() || !dirt_in_near_window { world.stream_regions(center_chunk, view_chunks); world.snapshot() } else { world.snapshot_near(center_chunk,view_chunks) };
                                cpu_stats.snapshot_ms =
                                    snapshot_start.elapsed().as_secs_f32() * 1000.0;
                                let precise_eye_m = player.precise_eye(world.voxel_scale) * world.voxel_scale.meters() as f64;
                                let altitude_m = altitude::altitude_m(precise_eye_m, altitude::planet_world(world));
                                let tier = altitude_state.update(altitude_m);
                                gpu.set_tier(tier);
                                let orbit = tier == altitude::Tier::Orbit;
                                chunk_cache.freeze(orbit);
                                let streaming_start = Instant::now();
                                let center_changed = chunk_cache.update_center_chunk_with_snapshot_radius(
                                    center_chunk,
                                    &snapshot,
                                    altitude::resident_radius_chunks(altitude_m, render_distance_settings().view_distance_chunks),
                                );
                                if center_changed {
                                    chunk_cache.retain_visible();
                                    if chunk_cache.has_horizon_pins() { chunk_streamer.retain_visible(&chunk_cache.visible_keys); } else { chunk_streamer.retain_within_range(center_chunk); }
                                    chunk_streamer.queue_needed_visible(&chunk_cache, center_chunk);
                                }

                                chunk_streamer.invalidate_changed_sources(&snapshot,&mut chunk_cache);
                                let proxy_keys=chunk_cache.update_edit_proxies(&snapshot,&dirty_chunks,world.revision(),gpu.horizon_published_revision());
                                if let Some(revision)=gpu.horizon_published_revision() {
                                    let baked=gpu.horizon_baked_columns(&chunk_cache.edit_proxy_columns());
                                    chunk_cache.retire_edit_proxies(&baked,revision);
                                }
                                if !proxy_keys.is_empty() { chunk_streamer.push_pending(proxy_keys,chunk_cache.center_chunk); }
                                let new_keys = if orbit { Vec::new() } else { chunk_cache
                                    .expand_visible(&snapshot, chunk_streamer.queued_jobs()) };
                                if !new_keys.is_empty() {
                                    chunk_streamer.push_pending(new_keys, chunk_cache.center_chunk);

                                }

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


                                if !orbit {
                                let quality_eye=player.precise_eye(world.voxel_scale).as_vec3()*world.voxel_scale.meters();
                                let wanted_quality=gpu.horizon_quality_columns(snapshot.cfg,quality_eye,player.forward_dir(),egui_ui.fov_degrees);
                                let previous_quality=chunk_cache.quality_columns();
                                let quality_parent_valid=gpu.horizon_quality_parent_valid(&snapshot,quality_eye,&previous_quality);
                                let quality_keys=chunk_cache.set_quality_columns(&snapshot,&wanted_quality,quality_parent_valid,chunk_streamer);
                                if !quality_keys.is_empty(){chunk_streamer.push_pending(quality_keys,chunk_cache.center_chunk);}
                                chunk_streamer.enqueue_missing(
                                    gpu,
                                    &snapshot,
                                    world.revision(),
                                    &mut chunk_cache,
                                );
                                }
                                for (coord,mask) in chunk_cache.take_simulation_seeds() {fluid_sim.seed(world,player.pos.floor().as_ivec3(),coord,mask);}
                                let resident_upload_start = Instant::now();
                                chunk_streamer.upload_finished(
                                    gpu,
                                    &snapshot,
                                    &mut chunk_cache,
                                    world.revision(),
                                    4,
                                );
                                cpu_stats.resident_upload_wall_ms =
                                    resident_upload_start.elapsed().as_secs_f32() * 1000.0;
                                cpu_stats.streaming_ms =
                                    streaming_start.elapsed().as_secs_f32() * 1000.0;

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
                                gpu.update_horizon(world, render_eye, chunk_cache.strict_coverage_meters(render_eye,render_settings.view_distance_chunks,world_scale), chunk_cache.resident_coverage(render_eye,render_settings.view_distance_chunks,world_scale), chunk_cache.desired_resident_coverage(render_eye,render_settings.view_distance_chunks,world_scale),&chunk_cache,egui_ui.fov_degrees);
                                cpu_stats.horizon_ms = horizon_start.elapsed().as_secs_f32() * 1000.0;
                                let cull_view = Mat4::look_to_rh(eye, forward, Vec3::Y);
                                // Rendering includes true orbital distances. The GPU
                                // converts this conventional infinite projection to
                                // reverse-Z; terrain culling keeps its finite budget.
                                let proj = Mat4::perspective_infinite_rh(
                                    view_zoom.lens(egui_ui.fov_degrees, 0.0).to_radians(),
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
                                let cull_proj = Mat4::perspective_rh(
                                    view_zoom.lens(egui_ui.fov_degrees, 0.0).to_radians(),
                                    gpu.aspect_ratio(),
                                    0.05,
                                    far_plane_voxels,
                                );
                                let culling_start = Instant::now();
                                draw_list = build_draw_list(&chunk_cache, cull_proj * cull_view);
                                cpu_stats.culling_ms =
                                    culling_start.elapsed().as_secs_f32() * 1000.0;
                                if benchmark.is_some() { visible_missing = chunk_cache.missing_in_view(cull_proj * cull_view); }
                                let held_torch =
                                    held_torch_light(&player, world.voxel_scale, world_scale, mesh::models::held_item::holds_torch(world, &player), world.expedition.lamp_enabled());
                                let torch_start = Instant::now();
                                let mut nearby_torches =
                                    collect_nearby_torches(&snapshot, &chunk_cache, player.pos, world_scale);
                                append_fire_lights(world, player.pos, world_scale, &mut nearby_torches);
                                for blast in world.adventure.blasts.iter().filter(|b| b.age<0.28).take(4) {
                                    if nearby_torches.len()>=MAX_TORCH_LIGHTS {nearby_torches.pop();}
                                    nearby_torches.insert(0,TorchLight {
                                        position:blast.pos+blast.normal*0.3,
                                        radius:blast.radius*2.0+2.0,color:[1.0,0.64,0.25],
                                        strength:5.0*(1.0-blast.age/0.28).powi(2),
                                        moving:true,
                                    });
                                }
                                for (pos, radius, color, strength) in world.adventure.spell_lights() {
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

                                nearby_torch_count = nearby_torches.len();

                                let weather_start = Instant::now();
                                let day_speed = DEFAULT_DAY_LENGTH_SECONDS
                                    / day_night.day_length_seconds().max(1.0);
                                let sky_motion_rate = 0.55 + day_speed * 0.65;
                                    sky_time = benchmark.as_ref().and_then(|b| b.config.review_sky_time)
                                        .unwrap_or(sky_time + gameplay_dt * sky_motion_rate);
                                let abs_player_world_xz_m: DVec2 = (player.pos.xz() * world_scale).as_dvec2();

                                if let Some(upload_manifest) = cloud_weather.advance(
                                    gameplay_dt,
                                    abs_player_world_xz_m.as_vec2(),
                                    &day_night.cloud_controls(),
                                    day_night.weather_seed(),
                                    sky_time,
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
                                let mut scene_lighting = day_night.lighting();
                                scene_lighting.mood[0] = if cosmos_smooth > 0.002 { cosmos_smooth } else { 0.0 };
                                // Living air and the sky-event grade only exist above ground.
                                let ground_m = world.terrain_height_at(
                                    (eye_m.x / world_scale).floor() as i32,
                                    (eye_m.z / world_scale).floor() as i32,
                                ) as f32 * world_scale;
                                let underground = eye_m.y < ground_m - 3.0;
                                let amb_target = if underground {
                                    [0.0; 4]
                                } else {
                                    world::mood::ambience(&world.sample_config(), eye_m, &scene_lighting, day_night.sky_event())
                                };
                                let amb_k = 1.0 - (-gameplay_dt.max(0.0) * 1.2).exp();
                                for (cur, target) in ambience_smooth.iter_mut().zip(amb_target) {
                                    *cur += (target - *cur) * amb_k;
                                }
                                scene_lighting.ambience = ambience_smooth;
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
                                if sky_event == Some(world::mood::SkyEvent::MeteorShower)
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
                                    &day_night.cloud_controls(),
                                    sky_time,
                                    held_torch,
                                    &nearby_torches,
                                    Some(&cloud_weather.field),
                                    render_debug_view,
                                    atmosphere_mode,
                                );
                                gpu.underwater = player.underwater;
                                if player.underwater {
                                    gpu.water_surface_m = water_surface_above_m(world, player.precise_eye(world.voxel_scale).as_vec3());
                                }
                                gpu.water_level_m = world.sample_config().water_level() as f32 * world.voxel_scale.meters();
                                gpu.upload_camera(player.precise_eye(world.voxel_scale) * world_scale as f64, world_scale, forward, proj, world.seed, world.generator_version, backdrop_coverage);
                                cpu_stats.uniform_ms =
                                    uniform_start.elapsed().as_secs_f32() * 1000.0;
                            } else if render_world {
                                let world = world.as_mut().unwrap();

                                let day_speed = DEFAULT_DAY_LENGTH_SECONDS
                                    / day_night.day_length_seconds().max(1.0);
                                let sky_motion_rate = 0.55 + day_speed * 0.65;
                                sky_time = benchmark.as_ref().and_then(|b| b.config.review_sky_time)
                                    .unwrap_or(sky_time + gameplay_dt * sky_motion_rate);
                                let precise_eye_m = player.precise_eye(world.voxel_scale) * world.voxel_scale.meters() as f64;
                                let tier = altitude_state.update(altitude::altitude_m(precise_eye_m, altitude::planet_world(world)));
                                gpu.set_tier(tier);
                                let orbit = tier == altitude::Tier::Orbit;
                                chunk_cache.freeze(orbit);
                                let streaming_start = Instant::now();
                                world.set_visual_region_interest(&chunk_cache.edit_proxy_source_coords());
                                chunk_cache.set_horizon_pins(gpu.horizon_retention_columns()); let snapshot = if chunk_cache.has_horizon_pins() { world.stream_regions(chunk_coord_from_world_block(player.pos.floor().as_ivec3()), render_distance_settings().view_distance_chunks); world.snapshot() } else { world.snapshot_near(chunk_coord_from_world_block(player.pos.floor().as_ivec3()), render_distance_settings().view_distance_chunks) };
                                chunk_streamer.invalidate_changed_sources(&snapshot,&mut chunk_cache);
                                let proxy_keys=chunk_cache.update_edit_proxies(&snapshot,&dirty_chunks,world.revision(),gpu.horizon_published_revision());
                                if !proxy_keys.is_empty() { chunk_streamer.push_pending(proxy_keys,chunk_cache.center_chunk); }
                                if !orbit { chunk_streamer.enqueue_missing(gpu, &snapshot, world.revision(), &mut chunk_cache); }
                                for (coord,mask) in chunk_cache.take_simulation_seeds() {fluid_sim.seed(world,player.pos.floor().as_ivec3(),coord,mask);}
                                chunk_streamer.upload_finished(
                                    gpu,
                                    &snapshot,
                                    &mut chunk_cache,
                                    world.revision(),
                                    3,
                                );
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
                                gpu.update_horizon(world, render_eye, chunk_cache.strict_coverage_meters(render_eye,render_settings.view_distance_chunks,world_scale), chunk_cache.resident_coverage(render_eye,render_settings.view_distance_chunks,world_scale), chunk_cache.desired_resident_coverage(render_eye,render_settings.view_distance_chunks,world_scale),&chunk_cache,egui_ui.fov_degrees);
                                cpu_stats.horizon_ms = horizon_start.elapsed().as_secs_f32() * 1000.0;
                                let cull_view = Mat4::look_to_rh(eye, forward, Vec3::Y);
                                let proj = Mat4::perspective_infinite_rh(
                                    view_zoom.lens(egui_ui.fov_degrees, 0.0).to_radians(),
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
                                let cull_proj = Mat4::perspective_rh(
                                    view_zoom.lens(egui_ui.fov_degrees, 0.0).to_radians(),
                                    gpu.aspect_ratio(),
                                    0.05,
                                    far_plane_voxels,
                                );
                                let culling_start = Instant::now();
                                draw_list = build_draw_list(&chunk_cache, cull_proj * cull_view);
                                cpu_stats.culling_ms =
                                    culling_start.elapsed().as_secs_f32() * 1000.0;
                                if benchmark.is_some() { visible_missing = chunk_cache.missing_in_view(cull_proj * cull_view); }
                                let held_torch =
                                    held_torch_light(&player, world.voxel_scale, world_scale, mesh::models::held_item::holds_torch(world, &player), world.expedition.lamp_enabled());
                                let torch_start = Instant::now();
                                let mut nearby_torches =
                                    collect_nearby_torches(&snapshot, &chunk_cache, player.pos, world_scale);
                                append_fire_lights(world, player.pos, world_scale, &mut nearby_torches);
                                for (pos, radius, color, strength) in world.adventure.spell_lights() {
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

                                nearby_torch_count = nearby_torches.len();
                                let weather_start = Instant::now();
                                let abs_player_world_xz_m: DVec2 = (player.pos.xz() * world_scale).as_dvec2();
                                if let Some(upload_manifest) = cloud_weather.advance(
                                    0.0,
                                    abs_player_world_xz_m.as_vec2(),
                                    &day_night.cloud_controls(),
                                    day_night.weather_seed(),
                                    sky_time,
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
                                let mut scene_lighting = day_night.lighting();
                                scene_lighting.mood[0] = if cosmos_smooth > 0.002 { cosmos_smooth } else { 0.0 };
                                // Living air and the sky-event grade only exist above ground.
                                let ground_m = world.terrain_height_at(
                                    (eye_m.x / world_scale).floor() as i32,
                                    (eye_m.z / world_scale).floor() as i32,
                                ) as f32 * world_scale;
                                let underground = eye_m.y < ground_m - 3.0;
                                let amb_target = if underground {
                                    [0.0; 4]
                                } else {
                                    world::mood::ambience(&world.sample_config(), eye_m, &scene_lighting, day_night.sky_event())
                                };
                                let amb_k = 1.0 - (-gameplay_dt.max(0.0) * 1.2).exp();
                                for (cur, target) in ambience_smooth.iter_mut().zip(amb_target) {
                                    *cur += (target - *cur) * amb_k;
                                }
                                scene_lighting.ambience = ambience_smooth;
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
                                if sky_event == Some(world::mood::SkyEvent::MeteorShower)
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
                                    &day_night.cloud_controls(),
                                    sky_time,
                                    held_torch,
                                    &nearby_torches,
                                    Some(&cloud_weather.field),
                                    render_debug_view,
                                    atmosphere_mode,
                                );
                                gpu.underwater = player.underwater;
                                if player.underwater {
                                    gpu.water_surface_m = water_surface_above_m(world, player.precise_eye(world.voxel_scale).as_vec3());
                                }
                                gpu.water_level_m = world.sample_config().water_level() as f32 * world.voxel_scale.meters();
                                gpu.upload_camera(player.precise_eye(world.voxel_scale) * world_scale as f64, world_scale, forward, proj, world.seed, world.generator_version, backdrop_coverage);
                                cpu_stats.uniform_ms =
                                    uniform_start.elapsed().as_secs_f32() * 1000.0;
                            } else {
                                let uniform_start = Instant::now();
                                gpu.upload_lighting(
                                    &day_night.lighting(),
                                    &day_night.cloud_controls(),
                                    sky_time,
                                    None,
                                    &[],
                                    None,
                                    render_debug_view,
                                    atmosphere_mode,
                                );
                                gpu.underwater = false;
                                gpu.upload_camera(glam::DVec3::ZERO, 1.0, Vec3::NEG_Z, Mat4::IDENTITY, 0, 0, -1.0);
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
                                // A step that changes nothing (already at that setting) is
                                // skipped at once, so the player never waits for a no-op.
                                while !step.apply(&mut view) {
                                    match quality_governor.force_next() {
                                        Some(next) => step = next,
                                        None => break,
                                    }
                                }
                                set_render_distance_settings(view);
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
                            let scene=world.as_ref().map(|w|mesh::models::build(w,&player,&sky_day,orbital_pixel_angle)).unwrap_or_default();
                            gpu.upload_scene(&scene);
                            mesh::models::recycle(scene);
                            let zone = world.as_ref().and_then(|w| {
                                // player.pos is in voxel units; zones work in metres.
                                let cfg = w.sample_config();
                                let metres = player.pos * cfg.voxel_meters();
                                let surface = cfg.world_y(
                                    w.terrain_height_at(
                                        player.pos.x.floor() as i32,
                                        player.pos.z.floor() as i32,
                                    ),
                                );
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
                            let preparing_horizon = world.is_some() && !gpu.horizon_ready();
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
                            let loader_visible = preparing_horizon
                                && (benchmark_config.enabled || !quality::horizon_wait_expired());
                            if loader_visible {
                                egui_ui.build_loading_ui(&LoadingUiState {
                                    title: "Loading terrain".into(),
                                    progress_01: None,
                                });
                            }
                            let overlay = |device: &wgpu::Device, queue: &wgpu::Queue,
                                encoder: &mut wgpu::CommandEncoder, target_view: &wgpu::TextureView,
                                width: u32, height: u32| {
                                egui_ui.end_frame(window.as_ref(), device, queue, encoder,
                                    target_view, width, height);
                            };
                            // The opaque title/browser needs only its UI pass,
                            // without the world's render setup and postprocessing.
                            let render_result = if loader_visible || matches!(mode, AppMode::WorldSelect) {
                                gpu.render_loading_frame([0.025, 0.035, 0.05], overlay)
                            } else { gpu.render_cached_with_overlay(
                                &chunk_cache,
                                &draw_list,
                                &outline,
                                &invert_outline,
                                &[],
                                paused_render,
                                overlay,
                            ) };
                            cpu_stats.render_ms = render_start.elapsed().as_secs_f32() * 1000.0;
                            cpu_stats.total_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
                            if let Some(state) = benchmark.as_mut()
                                && let Some(timing) = state.route_timing.as_mut() {
                                    let pose_scale = world.as_ref().map_or(selected_voxel_scale, |w| w.voxel_scale);
                                    timing.record(state.motion_started_at.elapsed().as_secs_f64(),
                                        !preparing_horizon, !preparing_horizon && state.measuring, physics_enabled,
                                        render_result.is_ok(), player.eye(pose_scale) * pose_scale.meters(),
                                        player.yaw, player.pitch, cpu_stats,
                                        [gpu.size().width, gpu.size().height]);
                                }
                            if !gpu.device_usable() {
                                eprintln!("[renderer-fatal] GPU device lost; stopped before resource recreation/publication");
                                quality::record_gpu_reset(gpu.adapter_summary(), render::device_loss_report().unwrap_or("device lost while rendering"), benchmark.is_none());
                                elwt.exit();return;
                            }
                            if render_result.is_err() && benchmark.as_ref().is_some_and(|b|b.auto_started) {
                                eprintln!("[benchmark] failed native rendering: {:?}",render_result.as_ref().err());
                                let _=gpu.device.poll(wgpu::PollType::Poll);
                                elwt.exit();return;
                            }
                            if !preparing_horizon && !spawning
                                && workshop_review.as_mut().is_some_and(workshop_review::Review::finished) {
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
                                    if let Some(review)=benchmark.as_mut() {
                                        let elapsed=review.motion_started_at.elapsed().as_secs_f64();
                                        // Bound diagnostics so a slow run cannot amplify its own
                                        // stalls with unbounded logging. Normal play is unaffected.
                                        if sample.cpu.total_ms > 50.0 && benchmark_hitch_reports < 24 {
                                            benchmark_hitch_reports += 1;
                                            println!("[frame-hitch] seconds={elapsed:.2} total_ms={:.2} ui={:.2} gameplay={:.2} snapshot={:.2} stream={:.2} cull={:.2} torch={:.2} weather={:.2} uniforms={:.2} render={:.2} acquire={:.2} submit={:.2} shadow_draws={} terrain_draws={} horizon={:.2} resident_upload_wall_ms={:.2}",
                                                sample.cpu.total_ms, sample.cpu.ui_ms, sample.cpu.gameplay_ms,
                                                sample.cpu.snapshot_ms, sample.cpu.streaming_ms, sample.cpu.culling_ms,
                                                sample.cpu.torch_ms, sample.cpu.weather_ms, sample.cpu.uniform_ms,
                                                sample.cpu.render_ms, sample.render.surface_acquire_ms, sample.render.submit_ms,
                                                sample.render.shadow_draw_calls, sample.render.terrain_draw_calls, sample.cpu.horizon_ms, sample.cpu.resident_upload_wall_ms);
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
                                            println!("[simulation-profile] active={} dormant={} inflight={} pending={} stepped={} updates={} source_mib={:.2} source_failed={} writeback_pressure={} recovering={}",
                                                physics.active_chunks, fluid_sim.dormant_chunks(), physics.inflight_chunks, physics.pending_updates,
                                                physics.simulated_chunks_last_step, physics.applied_updates_last_step,
                                                physics.source_bytes as f64/1048576.0, physics.source_failed,
                                                physics.writeback_pressure, physics.recovering);
                                            if review.config.profile_physics && let Some(w)=world.as_ref() {
                                                let env=&w.environment;
                                                println!("[environment-profile] tick={} target={} lag_s={} serviced={} blocked={} patches={} pending={} sediment={} fractions={} reserved_bytes={} sky_chunks={} first_event={:?} sky_cursor={:?}",
                                                    env.tick,env.target_tick,env.lag_seconds(),env.serviced,env.blocked,
                                                    env.patches.len(),env.due.len(),env.sediment.len(),env.water_deficit.len(),env.reserved_bytes(),
                                                    env.sky.len(),env.due.first(),env.due.first().and_then(|(_,p)|env.light.get(p)).map(|q|(q.next_y,q.shade)));
                                            }
                                        }
                                    }
                                    perf_tracker.push(sample);
                                    perf_snapshot = perf_tracker.snapshot(
                                        gpu.adapter_summary(),
                                        [gpu.size().width, gpu.size().height],
                                        render_distance_settings(),
                                    );
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
                                        // Loading frames have no world passes and cannot be
                                        // reported as world performance or used for warmup.
                                        if preparing_horizon {
                                            benchmark_state.measuring = false;
                                            benchmark_state.phase_started_at = Instant::now();
                                            benchmark_state.measured_samples.clear();
                                            if benchmark_state.motion_started_at.elapsed().as_secs_f32()
                                                > benchmark_state.config.warmup_seconds + benchmark_state.config.duration_seconds + 120.0 {
                                                eprintln!("[benchmark] failed: distant terrain never became ready; no world timing result");
                                                elwt.exit();
                                            }
                                        }
                                        if !preparing_horizon && benchmark_state.auto_started && !benchmark_state.measuring
                                            && benchmark_state.phase_started_at.elapsed().as_secs_f32()
                                                >= benchmark_state.config.warmup_seconds
                                            {
                                                benchmark_state.measuring = true;
                                                benchmark_state.phase_started_at = Instant::now();
                                                benchmark_state.measured_samples.clear();
                                                println!("[benchmark] warmup complete");
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
                                                let root = std::env::var_os("VOXEL_CAPTURE_DIR")
                                                    .map(std::path::PathBuf::from)
                                                    .unwrap_or_else(|| "target/benchmark-frames".into());
                                                let path = root.join(format!("bench-{shot:03}.png"));
                                                if !path.exists() && benchmark_state.last_periodic_capture != Some(shot) {
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
                                                let globe = gpu.globe_stats();
                                                let upload_kb = summary.history.iter().map(|s| f64::from(s.cpu.globe_upload_bytes)).sum::<f64>() / 1024.0;
                                                println!("[benchmark] globe resident={} drawn={} wanted={} bake_ms={:.3} upload_kb={upload_kb:.1}", globe.resident, globe.drawn, globe.wanted, summary.cpu_avg.globe_bake_ms);
                                                elwt.exit();
                                            }
                                        }
                                    }
                                }
                                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                    egui_ui.end_frame_no_paint(window.as_ref());
                                    gpu.resize(gpu.size())
                                }
                                Err(wgpu::SurfaceError::OutOfMemory) => {
                                    egui_ui.end_frame_no_paint(window.as_ref());
                                    elwt.exit();
                                }
                                Err(wgpu::SurfaceError::Timeout)
                                | Err(wgpu::SurfaceError::Other) => {
                                    egui_ui.end_frame_no_paint(window.as_ref());
                                }
                            }
                        }

                        StartupState::WaitingForWindow => {}
                    },

                    _ => {}
                }
            }

            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } if grabbed && focused && matches!(mode, AppMode::Playing) => {
                if matches!(startup, StartupState::Ready { .. }) {
                    let sensitivity = egui_ui.as_ref().map_or(0.0025, |ui| ui.mouse_sensitivity)
                        * view_zoom.sensitivity_scale();
                    player.yaw += delta.0 as f32 * sensitivity;
                    let y_sign = if egui_ui.as_ref().is_some_and(|ui| ui.invert_mouse) { -1.0 } else { 1.0 };
                    player.pitch -= delta.1 as f32 * sensitivity * y_sign;
                    player.pitch = player.pitch.clamp(-1.54, 1.54);
                }
            }

            Event::AboutToWait => {
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
                        if preferences_render_applied
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
                            let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                            w.stream_regions(center, render_distance_settings().view_distance_chunks);
                        }
                    // Review worlds are disposable and must not replace the
                    // player's autosave or introduce background save cost.
                    if !benchmark_config.enabled {
                        saves.tick(world.as_mut(), &player, &day_night);
                    }
                    if let Some(ui) = egui_ui.as_mut()
                        && let Some(status) = &saves.status {ui.persistence_status=Some(status.clone());}
                    if exit_pending {
                        if saves.failed {exit_pending=false;}
                        else if !saves.busy() && !preferences.busy() && !library.busy() {elwt.exit();return;}
                    }
                }
                if let Some(window) = window.as_ref() {
                    if (occluded && !benchmark_config.enabled) || window.inner_size().width == 0 || window.inner_size().height == 0 {
                        elwt.set_control_flow(ControlFlow::WaitUntil(Instant::now()+Duration::from_millis(100)));
                    } else if !focused && !benchmark_config.enabled && Instant::now() < next_background_frame {
                        elwt.set_control_flow(ControlFlow::WaitUntil(next_background_frame));
                    } else if !benchmark_config.enabled && matches!(mode, AppMode::WorldSelect)
                        && Instant::now() < next_menu_frame {
                        elwt.set_control_flow(ControlFlow::WaitUntil(next_menu_frame));
                    } else {
                        window.request_redraw();
                        next_background_frame = Instant::now() + Duration::from_millis(100);
                        next_menu_frame = Instant::now() + Duration::from_secs_f64(1.0 / 60.0);
                    }
                }
            }

            _ => {}
        }
    })?;

    Ok(())
}
