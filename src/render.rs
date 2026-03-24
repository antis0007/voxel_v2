//! Rendering, chunk streaming, terrain generation, and atmospheric shading.
//! This file is still large, but the shader sources have been externalized and the
//! authoritative target architecture is documented in docs/ENGINE_REFACTOR_PLAN.md.

mod atmosphere;
pub mod clouds;
mod debug;
pub mod shadow;

use anyhow::{Context, Result};
use glam::{IVec3, Mat4, Vec3, Vec4};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

use crate::material::terrain_generation_ids;
use crate::mesh::{
    ChunkMesh, GeneratedChunk, LOD_SEAM_EAST, LOD_SEAM_NORTH, LOD_SEAM_SOUTH, LOD_SEAM_WEST,
    LineVertex, PADDED_CHUNK_SIZE, UiVertex, Vertex3, build_chunk_mesh_lod_with_seams,
};
use crate::world::{
    BlockId, CHUNK_SIZE, CloudControls, CloudWeatherField, CloudWeatherUploadManifest,
    LightingState, WEATHER_CLIPMAP_LEVEL_COUNT, WEATHER_TEXTURE_RESOLUTION, WorldSampleConfig,
    WorldSnapshot, terrain_height,
};

pub use atmosphere::AtmosphereMode;
use atmosphere::{
    AtmosphereRuntime, AtmosphereRuntimeState, cloud_composite_shader_source, cloud_shader_source,
    cloud_temporal_shader_source, sky_shader_source, world_shader_source,
};
pub use debug::RenderDebugView;

const DEFAULT_VIEW_DISTANCE_CHUNKS: i32 = 12;
const DEFAULT_LOD_START_CHUNKS: i32 = 4;
const DEFAULT_LOD_LEVELS: u32 = 3;

static VIEW_DISTANCE_CHUNKS_ATOMIC: AtomicI32 = AtomicI32::new(DEFAULT_VIEW_DISTANCE_CHUNKS);
static LOD_START_CHUNKS_ATOMIC: AtomicI32 = AtomicI32::new(DEFAULT_LOD_START_CHUNKS);
static LOD_LEVELS_ATOMIC: AtomicU32 = AtomicU32::new(DEFAULT_LOD_LEVELS);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderDistanceSettings {
    pub view_distance_chunks: i32,
    pub lod_start_chunks: i32,
    pub lod_levels: u8,
}

fn sanitize_render_distance_settings(
    mut settings: RenderDistanceSettings,
) -> RenderDistanceSettings {
    settings.view_distance_chunks = settings.view_distance_chunks.clamp(12, 96);
    settings.lod_start_chunks = settings
        .lod_start_chunks
        .clamp(4, settings.view_distance_chunks.saturating_sub(1).max(4));
    settings.lod_levels = settings.lod_levels.clamp(1, 4);
    settings
}

pub fn render_distance_settings() -> RenderDistanceSettings {
    sanitize_render_distance_settings(RenderDistanceSettings {
        view_distance_chunks: VIEW_DISTANCE_CHUNKS_ATOMIC.load(Ordering::Relaxed),
        lod_start_chunks: LOD_START_CHUNKS_ATOMIC.load(Ordering::Relaxed),
        lod_levels: LOD_LEVELS_ATOMIC.load(Ordering::Relaxed) as u8,
    })
}

pub fn set_render_distance_settings(settings: RenderDistanceSettings) {
    let settings = sanitize_render_distance_settings(settings);
    VIEW_DISTANCE_CHUNKS_ATOMIC.store(settings.view_distance_chunks, Ordering::Relaxed);
    LOD_START_CHUNKS_ATOMIC.store(settings.lod_start_chunks, Ordering::Relaxed);
    LOD_LEVELS_ATOMIC.store(settings.lod_levels as u32, Ordering::Relaxed);
}

// Budgets for chunk loading pipeline. Higher budgets improve world streaming during fast flight
// at the cost of slightly more GPU/CPU work per frame. These values are tuned so that
// fast-flight chunk crosses don't leave visible holes in the terrain.
const CHUNK_SCHEDULE_BUDGET: usize = 64;
const CHUNK_MESH_DISPATCH_BUDGET: usize = 32;
const CHUNK_GENERATION_COLLECT_BUDGET: usize = CHUNK_SCHEDULE_BUDGET;
const CHUNK_UPLOAD_VERTEX_BUDGET: usize = 350_000;
const CHUNK_UPLOAD_INDEX_BUDGET: usize = 1_200_000;
pub const MAX_TORCH_LIGHTS: usize = 16;

#[derive(Clone, Copy, Debug, Default)]
pub struct ChunkStreamerStats {
    pub terrain_generated: usize,
    pub terrain_scheduled: usize,
    pub mesh_dispatched: usize,
    pub meshes_uploaded: usize,
    pub uploaded_vertices: usize,
    pub uploaded_indices: usize,
    pub requested: usize,
    pub generating: usize,
    pub meshing: usize,
    pub ready_chunks: usize,
    pub ready_meshes: usize,
    pub pending_schedule: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WeatherUploadStats {
    pub clipmap_batches: usize,
    pub dirty_rects: usize,
    pub uploaded_bytes: usize,
    pub cpu_upload_ms: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderFrameStats {
    pub surface_acquire_ms: f32,
    pub stream_upload_ms: f32,
    pub shadow_encode_ms: f32,
    pub terrain_encode_ms: f32,
    pub cloud_encode_ms: f32,
    pub ui_encode_ms: f32,
    pub overlay_encode_ms: f32,
    pub submit_ms: f32,
    pub present_ms: f32,
    pub terrain_draw_calls: usize,
    pub terrain_indices: usize,
    pub shadow_draw_calls: usize,
    pub shadow_redraw: bool,
    pub cloud_raymarch_passes: usize,
    pub cloud_temporal_passes: usize,
    pub outline_vertices: usize,
    pub invert_outline_vertices: usize,
    pub ui_vertices: usize,
    pub gpu_pass_total_ms: f32,
    pub gpu_shadow_ms: f32,
    pub gpu_terrain_ms: f32,
    pub gpu_cloud_raymarch_ms: f32,
    pub gpu_cloud_temporal_ms: f32,
    pub gpu_sky_ms: f32,
    pub gpu_cloud_composite_ms: f32,
    pub gpu_outline_ms: f32,
    pub gpu_ui_ms: f32,
    pub gpu_timestamps_supported: bool,
    pub cloud_history_invalidations: usize,
    pub cloud_history_camera_invalidations: usize,
    pub cloud_history_hard_cut_invalidations: usize,
    pub cloud_history_weather_invalidations: usize,
    pub cloud_history_external_invalidations: usize,
}

#[derive(Clone, Copy, Debug, Default)]
struct GpuPassTimings {
    total_ms: f32,
    shadow_ms: f32,
    terrain_ms: f32,
    cloud_raymarch_ms: f32,
    cloud_temporal_ms: f32,
    sky_ms: f32,
    cloud_composite_ms: f32,
    outline_ms: f32,
    ui_ms: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct CloudHistoryFrameStats {
    invalidations: usize,
    camera_invalidations: usize,
    hard_cut_invalidations: usize,
    weather_invalidations: usize,
    external_invalidations: usize,
}

#[derive(Clone, Copy, Debug)]
enum CloudHistoryInvalidationReason {
    CameraMotion,
    HardCut,
    WeatherUpload,
    External,
}

const GPU_TIMESTAMP_BUFFERED_FRAMES: usize = 8;
const GPU_TIMESTAMP_PASS_COUNT: u32 = 10;
const GPU_TIMESTAMP_QUERY_COUNT: u32 = GPU_TIMESTAMP_PASS_COUNT * 2;
const GPU_TIMESTAMP_RESULT_BYTES: u64 = GPU_TIMESTAMP_QUERY_COUNT as u64 * wgpu::QUERY_SIZE as u64;
const GPU_TIMESTAMP_RESOLVE_BYTES: u64 = GPU_TIMESTAMP_RESULT_BYTES
    .div_ceil(wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT)
    * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
const GPU_TIMESTAMP_SHADOW_PASS_COUNT: u32 = 3;
const GPU_TIMESTAMP_PASS_TERRAIN: u32 = 3;
const GPU_TIMESTAMP_PASS_CLOUD_RAYMARCH: u32 = 4;
const GPU_TIMESTAMP_PASS_CLOUD_TEMPORAL: u32 = 5;
const GPU_TIMESTAMP_PASS_SKY: u32 = 6;
const GPU_TIMESTAMP_PASS_CLOUD_COMPOSITE: u32 = 7;
const GPU_TIMESTAMP_PASS_OUTLINE: u32 = 8;
const GPU_TIMESTAMP_PASS_UI: u32 = 9;

fn allocate_pass_timestamp_writes<'a>(
    query_set: Option<&'a wgpu::QuerySet>,
    query_count: &mut u32,
    pass_queries: &mut [Option<(u32, u32)>; GPU_TIMESTAMP_PASS_COUNT as usize],
    pass_index: u32,
) -> Option<wgpu::RenderPassTimestampWrites<'a>> {
    let query_set = query_set?;
    let begin = *query_count;
    let end = begin + 1;
    *query_count += 2;
    pass_queries[pass_index as usize] = Some((begin, end));
    Some(wgpu::RenderPassTimestampWrites {
        query_set,
        beginning_of_pass_write_index: Some(begin),
        end_of_pass_write_index: Some(end),
    })
}

#[derive(Clone, Copy, Debug)]
struct GpuTimestampReadback {
    slot_index: usize,
    submission_id: u64,
    ok: bool,
}

struct GpuProfilerSlot {
    query_set: wgpu::QuerySet,
    resolve_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    pending: bool,
    submission_id: u64,
    query_count: u32,
    pass_queries: [Option<(u32, u32)>; GPU_TIMESTAMP_PASS_COUNT as usize],
}

struct GpuProfiler {
    slots: Vec<GpuProfilerSlot>,
    next_slot: usize,
    next_submission_id: u64,
    timestamp_period_ns: f32,
    done_tx: mpsc::Sender<GpuTimestampReadback>,
    done_rx: mpsc::Receiver<GpuTimestampReadback>,
    last_timings: GpuPassTimings,
}

impl GpuProfiler {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }

        let (done_tx, done_rx) = mpsc::channel();
        let slots = (0..GPU_TIMESTAMP_BUFFERED_FRAMES)
            .map(|slot_index| {
                let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some(&format!("gpu timestamp queries {slot_index}")),
                    ty: wgpu::QueryType::Timestamp,
                    count: GPU_TIMESTAMP_QUERY_COUNT,
                });
                let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("gpu timestamp resolve {slot_index}")),
                    size: GPU_TIMESTAMP_RESOLVE_BYTES,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
                let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("gpu timestamp readback {slot_index}")),
                    size: GPU_TIMESTAMP_RESOLVE_BYTES,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });

                GpuProfilerSlot {
                    query_set,
                    resolve_buffer,
                    readback_buffer,
                    pending: false,
                    submission_id: 0,
                    query_count: 0,
                    pass_queries: [None; GPU_TIMESTAMP_PASS_COUNT as usize],
                }
            })
            .collect();

        Some(Self {
            slots,
            next_slot: 0,
            next_submission_id: 0,
            timestamp_period_ns: queue.get_timestamp_period(),
            done_tx,
            done_rx,
            last_timings: GpuPassTimings::default(),
        })
    }

    fn acquire_slot(&mut self) -> Option<usize> {
        for _ in 0..self.slots.len() {
            let slot_index = self.next_slot;
            self.next_slot = (self.next_slot + 1) % self.slots.len();
            if !self.slots[slot_index].pending {
                self.slots[slot_index].query_count = 0;
                self.slots[slot_index].pass_queries = [None; GPU_TIMESTAMP_PASS_COUNT as usize];
                return Some(slot_index);
            }
        }
        None
    }

    fn query_set(&self, slot_index: usize) -> &wgpu::QuerySet {
        &self.slots[slot_index].query_set
    }

    fn resolve_results(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        slot_index: usize,
        query_count: u32,
    ) {
        if query_count == 0 {
            return;
        }
        let slot = &self.slots[slot_index];
        encoder.resolve_query_set(&slot.query_set, 0..query_count, &slot.resolve_buffer, 0);
        encoder.copy_buffer_to_buffer(
            &slot.resolve_buffer,
            0,
            &slot.readback_buffer,
            0,
            GPU_TIMESTAMP_RESOLVE_BYTES,
        );
    }

    fn mark_submitted(
        &mut self,
        slot_index: usize,
        query_count: u32,
        pass_queries: [Option<(u32, u32)>; GPU_TIMESTAMP_PASS_COUNT as usize],
    ) {
        if query_count == 0 {
            return;
        }

        let submission_id = self.next_submission_id;
        self.next_submission_id = self.next_submission_id.wrapping_add(1);
        let tx = self.done_tx.clone();
        let slot = &mut self.slots[slot_index];
        slot.pending = true;
        slot.submission_id = submission_id;
        slot.query_count = query_count;
        slot.pass_queries = pass_queries;
        slot.readback_buffer
            .slice(..GPU_TIMESTAMP_RESULT_BYTES)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(GpuTimestampReadback {
                    slot_index,
                    submission_id,
                    ok: result.is_ok(),
                });
            });
    }

    fn collect_finished(&mut self, device: &wgpu::Device) {
        let _ = device.poll(wgpu::PollType::Poll);

        while let Ok(done) = self.done_rx.try_recv() {
            let Some(slot) = self.slots.get_mut(done.slot_index) else {
                continue;
            };
            if !slot.pending || slot.submission_id != done.submission_id {
                continue;
            }

            slot.pending = false;
            if !done.ok {
                slot.readback_buffer.unmap();
                continue;
            }

            let mapped = slot
                .readback_buffer
                .slice(..GPU_TIMESTAMP_RESULT_BYTES)
                .get_mapped_range();
            let raw: &[u64] = bytemuck::cast_slice(&mapped);
            self.last_timings =
                Self::decode_timings(raw, &slot.pass_queries, self.timestamp_period_ns);
            drop(mapped);
            slot.readback_buffer.unmap();
        }
    }

    fn last_timings(&self) -> GpuPassTimings {
        self.last_timings
    }

    fn decode_timings(
        raw: &[u64],
        pass_queries: &[Option<(u32, u32)>; GPU_TIMESTAMP_PASS_COUNT as usize],
        timestamp_period_ns: f32,
    ) -> GpuPassTimings {
        let shadow_ms = (0..GPU_TIMESTAMP_SHADOW_PASS_COUNT)
            .map(|pass_index| {
                Self::pass_duration_ms(raw, pass_queries, pass_index, timestamp_period_ns)
            })
            .sum::<f32>();
        let terrain_ms = Self::pass_duration_ms(
            raw,
            pass_queries,
            GPU_TIMESTAMP_PASS_TERRAIN,
            timestamp_period_ns,
        );
        let cloud_raymarch_ms = Self::pass_duration_ms(
            raw,
            pass_queries,
            GPU_TIMESTAMP_PASS_CLOUD_RAYMARCH,
            timestamp_period_ns,
        );
        let cloud_temporal_ms = Self::pass_duration_ms(
            raw,
            pass_queries,
            GPU_TIMESTAMP_PASS_CLOUD_TEMPORAL,
            timestamp_period_ns,
        );
        let sky_ms = Self::pass_duration_ms(
            raw,
            pass_queries,
            GPU_TIMESTAMP_PASS_SKY,
            timestamp_period_ns,
        );
        let cloud_composite_ms = Self::pass_duration_ms(
            raw,
            pass_queries,
            GPU_TIMESTAMP_PASS_CLOUD_COMPOSITE,
            timestamp_period_ns,
        );
        let outline_ms = Self::pass_duration_ms(
            raw,
            pass_queries,
            GPU_TIMESTAMP_PASS_OUTLINE,
            timestamp_period_ns,
        );
        let ui_ms = Self::pass_duration_ms(
            raw,
            pass_queries,
            GPU_TIMESTAMP_PASS_UI,
            timestamp_period_ns,
        );

        GpuPassTimings {
            total_ms: shadow_ms
                + terrain_ms
                + cloud_raymarch_ms
                + cloud_temporal_ms
                + sky_ms
                + cloud_composite_ms
                + outline_ms
                + ui_ms,
            shadow_ms,
            terrain_ms,
            cloud_raymarch_ms,
            cloud_temporal_ms,
            sky_ms,
            cloud_composite_ms,
            outline_ms,
            ui_ms,
        }
    }

    fn pass_duration_ms(
        raw: &[u64],
        pass_queries: &[Option<(u32, u32)>; GPU_TIMESTAMP_PASS_COUNT as usize],
        pass_index: u32,
        timestamp_period_ns: f32,
    ) -> f32 {
        let Some((begin_index, end_index)) = pass_queries[pass_index as usize] else {
            return 0.0;
        };
        let begin = raw[begin_index as usize];
        let end = raw[end_index as usize];
        if end <= begin {
            return 0.0;
        }

        ((end - begin) as f64 * timestamp_period_ns as f64 / 1_000_000.0) as f32
    }
}

// CLOUD_RENDER_SCALE = 2: renders clouds at half resolution (1/4 pixel count).
// Combined with per-frame blue-noise golden-ratio rotation and the temporal
// accumulation pass, effective quality exceeds full-res single-frame rendering
// while cutting the cloud pass GPU cost by ~4× AND enabling temporal anti-flicker.
// Set to 1 to disable temporal and render full-resolution (no temporal smoothing).
//
// IMPORTANT: With recent architecture fixes (per-cell AABB interpolation),
// full-resolution rendering without temporal causes severe performance issues
// and flickering. Always use CLOUD_RENDER_SCALE = 2 for stable performance.
const CLOUD_RENDER_SCALE: u32 = 2;
const CLOUD_TEMPORAL_ENABLED: bool = CLOUD_RENDER_SCALE > 1;

const LINE_SHADER: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
    world_params: vec4<f32>,
    prev_view_proj: mat4x4<f32>,
    camera_forward: vec4<f32>,
}
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = camera.view_proj * vec4<f32>(model.pos, 1.0);
    out.color = model.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

const SHADOW_SHADER: &str = include_str!("shaders/shadow_shader.wgsl");

const UI_SHADER: &str = include_str!("shaders/ui_shader.wgsl");

const TERRAIN_COMPUTE_SHADER: &str = include_str!("terrain_compute.wgsl");

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    inv_view_proj: [[f32; 4]; 4],
    world_params: [f32; 4],
    prev_view_proj: [[f32; 4]; 4],
    camera_forward: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LightingUniform {
    sun_dir_intensity: [f32; 4],
    moon_dir_intensity: [f32; 4],
    sun_color_daylight: [f32; 4],
    moon_color_twilight: [f32; 4],
    ambient_exposure: [f32; 4],
    sky_top_saturation: [f32; 4],
    sky_horizon_reserved: [f32; 4],
    fog_color_density: [f32; 4],
    fog_params: [f32; 4],
    weather_globals: [[f32; 4]; 4],
    cloud_authoring: [[f32; 4]; 2],
    weather_clip_regions: [[f32; 4]; WEATHER_CLIPMAP_LEVEL_COUNT],
    weather_clip_summaries: [[f32; 4]; WEATHER_CLIPMAP_LEVEL_COUNT],
    held_torch_pos_radius: [f32; 4],
    held_torch_color_strength: [f32; 4],
    torch_meta: [f32; 4],
    torch_positions: [[f32; 4]; MAX_TORCH_LIGHTS],
    torch_colors: [[f32; 4]; MAX_TORCH_LIGHTS],
}

#[derive(Clone, Copy, Debug)]
struct CloudWeatherUniformState {
    regions: [[f32; 4]; WEATHER_CLIPMAP_LEVEL_COUNT],
    summaries: [[f32; 4]; WEATHER_CLIPMAP_LEVEL_COUNT],
}

impl Default for CloudWeatherUniformState {
    fn default() -> Self {
        Self {
            regions: [[0.0, 0.0, 1.0, 1.0]; WEATHER_CLIPMAP_LEVEL_COUNT],
            summaries: [[1.0, 0.0, 0.0, 0.0]; WEATHER_CLIPMAP_LEVEL_COUNT],
        }
    }
}

fn cloud_weather_uniform_state(
    weather_field: Option<&CloudWeatherField>,
) -> CloudWeatherUniformState {
    let Some(weather_field) = weather_field else {
        return CloudWeatherUniformState::default();
    };
    let meta = weather_field.gpu_meta();
    CloudWeatherUniformState {
        regions: std::array::from_fn(|level| {
            [
                meta[level].origin_x,
                meta[level].origin_z,
                meta[level].span_m,
                meta[level].span_m.recip(),
            ]
        }),
        summaries: std::array::from_fn(|level| {
            [
                meta[level].resolution,
                meta[level].avg_coverage,
                meta[level].avg_storm,
                meta[level].avg_precipitation,
            ]
        }),
    }
}

fn lighting_uniform_from_state(
    state: &LightingState,
    cloud_controls: &CloudControls,
    sky_time: f32,
    held_torch: Option<TorchLight>,
    torch_lights: &[TorchLight],
    weather_state: CloudWeatherUniformState,
    debug_view: RenderDebugView,
) -> LightingUniform {
    let mut torch_positions = [[0.0; 4]; MAX_TORCH_LIGHTS];
    let mut torch_colors = [[0.0; 4]; MAX_TORCH_LIGHTS];

    for (index, torch) in torch_lights.iter().take(MAX_TORCH_LIGHTS).enumerate() {
        torch_positions[index] = [
            torch.position.x,
            torch.position.y,
            torch.position.z,
            torch.radius.max(0.0),
        ];
        torch_colors[index] = [
            torch.color[0],
            torch.color[1],
            torch.color[2],
            torch.strength.max(0.0),
        ];
    }

    let held_torch_pos_radius = held_torch.map_or([0.0; 4], |torch| {
        [
            torch.position.x,
            torch.position.y,
            torch.position.z,
            torch.radius.max(0.0),
        ]
    });

    let held_torch_color_strength = held_torch.map_or([0.0; 4], |torch| {
        [
            torch.color[0],
            torch.color[1],
            torch.color[2],
            torch.strength.max(0.0),
        ]
    });

    let render_settings = render_distance_settings();
    let visible_world = render_settings.view_distance_chunks as f32 * CHUNK_SIZE as f32;
    let distance_scale = (visible_world / 576.0).clamp(0.45, 3.0);

    // Start fog earlier and force it to saturate almost at the terrain cutoff.
    // This avoids the visible “flat wall” following the camera.
    let fog_start = (state.fog_start * distance_scale).min(visible_world * 0.36);
    let fog_end_target = (state.fog_end * distance_scale).max(visible_world * 0.82);
    let fog_end = fog_end_target
        .min(visible_world * 0.985)
        .max(fog_start + 24.0);

    LightingUniform {
        sun_dir_intensity: [
            state.sun_dir.x,
            state.sun_dir.y,
            state.sun_dir.z,
            state.sun_intensity,
        ],
        moon_dir_intensity: [
            state.moon_dir.x,
            state.moon_dir.y,
            state.moon_dir.z,
            state.moon_intensity,
        ],
        sun_color_daylight: [
            state.sun_color[0],
            state.sun_color[1],
            state.sun_color[2],
            state.daylight,
        ],
        moon_color_twilight: [
            state.moon_color[0],
            state.moon_color[1],
            state.moon_color[2],
            state.twilight,
        ],
        ambient_exposure: [
            state.ambient_color[0],
            state.ambient_color[1],
            state.ambient_color[2],
            state.exposure,
        ],
        sky_top_saturation: [
            state.sky_top[0],
            state.sky_top[1],
            state.sky_top[2],
            state.saturation,
        ],
        sky_horizon_reserved: [
            state.sky_horizon[0],
            state.sky_horizon[1],
            state.sky_horizon[2],
            0.0,
        ],
        fog_color_density: [
            state.fog_color[0],
            state.fog_color[1],
            state.fog_color[2],
            0.65, // Base falloff density
        ],
        fog_params: [fog_start, fog_end, state.fog_height, state.fog_falloff],
        weather_globals: [
            [
                sky_time,
                cloud_controls.wind_speed,
                cloud_controls.wind_dir_angle.to_radians().cos(),
                cloud_controls.wind_dir_angle.to_radians().sin(),
            ],
            [
                cloud_controls.base_height,
                cloud_controls.layer_thickness,
                cloud_controls.wind_shear,
                cloud_controls.erosion,
            ],
            [
                cloud_controls.noise_scale,
                cloud_controls.detail_strength,
                cloud_controls.silver_lining,
                cloud_controls.layer_separation,
            ],
            [
                cloud_controls.cirrus,
                cloud_controls.mid_layer_height,
                cloud_controls.cumulus,
                cloud_controls.stratus,
            ],
        ],
        cloud_authoring: [
            [
                cloud_controls.density,
                cloud_controls.altocumulus,
                cloud_controls.altostratus,
                cloud_controls.storm_thickness,
            ],
            [cloud_controls.anvil_spread, 0.0, 0.0, 0.0],
        ],
        weather_clip_regions: weather_state.regions,
        weather_clip_summaries: weather_state.summaries,
        held_torch_pos_radius,
        held_torch_color_strength,
        torch_meta: [
            torch_lights.len().min(MAX_TORCH_LIGHTS) as f32,
            sky_time,
            weather_state.summaries[0][3],
            debug_view.as_u32() as f32,
        ],
        torch_positions,
        torch_colors,
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GenerationUniform {
    chunk_coord: [i32; 4], // x, y, z, pad
    seed: u32,
    world_type: u32,
    _padding: [u32; 2],
    generation_params: [f32; 4],
    block_ids0: [u32; 4],
    block_ids1: [u32; 4],
    block_ids2: [u32; 4],
}

pub struct ChunkGpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub lod: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct TorchLight {
    pub position: Vec3,
    pub radius: f32,
    pub color: [f32; 3],
    pub strength: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkKey {
    pub coord: IVec3,
    pub lod: u8,
}

pub struct ChunkCache {
    pub visible: Vec<ChunkKey>,
    pub visible_keys: HashSet<ChunkKey>,
    pub visible_coords: HashSet<IVec3>,
    pub gpu_meshes: HashMap<IVec3, ChunkGpuMesh>,
    /// Increments whenever the GPU mesh set changes so render passes can cache
    /// derived work, such as shadow maps, without guessing.
    pub gpu_mesh_revision: u64,
    pub cpu_chunks: HashMap<IVec3, Arc<GeneratedChunk>>,
    pub center_chunk: IVec3,
    /// Cached column Y-ranges to avoid repeated terrain sampling.
    /// Keyed by (cx, cz). Invalidated when world config changes.
    column_y_cache: HashMap<(i32, i32), (i32, i32)>,
    /// How far we've expanded the visible set from center (in chunks).
    /// Starts at 0 on reset, grows each frame until it reaches view distance.
    expansion_radius: i32,
    /// Whether expansion is complete (reached view distance).
    pub fully_expanded: bool,
}

impl ChunkCache {
    pub fn new(center_chunk: IVec3) -> Self {
        let visible = desired_chunk_keys(center_chunk);
        let visible_keys = visible.iter().copied().collect();
        let visible_coords = visible.iter().map(|key| key.coord).collect();

        Self {
            visible,
            visible_keys,
            visible_coords,
            gpu_meshes: HashMap::new(),
            gpu_mesh_revision: 0,
            cpu_chunks: HashMap::new(),
            center_chunk,
            column_y_cache: HashMap::new(),
            expansion_radius: 0,
            fully_expanded: false,
        }
    }

    pub fn new_with_snapshot(center_chunk: IVec3, snapshot: &WorldSnapshot) -> Self {
        // Start with small local set — expand incrementally each frame.
        let mut cache = Self {
            visible: Vec::new(),
            visible_keys: HashSet::new(),
            visible_coords: HashSet::new(),
            gpu_meshes: HashMap::new(),
            gpu_mesh_revision: 0,
            cpu_chunks: HashMap::new(),
            center_chunk,
            column_y_cache: HashMap::new(),
            expansion_radius: 2,
            fully_expanded: false,
        };
        cache.visible = desired_chunk_keys_with_snapshot_cached(
            center_chunk,
            snapshot,
            &mut cache.column_y_cache,
            cache.expansion_radius,
        );
        cache.rebuild_visible_sets();
        cache
    }

    fn rebuild_visible_sets(&mut self) {
        self.visible_keys = self.visible.iter().copied().collect();
        self.visible_coords = self.visible.iter().map(|key| key.coord).collect();
    }

    pub fn rebuild_visible(&mut self) {
        self.visible = desired_chunk_keys(self.center_chunk);
        self.rebuild_visible_sets();
    }

    pub fn rebuild_visible_with_snapshot(&mut self, snapshot: &WorldSnapshot) {
        let target = render_distance_settings().view_distance_chunks;
        self.expansion_radius = self.expansion_radius.clamp(0, target);
        self.fully_expanded = self.expansion_radius >= target;
        self.visible = desired_chunk_keys_with_snapshot_cached(
            self.center_chunk,
            snapshot,
            &mut self.column_y_cache,
            self.expansion_radius,
        );
        self.rebuild_visible_sets();
    }

    pub fn sync_visible_to_render_settings(&mut self) {
        self.expansion_radius = render_distance_settings().view_distance_chunks;
        self.fully_expanded = true;
        self.rebuild_visible();
    }

    pub fn sync_visible_to_render_settings_with_snapshot(&mut self, snapshot: &WorldSnapshot) {
        self.expansion_radius = render_distance_settings().view_distance_chunks;
        self.fully_expanded = true;
        self.visible = desired_chunk_keys_with_snapshot_cached(
            self.center_chunk,
            snapshot,
            &mut self.column_y_cache,
            self.expansion_radius,
        );
        self.rebuild_visible_sets();
    }

    pub fn reset(&mut self, center_chunk: IVec3) {
        self.center_chunk = center_chunk;
        self.visible = desired_chunk_keys(center_chunk);
        self.rebuild_visible_sets();
        if !self.gpu_meshes.is_empty() {
            self.gpu_meshes.clear();
            self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
        }
        self.cpu_chunks.clear();
        self.column_y_cache.clear();
        self.expansion_radius = 0;
        self.fully_expanded = false;
    }

    pub fn reset_with_snapshot(&mut self, center_chunk: IVec3, snapshot: &WorldSnapshot) {
        self.center_chunk = center_chunk;
        self.column_y_cache.clear();
        self.expansion_radius = 2;
        self.fully_expanded = false;
        self.visible = desired_chunk_keys_with_snapshot_cached(
            center_chunk,
            snapshot,
            &mut self.column_y_cache,
            self.expansion_radius,
        );
        self.rebuild_visible_sets();
        if !self.gpu_meshes.is_empty() {
            self.gpu_meshes.clear();
            self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
        }
        self.cpu_chunks.clear();
    }

    pub fn update_center_chunk(&mut self, center_chunk: IVec3) -> bool {
        if self.center_chunk == center_chunk {
            return false;
        }

        let old_center = self.center_chunk;
        self.center_chunk = center_chunk;
        self.visible = desired_chunk_keys(center_chunk);
        self.rebuild_visible_sets();
        // Shrink expansion radius by movement distance, keep at least 4 rings.
        let move_dist = (center_chunk.x - old_center.x)
            .abs()
            .max((center_chunk.y - old_center.y).abs())
            .max((center_chunk.z - old_center.z).abs());
        self.expansion_radius = (self.expansion_radius - move_dist).max(4);
        self.fully_expanded = false;
        true
    }

    pub fn update_center_chunk_with_snapshot(
        &mut self,
        center_chunk: IVec3,
        snapshot: &WorldSnapshot,
    ) -> bool {
        if self.center_chunk == center_chunk {
            return false;
        }

        let old_center = self.center_chunk;
        self.center_chunk = center_chunk;

        // When the player moves, shrink the expansion radius by the movement distance
        // rather than resetting to 2. This preserves most of the loaded world and only
        // needs to fill in the new direction-of-travel edge, dramatically reducing
        // chunk loading pressure during fast flight.
        let move_dist = (center_chunk.x - old_center.x)
            .abs()
            .max((center_chunk.y - old_center.y).abs())
            .max((center_chunk.z - old_center.z).abs());

        let settings = render_distance_settings();
        let target = settings.view_distance_chunks;
        // Subtract movement from expansion radius but keep at least 4 rings loaded.
        // For single-chunk moves (walking), this barely shrinks the radius.
        // For multi-chunk jumps (fast flight), it contracts proportionally.
        let rebuild_radius = (self.expansion_radius - move_dist).max(4).min(target);

        self.visible = desired_chunk_keys_with_snapshot_cached(
            center_chunk,
            snapshot,
            &mut self.column_y_cache,
            rebuild_radius,
        );
        self.rebuild_visible_sets();
        self.expansion_radius = rebuild_radius;
        self.fully_expanded = rebuild_radius >= target;
        true
    }

    /// Incrementally expand the visible set by a few chunk-rings per frame.
    /// Returns newly added keys so the caller can enqueue them for scheduling
    /// (avoids a full O(visible) scan in enqueue_missing each frame).
    pub fn expand_visible(
        &mut self,
        snapshot: &WorldSnapshot,
        queue_pressure: usize,
    ) -> Vec<ChunkKey> {
        if self.fully_expanded {
            return Vec::new();
        }
        let settings = render_distance_settings();
        let target = settings.view_distance_chunks;
        if self.expansion_radius >= target {
            self.fully_expanded = true;
            return Vec::new();
        }

        let old_radius = self.expansion_radius;
        // Aggressive expansion: 3-5 rings per frame when far from target.
        // Ring d contains ~8d chunks, so 4 rings at radius 20 = ~640 chunks.
        // With CHUNK_SCHEDULE_BUDGET=96, many will queue for later frames,
        // but the visible set is populated immediately so chunks that already
        // have CPU data can be meshed and uploaded without waiting for expansion.
        // This dramatically improves fast-flight world loading.
        let remaining = target - old_radius;
        let base_expansion = if remaining >= 20 {
            5 // Very far: aggressive expansion to populate visible set fast
        } else if remaining >= 10 {
            4 // Far out: expand 4 rings per frame
        } else if remaining >= 5 {
            3 // Mid distance: 3 rings
        } else {
            2 // Close to target: 2 rings minimum
        };
        let pressure_limited_expansion = if queue_pressure >= 384 {
            1
        } else if queue_pressure >= 256 {
            2
        } else if queue_pressure >= 128 {
            3
        } else {
            base_expansion
        };
        let expansion_per_frame = base_expansion.min(pressure_limited_expansion).max(1);
        let new_radius = (old_radius + expansion_per_frame).min(target);
        self.expansion_radius = new_radius;

        // Only generate keys for the NEW ring (old_radius..new_radius) and append
        // them to the existing visible list.  This avoids rebuilding the entire
        // O(radius²) set every frame during expansion.
        let new_keys = desired_chunk_keys_ring(
            self.center_chunk,
            snapshot,
            &mut self.column_y_cache,
            old_radius,
            new_radius,
        );
        let mut added = Vec::with_capacity(new_keys.len());
        for key in new_keys {
            if self.visible_keys.insert(key) {
                self.visible_coords.insert(key.coord);
                self.visible.push(key);
                added.push(key);
            }
        }

        if new_radius >= target {
            self.fully_expanded = true;
        }
        added
    }

    pub fn retain_visible(&mut self) {
        // Evict chunks that are genuinely too far away rather than anything
        // outside the current (possibly mid-expansion) visible_coords.  Using a
        // hard distance cap prevents "blank world" flicker when the player crosses
        // a chunk boundary and visible_coords briefly collapses to the small
        // 5×5×3 seed set before expand_visible() has had a chance to grow it.
        let settings = render_distance_settings();
        let hard_limit = settings.view_distance_chunks + 2; // 2-chunk grace buffer
        let center = self.center_chunk;
        let is_in_range = |coord: &IVec3| {
            let dx = (coord.x - center.x).abs();
            let dy = (coord.y - center.y).abs();
            let dz = (coord.z - center.z).abs();
            dx.max(dy).max(dz) <= hard_limit
        };
        let mesh_count_before = self.gpu_meshes.len();
        self.gpu_meshes.retain(|coord, _| is_in_range(coord));
        if self.gpu_meshes.len() != mesh_count_before {
            self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
        }
        self.cpu_chunks.retain(|coord, _| is_in_range(coord));
    }
}
pub struct CpuChunkMesh {
    pub key: ChunkKey,
    pub revision: u64,
    pub job_id: u64,
    pub mesh: ChunkMesh,
}

struct GpuReadback {
    job_id: u64,
    revision: u64,
    buffer: wgpu::Buffer,
}

struct GpuCallback {
    key: ChunkKey,
    job_id: u64,
    ok: bool,
}

struct PendingTerrainReadback {
    key: ChunkKey,
    job_id: u64,
    revision: u64,
    buffer: wgpu::Buffer,
}

struct TerrainGenerator {
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    next_job_id: u64,
    inflight: HashMap<ChunkKey, GpuReadback>,
    done_tx: mpsc::Sender<GpuCallback>,
    done_rx: mpsc::Receiver<GpuCallback>,
}

impl TerrainGenerator {
    fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain compute layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain compute shader"),
            source: wgpu::ShaderSource::Wgsl(TERRAIN_COMPUTE_SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain compute pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("terrain compute pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let (done_tx, done_rx) = mpsc::channel();

        Self {
            bind_group_layout,
            pipeline,
            next_job_id: 0,
            inflight: HashMap::new(),
            done_tx,
            done_rx,
        }
    }

    fn reset(&mut self) {
        self.inflight.clear();
        while self.done_rx.try_recv().is_ok() {}
    }

    fn retain_visible(&mut self, visible: &HashSet<ChunkKey>) {
        self.inflight.retain(|key, _| visible.contains(key));
    }

    fn invalidate_coords(&mut self, dirty_coords: &HashSet<IVec3>) {
        self.inflight
            .retain(|key, _| !dirty_coords.contains(&key.coord));
    }

    fn schedule_many<I>(&mut self, gpu: &Gpu, keys: I, cfg: WorldSampleConfig, revision: u64)
    where
        I: IntoIterator<Item = ChunkKey>,
    {
        let keys: Vec<_> = keys.into_iter().collect();
        if keys.is_empty() {
            return;
        }

        let ids = terrain_generation_ids();
        let voxel_count = (PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE * PADDED_CHUNK_SIZE) as u64;
        let buffer_size = voxel_count * std::mem::size_of::<u32>() as u64;
        let workgroup_count = div_ceil_i32(PADDED_CHUNK_SIZE, 4) as u32;
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("terrain generation encoder"),
            });
        let mut pending_readbacks = Vec::with_capacity(keys.len());

        for key in keys {
            let job_id = self.next_job_id;
            self.next_job_id = self.next_job_id.wrapping_add(1);

            let uniform = GenerationUniform {
                chunk_coord: [key.coord.x, key.coord.y, key.coord.z, 0],
                seed: cfg.seed,
                world_type: cfg.world_type as u32,
                _padding: [0; 2],
                generation_params: [cfg.voxel_scale.meters(), 0.0, 0.0, 0.0],
                block_ids0: ids.block_ids0,
                block_ids1: ids.block_ids1,
                block_ids2: ids.block_ids2,
            };
            let uniform_buffer = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("terrain generation uniform"),
                    contents: bytemuck::bytes_of(&uniform),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

            let storage = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("terrain generation storage"),
                size: buffer_size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("terrain generation readback"),
                size: buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("terrain generation bind group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: storage.as_entire_binding(),
                    },
                ],
            });

            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("terrain generation pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(workgroup_count, workgroup_count, workgroup_count);
            }
            encoder.copy_buffer_to_buffer(&storage, 0, &readback, 0, buffer_size);
            pending_readbacks.push(PendingTerrainReadback {
                key,
                job_id,
                revision,
                buffer: readback,
            });
        }

        gpu.queue.submit(Some(encoder.finish()));

        for pending in pending_readbacks {
            let tx = self.done_tx.clone();
            let key = pending.key;
            let job_id = pending.job_id;
            pending
                .buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = tx.send(GpuCallback {
                        key,
                        job_id,
                        ok: result.is_ok(),
                    });
                });

            self.inflight.insert(
                pending.key,
                GpuReadback {
                    job_id: pending.job_id,
                    revision: pending.revision,
                    buffer: pending.buffer,
                },
            );
        }
    }

    fn collect_finished(
        &mut self,
        device: &wgpu::Device,
        snapshot: &WorldSnapshot,
    ) -> Vec<(ChunkKey, u64, GeneratedChunk)> {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut out = Vec::new();
        // CRITICAL: Must match or exceed CHUNK_SCHEDULE_BUDGET to prevent queue buildup.
        // If schedule > collect, terrain generation queue grows unbounded → stalls and FPS drops.
        // With CHUNK_SCHEDULE_BUDGET=32, we must collect at least 32 to maintain balance.
        // This is the PRIMARY BOTTLENECK - was capped at 8, causing 24/frame backlog.
        let max_collect = CHUNK_GENERATION_COLLECT_BUDGET;
        let mut collected = 0usize;

        while collected < max_collect {
            let Ok(done) = self.done_rx.try_recv() else {
                break;
            };
            let Some(readback) = self.inflight.get(&done.key) else {
                continue;
            };
            if readback.job_id != done.job_id {
                continue;
            }
            let Some(readback) = self.inflight.remove(&done.key) else {
                continue;
            };

            if !done.ok {
                readback.buffer.unmap();
                continue;
            }

            let data = readback.buffer.slice(..).get_mapped_range();
            let blocks = data
                .chunks_exact(4)
                .map(|chunk| {
                    BlockId::from_u32(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                })
                .collect::<Vec<_>>();
            drop(data);
            readback.buffer.unmap();

            let mut chunk_data =
                GeneratedChunk::new(done.key.coord, snapshot.cfg.water_level(), blocks);
            apply_snapshot_edits_cubic(&mut chunk_data, snapshot);
            out.push((done.key, readback.revision, chunk_data));
            collected += 1;
        }

        out
    }
}

fn div_ceil_i32(value: i32, divisor: i32) -> i32 {
    (value + divisor - 1) / divisor
}

#[derive(Clone, Copy)]
struct ChunkJob {
    key: ChunkKey,
    revision: u64,
}

struct MeshJob {
    key: ChunkKey,
    revision: u64,
    job_id: u64,
    chunk: Arc<GeneratedChunk>,
    lod_seams: u8,
}

fn lod_seams_for_chunk(cache: &ChunkCache, key: ChunkKey) -> u8 {
    if key.lod == 0 {
        return 0;
    }

    let mut seams = 0u8;
    let neighbors = [
        (IVec3::new(-1, 0, 0), LOD_SEAM_WEST),
        (IVec3::new(1, 0, 0), LOD_SEAM_EAST),
        (IVec3::new(0, 0, -1), LOD_SEAM_NORTH),
        (IVec3::new(0, 0, 1), LOD_SEAM_SOUTH),
    ];

    for (offset, flag) in neighbors {
        let coord = key.coord + offset;
        if !cache.visible_coords.contains(&coord) {
            seams |= flag;
            continue;
        }

        let neighbor_lod = lod_for_chunk(cache.center_chunk, coord);
        if neighbor_lod < key.lod {
            seams |= flag;
        }
    }

    seams
}

fn spawn_mesh_workers(
    worker_count: usize,
) -> (mpsc::Sender<MeshJob>, mpsc::Receiver<CpuChunkMesh>) {
    let (job_tx, job_rx) = mpsc::channel::<MeshJob>();
    let (result_tx, result_rx) = mpsc::channel::<CpuChunkMesh>();
    let shared_rx = Arc::new(Mutex::new(job_rx));

    for worker_index in 0..worker_count.max(1) {
        let shared_rx = Arc::clone(&shared_rx);
        let result_tx = result_tx.clone();
        thread::Builder::new()
            .name(format!("chunk-mesh-{worker_index}"))
            .spawn(move || {
                loop {
                    let job = match shared_rx.lock().expect("mesh worker queue poisoned").recv() {
                        Ok(job) => job,
                        Err(_) => break,
                    };

                    let mesh = build_chunk_mesh_lod_with_seams(
                        job.chunk.as_ref(),
                        job.key.lod,
                        job.lod_seams,
                    );
                    if result_tx
                        .send(CpuChunkMesh {
                            key: job.key,
                            revision: job.revision,
                            job_id: job.job_id,
                            mesh,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .expect("failed to spawn mesh worker");
    }

    drop(result_tx);
    (job_tx, result_rx)
}

pub struct ChunkStreamer {
    requested: HashSet<ChunkKey>,
    /// Coord-only set for O(1) "is any job in-flight for this coord?" checks.
    /// Kept in sync with `requested` and `meshing`.
    requested_coords: HashSet<IVec3>,
    generating: HashSet<ChunkKey>,
    meshing: HashMap<ChunkKey, u64>,
    ready_chunks: VecDeque<ChunkJob>,
    ready_meshes: VecDeque<CpuChunkMesh>,
    mesh_job_tx: mpsc::Sender<MeshJob>,
    mesh_result_rx: mpsc::Receiver<CpuChunkMesh>,
    next_mesh_job_id: u64,
    terrain_generator: TerrainGenerator,
    /// Chunks that need to be scheduled but haven't been yet.
    /// Populated when new keys are added to the visible set, avoiding an
    /// O(visible) scan inside enqueue_missing every frame.
    /// Stored as a min-heap keyed by radial XZ distance so near chunks are
    /// scheduled before distant ones.
    pending_schedule: BinaryHeap<Reverse<(i32, i32, i32, i32, i32, i32, i32, u8)>>,
    pending_keys: HashSet<ChunkKey>,
    pending_schedule_center: IVec3,
    last_stats: ChunkStreamerStats,
}

impl ChunkStreamer {
    pub fn new(device: &wgpu::Device, worker_count: usize) -> Self {
        let (mesh_job_tx, mesh_result_rx) = spawn_mesh_workers(worker_count);

        Self {
            requested: HashSet::new(),
            requested_coords: HashSet::new(),
            generating: HashSet::new(),
            meshing: HashMap::new(),
            ready_chunks: VecDeque::new(),
            ready_meshes: VecDeque::new(),
            mesh_job_tx,
            mesh_result_rx,
            next_mesh_job_id: 0,
            terrain_generator: TerrainGenerator::new(device),
            pending_schedule: BinaryHeap::new(),
            pending_keys: HashSet::new(),
            pending_schedule_center: IVec3::ZERO,
            last_stats: ChunkStreamerStats::default(),
        }
    }

    pub fn reset(&mut self) {
        self.requested.clear();
        self.requested_coords.clear();
        self.generating.clear();
        self.meshing.clear();
        self.ready_chunks.clear();
        self.ready_meshes.clear();
        self.pending_schedule.clear();
        self.pending_keys.clear();
        while self.mesh_result_rx.try_recv().is_ok() {}
        self.terrain_generator.reset();
        self.last_stats = ChunkStreamerStats::default();
    }

    pub fn begin_frame_stats(&mut self) {
        self.last_stats = ChunkStreamerStats::default();
    }

    pub fn last_stats(&self) -> ChunkStreamerStats {
        self.last_stats
    }

    fn rebuild_pending_schedule(&mut self, center: IVec3) {
        self.pending_schedule_center = center;
        let pending_keys: Vec<_> = self.pending_keys.iter().copied().collect();
        self.pending_schedule.clear();
        for key in pending_keys {
            self.pending_schedule.push(pending_priority(center, key));
        }
    }

    fn push_pending_key(&mut self, key: ChunkKey) {
        if self.pending_keys.insert(key) {
            self.pending_schedule
                .push(pending_priority(self.pending_schedule_center, key));
        }
    }

    fn dynamic_schedule_budget(&self) -> usize {
        let queue_pressure = self.generating.len()
            + self.meshing.len()
            + self.ready_chunks.len()
            + self.ready_meshes.len();
        if queue_pressure >= 384 {
            16
        } else if queue_pressure >= 224 {
            32
        } else {
            CHUNK_SCHEDULE_BUDGET
        }
    }

    fn ready_chunk_priority(cache: &ChunkCache, key: ChunkKey) -> (u8, u8, i32, i32, i32, i32) {
        let mesh_state = match cache.gpu_meshes.get(&key.coord) {
            None => 0u8,
            Some(mesh) if mesh.lod > key.lod => 1u8,
            _ => 2u8,
        };
        let base = chunk_sort_key(cache.center_chunk, key);
        (mesh_state, base.0, base.1, base.2, base.3, base.4)
    }

    fn reprioritize_ready_chunks(&mut self, cache: &ChunkCache) {
        if self.ready_chunks.len() <= 1 {
            return;
        }
        let mut jobs: Vec<_> = self.ready_chunks.drain(..).collect();
        jobs.sort_by_key(|job| Self::ready_chunk_priority(cache, job.key));
        self.ready_chunks = jobs.into_iter().collect();
    }

    fn reprioritize_ready_meshes(&mut self, cache: &ChunkCache) {
        if self.ready_meshes.len() <= 1 {
            return;
        }
        let mut meshes: Vec<_> = self.ready_meshes.drain(..).collect();
        meshes.sort_by_key(|mesh| Self::ready_chunk_priority(cache, mesh.key));
        self.ready_meshes = meshes.into_iter().collect();
    }

    pub fn queue_needed_visible(&mut self, cache: &ChunkCache, center: IVec3) {
        if self.pending_schedule_center != center {
            self.rebuild_pending_schedule(center);
        }
        for key in &cache.visible {
            if cache
                .gpu_meshes
                .get(&key.coord)
                .is_some_and(|mesh| mesh.lod == key.lod)
            {
                continue;
            }
            self.push_pending_key(*key);
        }
    }

    pub fn retain_visible(&mut self, visible: &HashSet<ChunkKey>) {
        self.requested.retain(|key| visible.contains(key));
        self.generating.retain(|key| visible.contains(key));
        self.meshing.retain(|key, _| visible.contains(key));
        self.ready_chunks.retain(|job| visible.contains(&job.key));
        self.ready_meshes.retain(|mesh| visible.contains(&mesh.key));
        self.pending_keys.retain(|key| visible.contains(key));
        self.rebuild_pending_schedule(self.pending_schedule_center);
        self.terrain_generator.retain_visible(visible);
        self.requested_coords = self.requested.iter().map(|k| k.coord).collect();
    }

    /// Looser eviction used after a player chunk-crossing: cancel work only for
    /// chunks beyond the hard distance limit, so in-flight jobs for still-valid
    /// chunks aren't thrown away and immediately re-requested.
    pub fn retain_within_range(&mut self, center: IVec3) {
        let settings = render_distance_settings();
        let hard_limit = settings.view_distance_chunks + 2;
        let in_range = |coord: &IVec3| -> bool {
            let dx = (coord.x - center.x).abs();
            let dy = (coord.y - center.y).abs();
            let dz = (coord.z - center.z).abs();
            dx.max(dy).max(dz) <= hard_limit
        };
        self.requested.retain(|key| in_range(&key.coord));
        self.generating.retain(|key| in_range(&key.coord));
        self.meshing.retain(|key, _| in_range(&key.coord));
        self.ready_chunks.retain(|job| in_range(&job.key.coord));
        self.ready_meshes.retain(|mesh| in_range(&mesh.key.coord));
        self.pending_keys.retain(|key| in_range(&key.coord));
        self.rebuild_pending_schedule(center);
        let terrain_keys: HashSet<ChunkKey> = self
            .requested
            .iter()
            .chain(self.generating.iter())
            .copied()
            .collect();
        self.terrain_generator.retain_visible(&terrain_keys);
        self.requested_coords = self.requested.iter().map(|k| k.coord).collect();
    }

    fn cancel_coords(&mut self, dirty_coords: &HashSet<IVec3>) {
        if dirty_coords.is_empty() {
            return;
        }

        self.requested
            .retain(|key| !dirty_coords.contains(&key.coord));
        self.generating
            .retain(|key| !dirty_coords.contains(&key.coord));
        self.meshing
            .retain(|key, _| !dirty_coords.contains(&key.coord));
        self.ready_chunks
            .retain(|job| !dirty_coords.contains(&job.key.coord));
        self.ready_meshes
            .retain(|mesh| !dirty_coords.contains(&mesh.key.coord));
        self.pending_keys
            .retain(|key| !dirty_coords.contains(&key.coord));
        self.rebuild_pending_schedule(self.pending_schedule_center);
        self.terrain_generator.invalidate_coords(dirty_coords);
        self.requested_coords
            .retain(|coord| !dirty_coords.contains(coord));
    }

    fn collect_finished_meshes(&mut self) {
        let mut collected = 0usize;
        while collected < 64 {
            let Ok(cpu_mesh) = self.mesh_result_rx.try_recv() else {
                break;
            };
            if self.meshing.get(&cpu_mesh.key).copied() != Some(cpu_mesh.job_id) {
                continue;
            }
            self.meshing.remove(&cpu_mesh.key);
            self.ready_meshes.push_back(cpu_mesh);
            collected += 1;
        }
    }

    fn dispatch_ready_chunks(&mut self, cache: &ChunkCache, current_revision: u64) -> usize {
        let mut dispatched = 0usize;
        self.reprioritize_ready_chunks(cache);

        while dispatched < CHUNK_MESH_DISPATCH_BUDGET {
            let Some(job) = self.ready_chunks.pop_front() else {
                break;
            };

            // Only skip if the chunk is no longer in the visible set.
            // A stale revision means a world edit happened; rebuild_dirty() will
            // re-queue affected chunks so we don't need to drop work here.
            if !cache.visible_coords.contains(&job.key.coord) {
                self.requested.remove(&job.key);
                self.requested_coords.remove(&job.key.coord);
                continue;
            }
            let _ = current_revision;

            if self.meshing.contains_key(&job.key) {
                continue;
            }

            let Some(chunk) = cache.cpu_chunks.get(&job.key.coord).cloned() else {
                self.requested.remove(&job.key);
                self.requested_coords.remove(&job.key.coord);
                continue;
            };

            let job_id = self.next_mesh_job_id;
            self.next_mesh_job_id = self.next_mesh_job_id.wrapping_add(1);

            match self.mesh_job_tx.send(MeshJob {
                key: job.key,
                revision: job.revision,
                job_id,
                chunk,
                lod_seams: lod_seams_for_chunk(cache, job.key),
            }) {
                Ok(()) => {
                    self.meshing.insert(job.key, job_id);
                    dispatched += 1;
                }
                Err(err) => {
                    self.ready_meshes.push_back(CpuChunkMesh {
                        key: err.0.key,
                        revision: err.0.revision,
                        job_id,
                        mesh: build_chunk_mesh_lod_with_seams(
                            err.0.chunk.as_ref(),
                            err.0.key.lod,
                            err.0.lod_seams,
                        ),
                    });
                    dispatched += 1;
                }
            }
        }
        dispatched
    }

    pub fn rebuild_dirty(
        &mut self,
        snapshot: &WorldSnapshot,
        cache: &mut ChunkCache,
        dirty_blocks: &HashSet<IVec3>,
        dirty_coords: &HashSet<IVec3>,
        revision: u64,
    ) {
        if dirty_coords.is_empty() {
            return;
        }

        self.cancel_coords(dirty_coords);

        for &coord in dirty_coords {
            if !cache.visible_coords.contains(&coord) {
                continue;
            }

            let lod = lod_for_chunk(cache.center_chunk, coord);
            let Some(chunk_data) = cache.cpu_chunks.get_mut(&coord) else {
                continue;
            };

            let chunk_data = Arc::make_mut(chunk_data);
            for &block in dirty_blocks {
                chunk_data.set_world(block, snapshot.get(block));
            }

            let key = ChunkKey { coord, lod };
            self.requested.insert(key);
            self.requested_coords.insert(key.coord);
            self.ready_chunks.push_front(ChunkJob { key, revision });
        }
    }

    /// Re-queue LOD1+ chunks whose seam flags have changed due to a player chunk crossing.
    /// This ensures that seam skirts are updated when a neighbor transitions LOD levels.
    /// Only chunks that already have CPU data are re-queued (no terrain regeneration).
    pub fn remesh_changed_seams(
        &mut self,
        cache: &ChunkCache,
        seam_cache: &mut HashMap<IVec3, u8>,
        revision: u64,
    ) {
        for key in &cache.visible {
            if key.lod == 0 {
                continue;
            }
            if !cache.cpu_chunks.contains_key(&key.coord) {
                continue;
            }
            let new_seams = lod_seams_for_chunk(cache, *key);
            let old_seams = seam_cache.get(&key.coord).copied().unwrap_or(u8::MAX);
            if new_seams != old_seams {
                seam_cache.insert(key.coord, new_seams);
                // Only re-queue if not already in-flight
                if !self.requested_coords.contains(&key.coord) {
                    self.requested.insert(*key);
                    self.requested_coords.insert(key.coord);
                    self.ready_chunks.push_back(ChunkJob {
                        key: *key,
                        revision,
                    });
                }
            }
        }
    }

    /// Push newly visible keys into the pending schedule queue.
    /// Called from the outside when new chunks enter the visible set so that
    /// enqueue_missing doesn't have to scan all visible chunks every frame.
    /// Keys are stored with radial XZ distance so near chunks are scheduled first.
    pub fn push_pending(&mut self, keys: Vec<ChunkKey>, center: IVec3) {
        if self.pending_schedule_center != center {
            self.rebuild_pending_schedule(center);
        }
        for key in keys {
            self.push_pending_key(key);
        }
    }

    pub fn enqueue_missing(
        &mut self,
        gpu: &Gpu,
        snapshot: &WorldSnapshot,
        revision: u64,
        cache: &mut ChunkCache,
    ) {
        self.collect_finished_meshes();
        let mut terrain_to_schedule = Vec::new();
        let mut terrain_generated = 0usize;

        for (key, _job_revision, chunk_data) in self
            .terrain_generator
            .collect_finished(&gpu.device, snapshot)
        {
            self.generating.remove(&key);
            // Accept terrain data as long as the chunk is still in the visible
            // range — don't discard on revision mismatch.  A world edit (which
            // bumps revision) will re-queue via rebuild_dirty anyway.
            if !cache.visible_coords.contains(&key.coord) {
                self.requested.remove(&key);
                self.requested_coords.remove(&key.coord);
                continue;
            }
            cache.cpu_chunks.insert(key.coord, Arc::new(chunk_data));
            self.ready_chunks.push_back(ChunkJob {
                key,
                revision, // use current revision so downstream checks pass
            });
            terrain_generated += 1;
        }

        let mesh_dispatched = self.dispatch_ready_chunks(cache, revision);

        // Drain the pending queue rather than scanning all visible chunks each frame.
        // pending_schedule is populated by push_pending() when expand_visible() adds
        // new rings, so on steady-state frames it's empty and this loop is O(0).
        let schedule_budget = self.dynamic_schedule_budget();
        let mut scheduled = 0usize;
        // Limit how many entries we scan from the pending queue per frame so we
        // don't stall when a large ring arrives at once.
        let scan_limit = self.pending_schedule.len().min(schedule_budget * 4);
        let mut scanned = 0usize;
        while scheduled < schedule_budget && scanned < scan_limit {
            let Some(Reverse((_, _, _, _, kx, ky, kz, klod))) = self.pending_schedule.pop() else {
                break;
            };
            let key = ChunkKey {
                coord: IVec3::new(kx, ky, kz),
                lod: klod,
            };
            self.pending_keys.remove(&key);
            scanned += 1;

            // Skip if no longer visible.
            if !cache.visible_coords.contains(&key.coord) {
                continue;
            }
            // Already have the correct LOD mesh — nothing to do.
            if cache
                .gpu_meshes
                .get(&key.coord)
                .is_some_and(|mesh| mesh.lod == key.lod)
            {
                continue;
            }
            // Already have a job in-flight for this coord — re-queue for later.
            if self.requested_coords.contains(&key.coord) {
                self.push_pending_key(key);
                continue;
            }

            if cache.cpu_chunks.contains_key(&key.coord) {
                self.requested.insert(key);
                self.requested_coords.insert(key.coord);
                self.ready_chunks.push_back(ChunkJob { key, revision });
                scheduled += 1;
                continue;
            }
            // Pipeline capacity caps: sized for fast-flight streaming.
            // cpu_chunk_cap is the total cache size — large enough to hold all chunks
            // within view distance without thrashing.
            let visible_target = cache.visible_coords.len().max(1);
            let cpu_chunk_cap = (visible_target.saturating_mul(2)).clamp(4096usize, 32768usize);
            let ready_chunk_cap = (visible_target / 2).clamp(256usize, 4096usize);
            let ready_mesh_cap = (visible_target / 3).clamp(128usize, 2048usize);
            let inflight_cap = (visible_target / 2).clamp(128usize, 1024usize);

            if cache.cpu_chunks.len() >= cpu_chunk_cap
                || self.ready_chunks.len() >= ready_chunk_cap
                || self.ready_meshes.len() >= ready_mesh_cap
                || self.generating.len() >= inflight_cap
                || self.meshing.len() >= inflight_cap
            {
                self.push_pending_key(key);
                break;
            }

            self.requested.insert(key);
            self.requested_coords.insert(key.coord);
            self.generating.insert(key);
            terrain_to_schedule.push(key);
            scheduled += 1;
        }

        // Fallback: if the pending queue is empty but some visible chunks still
        // lack meshes (e.g. after a world reset or retain_visible call), do a
        // bounded scan of the visible list to catch them.
        // Skip entirely when there is no in-flight work and the world is fully
        // expanded — steady-state frames cost O(1) instead of O(visible).
        let has_inflight = !self.generating.is_empty()
            || !self.meshing.is_empty()
            || !self.ready_chunks.is_empty()
            || !self.ready_meshes.is_empty();
        let should_scan = self.pending_schedule.is_empty()
            && scheduled < schedule_budget
            && (has_inflight || !cache.fully_expanded);
        if should_scan {
            for key in &cache.visible {
                if scheduled >= schedule_budget {
                    break;
                }
                if cache
                    .gpu_meshes
                    .get(&key.coord)
                    .is_some_and(|mesh| mesh.lod == key.lod)
                {
                    continue;
                }
                if self.requested_coords.contains(&key.coord) {
                    continue;
                }
                if cache.cpu_chunks.contains_key(&key.coord) {
                    self.requested.insert(*key);
                    self.requested_coords.insert(key.coord);
                    self.ready_chunks.push_back(ChunkJob {
                        key: *key,
                        revision,
                    });
                    scheduled += 1;
                    continue;
                }
                self.requested.insert(*key);
                self.requested_coords.insert(key.coord);
                self.generating.insert(*key);
                terrain_to_schedule.push(*key);
                scheduled += 1;
            }
        }

        let terrain_scheduled = terrain_to_schedule.len();
        self.terrain_generator
            .schedule_many(gpu, terrain_to_schedule, snapshot.cfg, revision);
        self.last_stats.terrain_generated = terrain_generated;
        self.last_stats.terrain_scheduled = terrain_scheduled;
        self.last_stats.mesh_dispatched = mesh_dispatched;
        self.last_stats.requested = self.requested.len();
        self.last_stats.generating = self.generating.len();
        self.last_stats.meshing = self.meshing.len();
        self.last_stats.ready_chunks = self.ready_chunks.len();
        self.last_stats.ready_meshes = self.ready_meshes.len();
        self.last_stats.pending_schedule = self.pending_schedule.len();
    }

    pub fn upload_finished(
        &mut self,
        gpu: &Gpu,
        cache: &mut ChunkCache,
        current_revision: u64,
        max_uploads: usize,
    ) {
        let mut uploaded = 0usize;
        let mut uploaded_vertices = 0usize;
        let mut uploaded_indices = 0usize;
        self.collect_finished_meshes();
        self.reprioritize_ready_meshes(cache);

        while uploaded < max_uploads {
            let Some(cpu_mesh) = self.ready_meshes.pop_front() else {
                break;
            };
            let mesh_vertices = cpu_mesh.mesh.vertices.len();
            let mesh_indices = cpu_mesh.mesh.indices.len();
            let would_exceed_budget = uploaded > 0
                && (uploaded_vertices + mesh_vertices > CHUNK_UPLOAD_VERTEX_BUDGET
                    || uploaded_indices + mesh_indices > CHUNK_UPLOAD_INDEX_BUDGET);
            if would_exceed_budget {
                self.ready_meshes.push_front(cpu_mesh);
                break;
            }

            self.requested.remove(&cpu_mesh.key);
            // requested_coords tracks any in-flight key for this coord.
            // Since we enforce at most one key per coord in requested, removing
            // unconditionally here is correct and O(1) instead of O(N).
            self.requested_coords.remove(&cpu_mesh.key.coord);

            // Only discard if the chunk is no longer visible.  Stale revision just
            // means the world was edited while this mesh was in-flight; we still
            // upload it — a subsequent rebuild_dirty() will re-mesh the affected
            // chunks with the latest data.  Discarding here was the main cause of
            // wasted work and blank-chunk flicker on every player chunk crossing.
            if !cache.visible_coords.contains(&cpu_mesh.key.coord) {
                continue;
            }
            // If the world was edited after this mesh started, the chunk will be
            // re-queued by rebuild_dirty.  Upload what we have now so there's no gap.
            let _ = current_revision;

            let gpu_mesh = upload_chunk_mesh(&gpu.device, &cpu_mesh.mesh, cpu_mesh.key.lod);
            cache.gpu_meshes.insert(cpu_mesh.key.coord, gpu_mesh);
            cache.gpu_mesh_revision = cache.gpu_mesh_revision.wrapping_add(1);
            uploaded += 1;
            uploaded_vertices += mesh_vertices;
            uploaded_indices += mesh_indices;
        }

        self.last_stats.meshes_uploaded = uploaded;
        self.last_stats.uploaded_vertices = uploaded_vertices;
        self.last_stats.uploaded_indices = uploaded_indices;
        self.last_stats.requested = self.requested.len();
        self.last_stats.generating = self.generating.len();
        self.last_stats.meshing = self.meshing.len();
        self.last_stats.ready_chunks = self.ready_chunks.len();
        self.last_stats.ready_meshes = self.ready_meshes.len();
        self.last_stats.pending_schedule = self.pending_schedule.len();
    }

    pub fn queued_jobs(&self) -> usize {
        self.generating.len()
            + self.meshing.len()
            + self.ready_chunks.len()
            + self.ready_meshes.len()
    }
}

fn apply_snapshot_edits_cubic(chunk_data: &mut GeneratedChunk, snapshot: &WorldSnapshot) {
    for dy in -1..=1 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let coord = chunk_data.chunk + IVec3::new(dx, dy, dz);
                let Some(chunk) = snapshot.chunks.get(&coord) else {
                    continue;
                };
                let ox = coord.x * CHUNK_SIZE;
                let oy = coord.y * CHUNK_SIZE;
                let oz = coord.z * CHUNK_SIZE;
                for z in 0..CHUNK_SIZE {
                    for y in 0..CHUNK_SIZE {
                        for x in 0..CHUNK_SIZE {
                            let idx = crate::world::ChunkData::local_index(x, y, z);
                            let block = chunk.blocks[idx];
                            let pos = IVec3::new(ox + x, oy + y, oz + z);
                            chunk_data.set_world(pos, block);
                        }
                    }
                }
            }
        }
    }
}

pub fn lod_for_chunk(center: IVec3, chunk: IVec3) -> u8 {
    let dx = (chunk.x - center.x).abs();
    let dy = (chunk.y - center.y).abs();
    let dz = (chunk.z - center.z).abs();
    let d = dx.max(dy).max(dz);
    let settings = render_distance_settings();

    if settings.lod_levels <= 1 || d <= settings.lod_start_chunks {
        0
    } else if settings.lod_levels == 2 {
        1
    } else if settings.lod_levels == 3 {
        let far_band = (settings.view_distance_chunks - settings.lod_start_chunks).max(2);
        let lod2_start = settings.lod_start_chunks + (far_band / 2).max(1);
        if d <= lod2_start { 1 } else { 2 }
    } else {
        // lod_levels >= 4: three LOD bands beyond lod_start_chunks
        let far_band = (settings.view_distance_chunks - settings.lod_start_chunks).max(3);
        let third = (far_band / 3).max(1);
        let lod2_start = settings.lod_start_chunks + third;
        let lod3_start = settings.lod_start_chunks + third * 2;
        if d <= lod2_start {
            1
        } else if d <= lod3_start {
            2
        } else {
            3
        }
    }
}

/// Compute the terrain-driven Y-chunk range that should be loaded for an XZ column.
/// Returns (min_cy, max_cy) inclusive chunk Y coords.
fn column_y_chunk_range(cx: i32, cz: i32, cfg: &WorldSampleConfig) -> (i32, i32) {
    // Sample a 5x5 grid of terrain heights within this column to find the Y extent.
    let ox = cx * CHUNK_SIZE;
    let oz = cz * CHUNK_SIZE;
    let mut min_h = i32::MAX;
    let mut max_h = i32::MIN;
    for dz in [0, 8, 16, 24, 31] {
        for dx in [0, 8, 16, 24, 31] {
            let h = terrain_height(cfg, ox + dx, oz + dz);
            min_h = min_h.min(h);
            max_h = max_h.max(h);
        }
    }
    if min_h == i32::MAX {
        min_h = cfg.min_world_y();
        max_h = cfg.max_world_y();
    }

    // Add margin for caves below and structures (trees) above.
    // This is terrain-driven only: do not force the player's Y chunk into every
    // XZ column, or flying high will generate a giant ring of empty air chunks.
    let cave_margin = CHUNK_SIZE * 2;
    let tree_margin = CHUNK_SIZE;
    let water_level = cfg.water_level();
    let effective_min = (min_h - cave_margin).min(water_level - CHUNK_SIZE);
    let effective_max = max_h + tree_margin;

    let min_cy =
        floor_div_i32(effective_min, CHUNK_SIZE).max(floor_div_i32(cfg.min_world_y(), CHUNK_SIZE));
    let max_cy = floor_div_i32(effective_max, CHUNK_SIZE)
        .min(floor_div_i32(cfg.max_world_y() - 1, CHUNK_SIZE));
    (min_cy, max_cy)
}

/// Compute a Y-range that excludes underground chunks for distant columns.
/// At close range (within `underground_radius` chunks) we load caves normally.
/// Beyond that, only load the surface layer and above so we don't generate
/// thousands of unseen underground chunks far away.
fn column_y_chunk_range_distance_cull(
    cx: i32,
    cz: i32,
    cfg: &WorldSampleConfig,
    _center: IVec3,
    chebyshev_dist: i32,
) -> (i32, i32) {
    let (min_cy, max_cy) = column_y_chunk_range(cx, cz, cfg);
    // Underground chunks are only useful when the player is close enough to
    // actually see into caves.  Beyond this threshold, skip sub-surface chunks.
    const UNDERGROUND_LOAD_RADIUS: i32 = 6;
    if chebyshev_dist > UNDERGROUND_LOAD_RADIUS {
        // Clamp min_cy to the surface chunk — no caves at distance.
        // Use the actual surface height from the 5×5 grid already sampled in column_y_chunk_range.
        // The previous single-point sample missed peaks, creating holes.
        // Load surface chunk plus 2 chunks below for LOD transitions and visual consistency.
        let surface_cy = max_cy;
        let culled_min = (surface_cy - 2).max(min_cy);
        return (culled_min, max_cy);
    }
    (min_cy, max_cy)
}

fn floor_div_i32(a: i32, b: i32) -> i32 {
    let d = a / b;
    if (a ^ b) < 0 && d * b != a { d - 1 } else { d }
}

fn chunk_radial_distance_sq(center: IVec3, chunk: IVec3) -> i32 {
    let dx = (chunk.x - center.x).abs();
    let dz = (chunk.z - center.z).abs();
    dx * dx + dz * dz
}

fn chunk_sort_key(center: IVec3, key: ChunkKey) -> (u8, i32, i32, i32, i32) {
    let dx = (key.coord.x - center.x).abs();
    let dy = (key.coord.y - center.y).abs();
    let dz = (key.coord.z - center.z).abs();
    (
        key.lod,
        chunk_radial_distance_sq(center, key.coord),
        dy,
        dx.max(dy).max(dz),
        dx + dy + dz,
    )
}

fn pending_priority(
    center: IVec3,
    key: ChunkKey,
) -> Reverse<(i32, i32, i32, i32, i32, i32, i32, u8)> {
    let dx = (key.coord.x - center.x).abs();
    let dy = (key.coord.y - center.y).abs();
    let dz = (key.coord.z - center.z).abs();
    Reverse((
        chunk_radial_distance_sq(center, key.coord),
        dy,
        key.lod as i32,
        dx.max(dy).max(dz),
        key.coord.x,
        key.coord.y,
        key.coord.z,
        key.lod,
    ))
}

pub fn desired_chunk_keys(center: IVec3) -> Vec<ChunkKey> {
    let mut out = Vec::new();

    // Snapshot-free fallback only.
    // Do NOT try to populate the whole distant horizon vertically from this path,
    // because without terrain sampling that recreates the high-altitude empty-air ring.
    //
    // This path should only provide a small local support volume until the snapshot-
    // aware path takes over.
    let local_air_radius = 2;
    let local_air_y_pad = 1;

    for dz in -local_air_radius..=local_air_radius {
        for dx in -local_air_radius..=local_air_radius {
            let cx = center.x + dx;
            let cz = center.z + dz;
            for cy in (center.y - local_air_y_pad)..=(center.y + local_air_y_pad) {
                let coord = IVec3::new(cx, cy, cz);
                let lod = lod_for_chunk(center, coord);
                out.push(ChunkKey { coord, lod });
            }
        }
    }

    out.sort_by_key(|key| chunk_sort_key(center, *key));
    out.dedup();
    out
}
/// Build visible chunk keys using a cached column Y-range lookup.
/// `radius` limits how far from center we scan (for incremental expansion).
fn desired_chunk_keys_with_snapshot_cached(
    center: IVec3,
    snapshot: &WorldSnapshot,
    y_cache: &mut HashMap<(i32, i32), (i32, i32)>,
    radius: i32,
) -> Vec<ChunkKey> {
    let mut out = Vec::with_capacity((radius as usize * 2 + 1).pow(2) * 4);
    let mut seen = HashSet::new();
    let radius_sq = radius * radius;

    let local_air_radius = 2;
    let local_air_y_pad = 1;

    for dz in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dz * dz > radius_sq {
                continue;
            }
            let cx = center.x + dx;
            let cz = center.z + dz;
            let chebyshev = dx.abs().max(dz.abs());
            let &mut (terrain_min_cy, terrain_max_cy) =
                y_cache.entry((cx, cz)).or_insert_with(|| {
                    column_y_chunk_range_distance_cull(cx, cz, &snapshot.cfg, center, chebyshev)
                });
            for cy in terrain_min_cy..=terrain_max_cy {
                let coord = IVec3::new(cx, cy, cz);
                if seen.insert(coord) {
                    let lod = lod_for_chunk(center, coord);
                    out.push(ChunkKey { coord, lod });
                }
            }

            if dx.abs() <= local_air_radius && dz.abs() <= local_air_radius {
                for cy in (center.y - local_air_y_pad)..=(center.y + local_air_y_pad) {
                    let coord = IVec3::new(cx, cy, cz);
                    if seen.insert(coord) {
                        let lod = lod_for_chunk(center, coord);
                        out.push(ChunkKey { coord, lod });
                    }
                }
            }
        }
    }

    out.sort_by_key(|key| chunk_sort_key(center, *key));
    out
}

/// Build chunk keys only for the annular ring between `min_radius` and
/// `max_radius` (inclusive, circular XZ distance from center). Used by
/// `expand_visible` to append only the newly uncovered ring each frame
/// instead of rebuilding the entire visible list from scratch.
fn desired_chunk_keys_ring(
    center: IVec3,
    snapshot: &WorldSnapshot,
    y_cache: &mut HashMap<(i32, i32), (i32, i32)>,
    min_radius: i32,
    max_radius: i32,
) -> Vec<ChunkKey> {
    let mut out = Vec::new();
    let min_radius_sq = min_radius * min_radius;
    let max_radius_sq = max_radius * max_radius;

    for dz in -max_radius..=max_radius {
        for dx in -max_radius..=max_radius {
            let dist_sq = dx * dx + dz * dz;
            if dist_sq > max_radius_sq || dist_sq <= min_radius_sq {
                continue;
            }

            let cx = center.x + dx;
            let cz = center.z + dz;
            let chebyshev = dx.abs().max(dz.abs());
            let &mut (terrain_min_cy, terrain_max_cy) =
                y_cache.entry((cx, cz)).or_insert_with(|| {
                    column_y_chunk_range_distance_cull(cx, cz, &snapshot.cfg, center, chebyshev)
                });
            for cy in terrain_min_cy..=terrain_max_cy {
                let coord = IVec3::new(cx, cy, cz);
                let lod = lod_for_chunk(center, coord);
                out.push(ChunkKey { coord, lod });
            }
        }
    }

    out.sort_by_key(|key| chunk_sort_key(center, *key));
    out
}

pub fn chunk_bounds_sphere(chunk: IVec3) -> (Vec3, f32) {
    let half = CHUNK_SIZE as f32 * 0.5;
    let center = Vec3::new(
        chunk.x as f32 * CHUNK_SIZE as f32 + half,
        chunk.y as f32 * CHUNK_SIZE as f32 + half,
        chunk.z as f32 * CHUNK_SIZE as f32 + half,
    );
    // Circumradius of a cube: half * sqrt(3)
    let radius = half * 3.0_f32.sqrt();
    (center, radius)
}

pub fn extract_planes(view_proj: Mat4) -> [Vec4; 6] {
    let m = view_proj.to_cols_array_2d();
    let row0 = Vec4::new(m[0][0], m[1][0], m[2][0], m[3][0]);
    let row1 = Vec4::new(m[0][1], m[1][1], m[2][1], m[3][1]);
    let row2 = Vec4::new(m[0][2], m[1][2], m[2][2], m[3][2]);
    let row3 = Vec4::new(m[0][3], m[1][3], m[2][3], m[3][3]);

    let mut planes = [
        row3 + row0,
        row3 - row0,
        row3 + row1,
        row3 - row1,
        row3 + row2,
        row3 - row2,
    ];

    for plane in &mut planes {
        let normal = plane.truncate();
        let len = normal.length();
        if len > 0.0 {
            *plane /= len;
        }
    }

    planes
}

pub fn is_chunk_visible_frustum(chunk: IVec3, view_proj: Mat4) -> bool {
    let planes = extract_planes(view_proj);
    let (center, radius) = chunk_bounds_sphere(chunk);
    planes
        .iter()
        .all(|plane| plane.truncate().dot(center) + plane.w >= -radius)
}

pub fn build_draw_list(cache: &ChunkCache, view_proj: Mat4) -> Vec<ChunkKey> {
    let planes = extract_planes(view_proj);
    let mut out = Vec::new();

    for key in &cache.visible {
        let Some(mesh) = cache.gpu_meshes.get(&key.coord) else {
            continue;
        };
        if mesh.index_count == 0 {
            continue;
        }

        let (center, radius) = chunk_bounds_sphere(key.coord);
        let outside = planes
            .iter()
            .any(|plane| plane.truncate().dot(center) + plane.w < -radius);
        if outside {
            continue;
        }

        out.push(*key);
    }

    out
}

fn upload_chunk_mesh(device: &wgpu::Device, mesh: &ChunkMesh, lod: u8) -> ChunkGpuMesh {
    let empty_vertex = [0.0f32; 12];
    let empty_index = [0u32; 1];

    let vertex_buffer = if mesh.vertices.is_empty() {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chunk vb dummy"),
            contents: bytemuck::cast_slice(&empty_vertex),
            usage: wgpu::BufferUsages::VERTEX,
        })
    } else {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chunk vb"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        })
    };

    let index_buffer = if mesh.indices.is_empty() {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chunk ib dummy"),
            contents: bytemuck::cast_slice(&empty_index),
            usage: wgpu::BufferUsages::INDEX,
        })
    } else {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("chunk ib"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        })
    };

    ChunkGpuMesh {
        vertex_buffer,
        index_buffer,
        index_count: mesh.indices.len() as u32,
        lod,
    }
}
fn unproject_clip(inv_view_proj: Mat4, ndc: Vec3) -> Vec3 {
    let clip = Vec4::new(ndc.x, ndc.y, ndc.z, 1.0);
    let world = inv_view_proj * clip;
    world.truncate() / world.w.max(1e-6)
}

struct DynamicVertexBuffer {
    buffer: wgpu::Buffer,
    capacity: usize,
    label: &'static str,
}

impl DynamicVertexBuffer {
    fn new<T: bytemuck::Pod>(device: &wgpu::Device, label: &'static str, capacity: usize) -> Self {
        let capacity = capacity.max(1);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (capacity * std::mem::size_of::<T>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            capacity,
            label,
        }
    }

    fn ensure_capacity<T: bytemuck::Pod>(&mut self, device: &wgpu::Device, len: usize) {
        if len <= self.capacity {
            return;
        }

        self.capacity = len.next_power_of_two().max(1);
        self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(self.label),
            size: (self.capacity * std::mem::size_of::<T>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }

    fn upload<T: bytemuck::Pod>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[T],
    ) -> bool {
        if data.is_empty() {
            return false;
        }

        self.ensure_capacity::<T>(device, data.len());
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
        true
    }

    fn slice<T>(&self, len: usize) -> wgpu::BufferSlice<'_> {
        self.buffer.slice(..(len * std::mem::size_of::<T>()) as u64)
    }
}

pub struct Gpu {
    surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    adapter_summary: String,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    lighting_buffer: wgpu::Buffer,
    lighting_bind_group: wgpu::BindGroup,
    atmosphere: AtmosphereRuntime,
    shadow_renderer: shadow::ShadowRenderer,
    world_surface_bind_group: wgpu::BindGroup,
    cloud_texture_layout: wgpu::BindGroupLayout,
    cloud_render_layout: wgpu::BindGroupLayout,
    cloud_sampler: wgpu::Sampler,
    /// Repeating sampler for world-space weather/noise fields. This must stay
    /// separate from the clamp sampler used for the composited cloud target.
    cloud_field_sampler: wgpu::Sampler,
    weather_primary_tex: wgpu::Texture,
    weather_primary_view: wgpu::TextureView,
    weather_secondary_tex: wgpu::Texture,
    weather_secondary_view: wgpu::TextureView,
    cloud_shadow_tex: wgpu::Texture,
    cloud_shadow_view: wgpu::TextureView,
    shape_noise_view: wgpu::TextureView,
    detail_noise_view: wgpu::TextureView,
    blue_noise_view: wgpu::TextureView,
    cloud_bind_group: wgpu::BindGroup,
    cloud_render_bind_group: wgpu::BindGroup,
    // cloud_target: final temporal-resolved cloud texture read by the composite pass.
    // cloud_current_*: per-frame raw raymarch output before temporal blending.
    // cloud_history_*: previous resolved result used for temporal accumulation.
    // History is explicitly invalidated on resize/bootstrap/discontinuous motion.
    cloud_target_tex: wgpu::Texture,
    cloud_target: wgpu::TextureView,
    cloud_current_tex: wgpu::Texture,
    cloud_current_view: wgpu::TextureView,
    cloud_history_tex: wgpu::Texture,
    cloud_history_view: wgpu::TextureView,
    cloud_temporal_layout: wgpu::BindGroupLayout,
    cloud_temporal_bind_group: wgpu::BindGroup,
    cloud_temporal_pipeline: wgpu::RenderPipeline,
    cloud_pipeline: wgpu::RenderPipeline,
    cloud_composite_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    world_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    line_invert_pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,
    outline_stream: DynamicVertexBuffer,
    invert_outline_stream: DynamicVertexBuffer,
    ui_stream: DynamicVertexBuffer,
    depth: wgpu::TextureView,
    clear_color: [f32; 3],
    current_lighting: LightingState,
    last_camera_pos: Vec3,
    last_inv_view_proj: Mat4,
    last_world_scale: f32,
    cloud_history_valid: bool,
    cloud_history_frame_stats: CloudHistoryFrameStats,
    last_weather_upload_stats: WeatherUploadStats,
    last_render_stats: RenderFrameStats,
    gpu_profiler: Option<GpuProfiler>,
}

// ---------------------------------------------------------------------------
// CPU-only noise data generators — called from background threads at startup
// so the main thread does not freeze during GPU initialisation.
// ---------------------------------------------------------------------------

fn gen_shape_noise_data() -> Vec<u8> {
    let size = 64u32;
    let voxels = (size * size * size) as usize;
    let mut data = vec![0u8; voxels * 4];

    fn hash3(ix: i32, iy: i32, iz: i32, seed: u32, period: i32) -> [f32; 3] {
        let wx = ((ix % period) + period) % period;
        let wy = ((iy % period) + period) % period;
        let wz = ((iz % period) + period) % period;
        let mut h = (wx as u32).wrapping_mul(374_761_393)
            ^ (wy as u32).wrapping_mul(668_265_263)
            ^ (wz as u32).wrapping_mul(1_274_126_177)
            ^ seed;
        h ^= h >> 13;
        h = h.wrapping_mul(0x5bd1e995);
        h ^= h >> 15;
        let x = (h & 0xFFFF) as f32 / 65535.0;
        h = h.wrapping_mul(0x27d4eb2d);
        h ^= h >> 13;
        let y = (h & 0xFFFF) as f32 / 65535.0;
        h = h.wrapping_mul(0x27d4eb2d);
        h ^= h >> 13;
        let z = (h & 0xFFFF) as f32 / 65535.0;
        [x, y, z]
    }

    fn worley_tileable(px: f32, py: f32, pz: f32, freq: i32, seed: u32) -> f32 {
        let fx = px * freq as f32;
        let fy = py * freq as f32;
        let fz = pz * freq as f32;
        let ix = fx.floor() as i32;
        let iy = fy.floor() as i32;
        let iz = fz.floor() as i32;
        let mut min_d = 10.0f32;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let nx = ix + dx;
                    let ny = iy + dy;
                    let nz = iz + dz;
                    let pt = hash3(nx, ny, nz, seed, freq);
                    let ddx = (nx as f32 + pt[0]) - fx;
                    let ddy = (ny as f32 + pt[1]) - fy;
                    let ddz = (nz as f32 + pt[2]) - fz;
                    let d = (ddx * ddx + ddy * ddy + ddz * ddz).sqrt();
                    min_d = min_d.min(d);
                }
            }
        }
        1.0 - min_d.clamp(0.0, 1.0)
    }

    fn grad3(hash: u32) -> [f32; 3] {
        const GRADS: [[f32; 3]; 12] = [
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
            [1.0, -1.0, 0.0],
            [-1.0, -1.0, 0.0],
            [1.0, 0.0, 1.0],
            [-1.0, 0.0, 1.0],
            [1.0, 0.0, -1.0],
            [-1.0, 0.0, -1.0],
            [0.0, 1.0, 1.0],
            [0.0, -1.0, 1.0],
            [0.0, 1.0, -1.0],
            [0.0, -1.0, -1.0],
        ];
        GRADS[(hash % 12) as usize]
    }

    fn perlin_tileable(px: f32, py: f32, pz: f32, freq: i32, seed: u32) -> f32 {
        let fx = px * freq as f32;
        let fy = py * freq as f32;
        let fz = pz * freq as f32;
        let x0 = fx.floor() as i32;
        let y0 = fy.floor() as i32;
        let z0 = fz.floor() as i32;
        let xf = fx - fx.floor();
        let yf = fy - fy.floor();
        let zf = fz - fz.floor();
        let sx = xf * xf * xf * (xf * (xf * 6.0 - 15.0) + 10.0);
        let sy = yf * yf * yf * (yf * (yf * 6.0 - 15.0) + 10.0);
        let sz = zf * zf * zf * (zf * (zf * 6.0 - 15.0) + 10.0);
        let period = freq;
        let corner_hash = |cx: i32, cy: i32, cz: i32| -> u32 {
            let wx = ((cx % period) + period) % period;
            let wy = ((cy % period) + period) % period;
            let wz = ((cz % period) + period) % period;
            let mut h = (wx as u32).wrapping_mul(374_761_393)
                ^ (wy as u32).wrapping_mul(668_265_263)
                ^ (wz as u32).wrapping_mul(1_274_126_177)
                ^ seed;
            h ^= h >> 13;
            h = h.wrapping_mul(0x5bd1e995);
            h ^= h >> 15;
            h
        };
        let dot_grad = |cx: i32, cy: i32, cz: i32, dx: f32, dy: f32, dz: f32| -> f32 {
            let g = grad3(corner_hash(cx, cy, cz));
            g[0] * dx + g[1] * dy + g[2] * dz
        };
        let n000 = dot_grad(x0, y0, z0, xf, yf, zf);
        let n100 = dot_grad(x0 + 1, y0, z0, xf - 1.0, yf, zf);
        let n010 = dot_grad(x0, y0 + 1, z0, xf, yf - 1.0, zf);
        let n110 = dot_grad(x0 + 1, y0 + 1, z0, xf - 1.0, yf - 1.0, zf);
        let n001 = dot_grad(x0, y0, z0 + 1, xf, yf, zf - 1.0);
        let n101 = dot_grad(x0 + 1, y0, z0 + 1, xf - 1.0, yf, zf - 1.0);
        let n011 = dot_grad(x0, y0 + 1, z0 + 1, xf, yf - 1.0, zf - 1.0);
        let n111 = dot_grad(x0 + 1, y0 + 1, z0 + 1, xf - 1.0, yf - 1.0, zf - 1.0);
        let x00 = n000 + (n100 - n000) * sx;
        let x10 = n010 + (n110 - n010) * sx;
        let x01 = n001 + (n101 - n001) * sx;
        let x11 = n011 + (n111 - n011) * sx;
        let y0v = x00 + (x10 - x00) * sy;
        let y1v = x01 + (x11 - x01) * sy;
        (y0v + (y1v - y0v) * sz) * 0.5 + 0.5
    }

    for z in 0..size {
        for y in 0..size {
            for x in 0..size {
                let i = ((z * size * size + y * size + x) * 4) as usize;
                let px = x as f32 / size as f32;
                let py = y as f32 / size as f32;
                let pz = z as f32 / size as f32;

                // Removed freq=64 octave (weight 0.03125, visually negligible at mip
                // distances but cost 2M×27 Worley-equivalent operations per frame of startup).
                let perlin_fbm = perlin_tileable(px, py, pz, 4, 0x1111) * 0.5
                    + perlin_tileable(px, py, pz, 8, 0x2222) * 0.25
                    + perlin_tileable(px, py, pz, 16, 0x3333) * 0.125
                    + perlin_tileable(px, py, pz, 32, 0x4444) * 0.0625;
                let perlin_fbm = perlin_fbm / 0.9375;

                let w_fbm = worley_tileable(px, py, pz, 4, 0xAAAA) * 0.625
                    + worley_tileable(px, py, pz, 8, 0xABBB) * 0.25
                    + worley_tileable(px, py, pz, 16, 0xACCC) * 0.125;
                let w_fbm = w_fbm.clamp(0.0, 1.0);

                let perlin_norm = perlin_fbm.clamp(0.0, 1.0);
                let lo = (1.0 - w_fbm).clamp(0.0, 0.999_f32);
                let pw = ((perlin_norm - lo) / (1.0 - lo)).clamp(0.0, 1.0);

                let w1 = (worley_tileable(px, py, pz, 8, 0xBBBB) * 0.625
                    + worley_tileable(px, py, pz, 16, 0xBCCC) * 0.25
                    + worley_tileable(px, py, pz, 32, 0xBDDD) * 0.125)
                    .clamp(0.0, 1.0);
                // Removed freq=64 from W2 and W3 — each saved 2M×27 = 54M distance ops.
                // W3 is the alpha channel (coarsest Worley detail), freq=32 is sufficient.
                let w2 = (worley_tileable(px, py, pz, 16, 0xCCCC) * 0.625
                    + worley_tileable(px, py, pz, 32, 0xCDDD) * 0.375)
                    .clamp(0.0, 1.0);
                let w3 = worley_tileable(px, py, pz, 32, 0xDDDD).clamp(0.0, 1.0);

                data[i] = (pw * 255.0) as u8;
                data[i + 1] = (w1 * 255.0) as u8;
                data[i + 2] = (w2 * 255.0) as u8;
                data[i + 3] = (w3 * 255.0) as u8;
            }
        }
    }

    data
}

fn gen_detail_noise_data() -> Vec<u8> {
    let size = 32u32;
    let voxels = (size * size * size) as usize;
    let mut data = vec![0u8; voxels * 4];

    fn hash3(ix: i32, iy: i32, iz: i32, seed: u32, period: i32) -> [f32; 3] {
        let wx = ((ix % period) + period) % period;
        let wy = ((iy % period) + period) % period;
        let wz = ((iz % period) + period) % period;
        let mut h = (wx as u32).wrapping_mul(374_761_393)
            ^ (wy as u32).wrapping_mul(668_265_263)
            ^ (wz as u32).wrapping_mul(1_274_126_177)
            ^ seed;
        h ^= h >> 13;
        h = h.wrapping_mul(0x5bd1e995);
        h ^= h >> 15;
        let x = (h & 0xFFFF) as f32 / 65535.0;
        h = h.wrapping_mul(0x27d4eb2d);
        h ^= h >> 13;
        let y = (h & 0xFFFF) as f32 / 65535.0;
        h = h.wrapping_mul(0x27d4eb2d);
        h ^= h >> 13;
        let z = (h & 0xFFFF) as f32 / 65535.0;
        [x, y, z]
    }

    fn worley_tileable(px: f32, py: f32, pz: f32, freq: i32, seed: u32) -> f32 {
        let fx = px * freq as f32;
        let fy = py * freq as f32;
        let fz = pz * freq as f32;
        let ix = fx.floor() as i32;
        let iy = fy.floor() as i32;
        let iz = fz.floor() as i32;
        let mut min_d = 10.0f32;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let nx = ix + dx;
                    let ny = iy + dy;
                    let nz = iz + dz;
                    let pt = hash3(nx, ny, nz, seed, freq);
                    let ddx = (nx as f32 + pt[0]) - fx;
                    let ddy = (ny as f32 + pt[1]) - fy;
                    let ddz = (nz as f32 + pt[2]) - fz;
                    let d = (ddx * ddx + ddy * ddy + ddz * ddz).sqrt();
                    min_d = min_d.min(d);
                }
            }
        }
        1.0 - (min_d * 1.5).min(1.0)
    }

    for z in 0..size {
        for y in 0..size {
            for x in 0..size {
                let i = ((z * size * size + y * size + x) * 4) as usize;
                let px = x as f32 / size as f32;
                let py = y as f32 / size as f32;
                let pz = z as f32 / size as f32;

                let w1 = (worley_tileable(px, py, pz, 4, 0xEEEE) * 0.625
                    + worley_tileable(px, py, pz, 8, 0xEFFF) * 0.25
                    + worley_tileable(px, py, pz, 16, 0xE111) * 0.125)
                    .clamp(0.0, 1.0);
                let w2 = (worley_tileable(px, py, pz, 8, 0xFFFF) * 0.625
                    + worley_tileable(px, py, pz, 16, 0xF111) * 0.25
                    + worley_tileable(px, py, pz, 32, 0xF222) * 0.125)
                    .clamp(0.0, 1.0);
                // Removed freq=64 octave — cost 262K×27 distance ops for negligible
                // visual quality on a 64³ texture (1 period per texel at freq=64).
                let w3 = (worley_tileable(px, py, pz, 16, 0x1234) * 0.667
                    + worley_tileable(px, py, pz, 32, 0x1345) * 0.333)
                    .clamp(0.0, 1.0);

                // Simplified curl: 4 samples instead of 6 (skip one axis pair).
                let eps = 2.0 / size as f32;
                let dnx_dz = worley_tileable(px, py, pz + eps, 8, 0x5678)
                    - worley_tileable(px, py, pz - eps, 8, 0x5678);
                let dny_dx = worley_tileable(px + eps, py, pz, 8, 0x9ABC)
                    - worley_tileable(px - eps, py, pz, 8, 0x9ABC);
                let curl = ((dnx_dz * dnx_dz + dny_dx * dny_dx).sqrt() * 3.0).clamp(0.0, 1.0);

                data[i] = (w1 * 255.0) as u8;
                data[i + 1] = (w2 * 255.0) as u8;
                data[i + 2] = (w3 * 255.0) as u8;
                data[i + 3] = (curl * 255.0) as u8;
            }
        }
    }

    data
}

/// Seed weather clipmap arrays with zeros so the bind groups are valid before the
/// CPU weather subsystem uploads its first real frame.
fn gen_weather_texture_data() -> Vec<u8> {
    vec![
        0u8;
        WEATHER_TEXTURE_RESOLUTION as usize
            * WEATHER_TEXTURE_RESOLUTION as usize
            * WEATHER_CLIPMAP_LEVEL_COUNT
            * 16
    ]
}
#[derive(Clone, Copy, Debug)]
pub struct LoadingProgress {
    pub stage_label: &'static str,
    pub detail: &'static str,
    pub progress_01: f32,
}

struct PendingNoiseThreads {
    shape_noise_rx: mpsc::Receiver<Vec<u8>>,
    detail_noise_rx: mpsc::Receiver<Vec<u8>>,
    weather_tex_rx: mpsc::Receiver<Vec<u8>>,
    shape_noise_data: Option<Vec<u8>>,
    detail_noise_data: Option<Vec<u8>>,
    weather_tex_data: Option<Vec<u8>>,
}

pub struct GpuBootstrap {
    surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    adapter_summary: String,
    default_lighting: LightingState,
    clear_color: [f32; 3],
    noise_threads: PendingNoiseThreads,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GpuInitStage {
    CoreBindings,
    CloudFields,
    ShaderModules,
    Pipelines,
    Finalize,
    Complete,
}

pub struct GpuPendingInit {
    bootstrap: Option<GpuBootstrap>,
    stage: GpuInitStage,

    camera_buffer: Option<wgpu::Buffer>,
    camera_layout: Option<wgpu::BindGroupLayout>,
    camera_bind_group: Option<wgpu::BindGroup>,

    lighting_buffer: Option<wgpu::Buffer>,
    lighting_layout: Option<wgpu::BindGroupLayout>,
    lighting_bind_group: Option<wgpu::BindGroup>,
    atmosphere_layout: Option<wgpu::BindGroupLayout>,
    atmosphere_runtime: Option<AtmosphereRuntime>,
    shadow_renderer: Option<shadow::ShadowRenderer>,

    world_surface_layout: Option<wgpu::BindGroupLayout>,
    cloud_texture_layout: Option<wgpu::BindGroupLayout>,
    cloud_render_layout: Option<wgpu::BindGroupLayout>,
    cloud_temporal_layout: Option<wgpu::BindGroupLayout>,

    world_surface_bind_group: Option<wgpu::BindGroup>,

    cloud_sampler: Option<wgpu::Sampler>,
    cloud_field_sampler: Option<wgpu::Sampler>,
    weather_primary_tex: Option<wgpu::Texture>,
    weather_primary_view: Option<wgpu::TextureView>,
    weather_secondary_tex: Option<wgpu::Texture>,
    weather_secondary_view: Option<wgpu::TextureView>,
    cloud_shadow_tex: Option<wgpu::Texture>,
    cloud_shadow_view: Option<wgpu::TextureView>,
    cloud_target_tex: Option<wgpu::Texture>,
    cloud_target: Option<wgpu::TextureView>,
    cloud_current_tex: Option<wgpu::Texture>,
    cloud_current_view: Option<wgpu::TextureView>,
    cloud_history_tex: Option<wgpu::Texture>,
    cloud_history_view: Option<wgpu::TextureView>,
    shape_noise_view: Option<wgpu::TextureView>,
    detail_noise_view: Option<wgpu::TextureView>,
    blue_noise_view: Option<wgpu::TextureView>,
    cloud_bind_group: Option<wgpu::BindGroup>,
    cloud_render_bind_group: Option<wgpu::BindGroup>,
    cloud_temporal_bind_group: Option<wgpu::BindGroup>,

    depth: Option<wgpu::TextureView>,

    cloud_shader: Option<wgpu::ShaderModule>,
    cloud_composite_shader: Option<wgpu::ShaderModule>,
    sky_shader: Option<wgpu::ShaderModule>,
    world_shader: Option<wgpu::ShaderModule>,
    line_shader: Option<wgpu::ShaderModule>,
    shadow_shader: Option<wgpu::ShaderModule>,
    ui_shader: Option<wgpu::ShaderModule>,
    cloud_temporal_shader: Option<wgpu::ShaderModule>,

    cloud_pipeline: Option<wgpu::RenderPipeline>,
    cloud_composite_pipeline: Option<wgpu::RenderPipeline>,
    sky_pipeline: Option<wgpu::RenderPipeline>,
    cloud_temporal_pipeline: Option<wgpu::RenderPipeline>,
    world_pipeline: Option<wgpu::RenderPipeline>,
    line_pipeline: Option<wgpu::RenderPipeline>,
    line_invert_pipeline: Option<wgpu::RenderPipeline>,
    ui_pipeline: Option<wgpu::RenderPipeline>,

    outline_stream: Option<DynamicVertexBuffer>,
    invert_outline_stream: Option<DynamicVertexBuffer>,
    ui_stream: Option<DynamicVertexBuffer>,

    last_error: Option<anyhow::Error>,
}
impl GpuBootstrap {
    pub async fn new(window: Arc<Window>) -> Result<Self> {
        let (shape_tx, shape_noise_rx) = mpsc::channel();
        let (detail_tx, detail_noise_rx) = mpsc::channel();
        let (weather_tx, weather_tex_rx) = mpsc::channel();

        thread::spawn(move || {
            let _ = shape_tx.send(gen_shape_noise_data());
        });
        thread::spawn(move || {
            let _ = detail_tx.send(gen_detail_noise_data());
        });
        thread::spawn(move || {
            let _ = weather_tx.send(gen_weather_texture_data());
        });

        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("No suitable GPU adapter found")?;
        let adapter_info = adapter.get_info();
        let adapter_summary = format!(
            "{} / {:?} / {:?}",
            adapter_info.name, adapter_info.backend, adapter_info.device_type
        );
        let requested_features = if adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            wgpu::Features::TIMESTAMP_QUERY
        } else {
            wgpu::Features::empty()
        };

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("device"),
                required_features: requested_features,
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let default_lighting = LightingState::from_time(0.50);
        let clear_color = [
            default_lighting.sky_horizon[0] * 0.72 + default_lighting.sky_top[0] * 0.28,
            default_lighting.sky_horizon[1] * 0.72 + default_lighting.sky_top[1] * 0.28,
            default_lighting.sky_horizon[2] * 0.72 + default_lighting.sky_top[2] * 0.28,
        ];

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
            adapter_summary,
            default_lighting,
            clear_color,
            noise_threads: PendingNoiseThreads {
                shape_noise_rx,
                detail_noise_rx,
                weather_tex_rx,
                shape_noise_data: None,
                detail_noise_data: None,
                weather_tex_data: None,
            },
        })
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render_loading_frame<F>(
        &mut self,
        clear_color: [f32; 3],
        overlay: F,
    ) -> Result<(), wgpu::SurfaceError>
    where
        F: FnOnce(
            &wgpu::Device,
            &wgpu::Queue,
            &mut wgpu::CommandEncoder,
            &wgpu::TextureView,
            u32,
            u32,
        ),
    {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("loading encoder"),
            });

        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("loading clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear_color[0] as f64,
                            g: clear_color[1] as f64,
                            b: clear_color[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        overlay(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            self.config.width,
            self.config.height,
        );

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }

    pub fn into_pending_init(self) -> GpuPendingInit {
        GpuPendingInit {
            bootstrap: Some(self),
            stage: GpuInitStage::CoreBindings,

            camera_buffer: None,
            camera_layout: None,
            camera_bind_group: None,

            lighting_buffer: None,
            lighting_layout: None,
            lighting_bind_group: None,
            atmosphere_layout: None,
            atmosphere_runtime: None,
            shadow_renderer: None,

            world_surface_layout: None,
            cloud_texture_layout: None,
            cloud_render_layout: None,
            cloud_temporal_layout: None,

            world_surface_bind_group: None,

            cloud_sampler: None,
            cloud_field_sampler: None,
            weather_primary_tex: None,
            weather_primary_view: None,
            weather_secondary_tex: None,
            weather_secondary_view: None,
            cloud_shadow_tex: None,
            cloud_shadow_view: None,
            cloud_target_tex: None,
            cloud_target: None,
            cloud_current_tex: None,
            cloud_current_view: None,
            cloud_history_tex: None,
            cloud_history_view: None,
            shape_noise_view: None,
            detail_noise_view: None,
            blue_noise_view: None,
            cloud_bind_group: None,
            cloud_render_bind_group: None,
            cloud_temporal_bind_group: None,

            depth: None,

            cloud_shader: None,
            cloud_composite_shader: None,
            sky_shader: None,
            world_shader: None,
            line_shader: None,
            shadow_shader: None,
            ui_shader: None,
            cloud_temporal_shader: None,

            cloud_pipeline: None,
            cloud_composite_pipeline: None,
            sky_pipeline: None,
            cloud_temporal_pipeline: None,
            world_pipeline: None,
            line_pipeline: None,
            line_invert_pipeline: None,
            ui_pipeline: None,

            outline_stream: None,
            invert_outline_stream: None,
            ui_stream: None,

            last_error: None,
        }
    }
}
impl GpuPendingInit {
    pub fn progress(&self) -> LoadingProgress {
        match self.stage {
            GpuInitStage::CoreBindings => LoadingProgress {
                stage_label: "Renderer bootstrap",
                detail: "Creating uniform buffers and bind-group layouts…",
                progress_01: 0.20,
            },
            GpuInitStage::CloudFields => LoadingProgress {
                stage_label: "Renderer bootstrap",
                detail: "Uploading cloud/weather textures…",
                progress_01: 0.45,
            },
            GpuInitStage::ShaderModules => LoadingProgress {
                stage_label: "Renderer bootstrap",
                detail: "Compiling shader modules…",
                progress_01: 0.65,
            },
            GpuInitStage::Pipelines => LoadingProgress {
                stage_label: "Renderer bootstrap",
                detail: "Creating render pipelines…",
                progress_01: 0.82,
            },
            GpuInitStage::Finalize => LoadingProgress {
                stage_label: "Renderer bootstrap",
                detail: "Finalizing renderer resources…",
                progress_01: 0.92,
            },
            GpuInitStage::Complete => LoadingProgress {
                stage_label: "Renderer bootstrap",
                detail: "Done",
                progress_01: 1.0,
            },
        }
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.bootstrap
            .as_ref()
            .expect("bootstrap missing in pending init")
            .size()
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if let Some(bootstrap) = self.bootstrap.as_mut() {
            bootstrap.resize(new_size);
        }
    }

    pub fn render_loading_frame<F>(
        &mut self,
        clear_color: [f32; 3],
        overlay: F,
    ) -> Result<(), wgpu::SurfaceError>
    where
        F: FnOnce(
            &wgpu::Device,
            &wgpu::Queue,
            &mut wgpu::CommandEncoder,
            &wgpu::TextureView,
            u32,
            u32,
        ),
    {
        self.bootstrap
            .as_mut()
            .expect("bootstrap missing in pending init")
            .render_loading_frame(clear_color, overlay)
    }

    pub fn step(&mut self) -> Result<Option<Gpu>> {
        match self.stage {
            GpuInitStage::CoreBindings => {
                let bootstrap = self.bootstrap.as_ref().expect("bootstrap missing");
                let device = &bootstrap.device;
                let default_lighting = bootstrap.default_lighting;

                let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("camera buffer"),
                    contents: bytemuck::bytes_of(&CameraUniform {
                        view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                        camera_pos: [0.0, 0.0, 0.0, 0.0],
                        inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                        world_params: [1.0, 1.0, 0.0, 0.0],
                        prev_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                        camera_forward: [0.0, 0.0, -1.0, 0.0],
                    }),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

                let camera_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("camera layout"),
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                | wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        }],
                    });

                let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("camera bind group"),
                    layout: &camera_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    }],
                });

                let lighting_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("lighting layout"),
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                | wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        }],
                    });

                let lighting_buffer =
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("lighting buffer"),
                        contents: bytemuck::bytes_of(&lighting_uniform_from_state(
                            &default_lighting,
                            &CloudControls::default(),
                            0.0,
                            None,
                            &[],
                            CloudWeatherUniformState::default(),
                            RenderDebugView::default(),
                        )),
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    });

                let lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("lighting bind group"),
                    layout: &lighting_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: lighting_buffer.as_entire_binding(),
                    }],
                });

                let atmosphere_layout = AtmosphereRuntime::create_bind_group_layout(device);

                let world_surface_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("world surface layout"),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Depth,
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Sampler(
                                    wgpu::SamplerBindingType::Comparison,
                                ),
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 4,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                        ],
                    });

                let cloud_texture_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("cloud texture layout"),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                        ],
                    });

                let cloud_render_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("cloud render layout"),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D3,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 4,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D3,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 5,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 6,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Depth,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 7,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: false,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                    multisampled: false,
                                },
                                count: None,
                            },
                        ],
                    });

                let cloud_temporal_bg_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("cloud temporal bind group layout"),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                        ],
                    });

                self.camera_buffer = Some(camera_buffer);
                self.camera_layout = Some(camera_layout);
                self.camera_bind_group = Some(camera_bind_group);

                self.lighting_buffer = Some(lighting_buffer);
                self.lighting_layout = Some(lighting_layout);
                self.lighting_bind_group = Some(lighting_bind_group);
                self.atmosphere_layout = Some(atmosphere_layout);

                self.world_surface_layout = Some(world_surface_layout);
                self.cloud_texture_layout = Some(cloud_texture_layout);
                self.cloud_render_layout = Some(cloud_render_layout);
                self.cloud_temporal_layout = Some(cloud_temporal_bg_layout);

                self.stage = GpuInitStage::CloudFields;
                Ok(None)
            }

            GpuInitStage::CloudFields => {
                let bootstrap = self.bootstrap.as_mut().expect("bootstrap missing");

                // Poll all workers each step and keep any partial results we already have.
                // This stays non-blocking and avoids artificially serializing readiness.
                let mut waiting = false;

                if bootstrap.noise_threads.shape_noise_data.is_none() {
                    match bootstrap.noise_threads.shape_noise_rx.try_recv() {
                        Ok(data) => bootstrap.noise_threads.shape_noise_data = Some(data),
                        Err(mpsc::TryRecvError::Empty) => waiting = true,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            anyhow::bail!("shape noise worker disconnected");
                        }
                    }
                }

                if bootstrap.noise_threads.detail_noise_data.is_none() {
                    match bootstrap.noise_threads.detail_noise_rx.try_recv() {
                        Ok(data) => bootstrap.noise_threads.detail_noise_data = Some(data),
                        Err(mpsc::TryRecvError::Empty) => waiting = true,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            anyhow::bail!("detail noise worker disconnected");
                        }
                    }
                }

                if bootstrap.noise_threads.weather_tex_data.is_none() {
                    match bootstrap.noise_threads.weather_tex_rx.try_recv() {
                        Ok(data) => bootstrap.noise_threads.weather_tex_data = Some(data),
                        Err(mpsc::TryRecvError::Empty) => waiting = true,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            anyhow::bail!("weather texture worker disconnected");
                        }
                    }
                }

                if waiting
                    || bootstrap.noise_threads.shape_noise_data.is_none()
                    || bootstrap.noise_threads.detail_noise_data.is_none()
                    || bootstrap.noise_threads.weather_tex_data.is_none()
                {
                    return Ok(None);
                }

                let shape_noise_data = bootstrap
                    .noise_threads
                    .shape_noise_data
                    .take()
                    .expect("shape noise data missing");
                let detail_noise_data = bootstrap
                    .noise_threads
                    .detail_noise_data
                    .take()
                    .expect("detail noise data missing");
                let weather_tex_data = bootstrap
                    .noise_threads
                    .weather_tex_data
                    .take()
                    .expect("weather texture data missing");

                let device = &bootstrap.device;
                let queue = &bootstrap.queue;
                let config = &bootstrap.config;

                let cloud_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("cloud sampler"),
                    address_mode_u: wgpu::AddressMode::ClampToEdge,
                    address_mode_v: wgpu::AddressMode::ClampToEdge,
                    address_mode_w: wgpu::AddressMode::ClampToEdge,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Linear,
                    ..Default::default()
                });

                let cloud_field_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("cloud field sampler"),
                    address_mode_u: wgpu::AddressMode::Repeat,
                    address_mode_v: wgpu::AddressMode::Repeat,
                    address_mode_w: wgpu::AddressMode::Repeat,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::FilterMode::Linear,
                    ..Default::default()
                });

                let (weather_primary_tex, weather_primary_view) = Gpu::create_weather_texture(
                    device,
                    queue,
                    "weather primary",
                    &weather_tex_data,
                );
                let zero_secondary = vec![0u8; weather_tex_data.len()];
                let (weather_secondary_tex, weather_secondary_view) = Gpu::create_weather_texture(
                    device,
                    queue,
                    "weather secondary",
                    &zero_secondary,
                );
                let zero_shadow = vec![0u8; weather_tex_data.len()];
                let (cloud_shadow_tex, cloud_shadow_view) =
                    Gpu::create_weather_texture(device, queue, "cloud shadow field", &zero_shadow);
                let shape_noise_view = Gpu::create_shape_noise_3d(device, queue, shape_noise_data);
                let detail_noise_view =
                    Gpu::create_detail_noise_3d(device, queue, detail_noise_data);
                let blue_noise_view = Gpu::create_blue_noise_texture(device, queue);

                let (cloud_target_tex, cloud_target) = Gpu::create_cloud_target(device, config);

                // create_cloud_texture() already adds RENDER_ATTACHMENT | TEXTURE_BINDING.
                // extra usages are only the additional flags specific to each role.
                let (cloud_current_tex, cloud_current_view) = Gpu::create_cloud_texture(
                    device,
                    config,
                    "cloud current",
                    wgpu::TextureUsages::COPY_SRC,
                );

                let (cloud_history_tex, cloud_history_view) = Gpu::create_cloud_texture(
                    device,
                    config,
                    "cloud history",
                    wgpu::TextureUsages::COPY_DST,
                );

                let cloud_temporal_bind_group = Gpu::create_cloud_temporal_bind_group(
                    device,
                    self.cloud_temporal_layout.as_ref().unwrap(),
                    &cloud_sampler,
                    &cloud_current_view,
                    &cloud_history_view,
                );

                let cloud_bind_group = Gpu::create_cloud_bind_group(
                    device,
                    self.cloud_texture_layout.as_ref().unwrap(),
                    &cloud_sampler,
                    &cloud_target,
                );

                let depth = Gpu::create_depth(device, config);
                let cloud_render_bind_group = Gpu::create_cloud_render_bind_group(
                    device,
                    self.cloud_render_layout.as_ref().unwrap(),
                    &cloud_field_sampler,
                    &weather_primary_view,
                    &weather_secondary_view,
                    &shape_noise_view,
                    &detail_noise_view,
                    &blue_noise_view,
                    &depth,
                    &cloud_shadow_view,
                );
                let atmosphere_runtime = AtmosphereRuntime::new(
                    device,
                    queue,
                    self.atmosphere_layout.as_ref().unwrap().clone(),
                    self.camera_layout.as_ref().unwrap(),
                    self.camera_bind_group.as_ref().unwrap(),
                    config.width,
                    config.height,
                    &bootstrap.default_lighting,
                    &CloudControls::default(),
                );

                self.cloud_sampler = Some(cloud_sampler);
                self.cloud_field_sampler = Some(cloud_field_sampler);
                self.weather_primary_tex = Some(weather_primary_tex);
                self.weather_primary_view = Some(weather_primary_view);
                self.weather_secondary_tex = Some(weather_secondary_tex);
                self.weather_secondary_view = Some(weather_secondary_view);
                self.cloud_shadow_tex = Some(cloud_shadow_tex);
                self.cloud_shadow_view = Some(cloud_shadow_view);
                self.shape_noise_view = Some(shape_noise_view);
                self.detail_noise_view = Some(detail_noise_view);
                self.blue_noise_view = Some(blue_noise_view);
                self.cloud_target_tex = Some(cloud_target_tex);
                self.cloud_target = Some(cloud_target);
                self.cloud_current_tex = Some(cloud_current_tex);
                self.cloud_current_view = Some(cloud_current_view);
                self.cloud_history_tex = Some(cloud_history_tex);
                self.cloud_history_view = Some(cloud_history_view);
                self.cloud_bind_group = Some(cloud_bind_group);
                self.cloud_render_bind_group = Some(cloud_render_bind_group);
                self.cloud_temporal_bind_group = Some(cloud_temporal_bind_group);

                self.depth = Some(depth);
                self.atmosphere_runtime = Some(atmosphere_runtime);

                self.stage = GpuInitStage::ShaderModules;
                Ok(None)
            }

            GpuInitStage::ShaderModules => {
                let bootstrap = self.bootstrap.as_ref().expect("bootstrap missing");
                let device = &bootstrap.device;

                self.cloud_shader =
                    Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("cloud shader"),
                        source: wgpu::ShaderSource::Wgsl(cloud_shader_source().into()),
                    }));
                self.cloud_composite_shader =
                    Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("cloud composite shader"),
                        source: wgpu::ShaderSource::Wgsl(cloud_composite_shader_source().into()),
                    }));
                self.sky_shader = Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("sky shader"),
                    source: wgpu::ShaderSource::Wgsl(sky_shader_source().into()),
                }));
                self.world_shader =
                    Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("world shader"),
                        source: wgpu::ShaderSource::Wgsl(world_shader_source().into()),
                    }));
                self.line_shader =
                    Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("line shader"),
                        source: wgpu::ShaderSource::Wgsl(LINE_SHADER.into()),
                    }));
                self.shadow_shader =
                    Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("shadow shader"),
                        source: wgpu::ShaderSource::Wgsl(SHADOW_SHADER.into()),
                    }));
                self.ui_shader = Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("ui shader"),
                    source: wgpu::ShaderSource::Wgsl(UI_SHADER.into()),
                }));
                self.cloud_temporal_shader =
                    Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("cloud temporal shader"),
                        source: wgpu::ShaderSource::Wgsl(cloud_temporal_shader_source().into()),
                    }));

                self.stage = GpuInitStage::Pipelines;
                Ok(None)
            }

            GpuInitStage::Pipelines => {
                let bootstrap = self.bootstrap.as_ref().expect("bootstrap missing");
                let device = &bootstrap.device;
                let config = &bootstrap.config;

                let cloud_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("cloud layout"),
                    bind_group_layouts: &[
                        self.camera_layout.as_ref().unwrap(),
                        self.lighting_layout.as_ref().unwrap(),
                        self.atmosphere_layout.as_ref().unwrap(),
                        self.cloud_render_layout.as_ref().unwrap(),
                    ],
                    push_constant_ranges: &[],
                });

                let sky_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("sky layout"),
                    bind_group_layouts: &[
                        self.camera_layout.as_ref().unwrap(),
                        self.lighting_layout.as_ref().unwrap(),
                        self.atmosphere_layout.as_ref().unwrap(),
                    ],
                    push_constant_ranges: &[],
                });

                let cloud_composite_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("cloud composite layout"),
                        bind_group_layouts: &[
                            self.lighting_layout.as_ref().unwrap(),
                            self.cloud_texture_layout.as_ref().unwrap(),
                            self.atmosphere_layout.as_ref().unwrap(),
                            self.cloud_render_layout.as_ref().unwrap(),
                        ],
                        push_constant_ranges: &[],
                    });

                let world_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("world layout"),
                    bind_group_layouts: &[
                        self.camera_layout.as_ref().unwrap(),
                        self.lighting_layout.as_ref().unwrap(),
                        self.atmosphere_layout.as_ref().unwrap(),
                        self.world_surface_layout.as_ref().unwrap(),
                    ],
                    push_constant_ranges: &[],
                });

                let line_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("line layout"),
                    bind_group_layouts: &[self.camera_layout.as_ref().unwrap()],
                    push_constant_ranges: &[],
                });

                let ui_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("ui layout"),
                    bind_group_layouts: &[],
                    push_constant_ranges: &[],
                });

                let cloud_temporal_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("cloud temporal pipeline layout"),
                        bind_group_layouts: &[
                            self.camera_layout.as_ref().unwrap(),
                            self.lighting_layout.as_ref().unwrap(),
                            self.cloud_temporal_layout.as_ref().unwrap(),
                        ],
                        push_constant_ranges: &[],
                    });

                self.cloud_pipeline = Some(device.create_render_pipeline(
                    &wgpu::RenderPipelineDescriptor {
                        label: Some("cloud pipeline"),
                        layout: Some(&cloud_layout),
                        vertex: wgpu::VertexState {
                            module: self.cloud_shader.as_ref().unwrap(),
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            buffers: &[],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: self.cloud_shader.as_ref().unwrap(),
                            entry_point: Some("fs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: None,
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: None,
                        multisample: Default::default(),
                        multiview: None,
                        cache: None,
                    },
                ));

                self.sky_pipeline = Some(device.create_render_pipeline(
                    &wgpu::RenderPipelineDescriptor {
                        label: Some("sky pipeline"),
                        layout: Some(&sky_layout),
                        vertex: wgpu::VertexState {
                            module: self.sky_shader.as_ref().unwrap(),
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            buffers: &[],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: self.sky_shader.as_ref().unwrap(),
                            entry_point: Some("fs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: config.format,
                                blend: Some(wgpu::BlendState::REPLACE),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: Some(wgpu::DepthStencilState {
                            format: wgpu::TextureFormat::Depth24Plus,
                            depth_write_enabled: false,
                            depth_compare: wgpu::CompareFunction::Equal,
                            stencil: Default::default(),
                            bias: Default::default(),
                        }),
                        multisample: Default::default(),
                        multiview: None,
                        cache: None,
                    },
                ));

                self.cloud_composite_pipeline = Some(device.create_render_pipeline(
                    &wgpu::RenderPipelineDescriptor {
                        label: Some("cloud composite pipeline"),
                        layout: Some(&cloud_composite_layout),
                        vertex: wgpu::VertexState {
                            module: self.cloud_composite_shader.as_ref().unwrap(),
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            buffers: &[],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: self.cloud_composite_shader.as_ref().unwrap(),
                            entry_point: Some("fs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: config.format,
                                blend: Some(wgpu::BlendState {
                                    color: wgpu::BlendComponent {
                                        src_factor: wgpu::BlendFactor::One,
                                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                        operation: wgpu::BlendOperation::Add,
                                    },
                                    alpha: wgpu::BlendComponent {
                                        src_factor: wgpu::BlendFactor::One,
                                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                        operation: wgpu::BlendOperation::Add,
                                    },
                                }),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: None,
                        multisample: Default::default(),
                        multiview: None,
                        cache: None,
                    },
                ));

                self.cloud_temporal_pipeline = Some(device.create_render_pipeline(
                    &wgpu::RenderPipelineDescriptor {
                        label: Some("cloud temporal pipeline"),
                        layout: Some(&cloud_temporal_pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: self.cloud_temporal_shader.as_ref().unwrap(),
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            buffers: &[],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: self.cloud_temporal_shader.as_ref().unwrap(),
                            entry_point: Some("fs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: wgpu::TextureFormat::Rgba16Float,
                                blend: Some(wgpu::BlendState::REPLACE),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: None,
                        multisample: Default::default(),
                        multiview: None,
                        cache: None,
                    },
                ));

                self.world_pipeline = Some(device.create_render_pipeline(
                    &wgpu::RenderPipelineDescriptor {
                        label: Some("world pipeline"),
                        layout: Some(&world_layout),
                        vertex: wgpu::VertexState {
                            module: self.world_shader.as_ref().unwrap(),
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            buffers: &[wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<Vertex3>() as u64,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &wgpu::vertex_attr_array![
                                    0 => Float32x3,
                                    1 => Float32x3,
                                    2 => Float32x3,
                                    3 => Float32x4,
                                    4 => Float32
                                ],
                            }],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: self.world_shader.as_ref().unwrap(),
                            entry_point: Some("fs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: config.format,
                                blend: Some(wgpu::BlendState::REPLACE),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: wgpu::PrimitiveState {
                            cull_mode: Some(wgpu::Face::Back),
                            ..Default::default()
                        },
                        depth_stencil: Some(wgpu::DepthStencilState {
                            format: wgpu::TextureFormat::Depth24Plus,
                            depth_write_enabled: true,
                            depth_compare: wgpu::CompareFunction::Less,
                            stencil: Default::default(),
                            bias: Default::default(),
                        }),
                        multisample: Default::default(),
                        multiview: None,
                        cache: None,
                    },
                ));

                let create_line_pipeline = |label: &str, blend: wgpu::BlendState| {
                    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(label),
                        layout: Some(&line_layout),
                        vertex: wgpu::VertexState {
                            module: self.line_shader.as_ref().unwrap(),
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            buffers: &[wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<LineVertex>() as u64,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                            }],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: self.line_shader.as_ref().unwrap(),
                            entry_point: Some("fs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: config.format,
                                blend: Some(blend),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: wgpu::PrimitiveState {
                            topology: wgpu::PrimitiveTopology::LineList,
                            ..Default::default()
                        },
                        depth_stencil: Some(wgpu::DepthStencilState {
                            format: wgpu::TextureFormat::Depth24Plus,
                            depth_write_enabled: false,
                            depth_compare: wgpu::CompareFunction::LessEqual,
                            stencil: Default::default(),
                            bias: Default::default(),
                        }),
                        multisample: Default::default(),
                        multiview: None,
                        cache: None,
                    })
                };

                self.line_pipeline = Some(create_line_pipeline(
                    "line pipeline",
                    wgpu::BlendState::ALPHA_BLENDING,
                ));

                let invert_blend = wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::OneMinusDst,
                        dst_factor: wgpu::BlendFactor::Zero,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent::OVER,
                };
                self.line_invert_pipeline =
                    Some(create_line_pipeline("line invert pipeline", invert_blend));

                self.ui_pipeline = Some(device.create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("ui pipeline"),
                    layout: Some(&ui_layout),
                    vertex: wgpu::VertexState {
                        module: self.ui_shader.as_ref().unwrap(),
                        entry_point: Some("vs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<UiVertex>() as u64,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3],
                        }],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: self.ui_shader.as_ref().unwrap(),
                        entry_point: Some("fs_main"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: config.format,
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    multiview: None,
                    cache: None,
                },
            ));

                self.shadow_renderer = Some(shadow::ShadowRenderer::new(
                    device,
                    self.shadow_shader.as_ref().unwrap(),
                ));

                self.stage = GpuInitStage::Finalize;
                Ok(None)
            }

            GpuInitStage::Finalize => {
                let bootstrap = self.bootstrap.as_ref().expect("bootstrap missing");
                let device = &bootstrap.device;

                self.outline_stream = Some(DynamicVertexBuffer::new::<LineVertex>(
                    device,
                    "outline stream",
                    64,
                ));
                self.invert_outline_stream = Some(DynamicVertexBuffer::new::<LineVertex>(
                    device,
                    "invert outline stream",
                    64,
                ));
                self.ui_stream = Some(DynamicVertexBuffer::new::<UiVertex>(
                    device,
                    "ui stream",
                    1024,
                ));

                let world_surface_bind_group = Gpu::create_world_surface_bind_group(
                    device,
                    self.world_surface_layout.as_ref().unwrap(),
                    &self.shadow_renderer.as_ref().unwrap().uniform_buffer,
                    &self.shadow_renderer.as_ref().unwrap().depth_view,
                    &self.shadow_renderer.as_ref().unwrap().depth_sampler,
                    self.cloud_shadow_view.as_ref().unwrap(),
                    self.cloud_field_sampler.as_ref().unwrap(),
                );
                self.world_surface_bind_group = Some(world_surface_bind_group);

                // Do not recurse into self.step() here. Keep the staged init explicit and
                // let the outer bootstrap loop call step() again on the next tick.
                self.stage = GpuInitStage::Complete;
                Ok(None)
            }

            GpuInitStage::Complete => {
                let bootstrap = self.bootstrap.take().expect("bootstrap missing");
                let gpu_profiler = GpuProfiler::new(&bootstrap.device, &bootstrap.queue);

                let mut gpu = Gpu {
                    surface: bootstrap.surface,
                    device: bootstrap.device,
                    queue: bootstrap.queue,
                    config: bootstrap.config,
                    size: bootstrap.size,
                    adapter_summary: bootstrap.adapter_summary,

                    camera_buffer: self.camera_buffer.take().unwrap(),
                    camera_bind_group: self.camera_bind_group.take().unwrap(),

                    lighting_buffer: self.lighting_buffer.take().unwrap(),
                    lighting_bind_group: self.lighting_bind_group.take().unwrap(),
                    atmosphere: self.atmosphere_runtime.take().unwrap(),

                    shadow_renderer: self.shadow_renderer.take().unwrap(),
                    world_surface_bind_group: self.world_surface_bind_group.take().unwrap(),

                    cloud_texture_layout: self.cloud_texture_layout.take().unwrap(),
                    cloud_render_layout: self.cloud_render_layout.take().unwrap(),
                    cloud_sampler: self.cloud_sampler.take().unwrap(),
                    cloud_field_sampler: self.cloud_field_sampler.take().unwrap(),
                    weather_primary_tex: self.weather_primary_tex.take().unwrap(),
                    weather_primary_view: self.weather_primary_view.take().unwrap(),
                    weather_secondary_tex: self.weather_secondary_tex.take().unwrap(),
                    weather_secondary_view: self.weather_secondary_view.take().unwrap(),
                    cloud_shadow_tex: self.cloud_shadow_tex.take().unwrap(),
                    cloud_shadow_view: self.cloud_shadow_view.take().unwrap(),
                    shape_noise_view: self.shape_noise_view.take().unwrap(),
                    detail_noise_view: self.detail_noise_view.take().unwrap(),
                    blue_noise_view: self.blue_noise_view.take().unwrap(),
                    cloud_bind_group: self.cloud_bind_group.take().unwrap(),
                    cloud_render_bind_group: self.cloud_render_bind_group.take().unwrap(),

                    cloud_target_tex: self.cloud_target_tex.take().unwrap(),
                    cloud_target: self.cloud_target.take().unwrap(),
                    cloud_current_tex: self.cloud_current_tex.take().unwrap(),
                    cloud_current_view: self.cloud_current_view.take().unwrap(),
                    cloud_history_tex: self.cloud_history_tex.take().unwrap(),
                    cloud_history_view: self.cloud_history_view.take().unwrap(),

                    cloud_temporal_layout: self.cloud_temporal_layout.take().unwrap(),
                    cloud_temporal_bind_group: self.cloud_temporal_bind_group.take().unwrap(),
                    cloud_temporal_pipeline: self.cloud_temporal_pipeline.take().unwrap(),

                    cloud_pipeline: self.cloud_pipeline.take().unwrap(),
                    cloud_composite_pipeline: self.cloud_composite_pipeline.take().unwrap(),
                    sky_pipeline: self.sky_pipeline.take().unwrap(),
                    world_pipeline: self.world_pipeline.take().unwrap(),
                    line_pipeline: self.line_pipeline.take().unwrap(),
                    line_invert_pipeline: self.line_invert_pipeline.take().unwrap(),
                    ui_pipeline: self.ui_pipeline.take().unwrap(),

                    outline_stream: self.outline_stream.take().unwrap(),
                    invert_outline_stream: self.invert_outline_stream.take().unwrap(),
                    ui_stream: self.ui_stream.take().unwrap(),

                    depth: self.depth.take().unwrap(),

                    clear_color: bootstrap.clear_color,
                    current_lighting: bootstrap.default_lighting,
                    last_camera_pos: Vec3::ZERO,
                    last_inv_view_proj: Mat4::IDENTITY,
                    last_world_scale: 1.0,

                    // History is invalid until the first resolved cloud frame is produced.
                    cloud_history_valid: false,
                    cloud_history_frame_stats: CloudHistoryFrameStats::default(),
                    last_weather_upload_stats: WeatherUploadStats::default(),
                    last_render_stats: RenderFrameStats::default(),
                    gpu_profiler,
                };

                // If the window was resized during startup, textures created early
                // in the pipeline (like depth and clouds) will have the old size.
                // We force a resize here to ensure everything is perfectly in sync.
                let final_size = gpu.size;
                gpu.resize(final_size);

                Ok(Some(gpu))
            }
        }
    }
}
impl Gpu {
    pub fn begin_frame_stats(&mut self) {
        self.last_weather_upload_stats = WeatherUploadStats::default();
        self.cloud_history_frame_stats = CloudHistoryFrameStats::default();
    }

    pub fn last_render_stats(&self) -> RenderFrameStats {
        self.last_render_stats
    }

    pub fn last_weather_upload_stats(&self) -> WeatherUploadStats {
        self.last_weather_upload_stats
    }

    fn invalidate_cloud_history_with_reason(&mut self, reason: CloudHistoryInvalidationReason) {
        self.cloud_history_valid = false;
        self.cloud_history_frame_stats.invalidations = 1;
        match reason {
            CloudHistoryInvalidationReason::CameraMotion => {
                self.cloud_history_frame_stats.camera_invalidations = 1;
            }
            CloudHistoryInvalidationReason::HardCut => {
                self.cloud_history_frame_stats.hard_cut_invalidations = 1;
            }
            CloudHistoryInvalidationReason::WeatherUpload => {
                self.cloud_history_frame_stats.weather_invalidations = 1;
            }
            CloudHistoryInvalidationReason::External => {
                self.cloud_history_frame_stats.external_invalidations = 1;
            }
        }
    }

    pub fn adapter_summary(&self) -> &str {
        &self.adapter_summary
    }

    pub async fn new(window: Arc<Window>) -> Result<Self> {
        let bootstrap = GpuBootstrap::new(window).await?;
        let mut pending = bootstrap.into_pending_init();
        loop {
            if let Some(gpu) = pending.step()? {
                return Ok(gpu);
            }
        }
    }
    fn create_world_surface_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        shadow_uniform: &wgpu::Buffer,
        shadow_depth: &wgpu::TextureView,
        shadow_sampler: &wgpu::Sampler,
        cloud_shadow: &wgpu::TextureView,
        cloud_field_sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("world surface bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shadow_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(shadow_depth),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(cloud_shadow),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(cloud_field_sampler),
                },
            ],
        })
    }

    fn create_depth(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> wgpu::TextureView {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d {
                    width: config.width.max(1),
                    height: config.height.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth24Plus,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Create a cloud render texture.
    /// Base usages are always:
    ///   - RENDER_ATTACHMENT
    ///   - TEXTURE_BINDING
    /// `extra_usages` adds any additional flags needed by the specific role
    /// (for example COPY_SRC for cloud_current/cloud_target, COPY_DST for history).
    fn create_cloud_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        label: &'static str,
        extra_usages: wgpu::TextureUsages,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: (config.width / CLOUD_RENDER_SCALE).max(1),
                height: (config.height / CLOUD_RENDER_SCALE).max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            // COPY_SRC/COPY_DST are both required now:
            // - bootstrap current -> target on invalid history frames
            // - target -> history each frame
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | extra_usages,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    }

    fn create_cloud_target(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        // Target is both render destination and copy source/destination.
        Self::create_cloud_texture(device, config, "cloud target", wgpu::TextureUsages::empty())
    }

    fn create_cloud_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        target: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cloud bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(target),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    fn create_cloud_temporal_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        current: &wgpu::TextureView,
        history: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cloud temporal bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(current),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(history),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    fn create_cloud_render_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        weather_primary: &wgpu::TextureView,
        weather_secondary: &wgpu::TextureView,
        shape_noise: &wgpu::TextureView,
        detail_noise: &wgpu::TextureView,
        blue_noise: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        cloud_shadow: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cloud render bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(weather_primary),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(weather_secondary),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(shape_noise),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(detail_noise),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(blue_noise),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(cloud_shadow),
                },
            ],
        })
    }

    fn hash_u32(mut x: u32) -> u32 {
        x ^= x >> 16;
        x = x.wrapping_mul(0x7feb352d);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846ca68b);
        x ^= x >> 16;
        x
    }

    fn hash01(x: u32) -> f32 {
        Self::hash_u32(x) as f32 / u32::MAX as f32
    }

    fn smoothstep01(x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    }

    fn value_noise_2d(x: f32, y: f32, seed: u32) -> f32 {
        fn lattice(ix: i32, iy: i32, seed: u32) -> f32 {
            let v = (ix as u32).wrapping_mul(374_761_393)
                ^ (iy as u32).wrapping_mul(668_265_263)
                ^ seed.wrapping_mul(2_246_822_519);
            Gpu::hash_u32(v) as f32 / u32::MAX as f32
        }

        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let fx = x - x.floor();
        let fy = y - y.floor();
        let sx = Self::smoothstep01(fx);
        let sy = Self::smoothstep01(fy);

        let a = lattice(x0, y0, seed);
        let b = lattice(x0 + 1, y0, seed);
        let c = lattice(x0, y0 + 1, seed);
        let d = lattice(x0 + 1, y0 + 1, seed);

        let x1 = a + (b - a) * sx;
        let x2 = c + (d - c) * sx;
        x1 + (x2 - x1) * sy
    }

    fn fbm_noise_2d(x: f32, y: f32, seed: u32, octaves: usize) -> f32 {
        let mut sum = 0.0f32;
        let mut amp = 0.5f32;
        let mut freq = 1.0f32;
        let mut norm = 0.0f32;
        for octave in 0..octaves.max(1) {
            sum += Self::value_noise_2d(x * freq, y * freq, seed.wrapping_add(octave as u32 * 977))
                * amp;
            norm += amp;
            freq *= 2.03;
            amp *= 0.5;
        }
        if norm > 0.0 { sum / norm } else { 0.0 }
    }

    /// Upload pre-computed shape noise data (128³, RGBA8) to a GPU texture.
    fn create_shape_noise_3d(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: Vec<u8>,
    ) -> wgpu::TextureView {
        let size = 64u32;

        // Compute mip count for 64³
        let mip_count = (64f32).log2() as u32 + 1; // 7 mips for 64

        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shape noise 3D"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: size,
            },
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Write mip 0
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: size,
            },
        );

        // Generate and upload mip levels via box filter
        let mut prev = data;
        let mut s = size;
        for mip in 1..mip_count {
            let ns = (s / 2).max(1);
            let nvoxels = (ns * ns * ns) as usize;
            let mut mip_data = vec![0u8; nvoxels * 4];
            for mz in 0..ns {
                for my in 0..ns {
                    for mx in 0..ns {
                        let di = ((mz * ns * ns + my * ns + mx) * 4) as usize;
                        for ch in 0..4usize {
                            let mut sum = 0u32;
                            for dz in 0..2u32 {
                                for dy in 0..2u32 {
                                    for dx in 0..2u32 {
                                        let sx2 = mx * 2 + dx;
                                        let sy2 = my * 2 + dy;
                                        let sz2 = mz * 2 + dz;
                                        let si = ((sz2 * s * s + sy2 * s + sx2) * 4) as usize;
                                        sum += prev[si + ch] as u32;
                                    }
                                }
                            }
                            mip_data[di + ch] = (sum / 8) as u8;
                        }
                    }
                }
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: mip,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &mip_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * ns),
                    rows_per_image: Some(ns),
                },
                wgpu::Extent3d {
                    width: ns,
                    height: ns,
                    depth_or_array_layers: ns,
                },
            );
            prev = mip_data;
            s = ns;
        }

        tex.create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Upload pre-computed detail noise data (48³, RGBA8) to a GPU texture.
    fn create_detail_noise_3d(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: Vec<u8>,
    ) -> wgpu::TextureView {
        let size = 32u32;

        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("detail noise 3D"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: size,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: size,
            },
        );
        tex.create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn create_blue_noise_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
        // Generate a 64x64 R8 blue noise texture using a simple void-and-cluster approximation.
        // This is a good-enough approximation; a proper blue noise LUT would be baked offline.
        let size = 64u32;
        let total = (size * size) as usize;
        let mut data = vec![0u8; total];

        // Use interleaved gradient noise as a fast blue-noise-like pattern
        for y in 0..size {
            for x in 0..size {
                let fx = x as f32 + 0.5;
                let fy = y as f32 + 0.5;
                // Interleaved gradient noise (Jorge Jimenez, 2014)
                let v = (52.9829189 * ((fx * 0.06711056 + fy * 0.00583715).fract())).fract();
                data[(y * size + x) as usize] = (v * 255.0) as u8;
            }
        }

        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("blue noise"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );
        tex.create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// True 2D gradient (Perlin) noise with quintic interpolation.
    /// Returns a value in [0, 1] (remapped from the raw [-~0.7, ~0.7] range).
    fn perlin_2d(x: f32, y: f32, seed: u32) -> f32 {
        // 8 unit-circle gradient directions
        const GRADS: [[f32; 2]; 8] = [
            [1.0, 0.0],
            [-1.0, 0.0],
            [0.0, 1.0],
            [0.0, -1.0],
            [0.707, 0.707],
            [-0.707, 0.707],
            [0.707, -0.707],
            [-0.707, -0.707],
        ];
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let xf = x - x.floor();
        let yf = y - y.floor();
        // Quintic smoothstep for C2 continuity
        let sx = xf * xf * xf * (xf * (xf * 6.0 - 15.0) + 10.0);
        let sy = yf * yf * yf * (yf * (yf * 6.0 - 15.0) + 10.0);

        let corner_hash = |cx: i32, cy: i32| -> usize {
            let mut h = (cx as u32).wrapping_mul(374_761_393)
                ^ (cy as u32).wrapping_mul(668_265_263)
                ^ seed;
            h ^= h >> 13;
            h = h.wrapping_mul(0x5bd1e995);
            h ^= h >> 15;
            (h % 8) as usize
        };
        let dot = |cx: i32, cy: i32, dx: f32, dy: f32| -> f32 {
            let g = GRADS[corner_hash(cx, cy)];
            g[0] * dx + g[1] * dy
        };
        let n00 = dot(x0, y0, xf, yf);
        let n10 = dot(x0 + 1, y0, xf - 1.0, yf);
        let n01 = dot(x0, y0 + 1, xf, yf - 1.0);
        let n11 = dot(x0 + 1, y0 + 1, xf - 1.0, yf - 1.0);
        let ix0 = n00 + (n10 - n00) * sx;
        let ix1 = n01 + (n11 - n01) * sx;
        let v = ix0 + (ix1 - ix0) * sy;
        // Perlin range ~[-0.7, 0.7] → remap to [0, 1]
        (v / 0.7 * 0.5 + 0.5).clamp(0.0, 1.0)
    }

    /// FBM using perlin_2d with INCOMMENSURABLE frequency ratios.
    /// Exact powers-of-2 cause constructive interference that reinstates the
    /// fundamental tiling period — incommensurable ratios prevent this.
    fn fbm_perlin_2d(x: f32, y: f32, seed: u32, octaves: usize) -> f32 {
        const FREQS: [f32; 6] = [1.0, 2.13, 4.31, 8.73, 17.11, 34.37];
        const AMPS: [f32; 6] = [0.5, 0.25, 0.125, 0.0625, 0.03125, 0.015625];
        let n = octaves.min(6);
        let norm: f32 = AMPS[..n].iter().sum();
        let mut sum = 0.0f32;
        for i in 0..n {
            let s = seed ^ (i as u32).wrapping_mul(0x9e3779b9);
            sum += Self::perlin_2d(x * FREQS[i], y * FREQS[i], s) * AMPS[i];
        }
        (sum / norm).clamp(0.0, 1.0)
    }

    /// Create a weather clipmap texture array.
    fn create_weather_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        data: &[u8],
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let size = WEATHER_TEXTURE_RESOLUTION;

        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: WEATHER_CLIPMAP_LEVEL_COUNT as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(16 * size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: WEATHER_CLIPMAP_LEVEL_COUNT as u32,
            },
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        (tex, view)
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        self.atmosphere
            .resize(&self.queue, self.config.width, self.config.height);
        self.depth = Self::create_depth(&self.device, &self.config);
        self.cloud_render_bind_group = Self::create_cloud_render_bind_group(
            &self.device,
            &self.cloud_render_layout,
            &self.cloud_field_sampler,
            &self.weather_primary_view,
            &self.weather_secondary_view,
            &self.shape_noise_view,
            &self.detail_noise_view,
            &self.blue_noise_view,
            &self.depth,
            &self.cloud_shadow_view,
        );
        (self.cloud_target_tex, self.cloud_target) =
            Self::create_cloud_target(&self.device, &self.config);
        (self.cloud_current_tex, self.cloud_current_view) = Self::create_cloud_texture(
            &self.device,
            &self.config,
            "cloud current",
            wgpu::TextureUsages::empty(),
        );
        (self.cloud_history_tex, self.cloud_history_view) = Self::create_cloud_texture(
            &self.device,
            &self.config,
            "cloud history",
            wgpu::TextureUsages::empty(),
        );
        self.cloud_bind_group = Self::create_cloud_bind_group(
            &self.device,
            &self.cloud_texture_layout,
            &self.cloud_sampler,
            &self.cloud_target,
        );
        self.cloud_temporal_bind_group = Self::create_cloud_temporal_bind_group(
            &self.device,
            &self.cloud_temporal_layout,
            &self.cloud_sampler,
            &self.cloud_current_view,
            &self.cloud_history_view,
        );

        self.cloud_history_valid = false;
    }

    pub fn upload_camera(&mut self, view_proj: Mat4, eye: Vec3, world_scale: f32) {
        let inv_view_proj = view_proj.inverse();

        let cur_center_world = unproject_clip(inv_view_proj, Vec3::new(0.0, 0.0, 1.0));
        let prev_center_world = unproject_clip(self.last_inv_view_proj, Vec3::new(0.0, 0.0, 1.0));

        let cur_forward = (cur_center_world - eye).normalize_or_zero();
        let prev_forward = (prev_center_world - self.last_camera_pos).normalize_or_zero();

        let angular_motion = (1.0 - cur_forward.dot(prev_forward).clamp(-1.0, 1.0)).clamp(0.0, 1.0);
        let translation_delta = eye - self.last_camera_pos;
        let translation_motion = (translation_delta.length() / 8.0).clamp(0.0, 1.0);
        let camera_motion = (angular_motion * 6.0 + translation_motion * 0.25).clamp(0.0, 1.0);

        if CLOUD_TEMPORAL_ENABLED && camera_motion > 0.28 {
            self.invalidate_cloud_history_with_reason(CloudHistoryInvalidationReason::CameraMotion);
        }

        let render_settings = render_distance_settings();
        let visible_world =
            render_settings.view_distance_chunks as f32 * CHUNK_SIZE as f32 * world_scale;

        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&CameraUniform {
                view_proj: view_proj.to_cols_array_2d(),
                camera_pos: [eye.x, eye.y, eye.z, 0.0],
                inv_view_proj: inv_view_proj.to_cols_array_2d(),
                world_params: [
                    world_scale,
                    world_scale.recip(),
                    camera_motion,
                    visible_world,
                ],
                prev_view_proj: self.last_inv_view_proj.inverse().to_cols_array_2d(),
                camera_forward: [cur_forward.x, cur_forward.y, cur_forward.z, 0.0],
            }),
        );

        let hard_cut = translation_delta.length() > 24.0 * world_scale.max(1.0)
            || angular_motion > 0.18
            || (world_scale - self.last_world_scale).abs() > 1e-4;

        if CLOUD_TEMPORAL_ENABLED && hard_cut {
            self.invalidate_cloud_history_with_reason(CloudHistoryInvalidationReason::HardCut);
        }

        self.last_camera_pos = eye;
        self.last_inv_view_proj = inv_view_proj;
        self.last_world_scale = world_scale;

        let render_settings = render_distance_settings();
        let far_plane_voxels =
            (render_settings.view_distance_chunks as f32 * CHUNK_SIZE as f32 * 1.20).max(640.0);
        let far_plane_world = far_plane_voxels * world_scale;

        self.shadow_renderer.update(
            self.current_lighting.sun_dir,
            inv_view_proj,
            0.01,
            far_plane_world,
            world_scale,
        );
        if self.atmosphere.needs_aerial_perspective() {
            self.atmosphere.update_aerial_perspective(
                &self.device,
                &self.queue,
                &self.camera_bind_group,
            );
        }
    }

    pub fn upload_lighting(
        &mut self,
        lighting: &LightingState,
        cloud_controls: &CloudControls,
        sky_time: f32,
        held_torch: Option<TorchLight>,
        torch_lights: &[TorchLight],
        weather_field: Option<&CloudWeatherField>,
        debug_view: RenderDebugView,
        atmosphere_mode: AtmosphereMode,
    ) {
        let uniform = lighting_uniform_from_state(
            lighting,
            cloud_controls,
            sky_time,
            held_torch,
            torch_lights,
            cloud_weather_uniform_state(weather_field),
            debug_view,
        );
        self.queue
            .write_buffer(&self.lighting_buffer, 0, bytemuck::bytes_of(&uniform));
        self.atmosphere.set_state(
            &self.queue,
            AtmosphereRuntimeState {
                mode: atmosphere_mode,
                debug_view,
            },
        );
        self.atmosphere.sync_from_lighting(
            &self.device,
            &self.queue,
            lighting,
            cloud_controls,
            &self.camera_bind_group,
        );
        self.current_lighting = *lighting;

        let render_settings = render_distance_settings();
        let far_plane_voxels =
            (render_settings.view_distance_chunks as f32 * CHUNK_SIZE as f32 * 1.20).max(640.0);
        let far_plane_world = far_plane_voxels * self.last_world_scale;

        self.shadow_renderer.update(
            self.current_lighting.sun_dir,
            self.last_inv_view_proj,
            0.01,
            far_plane_world,
            self.last_world_scale,
        );
        self.clear_color = [
            lighting.sky_horizon[0] * 0.72 + lighting.sky_top[0] * 0.28,
            lighting.sky_horizon[1] * 0.72 + lighting.sky_top[1] * 0.28,
            lighting.sky_horizon[2] * 0.72 + lighting.sky_top[2] * 0.28,
        ];
    }

    pub fn invalidate_cloud_history(&mut self) {
        self.invalidate_cloud_history_with_reason(CloudHistoryInvalidationReason::External);
    }

    pub fn upload_cloud_weather(
        &mut self,
        weather_field: &CloudWeatherField,
        upload_manifest: &CloudWeatherUploadManifest,
    ) {
        if upload_manifest.is_empty() {
            return;
        }

        let upload_start = Instant::now();
        let mut stats = WeatherUploadStats::default();
        stats.clipmap_batches = upload_manifest.clipmap_uploads().len();
        let mut should_invalidate_history = false;
        let mut primary_upload_bytes = Vec::new();
        let mut secondary_upload_bytes = Vec::new();
        let mut shadow_upload_bytes = Vec::new();
        for upload_batch in upload_manifest.clipmap_uploads() {
            for &dirty_rect in upload_batch.dirty_rects() {
                stats.dirty_rects += 1;
                let origin = wgpu::Origin3d {
                    x: dirty_rect.texel_x,
                    y: dirty_rect.texel_y,
                    z: upload_batch.array_layer(),
                };
                let layout = wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(dirty_rect.packed_bytes_per_row()),
                    rows_per_image: Some(dirty_rect.rows_per_image()),
                };
                let extent = wgpu::Extent3d {
                    width: dirty_rect.texel_width,
                    height: dirty_rect.texel_height,
                    depth_or_array_layers: 1,
                };
                let is_full_level_refresh = dirty_rect.texel_x == 0
                    && dirty_rect.texel_y == 0
                    && dirty_rect.texel_width == WEATHER_TEXTURE_RESOLUTION
                    && dirty_rect.texel_height == WEATHER_TEXTURE_RESOLUTION;
                should_invalidate_history |= is_full_level_refresh;

                weather_field.pack_primary_upload_bytes(
                    upload_batch.clipmap_level,
                    dirty_rect,
                    &mut primary_upload_bytes,
                );
                stats.uploaded_bytes += primary_upload_bytes.len();
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.weather_primary_tex,
                        mip_level: 0,
                        origin,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &primary_upload_bytes,
                    layout,
                    extent,
                );

                weather_field.pack_secondary_upload_bytes(
                    upload_batch.clipmap_level,
                    dirty_rect,
                    &mut secondary_upload_bytes,
                );
                stats.uploaded_bytes += secondary_upload_bytes.len();
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.weather_secondary_tex,
                        mip_level: 0,
                        origin,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &secondary_upload_bytes,
                    layout,
                    extent,
                );

                weather_field.pack_shadow_upload_bytes(
                    upload_batch.clipmap_level,
                    dirty_rect,
                    &mut shadow_upload_bytes,
                );
                stats.uploaded_bytes += shadow_upload_bytes.len();
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.cloud_shadow_tex,
                        mip_level: 0,
                        origin,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &shadow_upload_bytes,
                    layout,
                    extent,
                );
            }
        }

        if should_invalidate_history {
            self.invalidate_cloud_history_with_reason(
                CloudHistoryInvalidationReason::WeatherUpload,
            );
        }
        stats.cpu_upload_ms = upload_start.elapsed().as_secs_f32() * 1000.0;
        self.last_weather_upload_stats = stats;
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.config.width as f32 / self.config.height.max(1) as f32
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }
    pub fn render_loading_frame<F>(
        &mut self,
        clear_color: [f32; 3],
        overlay: F,
    ) -> Result<(), wgpu::SurfaceError>
    where
        F: FnOnce(
            &wgpu::Device,
            &wgpu::Queue,
            &mut wgpu::CommandEncoder,
            &wgpu::TextureView,
            u32,
            u32,
        ),
    {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("loading encoder"),
            });

        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("loading clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear_color[0] as f64,
                            g: clear_color[1] as f64,
                            b: clear_color[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        overlay(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            self.config.width,
            self.config.height,
        );

        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
    pub fn render_cached(
        &mut self,
        chunk_cache: &ChunkCache,
        draw_list: &[ChunkKey],
        outline_vertices: &[LineVertex],
        invert_outline_vertices: &[LineVertex],
        ui_vertices: &[UiVertex],
        paused: bool,
    ) -> Result<(), wgpu::SurfaceError> {
        self.render_cached_with_overlay(
            chunk_cache,
            draw_list,
            outline_vertices,
            invert_outline_vertices,
            ui_vertices,
            paused,
            |_, _, _, _, _, _| {},
        )
    }

    pub fn render_cached_with_overlay<F>(
        &mut self,
        chunk_cache: &ChunkCache,
        draw_list: &[ChunkKey],
        outline_vertices: &[LineVertex],
        invert_outline_vertices: &[LineVertex],
        ui_vertices: &[UiVertex],
        paused: bool,
        overlay: F,
    ) -> Result<(), wgpu::SurfaceError>
    where
        F: FnOnce(
            &wgpu::Device,
            &wgpu::Queue,
            &mut wgpu::CommandEncoder,
            &wgpu::TextureView,
            u32,
            u32,
        ),
    {
        if let Some(profiler) = self.gpu_profiler.as_mut() {
            profiler.collect_finished(&self.device);
        }

        let acquire_start = Instant::now();
        let frame = self.surface.get_current_texture()?;
        let mut frame_stats = RenderFrameStats::default();
        if let Some(profiler) = self.gpu_profiler.as_ref() {
            let gpu_timings = profiler.last_timings();
            frame_stats.gpu_pass_total_ms = gpu_timings.total_ms;
            frame_stats.gpu_shadow_ms = gpu_timings.shadow_ms;
            frame_stats.gpu_terrain_ms = gpu_timings.terrain_ms;
            frame_stats.gpu_cloud_raymarch_ms = gpu_timings.cloud_raymarch_ms;
            frame_stats.gpu_cloud_temporal_ms = gpu_timings.cloud_temporal_ms;
            frame_stats.gpu_sky_ms = gpu_timings.sky_ms;
            frame_stats.gpu_cloud_composite_ms = gpu_timings.cloud_composite_ms;
            frame_stats.gpu_outline_ms = gpu_timings.outline_ms;
            frame_stats.gpu_ui_ms = gpu_timings.ui_ms;
            frame_stats.gpu_timestamps_supported = true;
        }
        frame_stats.surface_acquire_ms = acquire_start.elapsed().as_secs_f32() * 1000.0;

        let surface_size = frame.texture.size();
        let frame = if surface_size.width != self.config.width
            || surface_size.height != self.config.height
        {
            drop(frame);
            self.resize(PhysicalSize::new(surface_size.width, surface_size.height));
            let reacquire_start = Instant::now();
            let reacquired = self.surface.get_current_texture()?;
            frame_stats.surface_acquire_ms += reacquire_start.elapsed().as_secs_f32() * 1000.0;
            reacquired
        } else {
            frame
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let upload_start = Instant::now();
        let has_outline = self
            .outline_stream
            .upload(&self.device, &self.queue, outline_vertices);
        let has_invert_outline =
            self.invert_outline_stream
                .upload(&self.device, &self.queue, invert_outline_vertices);
        let has_ui = self
            .ui_stream
            .upload(&self.device, &self.queue, ui_vertices);
        frame_stats.stream_upload_ms = upload_start.elapsed().as_secs_f32() * 1000.0;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });
        let gpu_timestamp_slot = self
            .gpu_profiler
            .as_mut()
            .and_then(GpuProfiler::acquire_slot);
        let gpu_query_set = gpu_timestamp_slot.map(|slot_index| {
            self.gpu_profiler
                .as_ref()
                .unwrap()
                .query_set(slot_index)
                .clone()
        });
        let mut gpu_query_count = 0u32;
        let mut gpu_pass_queries = [None; GPU_TIMESTAMP_PASS_COUNT as usize];

        // Static terrain does not need a full shadow redraw every frame. Re-render
        // only when the snapped light camera or the uploaded mesh set changes.
        let shadow_start = Instant::now();
        let redraw_shadow_map = !paused
            && self
                .shadow_renderer
                .apply_update_if_needed(&self.queue, chunk_cache.gpu_mesh_revision);

        if redraw_shadow_map {
            self.shadow_renderer.build_cascade_draw_lists(chunk_cache);
            let shadow_timestamp_start = gpu_query_count;
            if gpu_query_set.is_some() {
                for pass_index in 0..GPU_TIMESTAMP_SHADOW_PASS_COUNT {
                    let begin = gpu_query_count;
                    let end = begin + 1;
                    gpu_query_count += 2;
                    gpu_pass_queries[pass_index as usize] = Some((begin, end));
                }
            }
            self.shadow_renderer.render(
                &mut encoder,
                chunk_cache,
                gpu_query_set.as_ref(),
                shadow_timestamp_start,
            );
        }
        frame_stats.shadow_redraw = redraw_shadow_map;
        frame_stats.shadow_draw_calls = self.shadow_renderer.draw_call_count();
        frame_stats.shadow_encode_ms = shadow_start.elapsed().as_secs_f32() * 1000.0;

        let terrain_start = Instant::now();
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if paused {
                            wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.12,
                                g: 0.12,
                                b: 0.16,
                                a: 1.0,
                            })
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: allocate_pass_timestamp_writes(
                    gpu_query_set.as_ref(),
                    &mut gpu_query_count,
                    &mut gpu_pass_queries,
                    GPU_TIMESTAMP_PASS_TERRAIN,
                ),
                occlusion_query_set: None,
            });

            if !paused {
                let mut terrain_draw_calls = 0usize;
                let mut terrain_indices = 0usize;
                pass.set_pipeline(&self.world_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_bind_group(1, &self.lighting_bind_group, &[]);
                pass.set_bind_group(2, self.atmosphere.bind_group(), &[]);
                pass.set_bind_group(3, &self.world_surface_bind_group, &[]);
                for key in draw_list {
                    if let Some(mesh) = chunk_cache.gpu_meshes.get(&key.coord) {
                        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                        pass.set_index_buffer(
                            mesh.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                        terrain_draw_calls += 1;
                        terrain_indices += mesh.index_count as usize;
                    }
                }
                frame_stats.terrain_draw_calls = terrain_draw_calls;
                frame_stats.terrain_indices = terrain_indices;
            }
        }
        frame_stats.terrain_encode_ms = terrain_start.elapsed().as_secs_f32() * 1000.0;

        let cloud_start = Instant::now();
        if !paused {
            let cloud_extent = wgpu::Extent3d {
                width: (self.config.width / CLOUD_RENDER_SCALE).max(1),
                height: (self.config.height / CLOUD_RENDER_SCALE).max(1),
                depth_or_array_layers: 1,
            };

            {
                let mut cloud_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("cloud raymarch pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.cloud_current_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: allocate_pass_timestamp_writes(
                        gpu_query_set.as_ref(),
                        &mut gpu_query_count,
                        &mut gpu_pass_queries,
                        GPU_TIMESTAMP_PASS_CLOUD_RAYMARCH,
                    ),
                    occlusion_query_set: None,
                });
                cloud_pass.set_pipeline(&self.cloud_pipeline);
                cloud_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                cloud_pass.set_bind_group(1, &self.lighting_bind_group, &[]);
                cloud_pass.set_bind_group(2, self.atmosphere.bind_group(), &[]);
                cloud_pass.set_bind_group(3, &self.cloud_render_bind_group, &[]);
                cloud_pass.draw(0..3, 0..1);
            }
            frame_stats.cloud_raymarch_passes = 1;

            if CLOUD_TEMPORAL_ENABLED && self.cloud_history_valid {
                let mut temporal_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("cloud temporal pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.cloud_target,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: allocate_pass_timestamp_writes(
                        gpu_query_set.as_ref(),
                        &mut gpu_query_count,
                        &mut gpu_pass_queries,
                        GPU_TIMESTAMP_PASS_CLOUD_TEMPORAL,
                    ),
                    occlusion_query_set: None,
                });
                temporal_pass.set_pipeline(&self.cloud_temporal_pipeline);
                temporal_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                temporal_pass.set_bind_group(1, &self.lighting_bind_group, &[]);
                temporal_pass.set_bind_group(2, &self.cloud_temporal_bind_group, &[]);
                temporal_pass.draw(0..3, 0..1);
                frame_stats.cloud_temporal_passes = 1;
            } else {
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.cloud_current_tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.cloud_target_tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    cloud_extent,
                );
            }

            if CLOUD_TEMPORAL_ENABLED {
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.cloud_target_tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.cloud_history_tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    cloud_extent,
                );
                self.cloud_history_valid = true;
            } else {
                self.cloud_history_valid = false;
            }

            {
                let mut sky_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("sky pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: allocate_pass_timestamp_writes(
                        gpu_query_set.as_ref(),
                        &mut gpu_query_count,
                        &mut gpu_pass_queries,
                        GPU_TIMESTAMP_PASS_SKY,
                    ),
                    occlusion_query_set: None,
                });
                sky_pass.set_pipeline(&self.sky_pipeline);
                sky_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                sky_pass.set_bind_group(1, &self.lighting_bind_group, &[]);
                sky_pass.set_bind_group(2, self.atmosphere.bind_group(), &[]);
                sky_pass.draw(0..3, 0..1);
            }

            {
                let mut composite_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("cloud composite pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: allocate_pass_timestamp_writes(
                        gpu_query_set.as_ref(),
                        &mut gpu_query_count,
                        &mut gpu_pass_queries,
                        GPU_TIMESTAMP_PASS_CLOUD_COMPOSITE,
                    ),
                    occlusion_query_set: None,
                });
                composite_pass.set_pipeline(&self.cloud_composite_pipeline);
                composite_pass.set_bind_group(0, &self.lighting_bind_group, &[]);
                composite_pass.set_bind_group(1, &self.cloud_bind_group, &[]);
                composite_pass.set_bind_group(2, self.atmosphere.bind_group(), &[]);
                composite_pass.set_bind_group(3, &self.cloud_render_bind_group, &[]);
                composite_pass.draw(0..3, 0..1);
            }

            if has_outline || has_invert_outline {
                let mut outline_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("outline pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: allocate_pass_timestamp_writes(
                        gpu_query_set.as_ref(),
                        &mut gpu_query_count,
                        &mut gpu_pass_queries,
                        GPU_TIMESTAMP_PASS_OUTLINE,
                    ),
                    occlusion_query_set: None,
                });

                if has_outline {
                    outline_pass.set_pipeline(&self.line_pipeline);
                    outline_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    outline_pass.set_vertex_buffer(
                        0,
                        self.outline_stream
                            .slice::<LineVertex>(outline_vertices.len()),
                    );
                    outline_pass.draw(0..outline_vertices.len() as u32, 0..1);
                }

                if has_invert_outline {
                    outline_pass.set_pipeline(&self.line_invert_pipeline);
                    outline_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    outline_pass.set_vertex_buffer(
                        0,
                        self.invert_outline_stream
                            .slice::<LineVertex>(invert_outline_vertices.len()),
                    );
                    outline_pass.draw(0..invert_outline_vertices.len() as u32, 0..1);
                }
            }
        }
        frame_stats.cloud_encode_ms = cloud_start.elapsed().as_secs_f32() * 1000.0;

        let ui_start = Instant::now();
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ui pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: allocate_pass_timestamp_writes(
                    gpu_query_set.as_ref(),
                    &mut gpu_query_count,
                    &mut gpu_pass_queries,
                    GPU_TIMESTAMP_PASS_UI,
                ),
                occlusion_query_set: None,
            });
            if has_ui {
                pass.set_pipeline(&self.ui_pipeline);
                pass.set_vertex_buffer(0, self.ui_stream.slice::<UiVertex>(ui_vertices.len()));
                pass.draw(0..ui_vertices.len() as u32, 0..1);
            }
        }
        frame_stats.outline_vertices = outline_vertices.len();
        frame_stats.invert_outline_vertices = invert_outline_vertices.len();
        frame_stats.ui_vertices = ui_vertices.len();
        frame_stats.ui_encode_ms = ui_start.elapsed().as_secs_f32() * 1000.0;

        let overlay_start = Instant::now();
        overlay(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            self.config.width,
            self.config.height,
        );
        frame_stats.overlay_encode_ms = overlay_start.elapsed().as_secs_f32() * 1000.0;

        if let Some(slot_index) = gpu_timestamp_slot {
            if gpu_query_count != 0 {
                self.gpu_profiler
                    .as_ref()
                    .expect("profiler slot allocated without profiler")
                    .resolve_results(&mut encoder, slot_index, gpu_query_count);
            }
        }

        let submit_start = Instant::now();
        self.queue.submit(Some(encoder.finish()));
        if let Some(slot_index) = gpu_timestamp_slot {
            if let Some(profiler) = self.gpu_profiler.as_mut() {
                profiler.mark_submitted(slot_index, gpu_query_count, gpu_pass_queries);
            }
        }
        frame_stats.submit_ms = submit_start.elapsed().as_secs_f32() * 1000.0;

        let present_start = Instant::now();
        frame.present();
        frame_stats.present_ms = present_start.elapsed().as_secs_f32() * 1000.0;
        frame_stats.cloud_history_invalidations = self.cloud_history_frame_stats.invalidations;
        frame_stats.cloud_history_camera_invalidations =
            self.cloud_history_frame_stats.camera_invalidations;
        frame_stats.cloud_history_hard_cut_invalidations =
            self.cloud_history_frame_stats.hard_cut_invalidations;
        frame_stats.cloud_history_weather_invalidations =
            self.cloud_history_frame_stats.weather_invalidations;
        frame_stats.cloud_history_external_invalidations =
            self.cloud_history_frame_stats.external_invalidations;
        self.last_render_stats = frame_stats;
        Ok(())
    }
}
