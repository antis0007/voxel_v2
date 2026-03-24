use anyhow::Result;
use glam::{Mat4, Vec3, Vec3Swizzles};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use winit::{
    dpi::PhysicalSize,
    event::*,
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowAttributes},
};

mod creative;
mod egui_ui;
mod fluid;
mod material;
mod mesh;
mod perf;
mod player;
mod render;
mod world;

use creative::CreativeState;
use egui_ui::{
    EguiUi, LoadingUiState, PhysicsUiState, RenderUiState, TimeUiState, UiAction, UiMode,
};
use fluid::{FluidSimulation, VoxelScalePreset};
use mesh::{build_outline_vertices, build_outline_vertices_many};
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
    BuildingFluidSimulation {
        gpu: Gpu,
        chunk_streamer: ChunkStreamer,
    },
    Ready {
        gpu: Gpu,
        chunk_streamer: ChunkStreamer,
        fluid_sim: FluidSimulation,
    },
    Failed(String),
}

#[derive(Clone, Copy, Debug)]
struct BenchmarkConfig {
    enabled: bool,
    world_type: WorldType,
    voxel_scale: VoxelScalePreset,
    seed: u32,
    weather_preset: Option<WeatherPreset>,
    width: u32,
    height: u32,
    warmup_seconds: f32,
    duration_seconds: f32,
    motion: BenchmarkMotionMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BenchmarkMotionMode {
    Static,
    Orbit,
}

impl BenchmarkMotionMode {
    fn label(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Orbit => "orbit",
        }
    }
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            world_type: WorldType::Hills,
            voxel_scale: VoxelScalePreset::OneMeter,
            seed: 0xA341_316C,
            weather_preset: None,
            width: 1280,
            height: 720,
            warmup_seconds: 4.0,
            duration_seconds: 8.0,
            motion: BenchmarkMotionMode::Static,
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
                _ => BenchmarkMotionMode::Static,
            };
        }
    }

    config
}

fn benchmark_motion_active(benchmark: Option<&BenchmarkState>) -> bool {
    benchmark.is_some_and(|state| {
        state.auto_started && !matches!(state.config.motion, BenchmarkMotionMode::Static)
    })
}

fn apply_benchmark_camera_motion(
    state: &BenchmarkState,
    player: &mut Player,
    voxel_scale: VoxelScalePreset,
) {
    if matches!(state.config.motion, BenchmarkMotionMode::Static) {
        return;
    }

    let inv_scale = voxel_scale.meters().recip().max(1e-6);
    let elapsed = state.motion_started_at.elapsed().as_secs_f32();
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
    // Do NOT clear physical movement keys to prevent movement lockups
    input.break_block = false;
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
) -> Option<TorchLight> {
    if !player.selected_block.is_torch() {
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
        radius: TORCH_LIGHT_RADIUS * world_scale,
        color: TORCH_LIGHT_COLOR,
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
    let max_radius_blocks = 20.0f32;
    let max_radius_sq = max_radius_blocks * max_radius_blocks;
    let chunk_radius =
        ((max_radius_blocks.ceil() as i32) + world::CHUNK_SIZE - 1) / world::CHUNK_SIZE;
    let mut torches = Vec::new();
    for dz in -chunk_radius..=chunk_radius {
        for dy in -chunk_radius..=chunk_radius {
            for dx in -chunk_radius..=chunk_radius {
                let coord = center_chunk + glam::IVec3::new(dx, dy, dz);
                let Some(chunk) = snapshot.chunks.get(&coord) else {
                    continue;
                };
                let ox = coord.x * world::CHUNK_SIZE;
                let oy = coord.y * world::CHUNK_SIZE;
                let oz = coord.z * world::CHUNK_SIZE;
                for z in 0..world::CHUNK_SIZE {
                    for y in 0..world::CHUNK_SIZE {
                        for x in 0..world::CHUNK_SIZE {
                            let block = chunk.blocks[world::ChunkData::local_index(x, y, z)];
                            if !block.is_torch() {
                                continue;
                            }
                            let pos = glam::IVec3::new(ox + x, oy + y, oz + z);
                            let delta = (pos - center).as_vec3();
                            let dist_sq = delta.length_squared();
                            if dist_sq > max_radius_sq {
                                continue;
                            }
                            torches.push((dist_sq, pos));
                        }
                    }
                }
            }
        }
    }
    torches.sort_by(|a, b| a.0.total_cmp(&b.0));
    torches.truncate(MAX_TORCH_LIGHTS);
    torches
        .into_iter()
        .map(|(_, pos)| {
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
                position: (pos.as_vec3() + light_offset) * world_scale,
                radius: TORCH_LIGHT_RADIUS * world_scale,
                color: TORCH_LIGHT_COLOR,
                strength: TORCH_LIGHT_STRENGTH,
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
    fluid_sim: &mut FluidSimulation,
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

    let start_chunk = chunk_coord_from_world_block(player.pos.floor().as_ivec3());
    let snapshot = new_world.snapshot();
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
    let _ = window
        .set_cursor_grab(CursorGrabMode::Locked)
        .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
    window.set_cursor_visible(false);
    *grabbed = true;
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
            "[benchmark] gpu_avg_ms passes={:.3} shadow={:.3} terrain={:.3} raymarch={:.3} temporal={:.3} sky={:.3} composite={:.3} outline={:.3} ui={:.3}",
            snapshot.render_avg.gpu_pass_total_ms,
            snapshot.render_avg.gpu_shadow_ms,
            snapshot.render_avg.gpu_terrain_ms,
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
    let event_loop = EventLoop::new()?;
    let mut window: Option<Arc<Window>> = None;

    let mesh_workers = thread::available_parallelism()
        .map(|count| count.get().saturating_sub(1).max(1))
        .unwrap_or(1);

    let mut startup = StartupState::WaitingForWindow;
    let mut egui_ui: Option<EguiUi> = None;

    let mut selected_world = WorldType::Superflat;
    let mut selected_voxel_scale = VoxelScalePreset::OneMeter;
    let mut selected_seed = default_random_seed();
    if benchmark_config.enabled {
        selected_world = benchmark_config.world_type;
        selected_voxel_scale = benchmark_config.voxel_scale;
        selected_seed = benchmark_config.seed;
    }
    let initial_window_size = if benchmark_config.enabled {
        PhysicalSize::new(benchmark_config.width, benchmark_config.height)
    } else {
        PhysicalSize::new(1280, 720)
    };

    let mut world: Option<World> = None;
    let mut player = Player::new();
    let mut day_night = DayNightCycle::new(DEFAULT_START_TIME_OF_DAY, DEFAULT_DAY_LENGTH_SECONDS);
    let mut cloud_weather = WeatherRuntimeState::new(selected_seed);
    let mut freeze_time = false;
    let mut input = Input::default();
    let mut creative = CreativeState::default();
    player.selected_block = creative.selected_block();
    let mut render_debug_view = RenderDebugView::default();
    let mut atmosphere_mode = AtmosphereMode::default();

    let mut grabbed = false;
    let mut focused = true;
    let mut mode = AppMode::WorldSelect;
    let mut last = Instant::now();
    let mut sky_time = 0.0f32;

    let mut chunk_cache = ChunkCache::new(glam::IVec3::ZERO);
    chunk_cache.reset(glam::IVec3::ZERO);
    let mut dirty_blocks = HashSet::new();
    let mut dirty_chunks = HashSet::new();
    let mut seam_cache: HashMap<glam::IVec3, u8> = HashMap::new();
    let mut physics_enabled = false;
    let mut fps_counter = FpsCounter::new();
    let mut place_was_down = false;
    let mut break_was_down = false;
    let mut place_repeat_timer = 0.0f32;
    let mut break_repeat_timer = 0.0f32;
    let mut pending_tool_action: Option<egui_ui::ToolUseAction> = None;
    let mut perf_tracker = PerfTracker::default();
    let mut perf_snapshot = PerfSnapshot::default();
    let mut benchmark = benchmark_config.enabled.then(|| BenchmarkState {
        config: benchmark_config,
        auto_started: false,
        measuring: false,
        phase_started_at: Instant::now(),
        motion_started_at: Instant::now(),
        motion_anchor_blocks: Vec3::ZERO,
        measured_samples: Vec::new(),
    });

    #[allow(deprecated)]
    event_loop.run(move |event, elwt| {
        let loading = !matches!(
            startup,
            StartupState::Ready { .. } | StartupState::Failed(_)
        );
        let active_gameplay = matches!(mode, AppMode::Playing);
        elwt.set_control_flow(if loading || active_gameplay {
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
                                .with_title("Minecraft-style voxel prototype")
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
                            startup = StartupState::BuildingRenderer {
                                pending: bootstrap.into_pending_init(),
                            };
                            window_ref.request_redraw();
                        }
                        Err(err) => {
                            startup = StartupState::Failed(format!("{err:#}"));
                            elwt.exit();
                        }
                    }
                }
            }

            Event::WindowEvent { event, .. } => {
                let Some(window) = window.as_ref() else {
                    return;
                };

                let egui_consumed = egui_ui
                    .as_mut()
                    .map(|ui| ui.on_window_event(window, &event))
                    .unwrap_or(false);

                match event {
                    WindowEvent::CloseRequested => elwt.exit(),

                    WindowEvent::Resized(size) => match &mut startup {
                        StartupState::BuildingRenderer { pending } => pending.resize(size),
                        StartupState::BuildingChunkStreamer { gpu } => gpu.resize(size),
                        StartupState::BuildingFluidSimulation { gpu, .. } => gpu.resize(size),
                        StartupState::Ready { gpu, .. } => gpu.resize(size),
                        _ => {}
                    },

                    WindowEvent::Focused(is_focused) => {
                        focused = is_focused;
                        if !focused && world.is_some() {
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
                        // Track raw keyboard state for movement immediately so it never gets out of sync
                        match key.physical_key {
                            PhysicalKey::Code(KeyCode::KeyW) => input.forward = pressed,
                            PhysicalKey::Code(KeyCode::KeyS) => input.back = pressed,
                            PhysicalKey::Code(KeyCode::KeyA) => input.left = pressed,
                            PhysicalKey::Code(KeyCode::KeyD) => input.right = pressed,
                            PhysicalKey::Code(KeyCode::Space) => input.jump = pressed,
                            PhysicalKey::Code(KeyCode::ShiftLeft)
                            | PhysicalKey::Code(KeyCode::ShiftRight) => input.sneak = pressed,
                            PhysicalKey::Code(KeyCode::AltLeft) => input.alt_down = pressed,
                            PhysicalKey::Code(KeyCode::ControlLeft)
                            | PhysicalKey::Code(KeyCode::ControlRight) => {
                                input.sprint = pressed;
                                input.ctrl_down = pressed;
                            }
                            _ => {}
                        }
                        if egui_consumed
                            && !matches!(
                                key.physical_key,
                                PhysicalKey::Code(KeyCode::Escape)
                                    | PhysicalKey::Code(KeyCode::KeyP)
                                    | PhysicalKey::Code(KeyCode::KeyB)
                                    | PhysicalKey::Code(KeyCode::KeyQ)
                                    | PhysicalKey::Code(KeyCode::KeyZ)
                                    | PhysicalKey::Code(KeyCode::KeyX)
                                    | PhysicalKey::Code(KeyCode::KeyC)
                                    | PhysicalKey::Code(KeyCode::KeyV)
                                    | PhysicalKey::Code(KeyCode::KeyG)
                                    | PhysicalKey::Code(KeyCode::Tab)
                                    | PhysicalKey::Code(KeyCode::Digit1)
                                    | PhysicalKey::Code(KeyCode::Digit2)
                                    | PhysicalKey::Code(KeyCode::Digit3)
                                    | PhysicalKey::Code(KeyCode::Digit4)
                                    | PhysicalKey::Code(KeyCode::Digit5)
                                    | PhysicalKey::Code(KeyCode::Digit6)
                                    | PhysicalKey::Code(KeyCode::Digit7)
                                    | PhysicalKey::Code(KeyCode::Digit8)
                                    | PhysicalKey::Code(KeyCode::Digit9)
                            )
                        {
                            return;
                        }

                        if matches!(mode, AppMode::WorldSelect) {
                            if pressed {
                                match key.physical_key {
                                    PhysicalKey::Code(KeyCode::Digit1) => {
                                        selected_world = WorldType::Superflat
                                    }
                                    PhysicalKey::Code(KeyCode::Digit2) => {
                                        selected_world = WorldType::Hills
                                    }
                                    PhysicalKey::Code(KeyCode::Digit3) => {
                                        selected_world = WorldType::Ridged
                                    }
                                    PhysicalKey::Code(KeyCode::ArrowDown)
                                    | PhysicalKey::Code(KeyCode::KeyS) => {
                                        selected_world = cycle_world(selected_world, 1)
                                    }
                                    PhysicalKey::Code(KeyCode::ArrowUp)
                                    | PhysicalKey::Code(KeyCode::KeyW) => {
                                        selected_world = cycle_world(selected_world, -1)
                                    }
                                    PhysicalKey::Code(KeyCode::ArrowLeft)
                                    | PhysicalKey::Code(KeyCode::KeyA) => {
                                        selected_voxel_scale =
                                            cycle_voxel_scale(selected_voxel_scale, -1)
                                    }
                                    PhysicalKey::Code(KeyCode::ArrowRight)
                                    | PhysicalKey::Code(KeyCode::KeyD) => {
                                        selected_voxel_scale =
                                            cycle_voxel_scale(selected_voxel_scale, 1)
                                    }
                                    PhysicalKey::Code(KeyCode::Digit4) => {
                                        selected_voxel_scale = VoxelScalePreset::OneMeter
                                    }
                                    PhysicalKey::Code(KeyCode::Digit5) => {
                                        selected_voxel_scale = VoxelScalePreset::HalfMeter
                                    }
                                    PhysicalKey::Code(KeyCode::Digit6) => {
                                        selected_voxel_scale = VoxelScalePreset::QuarterMeter
                                    }
                                    PhysicalKey::Code(KeyCode::Enter) => {
                                        if let StartupState::Ready {
                                            chunk_streamer,
                                            fluid_sim,
                                            ..
                                        } = &mut startup
                                        {
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
                                            cloud_weather.reset(selected_seed);
                                            physics_enabled = false;
                                            freeze_time = false;
                                        }
                                    }
                                    PhysicalKey::Code(KeyCode::Escape) => elwt.exit(),
                                    _ => {}
                                }
                            }
                            return;
                        }

                        match key.physical_key {
                            PhysicalKey::Code(KeyCode::Escape) if fresh_press => {
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
                            PhysicalKey::Code(KeyCode::KeyP) if fresh_press => {
                                physics_enabled = !physics_enabled;
                                if let StartupState::Ready { fluid_sim, .. } = &mut startup {
                                    fluid_sim.set_enabled(physics_enabled);
                                }
                            }
                            PhysicalKey::Code(KeyCode::KeyB) if fresh_press => {
                                creative.show_panel = !creative.show_panel;
                                sync_creative_ui_cursor(
                                    window,
                                    &mut grabbed,
                                    focused,
                                    &mode,
                                    &creative,
                                );
                            }
                            PhysicalKey::Code(KeyCode::Tab) if fresh_press => {
                                creative.show_palette = !creative.show_palette;
                                sync_creative_ui_cursor(
                                    window,
                                    &mut grabbed,
                                    focused,
                                    &mode,
                                    &creative,
                                );
                            }
                            PhysicalKey::Code(KeyCode::KeyQ) if fresh_press => {
                                creative.show_quick_select = !creative.show_quick_select;
                                sync_creative_ui_cursor(
                                    window,
                                    &mut grabbed,
                                    focused,
                                    &mode,
                                    &creative,
                                );
                            }
                            PhysicalKey::Code(KeyCode::KeyZ) if fresh_press => {
                                creative.tool = creative::ToolKind::Hand;
                            }
                            PhysicalKey::Code(KeyCode::KeyX) if fresh_press => {
                                creative.tool = creative::ToolKind::Brush;
                            }
                            PhysicalKey::Code(KeyCode::KeyC) if fresh_press => {
                                creative.tool = creative::ToolKind::BuildersWand;
                            }
                            PhysicalKey::Code(KeyCode::KeyV) if fresh_press => {
                                creative.tool = creative::ToolKind::DestructorWand;
                            }
                            PhysicalKey::Code(KeyCode::KeyG) if fresh_press => {
                                creative.tool = creative::ToolKind::AreaTool;
                            }
                            _ if fresh_press => {
                                if let Some(slot) = hotbar_slot_from_key(key.physical_key) {
                                    player.selected_block = creative.select_slot(slot);
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
                            AppMode::WorldSelect => {
                                if button == MouseButton::Left && pressed {
                                    if let StartupState::Ready {
                                        chunk_streamer,
                                        fluid_sim,
                                        ..
                                    } = &mut startup
                                    {
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
                                        cloud_weather.reset(selected_seed);
                                        physics_enabled = false;
                                        freeze_time = false;
                                    }
                                }
                            }
                            AppMode::Paused => {
                                if button == MouseButton::Left && pressed {
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
                            }
                            AppMode::Playing => {
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
                        if matches!(mode, AppMode::WorldSelect) {
                            if delta.abs() > 0.01 {
                                selected_world =
                                    cycle_world(selected_world, if delta < 0.0 { 1 } else { -1 });
                            }
                        } else {
                            input.wheel += delta;
                        }
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
                                    startup = StartupState::Failed(format!("{err:#}"));
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

                            let chunk_streamer = ChunkStreamer::new(&gpu.device, mesh_workers);

                            match render_result {
                                Ok(()) => {
                                    startup = StartupState::BuildingFluidSimulation {
                                        gpu,
                                        chunk_streamer,
                                    };
                                    window.request_redraw();
                                }
                                Err(err) => {
                                    startup = StartupState::Failed(format!(
                                        "Surface error during startup: {err:?}"
                                    ));
                                    elwt.exit();
                                }
                            }
                        }

                        StartupState::BuildingFluidSimulation { .. } => {
                            let Some(egui_ui) = egui_ui.as_mut() else {
                                return;
                            };

                            let (mut gpu, chunk_streamer) = match std::mem::replace(
                                &mut startup,
                                StartupState::WaitingForWindow,
                            ) {
                                StartupState::BuildingFluidSimulation {
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

                            let mut fluid_sim = FluidSimulation::new(&gpu.device);
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
                                    startup = StartupState::Failed(format!(
                                        "Surface error during startup: {err:?}"
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

                            let frame_start = Instant::now();
                            let now = Instant::now();
                            let raw_frame_dt = (now - last).as_secs_f32();
                            let frame_dt = raw_frame_dt.min(0.033);
                            let display_frame_dt = raw_frame_dt.min(0.25);
                            last = now;
                            if !matches!(mode, AppMode::WorldSelect) {
                                fps_counter.tick(display_frame_dt);
                            }
                            let gameplay_dt = if matches!(mode, AppMode::Playing) {
                                frame_dt
                            } else {
                                0.0
                            };

                            gpu.begin_frame_stats();
                            chunk_streamer.begin_frame_stats();

                            if let Some(benchmark_state) = benchmark.as_mut() {
                                if !benchmark_state.auto_started && world.is_none() {
                                    seam_cache.clear();
                                    selected_world = benchmark_state.config.world_type;
                                    selected_voxel_scale = benchmark_state.config.voxel_scale;
                                    selected_seed = benchmark_state.config.seed;
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
                                    if let Some(preset) = benchmark_state.config.weather_preset {
                                        day_night.set_weather_preset(preset);
                                    }
                                    cloud_weather.reset(selected_seed);
                                    physics_enabled = false;
                                    freeze_time = false;
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

                            let ui_start = Instant::now();
                            egui_ui.begin_frame(window.as_ref());
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
                                settings: previous_render_settings,
                                debug_view: render_debug_view,
                                atmosphere_mode,
                            };
                            let action = egui_ui.build_ui(
                                ui_mode(&mode),
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
                            atmosphere_mode = render_ui.atmosphere_mode;
                            fluid_sim.set_enabled(physics_enabled);

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
                                    seam_cache.clear();
                                    chunk_cache.retain_visible();
                                    chunk_streamer.retain_visible(&chunk_cache.visible_keys);
                                    chunk_streamer.queue_needed_visible(
                                        &chunk_cache,
                                        chunk_cache.center_chunk,
                                    );
                                    chunk_streamer.remesh_changed_seams(
                                        &chunk_cache,
                                        &mut seam_cache,
                                        world_ref.revision(),
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

                            match action {
                                UiAction::None => {}
                                UiAction::StartWorld {
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
                                    seam_cache.clear();
                                    selected_world = world_type;
                                    selected_voxel_scale = voxel_scale;
                                    selected_seed = seed;
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
                                    cloud_weather.reset(selected_seed);
                                    physics_enabled = false;
                                    freeze_time = false;
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
                                    world = None;
                                    seam_cache.clear();
                                    chunk_cache.reset(glam::IVec3::ZERO);
                                    chunk_streamer.reset();
                                    dirty_blocks.clear();
                                    dirty_chunks.clear();
                                }
                                UiAction::Quit => {
                                    elwt.exit();
                                }
                            }

                            player.selected_block = creative.selected_block();

                            sync_creative_ui_cursor(
                                window,
                                &mut grabbed,
                                focused,
                                &mode,
                                &creative,
                            );

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

                                let benchmark_camera_active = benchmark_motion_active(benchmark.as_ref());
                                let mut active_input = if benchmark_camera_active {
                                    Input::default()
                                } else {
                                    input.clone()
                                };
                                if creative.any_ui_open() {
                                    active_input.forward = false;
                                    active_input.back = false;
                                    active_input.left = false;
                                    active_input.right = false;
                                    active_input.jump = false;
                                    active_input.sneak = false;
                                    active_input.break_block = false;
                                    active_input.place_block = false;
                                }

                                if input.wheel.abs() > 0.0 {
                                    if !creative.any_ui_open() {
                                        let dir = if input.wheel > 0.0 { 1 } else { -1 };
                                        if active_input.ctrl_down {
                                            creative.adjust_cursor_size(dir);
                                        } else if active_input.sneak {
                                            creative.adjust_range(dir);
                                        } else {
                                            player.selected_block = creative.cycle_hotbar_slot(dir);
                                        }
                                    }
                                    input.wheel = 0.0;
                                }

                                if benchmark_camera_active {
                                    if let Some(benchmark_state) = benchmark.as_ref() {
                                        apply_benchmark_camera_motion(
                                            benchmark_state,
                                            &mut player,
                                            world.voxel_scale,
                                        );
                                    }
                                } else {
                                    update_player(
                                        &mut player,
                                        &mut active_input,
                                        world,
                                        gameplay_dt,
                                        creative.shift_lock,
                                    );
                                }
                                let edit_repeat_interval = creative.edit_repeat_interval_seconds();
                                let place_edit = consume_tool_action(
                                    active_input.place_block,
                                    &mut place_was_down,
                                    &mut place_repeat_timer,
                                    edit_repeat_interval,
                                    gameplay_dt,
                                );
                                let break_edit = consume_tool_action(
                                    active_input.break_block,
                                    &mut break_was_down,
                                    &mut break_repeat_timer,
                                    edit_repeat_interval,
                                    gameplay_dt,
                                );

                                let eye = player.eye(world.voxel_scale);
                                let forward = player.forward_dir();
                                let hit = raycast_block(
                                    world,
                                    eye + forward * 0.0005,
                                    forward,
                                    creative.reach_distance_blocks(world.voxel_scale),
                                );
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

                                let preview = creative::get_preview(
                                    &creative,
                                    world,
                                    raycast_target,
                                    active_input.place_block,
                                    active_input.break_block,
                                    active_input.sneak,
                                    false,
                                );

                                let lines = build_outline_vertices_many(
                                    &preview.blocks,
                                    preview.color,
                                    0.002,
                                );
                                if preview.invert_blend {
                                    invert_outline = lines;
                                } else {
                                    outline = lines;
                                }

                                if let Some(center) = preview.center_block {
                                    let center_lines =
                                        build_outline_vertices(center, [0.0, 0.0, 0.0, 0.8], 0.005);
                                    outline.extend(center_lines);
                                }

                                if let Some((edit_mode, targets)) = creative::edit_blocks(
                                    &creative,
                                    world,
                                    &player,
                                    raycast_target,
                                    place_edit,
                                    break_edit,
                                    active_input.sneak,
                                ) {
                                    pending_tool_action = Some(match edit_mode {
                                        creative::BrushMode::Place => egui_ui::ToolUseAction::Place,
                                        creative::BrushMode::Erase => egui_ui::ToolUseAction::Break,
                                    });
                                    let placement_block = hit
                                        .and_then(|(cell, prev)| {
                                            resolve_placement_block(
                                                creative.selected_block(),
                                                prev - cell,
                                            )
                                        })
                                        .or_else(|| Some(creative.selected_block()));
                                    for cell in targets {
                                        match edit_mode {
                                            creative::BrushMode::Erase => {
                                                if world.get(cell) == BlockId::Air {
                                                    continue;
                                                }
                                                world.set(cell, BlockId::Air);
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
                                                if place_block.is_torch()
                                                    && !is_torch_supported(world, cell, place_block)
                                                {
                                                    continue;
                                                }
                                                if world.get(cell) == place_block {
                                                    continue;
                                                }
                                                world.set(cell, place_block);
                                                dirty_blocks.insert(cell);
                                                mark_chunk_and_neighbors_dirty(
                                                    &mut dirty_chunks,
                                                    cell,
                                                );
                                            }
                                        }
                                    }
                                }
                                fluid_sim.update(
                                    gpu,
                                    world,
                                    &chunk_cache,
                                    player.pos,
                                    gameplay_dt,
                                    &mut dirty_blocks,
                                    &mut dirty_chunks,
                                );
                                cpu_stats.gameplay_ms =
                                    gameplay_start.elapsed().as_secs_f32() * 1000.0;

                                let center_chunk =
                                    chunk_coord_from_world_block(player.pos.floor().as_ivec3());
                                let snapshot_start = Instant::now();
                                let snapshot = world.snapshot();
                                cpu_stats.snapshot_ms =
                                    snapshot_start.elapsed().as_secs_f32() * 1000.0;
                                let streaming_start = Instant::now();
                                let center_changed = chunk_cache
                                    .update_center_chunk_with_snapshot(center_chunk, &snapshot);
                                if center_changed {
                                    chunk_cache.retain_visible();
                                    chunk_streamer.retain_within_range(center_chunk);
                                    chunk_streamer.remesh_changed_seams(
                                        &chunk_cache,
                                        &mut seam_cache,
                                        world.revision(),
                                    );
                                    chunk_streamer.queue_needed_visible(&chunk_cache, center_chunk);
                                }

                                let new_keys = chunk_cache
                                    .expand_visible(&snapshot, chunk_streamer.queued_jobs());
                                if !new_keys.is_empty() {
                                    chunk_streamer.push_pending(new_keys, chunk_cache.center_chunk);
                                    chunk_streamer.retain_within_range(chunk_cache.center_chunk);
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
                                let far_plane_world = far_plane_voxels * world_scale;
                                let render_eye = eye * world_scale;
                                let view = Mat4::look_to_rh(render_eye, forward, Vec3::Y);
                                let cull_view = Mat4::look_to_rh(eye, forward, Vec3::Y);
                                let proj = Mat4::perspective_rh(
                                    70f32.to_radians(),
                                    gpu.aspect_ratio(),
                                    0.01,
                                    far_plane_world,
                                );
                                let cull_proj = Mat4::perspective_rh(
                                    70f32.to_radians(),
                                    gpu.aspect_ratio(),
                                    0.01 / world_scale.max(0.01),
                                    far_plane_voxels,
                                );
                                let view_proj = proj * view;
                                let culling_start = Instant::now();
                                draw_list = build_draw_list(&chunk_cache, cull_proj * cull_view);
                                cpu_stats.culling_ms =
                                    culling_start.elapsed().as_secs_f32() * 1000.0;
                                let held_torch =
                                    held_torch_light(&player, world.voxel_scale, world_scale);
                                let torch_start = Instant::now();
                                let nearby_torches =
                                    collect_nearby_torches(&snapshot, player.pos, world_scale);
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
                                sky_time += frame_dt * sky_motion_rate;
                                if let Some(upload_manifest) = cloud_weather.advance(
                                    gameplay_dt,
                                    player.pos.xz() * world_scale,
                                    &day_night.cloud_controls(),
                                    day_night.weather_seed(),
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
                                gpu.upload_camera(view_proj, render_eye, world_scale);
                                cpu_stats.uniform_ms =
                                    uniform_start.elapsed().as_secs_f32() * 1000.0;
                            } else if render_world {
                                let world = world.as_mut().unwrap();

                                let day_speed = DEFAULT_DAY_LENGTH_SECONDS
                                    / day_night.day_length_seconds().max(1.0);
                                let sky_motion_rate = 0.55 + day_speed * 0.65;
                                sky_time += frame_dt * sky_motion_rate;
                                let streaming_start = Instant::now();
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
                                let far_plane_world = far_plane_voxels * world_scale;
                                let render_eye = eye * world_scale;
                                let view = Mat4::look_to_rh(render_eye, forward, Vec3::Y);
                                let cull_view = Mat4::look_to_rh(eye, forward, Vec3::Y);
                                let proj = Mat4::perspective_rh(
                                    70f32.to_radians(),
                                    gpu.aspect_ratio(),
                                    0.01,
                                    far_plane_world,
                                );
                                let cull_proj = Mat4::perspective_rh(
                                    70f32.to_radians(),
                                    gpu.aspect_ratio(),
                                    0.01 / world_scale.max(0.01),
                                    far_plane_voxels,
                                );
                                let view_proj = proj * view;
                                let culling_start = Instant::now();
                                draw_list = build_draw_list(&chunk_cache, cull_proj * cull_view);
                                cpu_stats.culling_ms =
                                    culling_start.elapsed().as_secs_f32() * 1000.0;
                                let held_torch =
                                    held_torch_light(&player, world.voxel_scale, world_scale);
                                let torch_start = Instant::now();
                                let nearby_torches =
                                    collect_nearby_torches(&snapshot, player.pos, world_scale);
                                cpu_stats.torch_ms =
                                    torch_start.elapsed().as_secs_f32() * 1000.0;
                                nearby_torch_count = nearby_torches.len();
                                let weather_start = Instant::now();
                                if let Some(upload_manifest) = cloud_weather.advance(
                                    0.0,
                                    player.pos.xz() * world_scale,
                                    &day_night.cloud_controls(),
                                    day_night.weather_seed(),
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
                                gpu.upload_camera(view_proj, render_eye, world_scale);
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
                                gpu.upload_camera(Mat4::IDENTITY, Vec3::ZERO, 1.0);
                                cpu_stats.uniform_ms =
                                    uniform_start.elapsed().as_secs_f32() * 1000.0;
                            }

                            let render_start = Instant::now();
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
                                    perf_tracker.push(sample);
                                    perf_snapshot = perf_tracker.snapshot(
                                        gpu.adapter_summary(),
                                        [gpu.size().width, gpu.size().height],
                                        render_distance_settings(),
                                    );

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
                    let sensitivity = 0.0025;
                    player.yaw += delta.0 as f32 * sensitivity;
                    player.pitch -= delta.1 as f32 * sensitivity;
                    player.pitch = player.pitch.clamp(-1.54, 1.54);
                }
            }

            Event::AboutToWait => {
                if let Some(window) = window.as_ref() {
                    window.request_redraw();
                }
            }

            _ => {}
        }
    })?;

    Ok(())
}
