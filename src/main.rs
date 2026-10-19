use anyhow::Result;
use glam::{DVec2, Mat4, Vec3, Vec3Swizzles};
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

mod audio;
mod creative;
mod tool_effects;
mod egui_ui;
mod fluid;
mod material;
mod mesh;
mod perf;
mod player;
mod render;
mod shader_sources;
mod simulation;
mod preferences;
mod torch_review;
mod reference_environment;
mod material_studio;
mod water_review;
mod sandbox_review;
mod wand_review;
mod world;

use creative::CreativeState;
use egui_ui::{
    EguiUi, LoadingUiState, PhysicsUiState, RenderUiState, TimeUiState, UiAction, UiMode,
};
use fluid::VoxelScalePreset;
use simulation::WorldSimulation;
use mesh::build_outline_vertices;
use perf::{
    CpuFrameStats, FrameCounters, PerfSample, PerfSnapshot, PerfTracker, snapshot_from_samples,
};
use player::{FlyMode, FpsCounter, Input, Player, update_player};
use render::{
    AtmosphereMode, ChunkCache, ChunkStreamer, Gpu, GpuBootstrap, GpuPendingInit, MAX_TORCH_LIGHTS,
    RenderDebugView, TorchLight, build_draw_list, render_distance_settings,
    set_render_distance_settings,
};
use world::{
    BlockId, DEFAULT_DAY_LENGTH_SECONDS, DEFAULT_START_TIME_OF_DAY, DayNightCycle, WeatherPreset,
    WeatherRuntimeState, World, WorldType, chunk_coord_from_world_block,
    mark_chunk_and_neighbors_dirty, raycast_block,
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
    warmup_seconds: f32,
    duration_seconds: f32,
    motion: BenchmarkMotionMode,
    capture_frames: bool,
    capture_hud: bool,
    review_start_seconds: f32,
    review_time_of_day: Option<f32>,
    review_time_end: Option<f32>,
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
}

impl BenchmarkMotionMode {
    fn label(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Orbit => "orbit",
            Self::CloudReview => "cloud-review",
            Self::CloudReviewFar => "cloud-review-far",
        }
    }
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
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
            warmup_seconds: 4.0,
            duration_seconds: 8.0,
            motion: BenchmarkMotionMode::Static,
            capture_frames: false,
            capture_hud: false,
            review_start_seconds: 0.0,
            review_time_of_day: None,
            review_time_end: None,
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
    last_profile_second: usize,
    measured_samples: Vec<PerfSample>,
}

fn ui_mode(mode: &AppMode) -> UiMode {
    match mode {
        AppMode::WorldSelect => UiMode::WorldSelect,
        AppMode::Paused => UiMode::Paused,
        AppMode::Playing => UiMode::Playing,
    }
}

fn cycle_world(world_type: WorldType, dir: i32) -> WorldType {
    let idx = match world_type {
        WorldType::Superflat => 0_i32,
        WorldType::Hills => 1_i32,
        WorldType::Ridged => 2_i32,
    };
    match (idx + dir).rem_euclid(3) {
        0 => WorldType::Superflat,
        1 => WorldType::Hills,
        _ => WorldType::Ridged,
    }
}

fn cycle_voxel_scale(scale: VoxelScalePreset, dir: i32) -> VoxelScalePreset {
    let idx = match scale {
        VoxelScalePreset::OneMeter => 0_i32,
        VoxelScalePreset::HalfMeter => 1_i32,
        VoxelScalePreset::QuarterMeter => 2_i32,
    };
    match (idx + dir).rem_euclid(3) {
        0 => VoxelScalePreset::OneMeter,
        1 => VoxelScalePreset::HalfMeter,
        _ => VoxelScalePreset::QuarterMeter,
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

fn parse_benchmark_config() -> BenchmarkConfig {
    let mut config = BenchmarkConfig::default();

    for arg in std::env::args().skip(1) {
        if arg == "--benchmark-saved-world" {
            config.enabled = true;
            config.saved_review = true;
            continue;
        }
        if arg == "--benchmark-no-indirect" {
            config.enabled = true;
            render::set_indirect_lighting_enabled(false);
            continue;
        }
        if arg == "--benchmark-no-shadows" {
            config.enabled = true;
            render::shadow::set_enabled(false);
            continue;
        }
        if arg == "--benchmark-reference-review" {
            config.enabled=true; config.reference_review=true;
            config.world_type=WorldType::Superflat; config.view_distance_chunks=Some(8);
            config.warmup_seconds=1.0; config.duration_seconds=66.0; continue;
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
            config.enabled=true;config.sandbox_review=true;config.profile_physics=true;
            config.world_type=WorldType::Superflat;config.view_distance_chunks=Some(4);
            config.warmup_seconds=3.0;config.duration_seconds=65.0;continue;
        }
        if arg == "--benchmark-water-review" {
            config.enabled = true;
            config.water_review = true;
            config.world_type = WorldType::Superflat;
            config.view_distance_chunks = Some(4);
            config.warmup_seconds = 2.0;
            config.duration_seconds = 24.0;
            continue;
        }
        if let Some(value)=arg.strip_prefix("--benchmark-cpu-mib="){config.cpu_mib=value.parse().ok();config.enabled=true;continue;}
        if let Some(value)=arg.strip_prefix("--benchmark-gpu-mib="){config.gpu_mib=value.parse().ok();config.enabled=true;continue;}
        if arg=="--benchmark-physics" {config.profile_physics=true;config.enabled=true;continue;}
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
            let values: Vec<f32> = value.split(',').filter_map(|v| v.parse::<f32>().ok()).collect();
            if values.len() == 5 && values.iter().all(|v| v.is_finite()) {
                config.terrain_camera = Some([values[0], values[1], values[2], values[3], values[4]]);
            }
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-review-time-end=") {
            config.review_time_end = value.parse::<f32>().ok()
                .filter(|time| time.is_finite()).map(|time| time.rem_euclid(1.0));
            continue;
        }
        if let Some(value) = arg.strip_prefix("--benchmark-review-time=") {
            config.review_time_of_day = value.parse::<f32>().ok()
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
                "terrain-shadow" => RenderDebugView::CloudLayerAabb,
                "cascades" => RenderDebugView::ShadowCascadeIndex,
                _ => RenderDebugView::None,
            };
            continue;
        }
        if arg == "--benchmark-capture-hud" {
            config.enabled = true;
            config.capture_frames = true;
            config.capture_hud = true;
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
                _ => BenchmarkMotionMode::Static,
            };
        }
    }

    config
}

fn benchmark_motion_active(benchmark: Option<&BenchmarkState>) -> bool {
    benchmark.is_some_and(|state| {
        state.auto_started && (state.config.torch_review || !matches!(state.config.motion, BenchmarkMotionMode::Static))
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
    if state.config.torch_review {
        torch_review::motion_camera(player, terrain.voxel_meters(), state.motion_started_at.elapsed().as_secs_f32());
        return;
    }
    if let Some([x, z, clearance, yaw, pitch]) = state.config.terrain_camera {
        let scale = terrain.voxel_scale.meters();
        let ground = world::terrain_height(&terrain, (x / scale).floor() as i32, (z / scale).floor() as i32) as f32;
        player.pos = Vec3::new(x / scale, ground + clearance / scale, z / scale);
        player.yaw = yaw;
        player.pitch = pitch;
        if state.config.horizon_sweep {
            let t = (state.motion_started_at.elapsed().as_secs_f32() - 20.0).max(0.0);
            player.pos.x += t * 12.0 / scale;
            player.pos.z += (t * 0.1).sin() * 32.0 / scale;
            let ground = world::terrain_height(&terrain, player.pos.x.floor() as i32, player.pos.z.floor() as i32) as f32;
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
            player.yaw += if clearance < 0.0 { t * std::f32::consts::TAU / 24.0 } else { 0.65 * wave };
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

    let inv_scale = terrain.voxel_scale.meters().recip().max(1e-6);
    let elapsed = state.motion_started_at.elapsed().as_secs_f32() + state.config.review_start_seconds
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

fn hotbar_slot_from_key(key: PhysicalKey) -> Option<usize> {
    match key {
        PhysicalKey::Code(KeyCode::Digit1) => Some(0),
        PhysicalKey::Code(KeyCode::Digit2) => Some(1),
        PhysicalKey::Code(KeyCode::Digit3) => Some(2),
        PhysicalKey::Code(KeyCode::Digit4) => Some(3),
        PhysicalKey::Code(KeyCode::Digit5) => Some(4),
        PhysicalKey::Code(KeyCode::Digit6) => Some(5),
        PhysicalKey::Code(KeyCode::Digit7) => Some(6),
        PhysicalKey::Code(KeyCode::Digit8) => Some(7),
        PhysicalKey::Code(KeyCode::Digit9) => Some(8),
        _ => None,
    }
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

const TORCH_LIGHT_RADIUS: f32 = 13.5;
const TORCH_LIGHT_COLOR: [f32; 3] = [1.00, 0.48, 0.20];
const TORCH_LIGHT_STRENGTH: f32 = 2.10;

fn held_torch_light(
    player: &Player,
    voxel_scale: VoxelScalePreset,
    world_scale: f32,
    lumen_lantern: bool,
) -> Option<TorchLight> {
    if !player.selected_block.is_torch() && !lumen_lantern {
        return None;
    }

    let eye = player.eye(voxel_scale);
    let forward = player.forward_dir();
    let right = player.right_dir();
    let inv_world_scale = world_scale.recip().max(1e-6);
    Some(TorchLight {
        position: (eye
            + (forward * 0.42 + right * 0.18 + Vec3::new(0.0, -0.28, 0.0)) * inv_world_scale)
            * world_scale,
        radius: TORCH_LIGHT_RADIUS,
        color: if lumen_lantern {[0.30,0.90,0.74]} else {TORCH_LIGHT_COLOR},
        strength: TORCH_LIGHT_STRENGTH,
    })
}

fn collect_nearby_torches(
    snapshot: &world::WorldSnapshot,
    player_pos: Vec3,
    world_scale: f32,
) -> Vec<TorchLight> {
    let center = player_pos.floor().as_ivec3();
    let center_chunk = chunk_coord_from_world_block(center);
    // Match visible terrain range, with a bounded light-selection budget. Iterate
    // indexed sources instead of probing a large cube of empty chunk coordinates.
    let max_distance = (render::render_distance_settings().view_distance_chunks as f32
        * world::CHUNK_SIZE as f32 * world_scale).clamp(64.0, 192.0);
    let max_radius_blocks = max_distance / world_scale.max(1e-6);
    let max_radius_sq = max_radius_blocks * max_radius_blocks;
    let chunk_radius =
        ((max_radius_blocks.ceil() as i32) + world::CHUNK_SIZE - 1) / world::CHUNK_SIZE;
    let mut torches = Vec::new();
    for (coord, chunk) in snapshot.chunks.iter() {
                if (*coord - center_chunk).abs().max_element() > chunk_radius { continue; }
                for &pos in &chunk.torches {
                    let dist_sq = (pos.as_vec3() + Vec3::splat(0.5) - player_pos).length_squared();
                    if dist_sq <= max_radius_sq {
                        torches.push((dist_sq, pos));
                    }
                }
    }
    torches.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then_with(|| (a.1.x, a.1.y, a.1.z).cmp(&(b.1.x, b.1.y, b.1.z)))
    });
    // Fade out at the budget boundary as well as the distance boundary. Sources
    // exchanging the final slot therefore enter with zero intensity.
    let fade_end = torches.get(MAX_TORCH_LIGHTS)
        .map_or(max_distance, |t| t.0.sqrt() * world_scale).min(max_distance);
    torches.truncate(MAX_TORCH_LIGHTS);
    torches
        .into_iter()
        .map(|(distance_sq, pos)| {
            let block = snapshot.get(pos);
            let light_offset = match block {
                BlockId::Torch => Vec3::new(0.50, 0.82, 0.50),
                BlockId::TorchWallNorth => Vec3::new(0.50, 0.84, 0.34),
                BlockId::TorchWallSouth => Vec3::new(0.50, 0.84, 0.66),
                BlockId::TorchWallWest => Vec3::new(0.34, 0.84, 0.50),
                BlockId::TorchWallEast => Vec3::new(0.66, 0.84, 0.50),
                _ => Vec3::new(0.50, 0.82, 0.50),
            };
            TorchLight {
                position: block.torch_point_meters(pos.as_vec3(), light_offset, world_scale),
                radius: TORCH_LIGHT_RADIUS,
                color: TORCH_LIGHT_COLOR,
                strength: {
                    let distance = distance_sq.sqrt() * world_scale;
                    let fade_start = (fade_end * 0.8).max(fade_end - 24.0);
                    let t = ((fade_end - distance) / (fade_end - fade_start).max(0.001)).clamp(0.0, 1.0);
                    TORCH_LIGHT_STRENGTH * t * t * (3.0 - 2.0 * t)
                },
            }
        })
        .collect()
}

fn resolve_placement_block(selected: BlockId, support_normal: glam::IVec3) -> Option<BlockId> {
    if selected.is_torch() {
        BlockId::torch_from_attachment(support_normal)
    } else {
        Some(selected)
    }
}

fn is_torch_supported(world: &World, pos: glam::IVec3, block: BlockId) -> bool {
    let Some(offset) = block.torch_support_offset() else {
        return true;
    };
    world.get(pos + offset).is_solid()
}

fn cleanup_detached_torches(
    world: &mut World,
    origin: glam::IVec3,
    dirty_blocks: &mut HashSet<glam::IVec3>,
    dirty_chunks: &mut HashSet<glam::IVec3>,
) {
    for check in [
        origin + glam::IVec3::Y,
        origin + glam::IVec3::X,
        origin - glam::IVec3::X,
        origin + glam::IVec3::Z,
        origin - glam::IVec3::Z,
    ] {
        let block = world.get(check);
        if !block.is_torch() || is_torch_supported(world, check, block) {
            continue;
        }
        world.set(check, BlockId::Air);
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
) {
    let mut new_world = World::with_seed(selected_world, selected_voxel_scale, selected_seed);
    *player = Player::new();
    day_night.reset(DEFAULT_START_TIME_OF_DAY);
    day_night.set_day_length_seconds(DEFAULT_DAY_LENGTH_SECONDS);
    player.pos = new_world.suggested_spawn();
    if std::env::var_os("VOXEL_REVIEW_CAUSEWAY").is_some() {
        if let Some(pos) = world::sites::review_causeway_spawn(&new_world.sample_config()) {
            player.pos = pos;
            player.precise_pos = pos.as_dvec3();
            player.yaw = std::f32::consts::FRAC_PI_2;
            player.pitch = -0.12;
            eprintln!("[tool-review] natural causeway spawn {:?}", pos);
        }
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

fn log_benchmark_summary(snapshot: &PerfSnapshot, config: &BenchmarkConfig) {
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
        "[benchmark] avg_fps={:.2} avg_frame_ms={:.3} p99_frame_ms={:.3} max_frame_ms={:.3} fps_1pct={:.2}",
        snapshot.fps_avg,
        snapshot.frame_ms_avg,
        snapshot.frame_ms_p99,
        snapshot.frame_ms_max,
        snapshot.fps_1p_low
    );
    println!(
        "[benchmark] cpu_avg_ms ui={:.3} gameplay={:.3} snapshot={:.3} stream={:.3} cull={:.3} torch={:.3} weather={:.3} uniforms={:.3} render_call={:.3}",
        snapshot.cpu_avg.ui_ms,
        snapshot.cpu_avg.gameplay_ms,
        snapshot.cpu_avg.snapshot_ms,
        snapshot.cpu_avg.streaming_ms,
        snapshot.cpu_avg.culling_ms,
        snapshot.cpu_avg.torch_ms,
        snapshot.cpu_avg.weather_ms,
        snapshot.cpu_avg.uniform_ms,
        snapshot.cpu_avg.render_ms
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

    let benchmark_config = parse_benchmark_config();
    // Find reproducible visual-review locations without opening a window or
    // touching saves. Uses the same generator as streamed gameplay chunks.
    if std::env::args().any(|arg| arg == "--terrain-survey") {
        let review_world = World::with_seed(benchmark_config.world_type, benchmark_config.voxel_scale, benchmark_config.seed);
        let cfg = review_world.sample_config();
        let mut locations = std::collections::BTreeMap::new();
        for z in (-3072..=3072).step_by(192) {
            for x in (-3072..=3072).step_by(192) {
                let s = world::landscape::sample(&cfg, cfg.blocks_for_height(x as f32), cfg.blocks_for_height(z as f32));
                let distance = x * x + z * z;
                let entry = locations.entry(s.biome.label()).or_insert((i32::MAX, x, z, s.height));
                if distance < entry.0 { *entry = (distance, x, z, s.height); }
            }
        }
        for (biome, (_, x, z, height)) in locations {
            println!("[terrain-survey] {biome}: x={x} z={z} height_m={height:.1}");
        }
        return Ok(());
    }
    let mut preferences = preferences::PreferenceStore::new(!benchmark_config.enabled);
    if !benchmark_config.enabled {preferences.loaded.apply_globals();}
    let mut preferences_render_applied = false;
    let mut windowed_size = (preferences.loaded.width,preferences.loaded.height);
    let mut combat_audio=audio::Audio::new();
    let event_loop = EventLoop::new()?;
    let mut window: Option<Arc<Window>> = None;

    let mesh_workers = thread::available_parallelism()
        .map(|count| count.get().saturating_sub(2).clamp(1, 8))
        .unwrap_or(1);

    let mut startup = StartupState::WaitingForWindow;
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
    let mut day_night = DayNightCycle::new(DEFAULT_START_TIME_OF_DAY, DEFAULT_DAY_LENGTH_SECONDS);
    let mut cloud_weather = WeatherRuntimeState::new(selected_seed);
    let mut freeze_time = false;
    let mut input = Input::default();
    let mut creative = CreativeState::default();
    player.selected_block = creative.selected_block();
    let mut render_debug_view = benchmark_config.debug_view;
    let mut atmosphere_mode = AtmosphereMode::default();

    let mut grabbed = false;
    let mut focused = true;
    let mut occluded = false;
    let mut next_background_frame = Instant::now();
    let mut mode = AppMode::WorldSelect;
    let mut saves = world::storage::SaveService::new();
    let mut library = world::storage::WorldLibrary::new(!benchmark_config.enabled);
    let mut pending_loaded: Option<anyhow::Result<world::storage::SavedSession>> = None;
    let mut next_library_refresh = Instant::now();
    let mut exit_pending = false;
    let mut last = Instant::now();
    let mut simulation_clock = simulation::SimulationClock::default();
    let mut sky_time = 0.0f32;

    let mut chunk_cache = ChunkCache::new(glam::IVec3::ZERO);
    chunk_cache.reset(glam::IVec3::ZERO);
    let mut dirty_blocks = HashSet::new();
    let mut dirty_chunks = HashSet::new();
    let mut physics_enabled = true;
    let mut fps_counter = FpsCounter::new();
    let mut place_was_down = false;
    let mut break_was_down = false;
    let mut place_repeat_timer = 0.0f32;
    let mut break_repeat_timer = 0.0f32;
    let mut pending_tool_action: Option<egui_ui::ToolUseAction> = None;
    let mut perf_tracker = PerfTracker::default();
    let mut perf_snapshot = PerfSnapshot::default();
    let live_profile = std::env::var_os("VOXEL_LIVE_PROFILE").is_some();
    let mut last_live_profile = Instant::now();
    let mut benchmark_hitch_reports = 0u32;
    let mut sandbox_review = benchmark_config.sandbox_review.then(sandbox_review::SandboxReview::default);
    let mut wand_review = wand_review::Review::from_env(benchmark_config.enabled);
    let mut water_review = benchmark_config.water_review.then(water_review::WaterReview::default);
    let mut benchmark = benchmark_config.enabled.then(|| BenchmarkState {
        config: benchmark_config,
        auto_started: false,
        measuring: false,
        phase_started_at: Instant::now(),
        motion_started_at: Instant::now(),
        motion_anchor_blocks: Vec3::ZERO,
        cloud_review_phase: usize::MAX,
        last_capture_key: usize::MAX,
        last_profile_second: 0,
        measured_samples: Vec::new(),
    });

    #[allow(deprecated)]
    event_loop.run(move |event, elwt| {
        let loading = !matches!(
            startup,
            StartupState::Ready { .. } | StartupState::Failed(_)
        );
        let active_gameplay = matches!(mode, AppMode::Playing);
        elwt.set_control_flow(if !occluded && (focused || benchmark_config.enabled) && (loading || active_gameplay) {
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
                                .with_title("Voxel Workshop")
                                .with_active(!benchmark_config.enabled)
                                .with_inner_size(initial_window_size),
                        )
                        .expect("failed to create window"),
                    );
                    window = Some(created_window);
                }

                if matches!(startup, StartupState::WaitingForWindow) {
                    let Some(window_ref) = window.as_ref() else {
                        return;
                    };

                    match pollster::block_on(GpuBootstrap::new(window_ref.clone())) {
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
                if matches!(event, WindowEvent::RedrawRequested)
                    && (occluded || window.inner_size().width == 0 || window.inner_size().height == 0)
                {
                    last = Instant::now();
                    simulation_clock = simulation::SimulationClock::default();
                    return;
                }

                let egui_consumed = egui_ui
                    .as_mut()
                    .map(|ui| ui.on_window_event(window, &event))
                    .unwrap_or(false);

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
                        // Releases always reach gameplay; presses belong to the active surface.
                        if let PhysicalKey::Code(code) = key.physical_key {
                            if !pressed || (matches!(mode, AppMode::Playing) && grabbed && !egui_consumed) {
                                input.keyboard(code, pressed);
                            }
                        }
                        let text_editing = egui_ui.as_ref().is_some_and(|ui| ui.ctx.wants_keyboard_input());
                        if text_editing && !matches!(key.physical_key, PhysicalKey::Code(KeyCode::Escape)) {
                            return;
                        }
                        if fresh_press && key.physical_key == PhysicalKey::Code(KeyCode::F11) {
                            let fullscreen = if window.fullscreen().is_some() { None } else {
                                Some(winit::window::Fullscreen::Borderless(None))
                            };
                            window.set_fullscreen(fullscreen);
                            return;
                        }
                        if fresh_press && key.physical_key == PhysicalKey::Code(KeyCode::F3) {
                            if let Some(ui) = egui_ui.as_mut() { ui.toggle_debug(); }
                            return;
                        }
                        if fresh_press && key.physical_key == PhysicalKey::Code(KeyCode::F1) {
                            if let Some(ui) = egui_ui.as_mut() { ui.show_hud = !ui.show_hud; }
                            return;
                        }
                        if matches!(mode, AppMode::Paused)
                            && key.physical_key != PhysicalKey::Code(KeyCode::Escape) { return; }
                        if matches!(mode,AppMode::WorldSelect) {return;}

                        match key.physical_key {
                            PhysicalKey::Code(KeyCode::Escape) if fresh_press => {
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
                                    return;
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
                            PhysicalKey::Code(KeyCode::KeyB) if fresh_press => {
                                let open = !creative.show_panel; creative.close_panels(); creative.show_panel = open; input.clear(); player.jump_tap_timer = 0.0;
                                sync_creative_ui_cursor(
                                    window,
                                    &mut grabbed,
                                    focused,
                                    &mode,
                                    &creative,
                                );
                            }
                            PhysicalKey::Code(KeyCode::KeyF) if fresh_press && grabbed && !creative.any_ui_open() => {
                                input.interact = true;
                            }
                            PhysicalKey::Code(KeyCode::KeyT) if fresh_press && grabbed && !creative.any_ui_open() => { input.cycle_spell=true; }
                            PhysicalKey::Code(KeyCode::KeyE) | PhysicalKey::Code(KeyCode::Tab)
                                if fresh_press && (key.physical_key != PhysicalKey::Code(KeyCode::Tab) || !creative.show_palette) => {
                                let open = !creative.show_palette;
                                if !open {
                                    if let Some(w) = world.as_mut() {
                                        w.inventory.return_transient_to_pack();
                                        w.sync_from_inventory();
                                    }
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
                            PhysicalKey::Code(KeyCode::KeyQ) if fresh_press && !creative.show_palette => {
                                let open = !creative.show_quick_select; creative.close_panels(); creative.show_quick_select = open; input.clear(); player.jump_tap_timer = 0.0;
                                sync_creative_ui_cursor(
                                    window,
                                    &mut grabbed,
                                    focused,
                                    &mode,
                                    &creative,
                                );
                            }
                            PhysicalKey::Code(KeyCode::BracketLeft) | PhysicalKey::Code(KeyCode::BracketRight)
                                if fresh_press && grabbed && !creative.any_ui_open() => {
                                creative.adjust_range(if key.physical_key == PhysicalKey::Code(KeyCode::BracketRight) { 1 } else { -1 });
                            }
                            PhysicalKey::Code(KeyCode::KeyZ) if fresh_press && !creative.any_ui_open() => {
                                creative.tool = creative::ToolKind::Hand;
                            }
                            PhysicalKey::Code(KeyCode::KeyX) if fresh_press && !creative.any_ui_open() => {
                                creative.tool = creative::ToolKind::Brush;
                            }
                            PhysicalKey::Code(KeyCode::KeyC) if fresh_press && !creative.any_ui_open() => {
                                creative.tool = creative::ToolKind::BuildersWand;
                            }
                            PhysicalKey::Code(KeyCode::KeyV) if fresh_press && !creative.any_ui_open() => {
                                creative.tool = creative::ToolKind::DestructorWand;
                            }
                            PhysicalKey::Code(KeyCode::KeyG) if fresh_press && !creative.any_ui_open() => {
                                creative.tool = creative::ToolKind::AreaTool;
                            }
                            PhysicalKey::Code(KeyCode::KeyH) if fresh_press && !creative.any_ui_open() => {
                                creative.tool = creative::ToolKind::ExplosiveWand;
                            }
                            _ if fresh_press => {
                                if let Some(slot) = hotbar_slot_from_key(key.physical_key) {
                                    player.selected_block = creative.select_slot(slot);
                                    if creative.show_palette { creative.tool = creative::ToolKind::Hand; }
                                }
                            }
                            _ => {}
                        }
                    }

                    WindowEvent::MouseInput { state, button, .. } => {
                        if !matches!(startup, StartupState::Ready { .. }) {
                            return;
                        }

                        let pressed = state == ElementState::Pressed;

                        if egui_consumed {
                            if !pressed {
                                if button == MouseButton::Left {
                                    input.break_block = false;
                                }
                                if button == MouseButton::Right {
                                    input.place_block = false;
                                }
                            }
                            return;
                        }

                        match mode {
                            AppMode::WorldSelect | AppMode::Paused => {}
                            AppMode::Playing => {
                                if pressed && (!grabbed || creative.any_ui_open()) { return; }
                                if button == MouseButton::Middle && pressed { input.pick_block = true; }
                                if button == MouseButton::Left {
                                    input.break_block = pressed;
                                }
                                if button == MouseButton::Right {
                                    input.place_block = pressed;
                                }
                            }
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
                                title: "Initializing renderer".to_string(),
                                detail: progress.detail.to_string(),
                                progress_01: progress.progress_01,
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
                                title: "Initializing renderer".to_string(),
                                detail: "Starting chunk meshing workers…".to_string(),
                                progress_01: 0.88,
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
                                title: "Initializing renderer".to_string(),
                                detail: "Preparing fluid simulation resources…".to_string(),
                                progress_01: 0.96,
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
                                if !benchmark_config.enabled {
                                    gpu.set_vsync(preferences.loaded.vsync);
                                    gpu.fxaa_enabled=preferences.loaded.fxaa;
                                    atmosphere_mode=preferences.loaded.atmosphere();
                                }
                                preferences_render_applied=true;
                            }
                            if !exit_pending {
                                if let Some(result)=pending_loaded.take() {
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
                            }
                            let frame_start = Instant::now();
                            let now = Instant::now();
                            let raw_frame_dt = (now - last).as_secs_f32();
                            let display_frame_dt = raw_frame_dt.min(0.25);
                            last = now;
                            if !matches!(mode, AppMode::WorldSelect) {
                                fps_counter.tick(display_frame_dt);
                            }
                            let spawn_progress = chunk_cache.spawn_progress();
                            let spawning = world.is_some() && !chunk_cache.spawn_ready && matches!(mode, AppMode::Playing);
                            let simulation_steps = simulation_clock.advance(
                                raw_frame_dt,
                                matches!(mode, AppMode::Playing) && !spawning,
                            );
                            let gameplay_dt = simulation_steps as f32 * simulation::STEP_SECONDS;

                            gpu.begin_frame_stats();
                            chunk_streamer.begin_frame_stats();

                            if let Some(benchmark_state) = benchmark.as_mut() {
                                if !benchmark_state.auto_started && world.is_none() {
                                    gpu.set_vsync(false);
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
                                    );
                                    release_mouse(window, &mut grabbed);
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
                                    if benchmark_state.config.reference_review {
                                        if let Some(w)=world.as_mut() {
                                            reference_environment::install(w);
                                            reference_environment::camera(&mut player, selected_voxel_scale.meters());
                                            let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                            let snapshot=w.snapshot_near(center,8);
                                            chunk_cache.reset_with_snapshot(center,&snapshot);
                                            chunk_streamer.reset();
                                            chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                        }
                                    }
                                    if benchmark_state.config.torch_review {
                                        if let Some(w) = world.as_mut() {
                                            torch_review::install(w);
                                            torch_review::camera(&mut player, selected_voxel_scale.meters());
                                            let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                            let snapshot = w.snapshot_near(center, 4);
                                            chunk_cache.reset_with_snapshot(center, &snapshot);
                                            chunk_streamer.reset();
                                            chunk_streamer.queue_needed_visible(&chunk_cache, center);
                                        }
                                    }
                                    if let Some(preset) = benchmark_state.config.weather_preset {
                                        day_night.set_weather_preset(preset);
                                    }
                                    cloud_weather.reset(selected_seed);
                                    physics_enabled = benchmark_state.config.profile_physics;
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
                            if let Some(review) = benchmark.as_mut().filter(|b| b.auto_started && b.config.torch_review) {
                                let elapsed = review.motion_started_at.elapsed().as_secs_f32();
                                let mode = torch_review::mode();
                                let phase = if mode == "survival" {
                                    (elapsed / 3.0) as usize
                                } else {
                                    (elapsed / 12.0) as usize
                                };
                                torch_review::motion_camera(&mut player, selected_voxel_scale.meters(), elapsed);
                                day_night.set_time_of_day(if mode == "survival" { 0.35 } else if phase == 1 || !mode.is_empty() { 0.0 } else { 0.5 });
                                freeze_time = true;
                                if mode == "survival" {
                                    if let Some(w) = world.as_mut() {
                                        torch_review::apply_survival_phase(w, &mut player, &mut creative, &egui_ui.ctx, phase);
                                    }
                                } else if mode == "motion" {
                                    creative.show_palette = false;
                                    creative.assign_hotbar_slot(0, BlockId::Stone);
                                } else if !matches!(mode, "interactive" | "drag") || elapsed < 2.0 {
                                    creative.show_palette = matches!(mode, "interactive" | "drag") || phase >= 2;
                                    egui_ui.ctx.data_mut(|d| d.insert_temp(egui::Id::new("inventory_page"), if mode.is_empty() && phase == 3 { 2u32 } else { 4u32 }));
                                }
                                let capture_phase = if mode.is_empty() { phase } else { (elapsed / 3.0) as usize };
                                if (mode.is_empty() && elapsed % 12.0 >= 9.0 || (mode == "survival" && elapsed % 3.0 >= 2.2) || (!mode.is_empty() && mode != "survival")) && review.last_capture_key != capture_phase {
                                    let root = std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from)
                                        .unwrap_or_else(|| "target/torch-review".into());
                                    gpu.request_capture(root.join(format!("{:?}-{capture_phase:03}.png", selected_voxel_scale)));
                                    if mode == "motion" {
                                        let snapshot = world.as_mut().unwrap().snapshot();
                                        let lights = collect_nearby_torches(&snapshot, player.pos, selected_voxel_scale.meters());
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
                            if let Some(review) = benchmark.as_mut().filter(|b| b.auto_started && b.config.saved_review && b.config.capture_frames) {
                                if review.last_capture_key == usize::MAX && review.motion_started_at.elapsed().as_secs_f32()
                                    >= review.config.warmup_seconds + review.config.duration_seconds - 1.0 {
                                    let root = std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| "target/saved-scene-review".into());
                                    gpu.request_capture(root.join("saved-world.png"));
                                    review.last_capture_key = 0;
                                }
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
                            egui_ui.adventure = world.as_ref().map(|w|w.adventure.view()).unwrap_or_default();
                            if let Some(w) = world.as_ref() {
                                egui_ui.survival = w.inventory.clone();
                                egui_ui.survival_eat_timer = w.inventory.eat_timer;
                                egui_ui.survival_armor_points = w.inventory.armor_points();
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
                            if let Some(w) = world.as_mut() {
                                w.inventory = egui_ui.survival.clone();
                                w.sync_from_inventory();
                            }
                            if spawning {
                                egui_ui.build_loading_ui(&LoadingUiState {
                                    title: "Preparing your world".into(),
                                    detail: format!("Spawn terrain ready: {} / {} chunks", spawn_progress.0, spawn_progress.1),
                                    progress_01: spawn_progress.0 as f32 / spawn_progress.1 as f32,
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
                                                let mut pos = origin * scale;
                                                for i in 1..=8 {
                                                    let candidate = origin * scale + dir * (i as f32 * 0.2);
                                                    if !w.get((candidate / scale).floor().as_ivec3()).is_empty() { break; }
                                                    pos = candidate;
                                                }
                                                if std::env::var_os("VOXEL_TOOL_REVIEW").is_some() { eprintln!("[tool-review] dropped {:?} count={} at={:?}",stack.item,stack.count,pos); }
                                                w.inventory.dropped.push(world::inventory::DroppedStack { pos, stack });
                                                w.sync_from_inventory();
                                                w.expedition.say("Stack dropped. Aim nearby and press F to retrieve.");
                                            }
                                        } else { w.expedition.say("Retrieve some dropped stacks first."); }
                                    }
                                }
                                UiAction::Factory(action) => {
                                    if let Some(world) = world.as_mut() {
                                        if let Some(block) = world::factory::action(world,player.pos,action) {
                                            creative.assign_hotbar_slot(creative.selected_slot,block);
                                            creative.tool = creative::ToolKind::Hand;
                                            player.selected_block = block;
                                        }
                                    }
                                }
                                UiAction::SaveWorld => { saves.request(true); }
                                UiAction::OpenWorld(id)=>library.open(id),
                                UiAction::RefreshWorlds=>library.refresh(),
                                UiAction::RenameWorld {id,name}=>library.rename(id,name),
                                UiAction::FavoriteWorld {id,favorite}=>library.favorite(id,favorite),
                                UiAction::StartWorld {
                                    name,
                                    reference,
                                    world_type,
                                    voxel_scale,
                                    seed,
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
                                    );
                                    if reference {
                                        if let Some(w)=world.as_mut() {
                                            reference_environment::install(w);
                                            reference_environment::camera(&mut player,selected_voxel_scale.meters());
                                            let center=chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                            let snapshot=w.snapshot_near(center,8);
                                            chunk_cache.reset_with_snapshot(center,&snapshot);
                                            chunk_streamer.reset();
                                            chunk_streamer.queue_needed_visible(&chunk_cache,center);
                                        }
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

                            if benchmark_config.enabled && !wand_review.as_ref().is_some_and(|r| !r.automatic) {
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
                            let mut outline = Vec::new();
                            let mut invert_outline = Vec::new();
                            let mut nearby_torch_count = 0usize;
                            let render_world =
                                !matches!(mode, AppMode::WorldSelect) && world.is_some();
                            let paused_render = !render_world;

                            if matches!(mode, AppMode::Playing) && world.is_some() {
                                let world = world.as_mut().unwrap();
                                let gameplay_start = Instant::now();
                                world.expedition.tick(gameplay_dt);
                                creative.update_feedback(gameplay_dt);
                                let machine_size=world::factory::step(world);
                                world.factory.tick(gameplay_dt,machine_size);

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

                                if benchmark_camera_active {
                                    if let Some(benchmark_state) = benchmark.as_ref() {
                                        apply_benchmark_camera_motion(
                                            benchmark_state,
                                            &mut player,
                                            world.sample_config(),
                                        );
                                    }
                                } else {
                                    for _ in 0..simulation_steps {
                                        if let Some(review)=sandbox_review.as_mut(){review.input(&mut player,&mut active_input,simulation::STEP_SECONDS);}
                                        if let Some(review) = water_review.as_mut() {
                                            review.input(&mut player, &mut active_input, world.voxel_scale.meters(), simulation::STEP_SECONDS);
                                        }
                                        update_player(
                                        &mut player,
                                        &mut active_input,
                                        world,
                                        simulation::STEP_SECONDS,

                                        );
                                        if let Some(review)=sandbox_review.as_mut(){review.observe(&player);}
                                        if let Some(review) = water_review.as_mut() {
                                            if let Some(shot) = review.observe(&player, world.voxel_scale.meters()) {
                                                let root = std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from)
                                                    .unwrap_or_else(|| "target/water-review".into());
                                                gpu.request_capture(root.join(format!("{shot}.png")));
                                            }
                                        }
                                    }
                                }
                                if let Some(review)=wand_review.as_mut() {
                                    review.input(world,&mut player,&mut creative,&mut active_input,&mut dirty_blocks,&mut dirty_chunks);
                                }
                                let edit_repeat_interval = creative.edit_repeat_interval_seconds();
                                let mut place_edit = consume_tool_action(
                                    active_input.place_block,
                                    &mut place_was_down,
                                    &mut place_repeat_timer,
                                    edit_repeat_interval,
                                    gameplay_dt,
                                );
                                let mut break_edit = consume_tool_action(
                                    active_input.break_block,
                                    &mut break_was_down,
                                    &mut break_repeat_timer,
                                    if creative.tool==creative::ToolKind::ExplosiveWand {f32::INFINITY}else{edit_repeat_interval},
                                    gameplay_dt,
                                );

                                let can_act=(grabbed || wand_review.as_ref().is_some_and(|r|r.automatic)) && !creative.any_ui_open() && !spawning;
                                if !can_act {world.adventure.cancel_cast_input();}
                                combat_audio.set_active(matches!(mode,AppMode::Playing) && (focused || wand_review.as_ref().is_some_and(|r|r.automatic)) && !spawning);
                                if world::adventure::tick(world,&mut player,gameplay_dt,active_input.cast_spell&&can_act,active_input.cycle_spell&&can_act,break_edit&&can_act&&creative.tool==creative::ToolKind::Hand) {break_edit=false;}
                                input.cycle_spell=false;
                                let eye = player.eye(world.voxel_scale);
                                let forward = player.forward_dir();
                                let hit = world::raycast_block_precise(
                                    world,
                                    player.precise_eye(world.voxel_scale) + forward.as_dvec3() * 0.0005,
                                    forward,
                                    creative.reach_distance_blocks(world.voxel_scale),
                                );
                                if input.pick_block {
                                    if grabbed && !creative.any_ui_open() {
                                        if let Some((cell, _)) = hit {
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
                                    }
                                    input.pick_block = false;
                                }
                                egui_ui.target_block = hit.map(|(cell, _)| world.get(cell));
                                egui_ui.target_cell_temp_c = hit.and_then(|(cell, _)| {
                                    let t = world.sandbox.cell(cell).temperature;
                                    (t > 35.0).then_some(t)
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
                                if input.interact {
                                    if grabbed && !creative.any_ui_open() && !spawning {
                                        if world.inventory.retrieve_nearby(player.pos * world.voxel_scale.meters()) {
                                            world.sync_from_inventory();
                                            break_edit=false;place_edit=false;
                                        } else if world::adventure::reliquaries::interact(world,&player) {
                                            break_edit=false;place_edit=false;
                                        } else if let Some((cell,_))=hit {
                                            if (cell.as_vec3()-eye).length()*world.voxel_scale.meters()<=6.0 {
                                                let target = world.get(cell);
                                                world::expedition::interact(world,cell,player.pos,&mut dirty_blocks,&mut dirty_chunks);
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
                                    input.interact=false;
                                }
                                // Bore upgrades share the asynchronous, frame-budgeted edit path.
                                if break_edit && can_act && active_input.alt_down && creative.tool == creative::ToolKind::Hand {
                                    if creative.pending_cells()==0 {
                                        if let Some((cell,_))=hit {
                                            if (cell.as_vec3()-eye).length()*world.voxel_scale.meters()<=6.0 {
                                                let targets=world::expedition::bore_targets(world,cell,player.pos);
                                                creative.queue_edit(world,creative::BrushMode::Erase,BlockId::Air,targets);
                                            }
                                        }
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
                                        .filter(|s| matches!(s.item, world::inventory::Item::Block(_)))
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
                                {
                                    if let Some(center) = raycast_target.hit {
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
                                    && !egui_ui.tool_feedback.air_placement {
                                    if let Some((cell, _)) = sight {
                                        outline.extend(build_outline_vertices(cell, [1.0, 0.68, 0.22, 0.85], 0.003));
                                    }
                                }

                                if let Some(center) = preview.center_block.filter(|_|egui_ui.show_hud && !creative.any_ui_open()) {
                                    let center_lines =
                                        build_outline_vertices(center, preview.color, 0.003/world.voxel_scale.meters());
                                    outline.extend(center_lines);
                                }

                                let survival_hand = creative.tool == creative::ToolKind::Hand
                                    && !player.flying
                                    && !creative.show_panel;
                                if survival_hand {
                                    match world.inventory.hotbar[creative.selected_slot].map(|s| s.item) {
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
                                            Some(world::inventory::Item::Block(_)) => {}
                                            _ => {
                                                place_edit = false;
                                            }
                                        }
                                    }
                                }

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
                                            resolve_placement_block(active_block, prev - cell)
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
                                    if creative.pending_cells()==0 {
                                        if let Some((point,radius))=world.adventure.take_terrain_blast() {
                                            let cells=creative::spell_blast_targets(world.voxel_scale.meters(),point,radius);
                                            creative.queue_blast(world,cells,None);
                                        }
                                    }
                                    let edit_started=Instant::now();
                                    let explosive_edit=creative.pending_explosive();
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
                                                if explosive_edit {world::expedition::blast_harvest(world,cell,&mut dirty_blocks,&mut dirty_chunks);}
                                                else {world::expedition::harvest(world,cell,&mut dirty_blocks,&mut dirty_chunks);}
                                                dirty_blocks.insert(cell);
                                                mark_chunk_and_neighbors_dirty(
                                                    &mut dirty_chunks,
                                                    cell,
                                                );
                                                cleanup_detached_torches(
                                                    world,
                                                    cell,
                                                    &mut dirty_blocks,
                                                    &mut dirty_chunks,
                                                );
                                            }
                                            creative::BrushMode::Place => {
                                                let Some(place_block) = placement_block else {
                                                    continue;
                                                };
                                                if world::factory::place(world,cell,place_block,&player,&mut dirty_blocks,&mut dirty_chunks) {
                                                    continue;
                                                }
                                                if place_block.is_torch()
                                                    && !is_torch_supported(world, cell, place_block)
                                                {
                                                    continue;
                                                }
                                                if !world.get(cell).is_empty() && !world.get(cell).is_liquid() {
                                                    continue;
                                                }
                                                if survival_hand {
                                                    let Some(canon) = world::inventory::Item::canonical_block(place_block) else {
                                                        continue;
                                                    };
                                                    let item = world::inventory::Item::Block(canon);
                                                    if world.inventory.count_item(item) == 0 {
                                                        continue;
                                                    }
                                                    world.set(cell, place_block);
                                                    if world.get(cell) == place_block {
                                                        let _ = world
                                                            .inventory
                                                            .consume_from_hotbar(creative.selected_slot, item);
                                                        world.sync_from_inventory();
                                                    } else {
                                                        continue;
                                                    }
                                                } else {
                                                    world.set(cell, place_block);
                                                }
                                                dirty_blocks.insert(cell);
                                                mark_chunk_and_neighbors_dirty(
                                                    &mut dirty_chunks,
                                                    cell,
                                                );
                                            }
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
                                    if explosive_edit {world.sync_from_inventory();}
                                }
                                if let Some(review)=sandbox_review.as_ref(){fluid_sim.set_enabled(review.running());}
                                fluid_sim.update(
                                    world,
                                    &player,

                                    gameplay_dt,
                                    &mut dirty_blocks,
                                    &mut dirty_chunks,
                                );
                                if let Some(review)=wand_review.as_mut() {
                                    if let Some(shot)=review.observe(world,&player,&creative,&preview) {
                                        let root=std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(||"target/wand-review".into());
                                        gpu.request_capture(root.join(shot));
                                    }
                                }
                                // Drain after all producers, including tools. A tool blast used
                                // to enqueue after the drain and lose its sound on the next tick.
                                for sound in std::mem::take(&mut world.adventure.sounds) {
                                    combat_audio.play(sound,player.eye(world.voxel_scale)*world.voxel_scale.meters(),player.forward_dir());
                                }
                                if let Some(review)=sandbox_review.as_mut(){
                                    let (reloaded,shot)=review.tick(world,&mut player,&day_night,fluid_sim,&mut dirty_blocks,&mut dirty_chunks);
                                    if reloaded {chunk_streamer.reset();let c=chunk_coord_from_world_block(player.pos.floor().as_ivec3());chunk_cache.reset_with_snapshot(c,&world.snapshot_near(c,4));chunk_streamer.queue_needed_visible(&chunk_cache,c);}
                                    if let Some(shot)=shot {let root=std::env::var_os("VOXEL_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(||"target/sandbox-review".into());gpu.request_capture(root.join(format!("{shot}.png")));}
                                }
                                cpu_stats.gameplay_ms =
                                    gameplay_start.elapsed().as_secs_f32() * 1000.0;

                                let center_chunk =
                                    chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                let snapshot_start = Instant::now();
                                let snapshot = world.snapshot_near(center_chunk,render_distance_settings().view_distance_chunks);
                                cpu_stats.snapshot_ms =
                                    snapshot_start.elapsed().as_secs_f32() * 1000.0;
                                let streaming_start = Instant::now();
                                let center_changed = chunk_cache
                                    .update_center_chunk_with_snapshot(center_chunk, &snapshot);
                                if center_changed {
                                    chunk_cache.retain_visible();
                                    chunk_streamer.retain_within_range(center_chunk);
                                    chunk_streamer.queue_needed_visible(&chunk_cache, center_chunk);
                                }

                                let new_keys = chunk_cache
                                    .expand_visible(&snapshot, chunk_streamer.queued_jobs());
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

                                chunk_streamer.enqueue_missing(
                                    gpu,
                                    &snapshot,
                                    world.revision(),
                                    &mut chunk_cache,
                                );
                                for (coord,mask) in chunk_cache.take_simulation_seeds() {world.activity.merge(coord,&mask);}
                                chunk_streamer.upload_finished(
                                    gpu,
                                    &mut chunk_cache,
                                    world.revision(),
                                    4,
                                );
                                cpu_stats.streaming_ms =
                                    streaming_start.elapsed().as_secs_f32() * 1000.0;

                                let world_scale = world.voxel_scale.meters();
                                let render_settings = render_distance_settings();
                                let far_plane_voxels = (render_settings.view_distance_chunks
                                    as f32
                                    * world::CHUNK_SIZE as f32
                                    * 1.20)
                                    .max(640.0);
                                let far_plane_world = (far_plane_voxels * world_scale).max(render::HORIZON_FAR_PLANE);
                                let render_eye = eye * world_scale;
                                gpu.update_horizon(world, render_eye);
                                let cull_view = Mat4::look_to_rh(eye, forward, Vec3::Y);
                                let proj = Mat4::perspective_rh(
                                    egui_ui.fov_degrees.to_radians(),
                                    gpu.aspect_ratio(),
                                    0.05 * world_scale,
                                    far_plane_world,
                                );
                                let cull_proj = Mat4::perspective_rh(
                                    egui_ui.fov_degrees.to_radians(),
                                    gpu.aspect_ratio(),
                                    0.05,
                                    far_plane_voxels,
                                );
                                let culling_start = Instant::now();
                                draw_list = build_draw_list(&chunk_cache, cull_proj * cull_view);
                                cpu_stats.culling_ms =
                                    culling_start.elapsed().as_secs_f32() * 1000.0;
                                let held_torch =
                                    held_torch_light(&player, world.voxel_scale, world_scale, world.expedition.lamp);
                                let torch_start = Instant::now();
                                let mut nearby_torches =
                                    collect_nearby_torches(&snapshot, player.pos, world_scale);
                                for blast in world.adventure.blasts.iter().filter(|b| b.age<0.28).take(4) {
                                    if nearby_torches.len()>=MAX_TORCH_LIGHTS {nearby_torches.pop();}
                                    nearby_torches.insert(0,TorchLight {
                                        position:blast.pos+blast.normal*0.3,
                                        radius:blast.radius*2.0+2.0,color:[1.0,0.64,0.25],
                                        strength:5.0*(1.0-blast.age/0.28).powi(2),
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
                                        },
                                    );
                                }
                                cpu_stats.torch_ms =
                                    torch_start.elapsed().as_secs_f32() * 1000.0;
                                nearby_torch_count = nearby_torches.len();

                                let weather_start = Instant::now();
                                if !freeze_time {
                                    day_night.advance(gameplay_dt);
                                }
                                let day_speed = DEFAULT_DAY_LENGTH_SECONDS
                                    / day_night.day_length_seconds().max(1.0);
                                let sky_motion_rate = 0.55 + day_speed * 0.65;
                                    sky_time += gameplay_dt * sky_motion_rate;
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
                                gpu.upload_lighting(
                                    &day_night.lighting(),
                                    &day_night.cloud_controls(),
                                    sky_time,
                                    held_torch,
                                    &nearby_torches,
                                    Some(&cloud_weather.field),
                                    render_debug_view,
                                    atmosphere_mode,
                                );
                                gpu.underwater = player.underwater;
                                gpu.upload_camera(player.precise_eye(world.voxel_scale) * world_scale as f64, world_scale, forward, proj, world.seed, world.generator_version);
                                cpu_stats.uniform_ms =
                                    uniform_start.elapsed().as_secs_f32() * 1000.0;
                            } else if render_world {
                                let world = world.as_mut().unwrap();

                                let day_speed = DEFAULT_DAY_LENGTH_SECONDS
                                    / day_night.day_length_seconds().max(1.0);
                                let sky_motion_rate = 0.55 + day_speed * 0.65;
                                sky_time += gameplay_dt * sky_motion_rate;
                                let streaming_start = Instant::now();
                                let snapshot = world.snapshot_near(chunk_coord_from_world_block(player.pos.floor().as_ivec3()), render_distance_settings().view_distance_chunks);
                                chunk_streamer.enqueue_missing(gpu, &snapshot, world.revision(), &mut chunk_cache);
                                for (coord,mask) in chunk_cache.take_simulation_seeds() {world.activity.merge(coord,&mask);}
                                chunk_streamer.upload_finished(
                                    gpu,
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
                                let far_plane_world = (far_plane_voxels * world_scale).max(render::HORIZON_FAR_PLANE);
                                let render_eye = eye * world_scale;
                                gpu.update_horizon(world, render_eye);
                                let cull_view = Mat4::look_to_rh(eye, forward, Vec3::Y);
                                let proj = Mat4::perspective_rh(
                                    egui_ui.fov_degrees.to_radians(),
                                    gpu.aspect_ratio(),
                                    0.05 * world_scale,
                                    far_plane_world,
                                );
                                let cull_proj = Mat4::perspective_rh(
                                    egui_ui.fov_degrees.to_radians(),
                                    gpu.aspect_ratio(),
                                    0.05,
                                    far_plane_voxels,
                                );
                                let culling_start = Instant::now();
                                draw_list = build_draw_list(&chunk_cache, cull_proj * cull_view);
                                cpu_stats.culling_ms =
                                    culling_start.elapsed().as_secs_f32() * 1000.0;
                                let held_torch =
                                    held_torch_light(&player, world.voxel_scale, world_scale, world.expedition.lamp);
                                let torch_start = Instant::now();
                                let mut nearby_torches =
                                    collect_nearby_torches(&snapshot, player.pos, world_scale);
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
                                        },
                                    );
                                }
                                cpu_stats.torch_ms =
                                    torch_start.elapsed().as_secs_f32() * 1000.0;
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
                                gpu.upload_lighting(
                                    &day_night.lighting(),
                                    &day_night.cloud_controls(),
                                    sky_time,
                                    held_torch,
                                    &nearby_torches,
                                    Some(&cloud_weather.field),
                                    render_debug_view,
                                    atmosphere_mode,
                                );
                                gpu.underwater = player.underwater;
                                gpu.upload_camera(player.precise_eye(world.voxel_scale) * world_scale as f64, world_scale, forward, proj, world.seed, world.generator_version);
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
                                gpu.upload_camera(glam::DVec3::ZERO, 1.0, Vec3::NEG_Z, Mat4::IDENTITY, 0, 0);
                                cpu_stats.uniform_ms =
                                    uniform_start.elapsed().as_secs_f32() * 1000.0;
                            }

                            let render_start = Instant::now();
                            let scene=world.as_ref().map(|w|mesh::models::build(w,&player)).unwrap_or_default();
                            gpu.upload_scene(&scene);
                            match gpu.render_cached_with_overlay(
                                &chunk_cache,
                                &draw_list,
                                &outline,
                                &invert_outline,
                                &[],
                                paused_render,
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
                                Ok(()) => {
                                    cpu_stats.render_ms =
                                        render_start.elapsed().as_secs_f32() * 1000.0;
                                    cpu_stats.total_ms =
                                        frame_start.elapsed().as_secs_f32() * 1000.0;
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
                                        },
                                    };
                                    if let Some(review)=benchmark.as_mut() {
                                        let elapsed=review.motion_started_at.elapsed().as_secs_f64();
                                        // Bound diagnostics so a slow run cannot amplify its own
                                        // stalls with unbounded logging. Normal play is unaffected.
                                        if sample.cpu.total_ms > 50.0 && benchmark_hitch_reports < 24 {
                                            benchmark_hitch_reports += 1;
                                            println!("[frame-hitch] seconds={elapsed:.2} total_ms={:.2} ui={:.2} gameplay={:.2} snapshot={:.2} stream={:.2} cull={:.2} torch={:.2} weather={:.2} uniforms={:.2} render={:.2} acquire={:.2} submit={:.2} shadow_draws={} terrain_draws={}",
                                                sample.cpu.total_ms, sample.cpu.ui_ms, sample.cpu.gameplay_ms,
                                                sample.cpu.snapshot_ms, sample.cpu.streaming_ms, sample.cpu.culling_ms,
                                                sample.cpu.torch_ms, sample.cpu.weather_ms, sample.cpu.uniform_ms,
                                                sample.cpu.render_ms, sample.render.surface_acquire_ms, sample.render.submit_ms,
                                                sample.render.shadow_draw_calls, sample.render.terrain_draw_calls);
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
                                            println!("[simulation-profile] active={} inflight={} pending={} stepped={} updates={} source_mib={:.2} source_failed={} writeback_pressure={} recovering={}",
                                                physics.active_chunks, physics.inflight_chunks, physics.pending_updates,
                                                physics.simulated_chunks_last_step, physics.applied_updates_last_step,
                                                physics.source_bytes as f64/1048576.0, physics.source_failed,
                                                physics.writeback_pressure, physics.recovering);
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
                                        println!("[live-profile] fps={:.2} p99_ms={:.3} cpu_game_ms={:.3} cpu_stream_ms={:.3} cpu_uniform_ms={:.3} gpu_ms={:.3} terrain_ms={:.3} clouds_ms={:.3} visible={} missing={} physics={} active={} updates={} gpu_mib={:.2}",
                                            perf_snapshot.fps_avg, perf_snapshot.frame_ms_p99, perf_snapshot.cpu_avg.gameplay_ms,
                                            perf_snapshot.cpu_avg.streaming_ms, perf_snapshot.cpu_avg.uniform_ms,
                                            perf_snapshot.render_avg.gpu_pass_total_ms, perf_snapshot.render_avg.gpu_terrain_ms,
                                            perf_snapshot.render_avg.gpu_cloud_raymarch_ms, chunk_cache.visible.len(), missing,
                                            physics_enabled, physics.active_chunks, physics.applied_updates_last_step,
                                            sample.chunk.gpu_resident_bytes as f64/1048576.0);
                                    }

                                    if let Some(benchmark_state) = benchmark.as_mut() {
                                        if benchmark_state.auto_started && !benchmark_state.measuring {
                                            if benchmark_state.phase_started_at.elapsed().as_secs_f32()
                                                >= benchmark_state.config.warmup_seconds
                                            {
                                                benchmark_state.measuring = true;
                                                benchmark_state.phase_started_at = Instant::now();
                                                benchmark_state.measured_samples.clear();
                                                println!("[benchmark] warmup complete");
                                            }
                                        }

                                        if benchmark_state.measuring {
                                            benchmark_state.measured_samples.push(sample);
                                            if benchmark_state.phase_started_at.elapsed().as_secs_f32()
                                                >= benchmark_state.config.duration_seconds
                                            {
                                                let summary = snapshot_from_samples(
                                                    &benchmark_state.measured_samples,
                                                    gpu.adapter_summary(),
                                                    [gpu.size().width, gpu.size().height],
                                                    render_distance_settings(),
                                                );
                                                log_benchmark_summary(&summary, &benchmark_state.config);
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
                    let sensitivity = egui_ui.as_ref().map_or(0.0025, |ui| ui.mouse_sensitivity);
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
                        if !library.busy(){library.refresh();}
                        next_library_refresh=Instant::now()+Duration::from_secs(5);
                    }
                    if let Some(ui)=egui_ui.as_mut() {
                        if ui.library_revision!=library.revision {
                            ui.library_entries=std::sync::Arc::clone(&library.entries);
                            ui.library_revision=library.revision;
                        }
                        ui.library_busy=library.busy();ui.library_loading=library.loading();
                        ui.library_status=library.status.clone();
                        if preferences_render_applied {
                            if let StartupState::Ready {gpu,..}=&startup {
                                preferences.tick(preferences::Preferences::capture(ui,gpu.vsync(),gpu.fxaa_enabled,atmosphere_mode,selected_world,selected_voxel_scale,
                                    windowed_size.0,windowed_size.1,window.as_ref().is_some_and(|w|w.fullscreen().is_some())),exit_pending);
                            }
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
                    if let Some(w) = world.as_mut() {
                        if occluded || window.as_ref().is_some_and(|win|win.inner_size().width==0 || win.inner_size().height==0) {
                            let center = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                            w.stream_regions(center, render_distance_settings().view_distance_chunks);
                        }
                    }
                    // Review worlds are disposable and must not replace the
                    // player's autosave or introduce background save cost.
                    if !benchmark_config.enabled {
                        saves.tick(world.as_mut(), &player, &day_night);
                    }
                    if let Some(ui) = egui_ui.as_mut() {
                        if let Some(status) = &saves.status {ui.persistence_status=Some(status.clone());}
                    }
                    if exit_pending {
                        if saves.failed {exit_pending=false;}
                        else if !saves.busy() && !preferences.busy() && !library.busy() {elwt.exit();return;}
                    }
                }
                if let Some(window) = window.as_ref() {
                    if occluded || window.inner_size().width == 0 || window.inner_size().height == 0 {
                        elwt.set_control_flow(ControlFlow::WaitUntil(Instant::now()+Duration::from_millis(100)));
                    } else if !focused && !benchmark_config.enabled && Instant::now() < next_background_frame {
                        elwt.set_control_flow(ControlFlow::WaitUntil(next_background_frame));
                    } else {
                        window.request_redraw();
                        next_background_frame = Instant::now() + Duration::from_millis(100);
                    }
                }
            }

            _ => {}
        }
    })?;

    Ok(())
}

