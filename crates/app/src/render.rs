//! Rendering and atmospheric shading.
//! Chunk residency and terrain work are owned by render/streaming.rs.

mod cloud_noise;
use cloud_noise::{gen_shape_noise_data, gen_detail_noise_data};

mod capture;
pub mod portals;
mod frame_pacer;
mod upload_staging;
mod pipeline_cache;
pub mod coverage;
pub mod globe;
pub mod globe_driver;
pub(crate) fn flush_review_captures() {
    if !capture::flush(std::time::Duration::from_secs(2)) {
        eprintln!("[capture] end-of-review flush timed out");
    }
}
pub(crate) fn acceptance_capture_result(path: &std::path::Path) -> Option<bool> {
    capture::watched_result(path)
}
mod horizon;
mod planet;
mod planet_frame;
mod origin;
mod streaming;
pub use streaming::{ChunkCache, ChunkKey, ChunkStreamer, DrawScratch, build_draw_list_with_coverage};
#[cfg(feature = "multiplayer")]
pub use streaming::PortalWarmup;
pub use streaming::{
    MAX_GPU_MIB, gpu_residency_limit_mib, gpu_residency_status, mesh_budget_mib,
    set_mesh_budget_mib, set_world_residency_budgets_mib, world_residency_budgets_mib,
};
pub(crate) use streaming::{chunk_bounds_sphere, extract_planes};
pub(crate) use streaming::StreamInterestChange;
pub(crate) use streaming::occlusion;
pub use streaming::indirect_draws::set_enabled as set_multidraw_enabled;

mod atmosphere;
mod planet_air;
mod torch_occlusion;
pub mod sky_volume;
mod sky_volume_gpu;
mod sky_probe;
mod posed_assets;
pub mod atmosphere_config;
pub mod atmosphere_params;
pub mod cloud_governor;
mod debug;
#[cfg(test)]
mod material_finish_contract;
#[cfg(test)]
mod directional_facet_contract;
#[cfg(test)]
mod directional_facet_diffuse_contract;
#[cfg(test)]
mod axis_unroll_contract;
mod indirect;
mod postprocess;
mod viewmodel_texture;
pub use indirect::{
    enabled as indirect_lighting_enabled, set_enabled as set_indirect_lighting_enabled,
};
pub mod shadow;

use anyhow::{Context, Result};
use glam::{IVec3, Mat4, Vec3, Vec4};
use origin::{ABSOLUTE_SLOT, DrawOffsets, DrawTransform, RenderOrigin};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

const LOD_TRANSITION_TIME: Duration = Duration::from_millis(320);

fn lod_transition_phase(started: Instant, now: Instant) -> Option<u32> {
    let elapsed = now.saturating_duration_since(started);
    (elapsed < LOD_TRANSITION_TIME).then(|| {
        (elapsed.as_nanos() * 64 / LOD_TRANSITION_TIME.as_nanos()) as u32
    })
}
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

use crate::mesh::{LineVertex, UiVertex, Vertex3};
use crate::world::{
    CHUNK_SIZE, CloudControls, CloudWeatherField, CloudWeatherUploadManifest, LightingState,
    WEATHER_CLIPMAP_LEVEL_COUNT, WEATHER_TEXTURE_RESOLUTION,
};

use atmosphere::{
    AtmosphereRuntime, AtmosphereRuntimeState, cloud_composite_shader_source,
    cloud_shader_source_for_filtering, cloud_temporal_shader_source, sky_shader_source_for_filtering,
    world_shader_source_for_filtering,
};
pub use atmosphere_params::AtmosphereMode;
pub use debug::RenderDebugView;

const DEFAULT_VIEW_DISTANCE_CHUNKS: i32 = 12;
/// Preserve full detail through ring 5 and 2 m detail through ring 8.
/// The additional four rings use 4 m cells, beyond the former voxel boundary.
const DEFAULT_LOD_START_CHUNKS: i32 = 5;
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

impl RenderDistanceSettings {
    pub fn rescale_blocks(self, old_meters: f32, new_meters: f32) -> Self {
        let ratio = old_meters / new_meters.max(0.01);
        sanitize_render_distance_settings(Self {
            view_distance_chunks: (self.view_distance_chunks as f32 * ratio).round() as i32,
            lod_start_chunks: (self.lod_start_chunks as f32 * ratio).round() as i32,
            lod_levels: self.lod_levels,
        })
    }
}

fn sanitize_render_distance_settings(
    mut settings: RenderDistanceSettings,
) -> RenderDistanceSettings {
    settings.view_distance_chunks = settings.view_distance_chunks.clamp(4, 64);
    settings.lod_start_chunks = settings
        .lod_start_chunks
        .clamp(2, settings.view_distance_chunks - 1);
    settings.lod_levels = settings.lod_levels.clamp(1, 4);
    settings
}

/// Metres of distant terrain built beyond the streamed chunks.
pub fn horizon_range_m() -> u32 { horizon::horizon_range_m() }
pub fn set_horizon_range_m(metres: u32) { horizon::set_horizon_range_m(metres) }
pub const MIN_HORIZON_RANGE_M: u32 = horizon::MIN_HORIZON_RANGE_M;
pub const MAX_HORIZON_RANGE_M: u32 = horizon::FAR_PLANE as u32;

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

pub const MAX_TORCH_LIGHTS: usize = 64;

#[derive(Clone, Copy, Debug, Default)]
pub struct ChunkStreamerStats {
    pub cpu_budget_bytes: usize,
    pub gpu_budget_bytes: usize,
    pub budget_deferred: usize,
    pub cpu_evictions: usize,
    pub gpu_evictions: usize,
    pub lod_cache_hits: usize,
    pub lod_cache_bytes: usize,
    pub cpu_resident_bytes: usize,
    pub gpu_resident_bytes: usize,
    pub terrain_buffer_bytes: usize,
    pub queued_mesh_bytes: usize,
    pub upload_staging_bytes: usize,
    pub upload_staging_jobs: usize,
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
    pub parked_offscreen: usize,
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
    pub frame_id: u64,
    pub config_id: u64,
    /// Completed GPU samples, consumed once. Zero means the numeric timing is
    /// retained for the live display only and must not enter an average.
    pub gpu_sample_count: usize,
    pub gpu_sample_frame_id: u64,
    pub gpu_sample_config_id: u64,
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
    pub gpu_indirect_ms: f32,
    pub gpu_cloud_raymarch_ms: f32,
    pub gpu_cloud_temporal_ms: f32,
    pub gpu_sky_ms: f32,
    pub gpu_sky_probe_ms: f32,
    pub gpu_cloud_composite_ms: f32,
    pub gpu_outline_ms: f32,
    pub gpu_postprocess_ms: f32,
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
    indirect_ms: f32,
    cloud_raymarch_ms: f32,
    cloud_temporal_ms: f32,
    sky_ms: f32,
    sky_probe_ms: f32,
    cloud_composite_ms: f32,
    outline_ms: f32,
    postprocess_ms: f32,
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
const GPU_TIMESTAMP_PASS_COUNT: u32 = GPU_TIMESTAMP_SHADOW_PASS_COUNT + 10;
const GPU_TIMESTAMP_QUERY_COUNT: u32 = GPU_TIMESTAMP_PASS_COUNT * 2;
const GPU_TIMESTAMP_RESULT_BYTES: u64 = GPU_TIMESTAMP_QUERY_COUNT as u64 * wgpu::QUERY_SIZE as u64;
const GPU_TIMESTAMP_RESOLVE_BYTES: u64 = GPU_TIMESTAMP_RESULT_BYTES
    .div_ceil(wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT)
    * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
const GPU_TIMESTAMP_SHADOW_PASS_COUNT: u32 = shadow::CASCADE_COUNT as u32;
const GPU_TIMESTAMP_PASS_TERRAIN: u32 = GPU_TIMESTAMP_SHADOW_PASS_COUNT;
const GPU_TIMESTAMP_PASS_CLOUD_RAYMARCH: u32 = GPU_TIMESTAMP_PASS_TERRAIN + 1;
const GPU_TIMESTAMP_PASS_CLOUD_TEMPORAL: u32 = GPU_TIMESTAMP_PASS_TERRAIN + 2;
const GPU_TIMESTAMP_PASS_SKY: u32 = GPU_TIMESTAMP_PASS_TERRAIN + 3;
const GPU_TIMESTAMP_PASS_CLOUD_COMPOSITE: u32 = GPU_TIMESTAMP_PASS_TERRAIN + 4;
const GPU_TIMESTAMP_PASS_OUTLINE: u32 = GPU_TIMESTAMP_PASS_TERRAIN + 5;
const GPU_TIMESTAMP_PASS_UI: u32 = GPU_TIMESTAMP_PASS_TERRAIN + 6;
const GPU_TIMESTAMP_PASS_INDIRECT: u32 = GPU_TIMESTAMP_PASS_TERRAIN + 7;
const GPU_TIMESTAMP_PASS_POSTPROCESS: u32 = GPU_TIMESTAMP_PASS_TERRAIN + 8;
const GPU_TIMESTAMP_PASS_SKY_PROBE: u32 = GPU_TIMESTAMP_PASS_TERRAIN + 9;

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
    frame_id: u64,
    config_id: u64,
    query_count: u32,
    pass_queries: [Option<(u32, u32)>; GPU_TIMESTAMP_PASS_COUNT as usize],
}

#[derive(Debug)]
pub(crate) enum SurfaceFrameError {
    Lost,
    Outdated,
    Timeout,
    Other,
}

impl std::fmt::Display for SurfaceFrameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SurfaceFrameError {}

// One callback per device, installed before any optional profiler resources.
// A false flag means no loss has been reported, not a GPU-completion fence.
fn acquire_surface_frame(surface:&wgpu::Surface<'_>,lost:&AtomicBool,pacer:&frame_pacer::FramePacer,device:&wgpu::Device)->Result<wgpu::SurfaceTexture,SurfaceFrameError> {
    if lost.load(Ordering::Acquire) {return Err(SurfaceFrameError::Lost);}
    let ready = pacer.ready(device);
    if lost.load(Ordering::Acquire) {return Err(SurfaceFrameError::Lost);}
    if !ready { return Err(SurfaceFrameError::Timeout); }
    let result = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Ok(frame),
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => Err(SurfaceFrameError::Timeout),
        wgpu::CurrentSurfaceTexture::Outdated => Err(SurfaceFrameError::Outdated),
        wgpu::CurrentSurfaceTexture::Lost => Err(SurfaceFrameError::Lost),
        wgpu::CurrentSurfaceTexture::Validation => Err(SurfaceFrameError::Other),
    };
    if let Err(error)=&result {
        static FAILURES:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
        let count=FAILURES.fetch_add(1,Ordering::Relaxed)+1;
        if count.is_power_of_two() {eprintln!("[surface-acquire] failure={count} error={error:?}");}
    }
    result
}

/// First device-loss report of the process, kept for the crash log: stderr is
/// not visible in normal play.
static DEVICE_LOSS_REPORT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// wgpu's reason and message for the device loss, if one was reported.
pub fn device_loss_report() -> Option<&'static str> {
    DEVICE_LOSS_REPORT.get().map(String::as_str)
}

fn install_device_loss_signal(device: &wgpu::Device) -> Arc<AtomicBool> {
    let lost = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&lost);
    device.set_device_lost_callback(move |reason, message| {
        if !signal.swap(true, Ordering::AcqRel) {
            eprintln!("[device-lost] reason={reason:?} message={message}");
            let _ = DEVICE_LOSS_REPORT.set(format!("{reason:?}: {message}"));
        }
    });
    lost
}

struct GpuProfiler {
    slots: Vec<GpuProfilerSlot>,
    next_slot: usize,
    next_submission_id: u64,
    timestamp_period_ns: f32,
    done_tx: mpsc::Sender<GpuTimestampReadback>,
    done_rx: mpsc::Receiver<GpuTimestampReadback>,
    last_timings: GpuPassTimings,
    completed: VecDeque<GpuTimingSample>,
    device_lost: Arc<AtomicBool>,
    failed: bool,
}

#[derive(Clone, Copy)]
struct GpuTimingSample {
    timings: GpuPassTimings,
    frame_id: u64,
    config_id: u64,
}

impl GpuProfiler {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, device_lost: Arc<AtomicBool>) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }

        // Optional diagnostic allocations must not become invalid resources
        // silently published into the next render encoder.
        let internal_scope = device.push_error_scope(wgpu::ErrorFilter::Internal);
        let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let memory_scope = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
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
                    frame_id: 0,
                    config_id: 0,
                    query_count: 0,
                    pass_queries: [None; GPU_TIMESTAMP_PASS_COUNT as usize],
                }
            })
            .collect();

        // These scopes cover resource creation only. Resolve them at startup,
        // never by waiting on a submitted frame in the rendering loop.
        let memory = memory_scope.pop();
        let validation = validation_scope.pop();
        let internal = internal_scope.pop();
        let errors = pollster::block_on(async { [memory.await, validation.await, internal.await] });
        if device_lost.load(Ordering::Acquire) || errors.iter().any(Option::is_some) {
            eprintln!("GPU profiling unavailable: diagnostic resource allocation failed");
            return None;
        }
        Some(Self {
            slots,
            next_slot: 0,
            next_submission_id: 0,
            timestamp_period_ns: queue.get_timestamp_period(),
            done_tx,
            done_rx,
            last_timings: GpuPassTimings::default(),
            completed: VecDeque::with_capacity(GPU_TIMESTAMP_BUFFERED_FRAMES * 2),
            device_lost,
            failed: false,
        })
    }

    fn healthy(&self) -> bool {
        !self.failed && !self.device_lost.load(Ordering::Acquire)
    }

    fn disable(&mut self, reason: &str) {
        if !self.failed {
            eprintln!("GPU profiling disabled: {reason}");
        }
        self.failed = true;
        self.last_timings = GpuPassTimings::default();
        self.completed.clear();
    }

    fn acquire_slot(&mut self) -> Option<usize> {
        if !self.healthy() || self.completed.len() >= GPU_TIMESTAMP_BUFFERED_FRAMES {
            return None;
        }
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
        if query_count == 0 || !self.healthy() {
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
        frame_id: u64,
        config_id: u64,
    ) {
        if query_count == 0 || !self.healthy() {
            return;
        }

        let submission_id = self.next_submission_id;
        self.next_submission_id = self.next_submission_id.wrapping_add(1);
        let tx = self.done_tx.clone();
        let slot = &mut self.slots[slot_index];
        slot.pending = true;
        slot.submission_id = submission_id;
        slot.frame_id = frame_id;
        slot.config_id = config_id;
        slot.query_count = query_count;
        slot.pass_queries = pass_queries;
        let requested = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            slot.readback_buffer
                .slice(..GPU_TIMESTAMP_RESULT_BYTES)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = tx.send(GpuTimestampReadback {
                        slot_index,
                        submission_id,
                        ok: result.is_ok(),
                    });
                });
        }));
        if requested.is_err() {
            self.disable("timing map request became invalid");
        }
    }
    fn collect_finished(&mut self, device: &wgpu::Device) -> bool {
        if !self.healthy() {
            self.disable("device lost or profiling unavailable");
            return false;
        }
        if device.poll(wgpu::PollType::Poll).is_err() || !self.healthy() {
            self.disable("device polling failed or device lost");
            return false;
        }
        while let Ok(done) = self.done_rx.try_recv() {
            let Some(slot) = self.slots.get_mut(done.slot_index) else {
                continue;
            };
            if !slot.pending || slot.submission_id != done.submission_id {
                continue;
            }
            slot.pending = false;
            if !done.ok || self.device_lost.load(Ordering::Acquire) {
                // A failed/destroyed mapping is not safe to unmap or recycle.
                self.disable("timing map failed or device lost");
                return false;
            }
            let period = self.timestamp_period_ns;
            let frame_id = slot.frame_id;
            let config_id = slot.config_id;
            // A lost device can invalidate the range after the map callback.
            // Retire the profiler without retrying the invalid mapping.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mapped = slot
                    .readback_buffer
                    .slice(..GPU_TIMESTAMP_RESULT_BYTES)
                    .get_mapped_range().ok()?;
                let mut raw = [0u64; GPU_TIMESTAMP_QUERY_COUNT as usize];
                if mapped.len() != GPU_TIMESTAMP_RESULT_BYTES as usize {
                    return None;
                }
                for (value, bytes) in raw.iter_mut().zip(mapped.chunks_exact(8)) {
                    *value = u64::from_le_bytes(bytes.try_into().unwrap());
                }
                drop(mapped);
                slot.readback_buffer.unmap();
                Some(Self::decode_timings(&raw, &slot.pass_queries, period))
            }));
            match result {
                Ok(Some(timings)) if self.healthy() => {
                    // At most eight pending readbacks can join eight queued
                    // samples. Stop requesting timestamps under backpressure;
                    // gameplay never waits for the optional profiler.
                    self.completed.push_back(GpuTimingSample { timings, frame_id, config_id });
                }
                _ => {
                    self.disable("timing readback invalidated; timing resources retired");
                    return false;
                }
            }
        }
        true
    }
    fn take_sample(&mut self) -> Option<GpuTimingSample> {
        let sample = self.completed.pop_front()?;
        self.last_timings = sample.timings;
        Some(sample)
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
        let indirect_ms = Self::pass_duration_ms(
            raw,
            pass_queries,
            GPU_TIMESTAMP_PASS_INDIRECT,
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

        let postprocess_ms = Self::pass_duration_ms(
            raw, pass_queries, GPU_TIMESTAMP_PASS_POSTPROCESS, timestamp_period_ns,
        );
        let sky_probe_ms = Self::pass_duration_ms(
            raw, pass_queries, GPU_TIMESTAMP_PASS_SKY_PROBE, timestamp_period_ns,
        );
        GpuPassTimings {
            // Keep the legacy subtotal stable for adaptive quality and old benchmarks.
            total_ms: shadow_ms
                + terrain_ms
                + indirect_ms
                + cloud_raymarch_ms
                + cloud_temporal_ms
                + sky_ms
                + cloud_composite_ms
                + outline_ms
                + postprocess_ms
                + ui_ms,
            shadow_ms,
            terrain_ms,
            indirect_ms,
            cloud_raymarch_ms,
            cloud_temporal_ms,
            sky_ms,
            sky_probe_ms,
            cloud_composite_ms,
            outline_ms,
            postprocess_ms,
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

// Deterministic spatial integration. Temporal reprojection remains
// available for diagnostics, but an estimated volume depth can drag unrelated
// cloud samples together during translation. The default uses the current frame.
static CLOUD_RENDER_SCALE: AtomicU32 = AtomicU32::new(4);
/// Requested divisor: 2 = Ultra, 3 = High, 4 = Balanced.
/// Extra divisor added by the adaptive cloud governor (0 = none).
static CLOUD_AUTO_BUMP: AtomicU32 = AtomicU32::new(0);
static CLOUD_ADAPTIVE_ENABLED: AtomicBool = AtomicBool::new(true);
static CLOUDS_ENABLED: AtomicBool = AtomicBool::new(true);
/// Clouds cut by automatic quality for this session only; the saved choice stays on.
static CLOUDS_CUT: AtomicBool = AtomicBool::new(false);
static INTEGRATED_GPU: AtomicBool = AtomicBool::new(false);

/// The selected adapter shares system memory and power with the CPU (or is a
/// software rasterizer). Known once the GPU is initialised.
pub fn integrated_gpu() -> bool {
    INTEGRATED_GPU.load(Ordering::Relaxed)
}

/// Probe-based atmospheric shading and a bounded local-light set.
/// `VOXEL_LEAN_SHADING=0` opts into per-pixel sky sampling and 64 lights.
pub fn lean_shading() -> bool {
    static FORCED: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    FORCED
        .get_or_init(|| match std::env::var("VOXEL_LEAN_SHADING").as_deref() {
            Ok("1") => Some(true),
            Ok("0") => Some(false),
            _ => None,
        })
        .unwrap_or(true)
}

/// Nearest world lights uploaded per frame. Every terrain pixel tests each one.
pub fn torch_light_budget() -> usize {
    if lean_shading() { 16 } else { MAX_TORCH_LIGHTS }
}

/// Player setting. Off skips the raymarch and reports a cloudless sky to every
/// shader (no coverage, no storm, no terrain cloud shadow or cloud fog).
pub fn clouds_enabled() -> bool {
    CLOUDS_ENABLED.load(Ordering::Relaxed) && !CLOUDS_CUT.load(Ordering::Relaxed)
}
/// The player's choice. Settings writes the shown state back every frame, so an
/// unchanged value must not turn a session cut into a saved choice.
pub fn set_clouds_enabled(enabled: bool) {
    if enabled != clouds_enabled() {
        CLOUDS_ENABLED.store(enabled, Ordering::Relaxed);
        CLOUDS_CUT.store(false, Ordering::Relaxed);
    }
}
/// Clouds off until restart or until the player turns them on; not saved.
pub fn cut_clouds_for_session() {
    CLOUDS_CUT.store(true, Ordering::Relaxed);
}
/// The saved setting, ignoring a session cut.
pub fn clouds_chosen() -> bool {
    CLOUDS_ENABLED.load(Ordering::Relaxed)
}
fn draw_clouds() -> bool {
    clouds_enabled() && !render_isolation().skip_clouds
}

/// Controlled reviews must compare the same quality even when their GPU cost differs.
pub fn set_cloud_adaptive_enabled(enabled: bool) {
    CLOUD_ADAPTIVE_ENABLED.store(enabled, Ordering::Relaxed);
    if !enabled { CLOUD_AUTO_BUMP.store(0, Ordering::Relaxed); }
}

pub fn cloud_render_scale() -> u32 {
    CLOUD_RENDER_SCALE.load(Ordering::Relaxed)
}
pub fn set_cloud_render_scale(scale: u32) {
    CLOUD_RENDER_SCALE.store(scale.clamp(2, 4), Ordering::Relaxed);
}
/// Preserve the normal 640x360 quality floor at the requested setting. Sustained
/// GPU overload may temporarily use a 426x240 floor instead; otherwise the
/// governor's requests are clamped away at 720p/1080p and cannot relieve load.
///
/// `width` and `height` are the scene size (see `scene_dims`). The 360-row floor
/// scales with the render resolution, so a smaller scene gets a proportionally
/// smaller cloud image and the clouds cost less in step with everything else.
pub fn effective_cloud_render_scale(width: u32, height: u32) -> u32 {
    let percent = scene_scale_percent();
    let fraction = percent as f32 / 100.0;
    let divisor = |size: u32, floor: f32| {
        let ratio = size as f32 / (floor * fraction);
        // At full resolution keep the long-standing whole-number floor exactly.
        (if percent >= 100 { ratio.floor() } else { ratio.round() }) as u32
    };
    let normal_maximum = divisor(height, 240.0).min(divisor(width, 426.0)).clamp(1, 4);
    let base = cloud_render_scale().min(normal_maximum);
    let bump = CLOUD_AUTO_BUMP.load(Ordering::Relaxed);
    if bump == 0 { return base; }
    let adaptive_maximum = divisor(height, 240.0).min(divisor(width, 426.0)).clamp(1, 6);
    (base + bump).min(adaptive_maximum)
}
/// Divisor request including the governor's bump and the render resolution,
/// used to notice changes that need the scene targets rebuilt.
fn cloud_scale_request(width: u32, height: u32) -> u32 {
    let (width, height) = scene_dims(width, height);
    // Recreate targets only when their effective dimensions change. A governor
    // step clipped by the minimum image size must not cause a full resize hitch.
    effective_cloud_render_scale(width, height) + scene_scale_percent() * 8
}

/// Internal render resolution as a percentage of the window. The 3D scene (terrain,
/// sky, clouds, depth) is drawn at this size and the resolve pass scales it up to
/// the window; the UI and text stay at native resolution.
static SCENE_SCALE_PERCENT: AtomicU32 = AtomicU32::new(100);
pub const MIN_SCENE_SCALE_PERCENT: u32 = 50;
pub fn scene_scale_percent() -> u32 {
    SCENE_SCALE_PERCENT.load(Ordering::Relaxed)
}
pub fn set_scene_scale_percent(percent: u32) {
    SCENE_SCALE_PERCENT.store(percent.clamp(MIN_SCENE_SCALE_PERCENT, 100), Ordering::Relaxed);
}
/// The window size scaled by the render resolution, never below 320x180 (or the
/// window itself when that is smaller).
pub(crate) fn scene_dims(width: u32, height: u32) -> (u32, u32) {
    let percent = scene_scale_percent().clamp(MIN_SCENE_SCALE_PERCENT, 100);
    if percent >= 100 {
        return (width.max(1), height.max(1));
    }
    let scaled = |size: u32, floor: u32| ((size * percent + 50) / 100).max(floor.min(size)).max(1);
    (scaled(width, 320), scaled(height, 180))
}
/// A copy of the surface configuration at the scene size.
pub(crate) fn scene_config(config: &wgpu::SurfaceConfiguration) -> wgpu::SurfaceConfiguration {
    let (width, height) = scene_dims(config.width, config.height);
    let mut scene = config.clone();
    scene.width = width;
    scene.height = height;
    scene
}
const CLOUD_TEMPORAL_ENABLED: bool = false;

#[derive(Clone, Copy, Default)]
struct RenderIsolation {
    skip_clouds: bool,
    skip_horizon: bool,
    skip_horizon_work: bool,
}

fn render_isolation() -> RenderIsolation {
    static OPTIONS: std::sync::OnceLock<RenderIsolation> = std::sync::OnceLock::new();
    *OPTIONS.get_or_init(|| RenderIsolation {
        skip_clouds: std::env::var("VOXEL_REVIEW_SKIP_CLOUDS").as_deref() == Ok("1"),
        skip_horizon: crate::low_spec::active()
            || std::env::var("VOXEL_REVIEW_SKIP_HORIZON").as_deref() == Ok("1"),
        skip_horizon_work: std::env::var("VOXEL_REVIEW_SKIP_HORIZON_WORK").as_deref() == Ok("1"),
    })
}

use crate::shader_sources::{LINE_SHADER, SHADOW_SHADER, UI_SHADER};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    inv_view_proj: [[f32; 4]; 4],
    world_params: [f32; 4],
    prev_view_proj: [[f32; 4]; 4],
    camera_forward: [f32; 4],
    ray_right: [f32; 4],
    ray_up: [f32; 4],
    previous_camera_pos: [f32; 4],
    scene_seed: [u32; 4],
    /// x: conservative published resident coverage in metres. A negative
    /// value means "no override"; zero means no resident mesh is published.
    coverage: [f32; 4],
    resident_coverage: coverage::ResidentCoverage,
    /// xyz: `RenderOrigin` in voxels, the frame chunk vertices are drawn in.
    render_origin: [i32; 4],
    /// xyz: eye minus render origin, metres, less the view residual.
    eye_from_origin: [f32; 4],
    planet: [[f32; 4]; 8],
    /// Portal destination clip plane, eye-relative; zero in the primary view.
    portal_clip: [f32; 4],
    /// Chart shear undo around the eye, as `planet_frame::Eye::shader_rows`.
    planet_flat: [[f32; 4]; 3],
    /// Eye direction to fixed star axes, `planet::sky_axes`; zero: identity.
    sky_axes: [[f32; 4]; 3],
    terrain_arrival: coverage::TerrainArrival,
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
    /// xyz: horizon sky colour. w: distant terrain range in metres, zero
    /// when no backdrop is drawn; the world shader closes its far edge there.
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
    material_tints: [[f32; 4]; 256],
    material_properties: [[f32; 4]; 256],
    surface_weather: [f32; 4],
    /// cosmos chamber, aurora, meteor shower, rainbow.
    mood: [f32; 4],
    /// fireflies, pollen, diamond dust, embers.
    ambience: [f32; 4],
    biome_ambience: [f32; 4],
    mana_ambience: [f32; 4],
    location_mood: [f32; 4],
    /// Three linear palette colours, flow controls, then detail controls.
    magic: [[f32; 4]; 5],
    /// `SkyArt::lighting_words`: solar disc, sky grade and cloud art controls.
    sky_art: [[f32; 4]; 2],
    torch_group_bounds: [[f32; 4]; 16],
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
    let mut state = CloudWeatherUniformState {
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
    };
    if !clouds_enabled() {
        // A zero inverse span maps every position to the clip corner, where
        // each level's weight is zero, so the terrain samples no cloud shadow.
        // Zero coverage and storm clear the sky grade and the cloud fog.
        for (region, summary) in state.regions.iter_mut().zip(&mut state.summaries) {
            region[3] = 0.0;
            summary[1] = 0.0;
            summary[2] = 0.0;
        }
    }
    state
}

/// The material studio's tints with the season layered over the meadow (1) and
/// canopy (6) families, so the year turns without touching any mesh.
fn seasonal_tints(state: &LightingState) -> [[f32; 4]; 256] {
    let mut tints = crate::material_studio::snapshot().tints;
    for k in 0..3 {
        tints[1][k] *= state.grass_tint[k];
        tints[6][k] *= state.foliage_tint[k];
    }
    tints
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

    // Moving lights go first so the shader can time-seed exactly that prefix
    // (torch_meta.z). Light sums are order-independent, so no light changes.
    let moving_count = torch_lights.iter().filter(|torch| torch.moving).count().min(MAX_TORCH_LIGHTS);
    let ordered = torch_lights
        .iter()
        .filter(|torch| torch.moving)
        .chain(torch_lights.iter().filter(|torch| !torch.moving));
    for (index, torch) in ordered.take(MAX_TORCH_LIGHTS).enumerate() {
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

    // Eight finite light groups reject distant pixels before individual BRDFs.
    let mut torch_group_bounds = [[0.0; 4]; 16];
    for group in 0..8 {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(-f32::MAX);
        for index in group*8..((group+1)*8).min(torch_lights.len().min(MAX_TORCH_LIGHTS)) {
            let light = torch_positions[index];
            let position = Vec3::new(light[0],light[1],light[2]);
            let radius = Vec3::splat(light[3]);
            min = min.min(position-radius);
            max = max.max(position+radius);
        }
        torch_group_bounds[group*2] = min.extend(0.0).to_array();
        torch_group_bounds[group*2+1] = max.extend(0.0).to_array();
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
            if render_isolation().skip_horizon { 0.0 } else { horizon::horizon_range_m() as f32 },
        ],
        fog_color_density: [
            state.fog_color[0],
            state.fog_color[1],
            state.fog_color[2],
            0.0,
        ],
        fog_params: [fog_start, fog_end, state.fog_height, state.fog_falloff],
        weather_globals: [
            [
                sky_time * cloud_controls.advection,
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
            [
                cloud_controls.anvil_spread,
                cloud_controls.coverage,
                cloud_controls.cumulonimbus,
                cloud_controls.advection,
            ],
        ],
        weather_clip_regions: weather_state.regions,
        weather_clip_summaries: weather_state.summaries,
        held_torch_pos_radius,
        held_torch_color_strength,
        torch_meta: [
            torch_lights.len().min(MAX_TORCH_LIGHTS) as f32,
            sky_time,
            moving_count as f32,
            debug_view.as_u32() as f32,
        ],
        torch_positions,
        torch_colors,
        torch_group_bounds,
        material_tints: seasonal_tints(state),
        material_properties: crate::material_studio::snapshot().properties,
        surface_weather: crate::material_studio::weather_surface(cloud_controls.cumulonimbus),
        mood: state.mood,
        ambience: state.ambience,
        biome_ambience: state.biome_ambience,
        mana_ambience: state.mana_ambience,
        location_mood: state.location_mood,
        magic: crate::egui_ui::magic_style::world_uniform(),
        sky_art: {
            let mut words = atmosphere_params::SkyArt::active().lighting_words();
            words[1][3] = state.cave_darkness;
            words
        },
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TorchLight {
    pub position: Vec3,
    pub radius: f32,
    pub color: [f32; 3],
    pub strength: f32,
    /// Follows the player or a spell. Its flicker is seeded from time alone so
    /// walking does not re-roll it; static lights seed from their position.
    pub moving: bool,
}

/// Maps conventional clip depth (0 near .. 1 far) to reverse-Z (1 near .. 0 far):
/// z' = w - z. It is an involution, so it also converts inverse matrices back.
const REVERSE_Z: Mat4 = Mat4::from_cols(
    Vec4::new(1.0, 0.0, 0.0, 0.0),
    Vec4::new(0.0, 1.0, 0.0, 0.0),
    Vec4::new(0.0, 0.0, -1.0, 0.0),
    Vec4::new(0.0, 0.0, 1.0, 1.0),
);

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

#[derive(Clone, Copy, Default, PartialEq)]
pub enum CameraMedium {
    #[default]
    Air,
    Water { depth_m: f32 },
    Lava,
}

impl CameraMedium {
    fn uniform(self) -> f32 {
        match self {
            Self::Air => 0.0,
            Self::Water { depth_m } => 1.0 + depth_m,
            Self::Lava => -1.0,
        }
    }
}

pub struct Gpu {
    pub camera_medium: CameraMedium,
    /// World-space sea level in metres. Sent through the spare camera coverage
    /// lane so terrain can receive the shallow caustic spill without a second
    /// uniform buffer.
    pub water_level_m: f32,
    /// RGB + intensity of active spell effects for the trippy resolve.
    screen_magic: [f32; 4],
    motion_boost: f32,
    camera_roll: f32,
    camera_up: Vec3,
    finite_world: bool,
    /// The Nether: the sky draws the realm dome (`cam.planet[0].w = -1`).
    realm_sky: bool,
    /// Manyfold shell axis, metres. The interior fill aims here.
    manyfold_axis_y: f32,
    /// Underground zone grade: veil rgb+amount, then desat/pulse/swirl/gain.
    zone_grade: ([f32; 4], [f32; 4]),
    postprocess: postprocess::PostProcess,
    indirect: indirect::IndirectLighting,
    portals: portals::Stage,
    pub fxaa_enabled: bool,
    surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    device_lost: Arc<AtomicBool>,
    upload_staging: Arc<upload_staging::UploadStaging>,
    chunk_upload_credits: std::sync::Mutex<Vec<upload_staging::Credit>>,
    frame_pacer: frame_pacer::FramePacer,
    unthrottled_present_mode: wgpu::PresentMode,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    adapter_summary: String,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    lighting_buffer: wgpu::Buffer,
    lighting_bind_group: wgpu::BindGroup,
    torch_blockage: torch_occlusion::State,
    sky_volume: sky_volume_gpu::SkyVolumeGpu,
    atmosphere: AtmosphereRuntime,
    shadow_renderer: shadow::ShadowRenderer,
    /// Indirect arguments for the main terrain pass (multi-draw path).
    chunk_indirect: streaming::indirect_draws::IndirectDraws,
    /// Per-chunk draw transforms for the main pass and shadow cascades.
    draw_offsets: DrawOffsets,
    /// Chunk vertex frame; `None` before the first camera upload.
    render_origin: Option<RenderOrigin>,
    horizon_detail: horizon::detail::Gpu,
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
    terrain_backstop: wgpu::RenderPipeline,
    horizon: horizon::Horizon,
    planet_mesh: Option<planet::Mesh>,
    outgoing_planet_mesh: Option<(planet::Mesh, Instant)>,
    terrain_arrival: coverage::TerrainArrival,
    planet_progress: Option<Arc<planet::progress::Progress>>,
    planet_build: Option<planet::CoverJob>,
    scene_stream: Option<DynamicVertexBuffer>,
    posed_assets: Option<posed_assets::PosedAssets>,
    scene_vertices: usize,
    line_pipeline: wgpu::RenderPipeline,
    line_invert_pipeline: wgpu::RenderPipeline,
    survey_line_pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,
    outline_stream: DynamicVertexBuffer,
    invert_outline_stream: DynamicVertexBuffer,
    survey_outline_stream: DynamicVertexBuffer,
    ui_stream: DynamicVertexBuffer,
    depth: wgpu::TextureView,
    /// Opaque scene occupancy, copied before fallback terrain writes its real depth.
    terrain_backstop_depth: wgpu::TextureView,
    clear_color: [f32; 3],
    current_lighting: LightingState,
    current_debug_view: RenderDebugView,
    last_camera_pos: Vec3,
    last_camera_residual: Vec3,
    /// Planet placement for this frame's camera; chunks, outlines and the
    /// cover shader all read this one value.
    planet_eye: Option<planet_frame::Eye>,
    last_rotation_view_proj: Mat4,
    last_camera_forward: Vec3,
    last_world_scale: f32,
    globe: Option<globe::gpu::Globe>,
    globe_visible: bool,
    globe_driver: globe_driver::GlobeDriver,
    /// Origin of `GlobeView::clock_s`; render-thread wall time, never simulation time.
    globe_clock: std::time::Instant,
    tier: crate::altitude::Tier,
    /// Published resident coverage radius in metres; the globe fades in past it.
    globe_disc_radius_m: f32,
    /// Set by `update_horizon` when the horizon is cleared and the globe alone draws distant ground.
    horizon_globe_only: bool,
    cloud_history_valid: bool,
    cloud_render_scale: u32,
    cloud_governor: cloud_governor::CloudGovernor,
    last_frame_instant: Instant,
    frame_index: u32,
    render_frame_id: u64,
    // Monotonic sky time, used by the spell grade so its cadence is independent
    // of frame rate. frame_index is retained for temporal jitter only.
    effect_time: f32,
    cloud_history_frame_stats: CloudHistoryFrameStats,
    last_weather_upload_stats: WeatherUploadStats,
    last_render_stats: RenderFrameStats,
    capture_path: Option<std::path::PathBuf>,
    pending_captures: Vec<capture::PendingCapture>,
    gpu_profiler: Option<GpuProfiler>,
}

// ---------------------------------------------------------------------------
// CPU-only noise data generators — called from background threads at startup
// so the main thread does not freeze during GPU initialisation.
// ---------------------------------------------------------------------------

/// Periodic 3D hash shared by the cloud shape and detail noise generators.
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
    device_lost: Arc<AtomicBool>,
    frame_pacer: frame_pacer::FramePacer,
    unthrottled_present_mode: wgpu::PresentMode,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    adapter_summary: String,
    default_lighting: LightingState,
    clear_color: [f32; 3],
    /// Shared by the pipelines built during init, saved once they exist.
    pipeline_cache: Option<pipeline_cache::DiskCache>,
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
    torch_blockage: Option<torch_occlusion::State>,
    sky_volume: Option<sky_volume_gpu::SkyVolumeGpu>,
    atmosphere_layout: Option<wgpu::BindGroupLayout>,
    atmosphere_runtime: Option<AtmosphereRuntime>,
    shadow_renderer: Option<shadow::ShadowRenderer>,

    world_surface_layout: Option<wgpu::BindGroupLayout>,
    cloud_texture_layout: Option<wgpu::BindGroupLayout>,
    cloud_render_layout: Option<wgpu::BindGroupLayout>,
    cloud_temporal_layout: Option<wgpu::BindGroupLayout>,

    world_surface_bind_group: Option<wgpu::BindGroup>,
    horizon_detail: Option<horizon::detail::Gpu>,

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
    terrain_backstop_depth: Option<wgpu::TextureView>,

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
    terrain_backstop: Option<wgpu::RenderPipeline>,
    line_pipeline: Option<wgpu::RenderPipeline>,
    line_invert_pipeline: Option<wgpu::RenderPipeline>,
    survey_line_pipeline: Option<wgpu::RenderPipeline>,
    ui_pipeline: Option<wgpu::RenderPipeline>,
    /// Post-processing and bounce lighting built during `Pipelines`, with the
    /// surface and scene sizes they were built for.
    scene_passes: Option<(ScenePassKey, postprocess::PostProcess, indirect::IndirectLighting)>,

    outline_stream: Option<DynamicVertexBuffer>,
    invert_outline_stream: Option<DynamicVertexBuffer>,
    survey_outline_stream: Option<DynamicVertexBuffer>,
    ui_stream: Option<DynamicVertexBuffer>,

}

/// Surface size and scene size; the post-process and bounce targets follow both.
type ScenePassKey = (u32, u32, (u32, u32));

fn scene_pass_key(config: &wgpu::SurfaceConfiguration) -> ScenePassKey {
    (config.width, config.height, scene_dims(config.width, config.height))
}
impl GpuBootstrap {
    pub async fn new(window: Arc<Window>) -> Result<Self> {
        let probe = std::env::var_os("VOXEL_PLAYABLE_PROBE").is_some();
        let mut bootstrap_lap = std::time::Instant::now();
        let mut probe_stage = |stage| {
            if probe {
                let now = std::time::Instant::now();
                println!("[playable-probe] gpu_stage={stage} ms={:.1}", (now - bootstrap_lap).as_secs_f64() * 1000.0);
                bootstrap_lap = now;
            }
        };
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
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(window.clone())));
        let surface = instance.create_surface(window)?;
        probe_stage("instance_surface");

        // `VOXEL_GPU_POWER=low` picks the integrated GPU, to measure weak-hardware cost on a dev box.
        let power_preference = if std::env::var("VOXEL_GPU_POWER").as_deref() == Ok("low") {
            wgpu::PowerPreference::LowPower
        } else {
            wgpu::PowerPreference::HighPerformance
        };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .context("No suitable GPU adapter found")?;
        probe_stage("adapter");
        let adapter_info = adapter.get_info();
        INTEGRATED_GPU.store(
            matches!(adapter_info.device_type, wgpu::DeviceType::IntegratedGpu | wgpu::DeviceType::Cpu),
            Ordering::Relaxed,
        );
        streaming::configure_gpu_residency(&adapter);
        let adapter_summary = format!(
            "{} / {:?} / {:?}",
            adapter_info.name, adapter_info.backend, adapter_info.device_type
        );
        let requested_features = adapter.features()
            & (wgpu::Features::TIMESTAMP_QUERY
                | wgpu::Features::FLOAT32_FILTERABLE
                | wgpu::Features::INDIRECT_FIRST_INSTANCE)
            | pipeline_cache::feature(&adapter);

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
        probe_stage("device");
        streaming::indirect_draws::set_supported(&adapter, device.features());
        crate::mesh::models::posed::set_enabled(true);
        let pipeline_cache = pipeline_cache::DiskCache::open(&device, &adapter_info);
        probe_stage("pipeline_cache");

        let device_lost = install_device_loss_signal(&device);
        let caps = surface.get_capabilities(&adapter);
        // On composited Windows Vulkan surfaces, prefer a replaceable queued
        // image to Immediate's synchronous image recycling. Keep the portable
        // automatic fallback on surfaces without Mailbox.
        let unthrottled_present_mode = if cfg!(windows)
            && adapter_info.backend == wgpu::Backend::Vulkan
            && caps.present_modes.contains(&wgpu::PresentMode::Mailbox)
        {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | (caps.usages & wgpu::TextureUsages::COPY_SRC),
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width: size.width.max(1),
            height: size.height.max(1),
            // A GPU-bound integrated adapter would otherwise queue a whole
            // extra 30-60 ms frame between input and the screen.
            desired_maximum_frame_latency: if integrated_gpu() { 1 } else { 2 },
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: if format.is_srgb() {
                vec![format.remove_srgb_suffix()]
            } else {
                vec![]
            },
        };
        surface.configure(&device, &config);
        probe_stage("surface_configure");

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
            device_lost,
            frame_pacer: frame_pacer::FramePacer::new(config.desired_maximum_frame_latency),
            unthrottled_present_mode,
            config,
            size,
            adapter_summary,
            default_lighting,
            clear_color,
            pipeline_cache,
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
        self.frame_pacer.reset();
    }

    pub fn render_loading_frame<F>(
        &mut self,
        clear_color: [f32; 3],
        overlay: F,
    ) -> Result<(), SurfaceFrameError>
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
        let frame = acquire_surface_frame(&self.surface,&self.device_lost,&self.frame_pacer,&self.device)?;
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
                multiview_mask: None,
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

        // Blend egui gamma-space premultiplied colors after the scene resolve.
        let ui_view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.format.remove_srgb_suffix()),
            ..Default::default()
        });
        overlay(
            &self.device,
            &self.queue,
            &mut encoder,
            &ui_view,
            self.config.width,
            self.config.height,
        );

        self.queue.submit(Some(encoder.finish()));
        self.frame_pacer.track_submission(&self.queue);
        self.queue.present(frame);
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
            torch_blockage: None,
            sky_volume: None,
            atmosphere_layout: None,
            atmosphere_runtime: None,
            shadow_renderer: None,

            world_surface_layout: None,
            cloud_texture_layout: None,
            cloud_render_layout: None,
            cloud_temporal_layout: None,

            world_surface_bind_group: None,
            horizon_detail: None,

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
            terrain_backstop_depth: None,

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
            terrain_backstop: None,
            line_pipeline: None,
            line_invert_pipeline: None,
            survey_line_pipeline: None,
            ui_pipeline: None,
            scene_passes: None,

            outline_stream: None,
            invert_outline_stream: None,
            survey_outline_stream: None,
            ui_stream: None,

        }
    }
}
impl GpuPendingInit {
    pub fn progress(&self) -> LoadingProgress {
        match self.stage {
            GpuInitStage::CoreBindings => LoadingProgress {
                detail: "Starting graphics",
                progress_01: 0.20,
            },
            GpuInitStage::CloudFields => LoadingProgress {
                detail: "Loading textures",
                progress_01: 0.45,
            },
            GpuInitStage::ShaderModules => LoadingProgress {
                detail: "Loading shaders",
                progress_01: 0.65,
            },
            GpuInitStage::Pipelines => LoadingProgress {
                detail: "Starting graphics",
                progress_01: 0.82,
            },
            GpuInitStage::Finalize => LoadingProgress {
                detail: "Finishing",
                progress_01: 0.92,
            },
            GpuInitStage::Complete => LoadingProgress {
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
    ) -> Result<(), SurfaceFrameError>
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
        let stage = self.stage;
        let device = self
            .bootstrap
            .as_ref()
            .context("renderer bootstrap missing")?
            .device
            .clone();
        let validation_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let started = std::time::Instant::now();
        let result = self.step_inner();
        if let Some(error) = pollster::block_on(validation_scope.pop()) {
            anyhow::bail!("Renderer initialization failed at {stage:?}: {error}");
        }
        println!(
            "[startup-timing] renderer stage={stage:?} ms={:.1}",
            started.elapsed().as_secs_f64() * 1000.0
        );
        result
    }

    fn step_inner(&mut self) -> Result<Option<Gpu>> {
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
                        ray_right: [1.0, 0.0, 0.0, -1.0],
                        ray_up: [0.0, 1.0, 0.0, -0.01],
                        previous_camera_pos: [0.0; 4],
                        scene_seed: [0; 4],
                        coverage: [-1.0, 0.0, 0.0, 0.0],
                        resident_coverage: coverage::ResidentCoverage::default(),
                        render_origin: [0; 4],
                        eye_from_origin: [0.0; 4],
                        planet: [[0.0; 4]; 8],
                        portal_clip: [0.0; 4],
                        planet_flat: [[0.0; 4]; 3],
                        sky_axes: [[0.0; 4]; 3],
                        terrain_arrival: coverage::TerrainArrival::default(),
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
                        }, wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        }, wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        }, wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Uint,
                                view_dimension: wgpu::TextureViewDimension::D3,
                                multisampled: false,
                            },
                            count: None,
                        }, wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        }, wgpu::BindGroupLayoutEntry {
                            binding: 5,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Uint,
                                view_dimension: wgpu::TextureViewDimension::D3,
                                multisampled: false,
                            },
                            count: None,
                        }, wgpu::BindGroupLayoutEntry {
                            binding: 6,
                            visibility: wgpu::ShaderStages::FRAGMENT,
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

                let kit_atlas = viewmodel_texture::atlas(device, &bootstrap.queue);
                let torch_blockage = torch_occlusion::State::new(device);
                let sky_volume = sky_volume_gpu::SkyVolumeGpu::new(device);
                let [fine_probes, fine_window, coarse_probes, coarse_window] = sky_volume.bindings();
                let lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("lighting bind group"),
                    layout: &lighting_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: lighting_buffer.as_entire_binding(),
                    }, wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&kit_atlas),
                    }, wgpu::BindGroupEntry {
                        binding: 2,
                        resource: torch_blockage.buffer().as_entire_binding(),
                    }, wgpu::BindGroupEntry {
                        binding: 3,
                        resource: fine_probes,
                    }, wgpu::BindGroupEntry {
                        binding: 4,
                        resource: fine_window,
                    }, wgpu::BindGroupEntry {
                        binding: 5,
                        resource: coarse_probes,
                    }, wgpu::BindGroupEntry {
                        binding: 6,
                        resource: coarse_window,
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
                                        filterable: device.features().contains(wgpu::Features::FLOAT32_FILTERABLE),
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
                            // Distant-terrain detail map (render/horizon/detail.rs).
                            wgpu::BindGroupLayoutEntry {
                                binding: 5,
                                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT
                                    | wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2Array,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 6,
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
                                binding: 7,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Depth,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                        ],
                    });

                let cloud_texture_layout =
                    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("cloud texture layout"),
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Uniform,
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
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
                                        filterable: device
                                            .features()
                                            .contains(wgpu::Features::FLOAT32_FILTERABLE),
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
                                        filterable: device
                                            .features()
                                            .contains(wgpu::Features::FLOAT32_FILTERABLE),
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
                                        filterable: device
                                            .features()
                                            .contains(wgpu::Features::FLOAT32_FILTERABLE),
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
                self.torch_blockage = Some(torch_blockage);
                self.sky_volume = Some(sky_volume);
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
                    mipmap_filter: wgpu::MipmapFilterMode::Linear,
                    ..Default::default()
                });

                let cloud_field_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("cloud field sampler"),
                    address_mode_u: wgpu::AddressMode::Repeat,
                    address_mode_v: wgpu::AddressMode::Repeat,
                    address_mode_w: wgpu::AddressMode::Repeat,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::MipmapFilterMode::Linear,
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
                    self.camera_buffer.as_ref().unwrap(),
                );

                let depth = Gpu::create_depth(device, config);
                let terrain_backstop_depth = Gpu::create_terrain_backstop_depth(device, config);
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
                    self.lighting_layout.as_ref().unwrap(),
                    self.camera_bind_group.as_ref().unwrap(),
                    scene_dims(config.width, config.height).0,
                    scene_dims(config.width, config.height).1,
                    &bootstrap.default_lighting,
                    &CloudControls::default(),
                    bootstrap.pipeline_cache.as_ref().map(pipeline_cache::DiskCache::cache),
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
                self.terrain_backstop_depth = Some(terrain_backstop_depth);
                self.atmosphere_runtime = Some(atmosphere_runtime);

                self.stage = GpuInitStage::ShaderModules;
                Ok(None)
            }

            GpuInitStage::ShaderModules => {
                let bootstrap = self.bootstrap.as_ref().expect("bootstrap missing");
                let device = &bootstrap.device;

                self.cloud_shader = Some(
                    device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("cloud shader"),
                        source: wgpu::ShaderSource::Wgsl(
                            cloud_shader_source_for_filtering(
                                device
                                    .features()
                                    .contains(wgpu::Features::FLOAT32_FILTERABLE),
                            )
                            .into(),
                        ),
                    }),
                );
                self.cloud_composite_shader =
                    Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("cloud composite shader"),
                        source: wgpu::ShaderSource::Wgsl(cloud_composite_shader_source().into()),
                    }));
                self.sky_shader = Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("sky shader"),
                    source: wgpu::ShaderSource::Wgsl(sky_shader_source_for_filtering(
                        device.features().contains(wgpu::Features::FLOAT32_FILTERABLE)).into()),
                }));
                let hardware_weather_filtering = device.features().contains(wgpu::Features::FLOAT32_FILTERABLE)
                    && !std::env::args().any(|arg| arg == "--benchmark-reference-weather-filtering");
                let world_source=world_shader_source_for_filtering(hardware_weather_filtering);
                let world_fingerprint=format!("{:016x}",crate::shader_sources::source_fingerprint(&world_source));
                let validated=if hardware_weather_filtering { env!("VOXEL_VALIDATED_FILTERED_WORLD_FNV") }
                    else { env!("VOXEL_VALIDATED_WORLD_FNV") };
                // A stale build is fatal; a VOXEL_SHADER_DEV_DIR override is expected to
                // differ and is validated by wgpu when the module is created below.
                let build_match = world_fingerprint == validated;
                let dev_override = std::env::var_os("VOXEL_SHADER_DEV_DIR").is_some();
                anyhow::ensure!(build_match || dev_override,
                    "Build/runtime WGSL source mismatch: built validator={validated}, runtime={world_fingerprint}");
                println!("[shader-validation] world_fnv1a64={world_fingerprint} build_match={build_match} dev_override={dev_override} hardware_weather_filtering={hardware_weather_filtering}");
                self.world_shader =
                    Some(device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("world shader"),
                        source: wgpu::ShaderSource::Wgsl(world_source.into()),
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
                let pipeline_cache = bootstrap.pipeline_cache.as_ref().map(pipeline_cache::DiskCache::cache);

                let cloud_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("cloud layout"),
                    bind_group_layouts: &[
                        Some(self.camera_layout.as_ref().unwrap()),
                        Some(self.lighting_layout.as_ref().unwrap()),
                        Some(self.atmosphere_layout.as_ref().unwrap()),
                        Some(self.cloud_render_layout.as_ref().unwrap()),
                    ],
                    immediate_size: 0,
                });

                let sky_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("sky layout"),
                    bind_group_layouts: &[
                        Some(self.camera_layout.as_ref().unwrap()),
                        Some(self.lighting_layout.as_ref().unwrap()),
                        Some(self.atmosphere_layout.as_ref().unwrap()),
                        Some(self.world_surface_layout.as_ref().unwrap()),
                    ],
                    immediate_size: 0,
                });

                let cloud_composite_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("cloud composite layout"),
                        bind_group_layouts: &[
                            Some(self.lighting_layout.as_ref().unwrap()),
                            Some(self.cloud_texture_layout.as_ref().unwrap()),
                            Some(self.atmosphere_layout.as_ref().unwrap()),
                            Some(self.cloud_render_layout.as_ref().unwrap()),
                        ],
                        immediate_size: 0,
                    });

                let world_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("world layout"),
                    bind_group_layouts: &[
                        Some(self.camera_layout.as_ref().unwrap()),
                        Some(self.lighting_layout.as_ref().unwrap()),
                        Some(self.atmosphere_layout.as_ref().unwrap()),
                        Some(self.world_surface_layout.as_ref().unwrap()),
                    ],
                    immediate_size: 0,
                });

                let line_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("line layout"),
                    bind_group_layouts: &[Some(self.camera_layout.as_ref().unwrap())],
                    immediate_size: 0,
                });

                let ui_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("ui layout"),
                    bind_group_layouts: &[],
                    immediate_size: 0,
                });

                let cloud_temporal_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("cloud temporal pipeline layout"),
                        bind_group_layouts: &[
                            Some(self.camera_layout.as_ref().unwrap()),
                            Some(self.lighting_layout.as_ref().unwrap()),
                            Some(self.cloud_temporal_layout.as_ref().unwrap()),
                        ],
                        immediate_size: 0,
                    });

                // A pass with no vertex buffers that draws one full-screen triangle.
                let fullscreen = |label: &str,
                                  layout: &wgpu::PipelineLayout,
                                  module: &wgpu::ShaderModule,
                                  format: wgpu::TextureFormat,
                                  blend: Option<wgpu::BlendState>,
                                  depth_stencil: Option<wgpu::DepthStencilState>| {
                    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(label),
                        layout: Some(layout),
                        vertex: wgpu::VertexState {
                            module,
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            buffers: &[],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module,
                            entry_point: Some("fs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format,
                                blend,
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil,
                        multisample: Default::default(),
                        multiview_mask: None,
                        cache: pipeline_cache,
                    })
                };

                let world_shader = self.world_shader.as_ref().unwrap();
                let world_pipeline = |backstop| {
                    let backstop_constant = if backstop { 1.0 } else { 0.0 };
                    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(if backstop { "terrain depth backstop" } else { "world pipeline" }),
                        layout: Some(&world_layout),
                        vertex: wgpu::VertexState {
                            module: world_shader,
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions {
                                constants: &[("TERRAIN_BACKSTOP", backstop_constant)],
                                ..Default::default()
                            },
                            buffers: &[
                                Some(wgpu::VertexBufferLayout {
                                    array_stride: std::mem::size_of::<Vertex3>() as u64,
                                    step_mode: wgpu::VertexStepMode::Vertex,
                                    attributes: &wgpu::vertex_attr_array![
                                        0 => Float32x3,
                                        1 => Float32x3,
                                        2 => Float32x3,
                                        3 => Float32x4,
                                        4 => Float32
                                    ],
                                }),
                                Some(DrawTransform::LAYOUT),
                            ],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: world_shader,
                            entry_point: Some("fs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions {
                                constants: &[
                                    ("LEAN_SHADING", if lean_shading() { 1.0 } else { 0.0 }),
                                    ("CAVE_WATER_FAST", if std::env::var("VOXEL_CAVE_WATER_FAST").as_deref() == Ok("0") { 0.0 } else { 1.0 }),
                                    ("TERRAIN_BACKSTOP", backstop_constant),
                                ],
                                ..Default::default()
                            },
                            targets: &[
                                Some(wgpu::ColorTargetState {
                                    format: postprocess::SCENE_FORMAT,
                                    blend: Some(wgpu::BlendState::REPLACE),
                                    write_mask: wgpu::ColorWrites::ALL,
                                }),
                                Some(wgpu::ColorTargetState {
                                    format: postprocess::SCENE_FORMAT,
                                    blend: None,
                                    write_mask: wgpu::ColorWrites::ALL,
                                }),
                            ],
                        }),
                        primitive: wgpu::PrimitiveState {
                            cull_mode: (!backstop).then_some(wgpu::Face::Back),
                            ..Default::default()
                        },
                        depth_stencil: Some(wgpu::DepthStencilState {
                            format: wgpu::TextureFormat::Depth32Float,
                            depth_write_enabled: Some(true),
                            depth_compare: Some(wgpu::CompareFunction::Greater),
                            stencil: Default::default(),
                            bias: Default::default(),
                        }),
                        multisample: Default::default(),
                        multiview_mask: None,
                        cache: pipeline_cache,
                    })
                };

                let line_shader = self.line_shader.as_ref().unwrap();
                let line_pipeline = |label: &str, blend: wgpu::BlendState, depth_compare| {
                    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some(label),
                        layout: Some(&line_layout),
                        vertex: wgpu::VertexState {
                            module: line_shader,
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            buffers: &[Some(wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<LineVertex>() as u64,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                            })],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: line_shader,
                            entry_point: Some("fs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            targets: &[Some(wgpu::ColorTargetState {
                                format: postprocess::SCENE_FORMAT,
                                blend: Some(blend),
                                write_mask: wgpu::ColorWrites::ALL,
                            })],
                        }),
                        primitive: wgpu::PrimitiveState {
                            topology: wgpu::PrimitiveTopology::LineList,
                            ..Default::default()
                        },
                        depth_stencil: Some(wgpu::DepthStencilState {
                            format: wgpu::TextureFormat::Depth32Float,
                            depth_write_enabled: Some(false),
                            depth_compare: Some(depth_compare),
                            stencil: Default::default(),
                            bias: Default::default(),
                        }),
                        multisample: Default::default(),
                        multiview_mask: None,
                        cache: pipeline_cache,
                    })
                };

                let ui_shader = self.ui_shader.as_ref().unwrap();
                let ui_pipeline = || {
                    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("ui pipeline"),
                        layout: Some(&ui_layout),
                        vertex: wgpu::VertexState {
                            module: ui_shader,
                            entry_point: Some("vs_main"),
                            compilation_options: wgpu::PipelineCompilationOptions::default(),
                            buffers: &[Some(wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<UiVertex>() as u64,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3],
                            })],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: ui_shader,
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
                        multiview_mask: None,
                        cache: pipeline_cache,
                    })
                };

                let premultiplied_over = wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                };
                let invert_blend = wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::OneMinusDst,
                        dst_factor: wgpu::BlendFactor::Zero,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent::OVER,
                };
                let sky_depth = wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Equal),
                    stencil: Default::default(),
                    bias: Default::default(),
                };
                let cloud_shader = self.cloud_shader.as_ref().unwrap();
                let sky_shader = self.sky_shader.as_ref().unwrap();
                let composite_shader = self.cloud_composite_shader.as_ref().unwrap();
                let temporal_shader = self.cloud_temporal_shader.as_ref().unwrap();
                let shadow_shader = self.shadow_shader.as_ref().unwrap();
                let blue_noise_view = self.blue_noise_view.as_ref().unwrap();
                let camera_layout = self.camera_layout.as_ref().unwrap();
                let depth = self.depth.as_ref().unwrap();

                // The driver compiles each shader when its pipeline is created.
                // After a shader edit that took about 4.4 s in series, so the
                // pipelines are built on parallel threads. Post-processing and
                // bounce lighting join them here instead of compiling in series
                // in `Complete`, which reuses them if the window size still matches.
                let (cloud, sky, composite, temporal, world, backstop, (line, line_invert, survey_line), ui, shadow, scene_passes) =
                    std::thread::scope(|scope| {
                        let scene_passes = scope.spawn(|| {
                            let key = scene_pass_key(config);
                            let postprocess = postprocess::PostProcess::new(device, config, blue_noise_view, pipeline_cache);
                            let indirect = indirect::IndirectLighting::new(
                                device, &scene_config(config), camera_layout, depth, &postprocess.scene, pipeline_cache);
                            (key, postprocess, indirect)
                        });
                        let cloud = scope.spawn(|| {
                            fullscreen("cloud pipeline", &cloud_layout, cloud_shader,
                                wgpu::TextureFormat::Rgba16Float, None, None)
                        });
                        let sky = scope.spawn(|| {
                            fullscreen("sky pipeline", &sky_layout, sky_shader, postprocess::SCENE_FORMAT,
                                Some(wgpu::BlendState::REPLACE), Some(sky_depth))
                        });
                        let composite = scope.spawn(|| {
                            let blend = wgpu::BlendState { color: premultiplied_over, alpha: premultiplied_over };
                            fullscreen("cloud composite pipeline", &cloud_composite_layout, composite_shader,
                                postprocess::SCENE_FORMAT, Some(blend), None)
                        });
                        let temporal = scope.spawn(|| {
                            fullscreen("cloud temporal pipeline", &cloud_temporal_pipeline_layout,
                                temporal_shader, wgpu::TextureFormat::Rgba16Float,
                                Some(wgpu::BlendState::REPLACE), None)
                        });
                        let world = scope.spawn(|| world_pipeline(false));
                        let backstop = scope.spawn(|| world_pipeline(true));
                        let lines = scope.spawn(|| {
                            (
                                line_pipeline("line pipeline", wgpu::BlendState::ALPHA_BLENDING, wgpu::CompareFunction::GreaterEqual),
                                line_pipeline("line invert pipeline", invert_blend, wgpu::CompareFunction::GreaterEqual),
                                line_pipeline("survey line pipeline", wgpu::BlendState::ALPHA_BLENDING, wgpu::CompareFunction::Always),
                            )
                        });
                        let ui = scope.spawn(ui_pipeline);
                        let shadow = scope.spawn(|| shadow::ShadowRenderer::new(device, shadow_shader, pipeline_cache));
                        let failed = "pipeline creation thread panicked";
                        (
                            cloud.join().expect(failed),
                            sky.join().expect(failed),
                            composite.join().expect(failed),
                            temporal.join().expect(failed),
                            world.join().expect(failed),
                            backstop.join().expect(failed),
                            lines.join().expect(failed),
                            ui.join().expect(failed),
                            shadow.join().expect(failed),
                            scene_passes.join().expect(failed),
                        )
                    });
                // Every pipeline that takes the cache exists now (the atmosphere's
                // were built in `CloudFields`).
                if let Some(cache) = &bootstrap.pipeline_cache {
                    cache.save();
                }
                self.scene_passes = Some(scene_passes);
                self.cloud_pipeline = Some(cloud);
                self.sky_pipeline = Some(sky);
                self.cloud_composite_pipeline = Some(composite);
                self.cloud_temporal_pipeline = Some(temporal);
                self.world_pipeline = Some(world);
                self.terrain_backstop = Some(backstop);
                self.line_pipeline = Some(line);
                self.line_invert_pipeline = Some(line_invert);
                self.survey_line_pipeline = Some(survey_line);
                self.ui_pipeline = Some(ui);
                self.shadow_renderer = Some(shadow);

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
                self.survey_outline_stream = Some(DynamicVertexBuffer::new::<LineVertex>(
                    device,
                    "survey outline stream",
                    64,
                ));
                self.ui_stream = Some(DynamicVertexBuffer::new::<UiVertex>(
                    device,
                    "ui stream",
                    1024,
                ));

                let horizon_detail = horizon::detail::Gpu::new(device);
                let world_surface_bind_group = Gpu::create_world_surface_bind_group(
                    device,
                    self.world_surface_layout.as_ref().unwrap(),
                    &self.shadow_renderer.as_ref().unwrap().uniform_buffer,
                    &self.shadow_renderer.as_ref().unwrap().depth_view,
                    &self.shadow_renderer.as_ref().unwrap().depth_sampler,
                    self.cloud_shadow_view.as_ref().unwrap(),
                    self.cloud_field_sampler.as_ref().unwrap(),
                    &horizon_detail,
                    self.terrain_backstop_depth.as_ref().unwrap(),
                );
                self.world_surface_bind_group = Some(world_surface_bind_group);
                self.horizon_detail = Some(horizon_detail);

                // Do not recurse into self.step() here. Keep the staged init explicit and
                // let the outer bootstrap loop call step() again on the next tick.
                self.stage = GpuInitStage::Complete;
                Ok(None)
            }

            GpuInitStage::Complete => {
                let bootstrap = self.bootstrap.take().expect("bootstrap missing");
                let gpu_profiler = GpuProfiler::new(&bootstrap.device, &bootstrap.queue,
                    Arc::clone(&bootstrap.device_lost));
                // Built beside the other pipelines; rebuilt only if the window
                // or render resolution changed since, as their targets are sized.
                let (postprocess, indirect) = match self.scene_passes.take() {
                    Some((key, postprocess, indirect)) if key == scene_pass_key(&bootstrap.config) => {
                        (postprocess, indirect)
                    }
                    _ => {
                        let pipeline_cache = bootstrap.pipeline_cache.as_ref().map(pipeline_cache::DiskCache::cache);
                        let postprocess = postprocess::PostProcess::new(
                            &bootstrap.device,
                            &bootstrap.config,
                            self.blue_noise_view.as_ref().unwrap(),
                            pipeline_cache,
                        );
                        let indirect = indirect::IndirectLighting::new(
                            &bootstrap.device,
                            &scene_config(&bootstrap.config),
                            self.camera_layout.as_ref().unwrap(),
                            self.depth.as_ref().unwrap(),
                            &postprocess.scene,
                            pipeline_cache,
                        );
                        (postprocess, indirect)
                    }
                };

                let initial_cloud_scale = cloud_scale_request(bootstrap.config.width, bootstrap.config.height);
                let portal_stage = portals::Stage::new(&bootstrap.device, self.camera_layout.clone().unwrap());
                let mut gpu = Gpu {
                    portals: portal_stage,
                    camera_medium: CameraMedium::Air,
                    water_level_m: 0.0,
                    screen_magic: [0.0; 4],
            motion_boost: 0.0,
            camera_roll: 0.0,
            camera_up: Vec3::Y,
            finite_world: false,
            realm_sky: false,
            manyfold_axis_y: 0.0,
                    zone_grade: crate::world::zones::NO_GRADE,
                    postprocess,
                    indirect,
                    fxaa_enabled: true,
                    globe: globe::active().then(|| globe::gpu::Globe::new(&bootstrap.device, postprocess::SCENE_FORMAT)),
                    // Task 8 drives this from the altitude tier.
                    globe_visible: globe::active(),
                    globe_driver: globe_driver::GlobeDriver::default(),
                    globe_clock: std::time::Instant::now(),
                    tier: crate::altitude::Tier::Surface,
                    globe_disc_radius_m: 0.0,
                    horizon_globe_only: false,
                    surface: bootstrap.surface,
                    device: bootstrap.device,
                    queue: bootstrap.queue,
                    device_lost: bootstrap.device_lost,
                    upload_staging: Arc::default(),
                    chunk_upload_credits: Default::default(),
                    frame_pacer: bootstrap.frame_pacer,
                    unthrottled_present_mode: bootstrap.unthrottled_present_mode,
                    config: bootstrap.config,
                    size: bootstrap.size,
                    adapter_summary: bootstrap.adapter_summary,

                    camera_buffer: self.camera_buffer.take().unwrap(),
                    camera_bind_group: self.camera_bind_group.take().unwrap(),

                    lighting_buffer: self.lighting_buffer.take().unwrap(),
                    lighting_bind_group: self.lighting_bind_group.take().unwrap(),
                    torch_blockage: self.torch_blockage.take().unwrap(),
                    sky_volume: self.sky_volume.take().unwrap(),
                    atmosphere: self.atmosphere_runtime.take().unwrap(),

                    shadow_renderer: self.shadow_renderer.take().unwrap(),
                    chunk_indirect: Default::default(),
                    draw_offsets: Default::default(),
                    render_origin: None,
                    horizon_detail: self.horizon_detail.take().unwrap(),
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
                    terrain_backstop: self.terrain_backstop.take().unwrap(),
                    horizon: horizon::Horizon::default(),
                    planet_mesh: None,
                    outgoing_planet_mesh: None,
                    terrain_arrival: coverage::TerrainArrival::default(),
                    planet_progress: None,
                    planet_build: None,
                    scene_stream: None,
                    posed_assets: None,
                    scene_vertices: 0,
                    line_pipeline: self.line_pipeline.take().unwrap(),
                    line_invert_pipeline: self.line_invert_pipeline.take().unwrap(),
                    survey_line_pipeline: self.survey_line_pipeline.take().unwrap(),
                    ui_pipeline: self.ui_pipeline.take().unwrap(),

                    outline_stream: self.outline_stream.take().unwrap(),
                    invert_outline_stream: self.invert_outline_stream.take().unwrap(),
                    survey_outline_stream: self.survey_outline_stream.take().unwrap(),
                    ui_stream: self.ui_stream.take().unwrap(),

                    depth: self.depth.take().unwrap(),
                    terrain_backstop_depth: self.terrain_backstop_depth.take().unwrap(),

                    clear_color: bootstrap.clear_color,
                    current_lighting: bootstrap.default_lighting,
                    current_debug_view: RenderDebugView::None,
                    last_camera_pos: Vec3::ZERO,
                    last_camera_residual: Vec3::ZERO,
                    planet_eye: None,
                    last_rotation_view_proj: Mat4::IDENTITY,
                    last_camera_forward: Vec3::NEG_Z,
                    last_world_scale: 1.0,

                    // History is invalid until the first resolved cloud frame is produced.
                    cloud_history_valid: false,
                    cloud_render_scale: initial_cloud_scale,
                    cloud_governor: cloud_governor::CloudGovernor::default(),
                    last_frame_instant: Instant::now(),
                    frame_index: 0,
                    render_frame_id: 0,
                    effect_time: 0.0,
                    cloud_history_frame_stats: CloudHistoryFrameStats::default(),
                    last_weather_upload_stats: WeatherUploadStats::default(),
                    last_render_stats: RenderFrameStats::default(),
                    capture_path: None,
                    pending_captures: Vec::new(),
                    gpu_profiler,
                };

                // If the window was resized during startup, textures created early
                // in the pipeline (like depth and clouds) may have the old size.
                // Rebuilding all targets at the same size is especially expensive:
                // wgpu keeps the old resources alive until their last GPU use has
                // retired, so an unconditional resize can briefly double VRAM and
                // trigger a D3D12 out-of-memory failure during startup.
                let final_size = gpu.size;
                let configured_size = PhysicalSize::new(gpu.config.width, gpu.config.height);
                if configured_size != final_size {
                    gpu.resize(final_size);
                }

                Ok(Some(gpu))
            }
        }
    }
}
impl Gpu {
    pub fn device_usable(&self) -> bool {
        !self.device_lost.load(Ordering::Acquire)
    }

    pub fn upload_scene(&mut self, vertices: &[Vertex3]) {
        self.scene_vertices = vertices.len();
        let posed = self.posed_assets.get_or_insert_with(|| posed_assets::PosedAssets::new(&self.device));
        crate::mesh::models::posed::with_frame(|placements, pool, generation| {
            posed.upload(&self.device, &self.queue, placements, pool, generation)
        });
        if vertices.is_empty() {
            return;
        }
        let stream = self.scene_stream.get_or_insert_with(|| {
            DynamicVertexBuffer::new::<Vertex3>(
                &self.device,
                "animated scene models",
                vertices.len(),
            )
        });
        stream.upload(&self.device, &self.queue, vertices);
    }
    pub fn set_zone_grade(&mut self, grade: ([f32; 4], [f32; 4])) {
        self.zone_grade = grade;
    }

    /// Dash / slide screen boost, 0..1 (speed streaks and edge smear).
    pub fn set_motion_boost(&mut self, boost: f32) {
        self.motion_boost = if boost.is_finite() { boost.clamp(0.0, 1.0) } else { 0.0 };
    }

    /// View roll in radians (wall-run lean). Rendering only; culling stays upright.
    pub fn set_camera_up(&mut self, up: Vec3) {
        self.camera_up = up.try_normalize().unwrap_or(Vec3::Y);
    }

    pub fn set_camera_roll(&mut self, roll: f32) {
        self.camera_roll = if roll.is_finite() { roll.clamp(-0.5, 0.5) } else { 0.0 };
    }

    pub fn set_screen_magic(&mut self, intensity: f32, color: [f32; 3]) {
        self.screen_magic = [
            color[0].clamp(0.0, 2.0),
            color[1].clamp(0.0, 2.0),
            color[2].clamp(0.0, 2.0),
            intensity.clamp(0.0, 1.0),
        ];
    }

    pub fn vsync(&self) -> bool {
        self.config.present_mode == wgpu::PresentMode::AutoVsync
    }

    pub fn set_vsync(&mut self, enabled: bool) {
        if self.vsync() != enabled {
            self.config.present_mode = if enabled {
                wgpu::PresentMode::AutoVsync
            } else {
                self.unthrottled_present_mode
            };
            println!("[present] mode={:?}", self.config.present_mode);
            self.surface.configure(&self.device, &self.config);
            self.frame_pacer.reset();
        }
    }

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

    fn create_world_surface_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        shadow_uniform: &wgpu::Buffer,
        shadow_depth: &wgpu::TextureView,
        shadow_sampler: &wgpu::Sampler,
        cloud_shadow: &wgpu::TextureView,
        cloud_field_sampler: &wgpu::Sampler,
        horizon_detail: &horizon::detail::Gpu,
        terrain_backstop_depth: &wgpu::TextureView,
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
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&horizon_detail.view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: horizon_detail.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(terrain_backstop_depth),
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
                    width: scene_dims(config.width, config.height).0,
                    height: scene_dims(config.width, config.height).1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn create_terrain_backstop_depth(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> wgpu::TextureView {
        let (width, height) = scene_dims(config.width, config.height);
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("terrain backstop scene depth"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Create a cloud render texture.
    /// Base usages are always:
    ///     - RENDER_ATTACHMENT
    ///     - TEXTURE_BINDING
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
                width: scene_dims(config.width, config.height).0
                    .div_ceil(effective_cloud_render_scale(
                        scene_dims(config.width, config.height).0,
                        scene_dims(config.width, config.height).1,
                    ))
                    .max(1),
                height: scene_dims(config.width, config.height).1
                    .div_ceil(effective_cloud_render_scale(
                        scene_dims(config.width, config.height).0,
                        scene_dims(config.width, config.height).1,
                    ))
                    .max(1),
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
        camera: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cloud bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: camera.as_entire_binding(),
                },
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

    /// Upload pre-computed shape noise data (128³, RGBA8) to a GPU texture.
    fn create_shape_noise_3d(device: &wgpu::Device, queue: &wgpu::Queue, data: Vec<u8>) -> wgpu::TextureView {
        cloud_noise::upload(device, queue, data, 64).create_view(&Default::default())
    }

    /// Upload pre-computed detail noise data (32³, RGBA8) with filtered mips to a GPU texture.
    fn create_detail_noise_3d(device: &wgpu::Device, queue: &wgpu::Queue, data: Vec<u8>) -> wgpu::TextureView {
        cloud_noise::upload(device, queue, data, 32).create_view(&Default::default())
    }

    fn create_blue_noise_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
        // Hash noise with its low frequencies removed, rank mapped to uniform R8.
        // The former 64px interleaved-gradient lattice was visible on distant
        // cloud bases. This larger tile has no shared diagonal phase.
        let size = 256u32;
        let total = (size * size) as usize;
        let hash = |mut v: u32| {
            v ^= v >> 16;
            v = v.wrapping_mul(0x7feb352d);
            v ^= v >> 15;
            v = v.wrapping_mul(0x846ca68b);
            ((v ^ (v >> 16)) as f64 / u32::MAX as f64) as f32
        };
        let raw: Vec<f32> = (0..total).map(|i| hash(i as u32 ^ 0xa1539e27)).collect();
        let mut ranked: Vec<(usize, f32)> = (0..total)
            .map(|i| {
                let x = i as u32 % size;
                let y = i as u32 / size;
                let at = |dx: i32, dy: i32| {
                    raw[(((y as i32 + dy).rem_euclid(size as i32) as u32) * size
                        + (x as i32 + dx).rem_euclid(size as i32) as u32)
                        as usize]
                };
                (
                    i,
                    raw[i] - (at(-1, 0) + at(1, 0) + at(0, -1) + at(0, 1)) * 0.25,
                )
            })
            .collect();
        ranked.sort_unstable_by(|a, b| a.1.total_cmp(&b.1));
        let mut data = vec![0u8; total];
        for (rank, (index, _)) in ranked.into_iter().enumerate() {
            data[index] = (rank * 256 / total) as u8;
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
        self.frame_pacer.reset();
        self.resize_scene_targets();
    }

    fn resize_scene_targets(&mut self) {
        let (scene_width, scene_height) = scene_dims(self.config.width, self.config.height);
        self.atmosphere.resize(&self.queue, scene_width, scene_height);
        self.depth = Self::create_depth(&self.device, &self.config);
        self.terrain_backstop_depth = Self::create_terrain_backstop_depth(&self.device, &self.config);
        self.world_surface_bind_group = Self::create_world_surface_bind_group(
            &self.device,
            &self.world_pipeline.get_bind_group_layout(3),
            &self.shadow_renderer.uniform_buffer,
            &self.shadow_renderer.depth_view,
            &self.shadow_renderer.depth_sampler,
            &self.cloud_shadow_view,
            &self.cloud_field_sampler,
            &self.horizon_detail,
            &self.terrain_backstop_depth,
        );
        self.postprocess.resize(&self.device, &self.config);
        self.indirect.resize(
            &self.device,
            &scene_config(&self.config),
            &self.depth,
            &self.postprocess.scene,
        );
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
        self.resize_cloud_targets();
    }

    /// A cloud quality adjustment does not resize terrain, depth, atmosphere,
    /// postprocessing or the swapchain. Keep those allocations and bindings.
    fn resize_cloud_targets(&mut self) {
        self.cloud_render_scale = cloud_scale_request(self.size.width, self.size.height);
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
            &self.camera_buffer,
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

    /// Portal graph drawn as windows this frame; cloned only on a new revision.
    pub fn set_portals(&mut self, network: &crate::world::portals::Network, revision: u64,
        realm_gates: Vec<crate::world::portals::realm::Gate>) {
        self.portals.set_network(network, revision);
        self.portals.set_realm_gates(realm_gates);
    }

    pub fn request_acceptance_capture(&mut self, path: std::path::PathBuf) {
        capture::watch(path.clone());
        self.request_capture(path);
    }

    pub fn horizon_budget_exhausted(&self, world: &crate::world::World) -> bool { self.horizon.budget_exhausted_for(world) }

    pub fn request_capture(&mut self, path: std::path::PathBuf) {
        self.capture_path = Some(path);
    }

    pub fn set_tier(&mut self, tier: crate::altitude::Tier) {
        self.globe_driver.set_tier(tier);
        self.tier = tier;
    }

    /// Drives the globe for this frame; `projection` is the conventional terrain projection.
    pub fn update_globe(&mut self, world: &crate::world::World, day: &crate::world::DayNightCycle, eye_m: glam::DVec3, forward: Vec3, projection: Mat4, focal_px: f32) {
        if !self.device_usable() { return; }
        let Some(globe) = self.globe.as_mut() else { return };
        let view = globe_driver::GlobeView {
            eye_m,
            forward,
            projection: REVERSE_Z * projection,
            sun_dir: self.current_lighting.sun_dir,
            moon_dir: self.current_lighting.moon_dir,
            moon_intensity: self.current_lighting.moon_intensity,
            focal_px,
            sky_time_s: self.effect_time,
            clouds: day.cloud_controls(),
            weather_seed: day.weather_seed(),
            weather_days: crate::world::synoptic::days(day),
            clock_s: self.globe_clock.elapsed().as_secs_f64(),
            debug_view: self.current_debug_view.as_u32(),
        };
        let clusters = || globe::shell::factory_clusters(world);
        self.globe_driver.update(globe, &self.queue, world.sample_config(), &crate::world::storage::save_directory, &view, &clusters);
        if self.globe_driver.running() {
            let chunk_m = CHUNK_SIZE as f32 * world.voxel_scale.meters();
            // Fade only over a horizon which will actually be submitted. A
            // pending/disabled horizon, or a planet cover replaced by this
            // globe, cannot fill the transparent band beneath it.
            let horizon_drawn = !self.horizon_globe_only && !self.globe_driver.orbit_view()
                && self.planet_mesh.is_none() && self.horizon.mesh.is_some()
                && self.horizon.published_source() == Some(world.sample_config())
                && !render_isolation().skip_horizon;
            let radius = if horizon_drawn { self.globe_disc_radius_m } else { 0.0 };
            let mask = globe::band::GlobeMask::new(*self.horizon.resident_coverage(), chunk_m, radius, eye_m);
            globe.set_mask(&self.queue, &mask);
        }
    }

    pub fn globe_stats(&self) -> globe_driver::GlobeStats {
        self.globe_driver.stats()
    }

    pub fn update_horizon(
        &mut self,
        world: &mut crate::world::World,
        eye: Vec3,
        coverage_radius: f32,
        resident: coverage::ResidentCoverage,
        desired_resident: coverage::ResidentCoverage,
        cache:&streaming::ChunkCache,
        fov_degrees:f32,
        fast_start: bool,
    ) {
        let cfg = world.sample_config();
        self.terrain_arrival = coverage::TerrainArrival::default();
        self.finite_world = cfg.world_type == crate::world::WorldType::Manyfold;
        self.realm_sky = cfg.world_type == crate::world::WorldType::Nether;
        self.manyfold_axis_y = if self.finite_world {
            engine_worldgen::manyfold::axis_y(cfg.generator_version)
        } else {
            0.0
        };
        if self.outgoing_planet_mesh.as_ref().is_some_and(|(mesh, started)| {
            mesh.cfg != cfg || lod_transition_phase(*started, Instant::now()).is_none()
        }) {
            self.outgoing_planet_mesh = None;
        }
        if self.finite_world {
            self.planet_mesh = None;
            if let Some(job) = self.planet_build.take() { job.cancel.store(true, Ordering::Relaxed); }
            self.horizon.clear_mesh();
            return;
        }
        if render_isolation().skip_horizon_work {
            return;
        }
        // Both planet covers and ordinary horizons join the installed perimeter.
        // Capture the same publication proof before selecting either world path.
        let root=&world.regions.view.root;
        if !self.horizon.collar_observation_current(cache.gpu_mesh_revision,cfg,root,&resident) {
            let input=cache.collar_input(self.horizon.current_collar(),&resident);
            self.horizon.set_collar_observation(input,cache.gpu_mesh_revision,cfg,Arc::clone(root),resident);
        }
        if cfg.is_planet() {
            // `eye` arrives in metres. Check the target before publishing a
            // worker result, since a teleport can change it mid-build.
            let anchor = planet::anchor_for(&cfg, eye);
            if self.planet_mesh.as_ref().is_some_and(|mesh| mesh.cfg != cfg) { self.planet_mesh = None; }
            if let Some(job) = &self.planet_build && (job.cfg != cfg || job.anchor != anchor) {
                job.cancel.store(true, Ordering::Relaxed);
            }
            if self.planet_build.as_ref().is_some_and(|job| job.cfg != cfg) { self.planet_build = None; }
            // The shell is not a horizon mesh, but it uses the same residency
            // mask. Without this the cover never yields, so its facets stay
            // drawn through the ground after the player has arrived.
            self.horizon.set_resident_coverage(resident, desired_resident);
            // Both stages arrive as complete mapped buffers. Keep the previous
            // cover until the replacement is ready to crossfade.
            let mut published_startup = false;
            if self.outgoing_planet_mesh.is_none() {
                let ready = if self.planet_build.as_ref().is_some_and(|job| job.worker.is_finished()) {
                    self.planet_build.take().and_then(|job| job.worker.join().ok().flatten())
                } else {
                    self.planet_build.as_ref().filter(|job| !job.cancel.load(Ordering::Relaxed))
                        .and_then(|job| job.terrain.try_recv().ok().map(|mesh| {
                            eprintln!("[planet-cover-publish] stage=terrain anchor={:?} elapsed_ms={:.1}",
                                job.anchor, job.started.elapsed().as_secs_f64() * 1000.0);
                            mesh
                        }))
                };
                if let Some(mesh) = ready && mesh.cfg == cfg && mesh.anchor == anchor {
                    published_startup = !mesh.refined;
                    self.outgoing_planet_mesh = self.planet_mesh.replace(mesh)
                        .map(|previous| (previous, Instant::now()));
                }
            }
            let input = self.horizon.current_collar().cloned();
            let stale = self.planet_mesh.as_ref().is_none_or(|mesh| !mesh.refined || mesh.anchor != anchor
                || (input.is_some() && self.horizon.collar_settled(Instant::now())
                    && !mesh.matches_collar(input.as_ref())));
            if stale && !published_startup && self.planet_build.is_none() {
                let startup = fast_start && self.planet_mesh.is_none()
                    && engine_worldgen::planetary::active(&cfg);
                let source = self.planet_mesh.as_ref().filter(|m| m.refined && m.anchor == anchor)
                    .map(|m| Arc::clone(&m.source));
                let shells = self.planet_mesh.as_ref().map(|m| m.shells.clone()).unwrap_or_default();
                let device = self.device.clone();
                let progress = Arc::new(planet::progress::Progress::new(6 + u32::from(anchor.is_some())));
                self.planet_progress = Some(Arc::clone(&progress));
                let (send_terrain, terrain) = std::sync::mpsc::sync_channel(1);
                let cancel = Arc::new(AtomicBool::new(false));
                let cancel_worker = Arc::clone(&cancel);
                let started = Instant::now();
                let worker = std::thread::spawn(move || {
                    let stage = if startup { "startup" } else if source.is_some() { "join" } else { "refined" };
                    let terrain_ready = |cpu: planet::Cpu| {
                        if cancel_worker.load(Ordering::Relaxed) { return; }
                        let terrain = planet::Mesh::upload(&device, cpu);
                        let _ = send_terrain.try_send(terrain);
                    };
                    let mesh = if startup { planet::build_startup_with_progress(cfg, anchor, Some(&progress)) }
                        else { planet::build_with_progress(cfg, anchor, input, source, shells, Some(&progress),
                            Some(planet::Streaming { terrain_ready: &terrain_ready, cancel: &cancel_worker })) };
                    if cancel_worker.load(Ordering::Relaxed) { return None; }
                    eprintln!("[planet-cover] stage={stage} anchor={anchor:?} triangles={} build_ms={:.1}",
                        mesh.indices.len() / 3, started.elapsed().as_secs_f64() * 1000.0);
                    progress.begin("Terrain Upload", 0);
                    Some(planet::Mesh::upload(&device, mesh))
                });
                self.planet_build = Some(planet::CoverJob { cfg, anchor, cancel, started, terrain, worker });
            }
            let globe_drawn = self.globe.is_some() && self.globe_visible && self.globe_driver.ground_ready_for(cfg);
            if !globe_drawn && self.planet_mesh.is_some() {
                self.terrain_arrival = cache.terrain_arrival(&resident, |_| true);
            }
            return;
        }
        self.planet_mesh = None;
        if let Some(job) = self.planet_build.take() { job.cancel.store(true, Ordering::Relaxed); }
        self.horizon.set_device_loss_signal(&self.device_lost);
        if !self.device_usable() || crate::low_spec::active() { return; }
        self.horizon.set_refinement_projection(self.config.height as f32/(2.0*(fov_degrees.to_radians()*0.5).tan()));
        self.horizon.set_resident_coverage(resident, desired_resident);
        let scale = world.voxel_scale.meters();
        let revision = world.revision();
        let visible_world =
            render_distance_settings().view_distance_chunks as f32 * CHUNK_SIZE as f32 * scale;
        // The horizon may only hide where resident GPU coverage is actually
        // published. A fixed fraction of the requested radius left a gap when
        // streaming had not reached that fraction yet.
        let hidden_radius = if coverage_radius.is_finite() {
            coverage_radius.clamp(0.0, visible_world)
        } else {
            0.0
        };
        self.globe_disc_radius_m = hidden_radius;
        if globe::active() {
            let (clip, keep) = globe::band::horizon_clip(self.tier, eye.as_dvec3());
            let globe_ready = self.globe_visible && self.globe_driver.ground_ready_for(cfg);
            // At a chart edge the neighbouring part of the horizon is the
            // only cover until the current world's globe tiles can draw.
            self.horizon.set_chart_clip(if globe_ready { clip } else { None });
            self.horizon_globe_only = !keep && globe_ready;
            if self.horizon_globe_only {
                // Above the load stop the globe alone draws the ground.
                self.horizon.clear_mesh();
                return;
            }
        }
        self.horizon.update(
            &self.device,
            &self.queue,
            world,
            revision,
            eye,
            hidden_radius,
            fast_start,
        );
        if !render_isolation().skip_horizon && !(self.globe_visible && self.globe_driver.orbit_view()) {
            self.terrain_arrival = cache.terrain_arrival(&resident, |column| self.horizon.covers_streaming_column(cfg, column));
        }
        if let Some(update) = self.horizon.take_detail_update() {
            self.horizon_detail.apply(&self.queue, update.as_deref());
        }
    }
    pub fn horizon_quality_columns(&mut self,cfg:crate::world::WorldSampleConfig,eye:Vec3,forward:Vec3,fov_degrees:f32)->Vec<(i32,i32)> {
        if render_isolation().skip_horizon_work { return Vec::new(); }
        let focal=self.config.height as f32/(2.0*(fov_degrees.to_radians()*0.5).tan());
        let ordinary_lod0_m=render_distance_settings().lod_start_chunks as f32*32.0*cfg.voxel_meters();
        self.horizon.quality_columns(cfg,eye,forward,focal,[self.config.width,self.config.height],ordinary_lod0_m)
    }
    pub fn horizon_quality_parent_valid(&self,snapshot:&crate::world::WorldSnapshot,eye:Vec3,columns:&[(i32,i32)])->bool {
        self.horizon.quality_parent_valid(snapshot,eye,columns)
    }
    pub fn horizon_retention_columns(&self) -> Vec<(i32,i32)> { self.horizon.retention_columns() }
    pub fn planet_loading_progress(&self, cfg: crate::world::WorldSampleConfig) -> Option<(&'static str, u64, u64)> {
        let job = self.planet_build.as_ref()?;
        if job.cfg != cfg || job.cancel.load(Ordering::Relaxed) { return None; }
        if job.worker.is_finished() { return Some(("Terrain Publication", 0, 0)); }
        let p = self.planet_progress.as_ref()?.snapshot();
        Some((p.name, p.done, p.total))
    }

    pub fn horizon_work_stage(&self) -> &'static str { self.horizon.work_stage() }
    pub fn horizon_upload_pages(&self) -> Option<(usize, usize)> { self.horizon.upload_pages() }
    pub fn horizon_building_base(&self) -> bool { self.horizon.building_base() }
    pub fn globe_enabled(&self) -> bool { self.globe.is_some() }
    pub fn horizon_published_source(&self) -> Option<crate::world::WorldSampleConfig> { self.horizon.published_source() }
    pub fn horizon_published_revision(&self) -> Option<u64> { self.horizon.published_revision() }
    pub fn horizon_baked_columns(&self, columns:&[(i32,i32)]) -> std::collections::HashSet<(i32,i32)> { self.horizon.baked_columns(columns) }

    pub fn horizon_ready(&self) -> bool {
        // Minimum-spec mode deliberately disables horizon generation. Waiting
        // for its mesh there leaves an otherwise loaded world behind the loader.
        let resident = usize::from(self.globe_driver.ground_ready());
        self.finite_world || crate::low_spec::active() || self.planet_mesh.is_some() || render_isolation().skip_horizon_work
            || globe::band::horizon_ready(globe::active(), self.tier, self.horizon.mesh.is_some(), resident)
    }

    pub fn planet_detail_ready(&self) -> bool {
        self.planet_mesh.as_ref().is_none_or(|mesh| mesh.refined
            && self.planet_build.as_ref().is_none_or(|job| job.anchor == mesh.anchor))
    }

    /// True once the current world has a far-field mesh. A later rebuild
    /// (edits, walking) must not show the terrain loader over gameplay.
    pub fn horizon_loaded_this_world(&self) -> bool {
        self.finite_world || self.horizon.loaded_this_world() || self.planet_mesh.is_some() || render_isolation().skip_horizon_work
    }

    pub fn upload_camera(
        &mut self,
        precise_eye: glam::DVec3,
        world_scale: f32,
        forward: Vec3,
        projection: Mat4,
        world_seed: u32,
        generator_version: u32,
        planet_tick: u64,
        coverage_radius: f32,
    ) {
        self.horizon.set_refinement_projection(self.config.height as f32 * projection.y_axis.y.abs() * 0.5);
        // Invert only the rotation/projection. Inverting the translated f32 matrix
        // makes rays and motion estimates increasingly unstable away from the origin.
        // Capture-time A/B uses the same executable and shader pipeline.
        static MIST_DISABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let mist_disabled = *MIST_DISABLED.get_or_init(|| {
            let disabled = std::env::var("VOXEL_LOCAL_MIST").as_deref() == Ok("0");
            eprintln!("[mist] analytic_enabled={}", !disabled);
            disabled
        });
        let eye = precise_eye.as_vec3();
        let residual = (precise_eye - eye.as_dvec3()).as_vec3();
        let eye_voxels = precise_eye / f64::from(world_scale);
        let render_origin = RenderOrigin::follow(self.render_origin, eye_voxels, origin::origin_bias_voxels());
        self.render_origin = Some(render_origin);
        let eye_from_origin = render_origin.eye_uniform(precise_eye, f64::from(world_scale), residual);
        self.planet_eye = self.planet_mesh.is_some()
            .then(|| planet_frame::Eye::new(precise_eye, planet_frame::far_m(f64::from(world_scale))));
        let cur_forward = forward.normalize();
        // Callers supply a conventional 0 = near, 1 = far projection. The scene is
        // drawn reverse-Z into a 32-bit float depth buffer: 24-bit forward-Z gave
        // steps of ~1 m at 1 km and ~76 m at 8 km, so distant shoreline terrain and
        // water z-fought. Shaders that reconstruct depth read the reversed matrix.
        // Camera roll (wall run lean) tilts only the up vector.
        let roll = self.camera_roll;
        let up_axis = if roll.abs() > 1e-5 {
            glam::Quat::from_axis_angle(cur_forward, roll) * self.camera_up
        } else {
            self.camera_up
        };
        let standard_view_proj = projection
            * glam::camera::rh::view::look_to_mat4(Vec3::ZERO, cur_forward, up_axis)
            * Mat4::from_translation(-residual);
        let standard_inv_view_proj = Mat4::from_translation(eye) * standard_view_proj.inverse();
        // Cascades fit around the precise eye; f32 absolute corners lose
        // centimetres far from spawn.
        let cascade_view = shadow::CascadeView {
            inv_view_proj: (projection * glam::camera::rh::view::look_to_mat4(Vec3::ZERO, cur_forward, up_axis)).inverse(),
            eye_m: precise_eye,
            origin: render_origin,
        };
        let projection = REVERSE_Z * projection;
        let rotation_view_proj = REVERSE_Z * standard_view_proj;
        // REVERSE_Z is its own inverse, so the reversed inverse needs no second inversion.
        let inv_view_proj = standard_inv_view_proj * REVERSE_Z;
        let right = cur_forward.cross(up_axis).normalize() / projection.x_axis.x;
        let up = right.normalize().cross(cur_forward) / projection.y_axis.y;
        let prev_forward = self.last_camera_forward;

        let angular_motion = (1.0 - cur_forward.dot(prev_forward).clamp(-1.0, 1.0)).clamp(0.0, 1.0);
        let translation_delta =
            (eye - self.last_camera_pos) + (residual - self.last_camera_residual);
        let translation_motion = (translation_delta.length() / 8.0).clamp(0.0, 1.0);
        let camera_motion = (angular_motion * 6.0 + translation_motion * 0.25).clamp(0.0, 1.0);

        if CLOUD_TEMPORAL_ENABLED && camera_motion > 0.28 {
            self.invalidate_cloud_history_with_reason(CloudHistoryInvalidationReason::CameraMotion);
        }

        let render_settings = render_distance_settings();
        let visible_world =
            render_settings.view_distance_chunks as f32 * CHUNK_SIZE as f32 * world_scale;

        self.frame_index = self.frame_index.wrapping_add(1);

        let uniform = CameraUniform {
                // Terrain and outlines subtract the eye before projection.
                view_proj: rotation_view_proj.to_cols_array_2d(),
                camera_pos: [eye.x, eye.y, eye.z, self.camera_medium.uniform()],
                inv_view_proj: inv_view_proj.to_cols_array_2d(),
                world_params: [
                    world_scale,
                    world_scale.recip(),
                    camera_motion,
                    visible_world,
                ],
                scene_seed: [
                    world_seed,
                    generator_version,
                    u32::from(mist_disabled),
                    u32::from(!shadow::enabled()),
                ],
                // x: resident coverage radius; y: sea level in metres; z: radius of
                // the full-detail (LOD 0) ring in metres. The caustic spill and
                // the water waves read these without another uniform slot.
                coverage: [
                    coverage_radius,
                    self.water_level_m,
                    if render_settings.lod_levels > 1 {
                        render_settings.lod_start_chunks as f32 * CHUNK_SIZE as f32 * world_scale
                    } else {
                        visible_world
                    },
                    // Planet cover yields only to published resident columns.
                    if self.planet_eye.is_some() {
                        0.0
                    } else {
                        self.horizon.exclusion_radius()
                    },
                ],
                resident_coverage: *self.horizon.resident_coverage(),
                // Previous projection has no translation; temporal uses camera deltas.
                prev_view_proj: self.last_rotation_view_proj.to_cols_array_2d(),
                camera_forward: [
                    cur_forward.x,
                    cur_forward.y,
                    cur_forward.z,
                    self.frame_index as f32,
                ],
                ray_right: [right.x, right.y, right.z, projection.z_axis.z],
                ray_up: [up.x, up.y, up.z, projection.w_axis.z],
                previous_camera_pos: [
                    self.last_camera_pos.x,
                    self.last_camera_pos.y,
                    self.last_camera_pos.z,
                    self.horizon.visibility(),
                ],
                render_origin: render_origin.voxel.extend(0).to_array(),
                eye_from_origin: eye_from_origin.extend(0.0).to_array(),
                planet: {
                    let mesh_planet = self.planet_mesh.is_some();
                    let globe_sky = self.globe.is_some() && self.globe_visible && self.globe_driver.running();
                    // The globe needs the same eye/body frame for its limb even
                    // when no legacy planet cover mesh is enabled. Keep the mesh
                    // flag separate so this does not change terrain projection.
                    let mut planet = crate::planet::camera(precise_eye, mesh_planet || globe_sky, planet_tick);
                    if !mesh_planet { planet[0][3] = 0.0; }
                    if self.realm_sky {
                        planet[0][3] = if self.current_lighting.hell { 2.0 } else { -1.0 };
                        if self.current_lighting.hell {
                            // Hell's moons need the observer frame even when
                            // the distant globe is inactive at ground level.
                            let frame = engine_core::cube_sphere::atlas::frame(precise_eye);
                            planet[1][3] = precise_eye.y as f32;
                            planet[2] = frame.x_axis.as_vec3().extend(0.0).to_array();
                            planet[3] = frame.y_axis.as_vec3().extend(0.0).to_array();
                            planet[4] = frame.z_axis.as_vec3().extend(0.0).to_array();
                            planet[5..8].copy_from_slice(&crate::planet::hell_moons::sky(world_seed, precise_eye, planet_tick));
                        }
                    }
                    planet
                },
                portal_clip: [0.0; 4],
                planet_flat: self.planet_eye.as_ref().map_or([[0.0; 4]; 3], planet_frame::Eye::shader_rows),
                terrain_arrival: self.terrain_arrival,
                sky_axes: if self.planet_mesh.is_some()
                    || (self.globe.is_some() && self.globe_visible && self.globe_driver.running()) {
                    crate::planet::sky_axes(precise_eye, true, planet_tick)
                } else {
                    crate::planet::flat_sky_axes(self.current_lighting.sun_dir)
                },
            };
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        self.portals.set_root(uniform, precise_eye, cur_forward, up_axis, projection, world_scale, render_origin);

        let hard_cut = translation_delta.length() > 24.0 * world_scale.max(1.0)
            || angular_motion > 0.18
            || (world_scale - self.last_world_scale).abs() > 1e-4;

        if CLOUD_TEMPORAL_ENABLED && hard_cut {
            self.invalidate_cloud_history_with_reason(CloudHistoryInvalidationReason::HardCut);
        }

        self.last_camera_pos = eye;
        self.last_camera_residual = residual;
        self.last_rotation_view_proj = rotation_view_proj;
        self.last_camera_forward = cur_forward;
        self.last_world_scale = world_scale;

        self.shadow_renderer.update(
            self.current_lighting.sun_dir,
            cascade_view,
            0.05 * world_scale,
            world_scale,
        );
        self.atmosphere.sync_camera(
            &self.device,
            &self.queue,
            &self.camera_bind_group,
            eye.y,
            visible_world,
        );
    }

    /// Opaque voxels around `eye` (voxel coordinates) for torch ray tests.
    pub fn upload_torch_blockage(&mut self, cache: &ChunkCache, eye: Vec3, scale: f32) {
        self.torch_blockage.upload(&self.queue, cache, eye, scale);
        self.sky_volume.update(&self.device, &self.queue, cache, eye);
    }

    pub fn upload_extinguished_torches(&mut self, cells: Vec<[i32; 3]>) {
        self.torch_blockage.upload_extinguished(&self.queue, cells);
    }

    pub fn upload_lighting(
        &mut self,
        lighting: &LightingState,
        cloud_controls: &CloudControls,
        surface_wind: glam::Vec2,
        sky_time: f32,
        held_torch: Option<TorchLight>,
        torch_lights: &[TorchLight],
        weather_field: Option<&CloudWeatherField>,
        debug_view: RenderDebugView,
        atmosphere_mode: AtmosphereMode,
    ) {
        let frame = (sky_time - self.effect_time).clamp(0.0, 0.1);
        crate::mesh::water_motion::set_clock(sky_time, frame);
        self.effect_time = sky_time;
        let mut review_lighting = *lighting;
        if let Some(exposure) = crate::terrain_review::fixed_exposure() {
            review_lighting.exposure = exposure;
        }
        let lighting = &review_lighting;
        let mut review_clouds = *cloud_controls;
        if let Some(wind) = crate::terrain_review::wind_override() { review_clouds.wind_speed = wind; }
        let cloud_controls = &review_clouds;
        let mode = crate::terrain_review::light_mode();
        let mut held_torch = if mode == "off" { None } else { held_torch };
        let torch_lights = if mode == "off" || mode == "sweep" { &[][..] } else { torch_lights };
        if mode == "sweep" {
            static REVIEW_CLOCK: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
            let angle = REVIEW_CLOCK.get_or_init(std::time::Instant::now).elapsed().as_secs_f32() * 0.25;
            held_torch = Some(TorchLight {
                position: self.last_camera_pos + Vec3::new(angle.cos() * 6.0, 1.5, -10.0 + angle.sin() * 6.0),
                radius: 18.0, color: [1.0; 3], strength: 4.4, moving: true,
            });
        }
        if mode == "on" {
            held_torch = Some(TorchLight {
                position: self.last_camera_pos + self.last_camera_forward * 0.42
                    + self.last_camera_forward.cross(Vec3::Y) * 0.18 - Vec3::Y * 0.28,
                radius: 18.0, color: [1.0, 0.56, 0.22], strength: 4.4, moving: true,
            });
        }
        self.shadow_renderer.set_detail_wind(
            sky_time, surface_wind.to_array(),
        );
        let mut uniform = lighting_uniform_from_state(
            lighting,
            cloud_controls,
            sky_time,
            held_torch,
            torch_lights,
            cloud_weather_uniform_state(weather_field),
            debug_view,
        );
        // Planet has continuous far terrain. Its air must not become denser
        // when the chunk view distance is lowered; weather owns visibility.
        if self.planet_mesh.is_some() || self.planet_build.is_some() {
            uniform.fog_params[0] = lighting.fog_start.max(0.0);
            uniform.fog_params[1] = lighting.fog_end.max(uniform.fog_params[0] + 24.0);
        }
        // Previously unused fog word: finite Manyfold environment fill. Keep
        // the uniform layout stable while giving its enclosed sky a spectrum.
        // Values above 2 are the shell axis in metres. 1 keeps the stage-A axis.
        uniform.fog_color_density[3] = if self.finite_world { self.manyfold_axis_y.max(1.0) } else { 0.0 };
        // Previously unused surface words: the ground wind in real m/s
        // (`world::wind_at`), the same vector the shadow pass sways with.
        uniform.surface_weather[2] = surface_wind.x;
        uniform.surface_weather[3] = surface_wind.y;
        self.queue
            .write_buffer(&self.lighting_buffer, 0, bytemuck::bytes_of(&uniform));
        self.atmosphere.set_state(
            &self.queue,
            AtmosphereRuntimeState {
                mode: atmosphere_mode,
                debug_view,
            },
        );
        self.atmosphere.sync_from_lighting(lighting, cloud_controls);
        self.current_lighting = *lighting;
        self.current_debug_view = debug_view;

        // Camera upload fits shadows once, with the new camera and lighting.
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
            should_invalidate_history |= upload_batch.invalidate_history;
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

    /// Pixels per unit of tangent for a vertical field of view in radians.
    pub fn focal_px(&self, fov_radians: f32) -> f32 {
        self.config.height as f32 / (2.0 * (fov_radians * 0.5).tan())
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.config.width as f32 / self.config.height.max(1) as f32
    }

    /// Apply backpressure before streaming and uniform uploads, while letting
    /// the caller keep servicing gameplay and networking on every redraw.
    pub fn ready_for_frame(&self) -> bool {
        self.frame_pacer.ready(&self.device)
    }

    pub fn flush_chunk_uploads(&self) {
        let credits = std::mem::take(&mut *self.chunk_upload_credits.lock().unwrap());
        if !credits.is_empty() && self.device_usable() {
            self.queue.submit([]);
            upload_staging::release_after_submit(&self.queue, credits);
        }
    }

    pub fn flush_pending_frame_work(&mut self) {
        let bakes = self.sky_volume.take_pending();
        let credits = std::mem::take(&mut *self.chunk_upload_credits.lock().unwrap());
        if self.device_usable() && (!credits.is_empty() || bakes.iter().any(Option::is_some)) {
            self.queue.submit(bakes.into_iter().flatten());
            upload_staging::release_after_submit(&self.queue, credits);
        }
    }

    fn submit_frame(&mut self, encoder: wgpu::CommandEncoder) -> wgpu::SubmissionIndex {
        let credits = std::mem::take(&mut *self.chunk_upload_credits.lock().unwrap());
        let bakes = self.sky_volume.take_pending();
        let submission = self.queue.submit(bakes.into_iter().flatten().chain([encoder.finish()]));
        upload_staging::release_after_submit(&self.queue, credits);
        submission
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }
    pub fn render_loading_frame<F>(
        &mut self,
        clear_color: [f32; 3],
        overlay: F,
    ) -> Result<(), SurfaceFrameError>
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
        // Loading must not repeat the last world's GPU sample or draw counts.
        self.render_frame_id = self.render_frame_id.wrapping_add(1);
        self.last_render_stats = RenderFrameStats {
            frame_id: self.render_frame_id,
            ..Default::default()
        };
        let acquire_start = Instant::now();
        let result = acquire_surface_frame(&self.surface,&self.device_lost,&self.frame_pacer,&self.device);
        self.last_render_stats.surface_acquire_ms = acquire_start.elapsed().as_secs_f32() * 1000.0;
        let frame = result?;
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
                multiview_mask: None,
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

        // Blend egui gamma-space premultiplied colors after the scene resolve.
        let ui_view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.format.remove_srgb_suffix()),
            ..Default::default()
        });
        overlay(
            &self.device,
            &self.queue,
            &mut encoder,
            &ui_view,
            self.config.width,
            self.config.height,
        );

        if !self.pending_captures.is_empty() {
            let _ = self.device.poll(wgpu::PollType::Poll);
            self.pending_captures.retain(|capture| !capture.finish_if_ready());
        }
        let capture=self.capture_path.take().and_then(|path| {
            if self.pending_captures.len() >= capture::MAX_PENDING_READBACKS {
                eprintln!("[capture] dropped {}: readback queue full (capacity={})", path.display(), capture::MAX_PENDING_READBACKS);
                return None;
            }
            capture::PendingCapture::encode(&self.device, &mut encoder, &frame.texture, &self.config, path)
        });
        self.submit_frame(encoder);
        self.frame_pacer.track_submission(&self.queue);
        if let Some(mut capture)=capture { capture.map(); self.pending_captures.push(capture); }
        self.queue.present(frame);
        Ok(())
    }

    pub fn render_cached_with_overlay<F>(
        &mut self,
        chunk_cache: &ChunkCache,
        draw_list: &[ChunkKey],
        outline_vertices: &[LineVertex],
        invert_outline_vertices: &[LineVertex],
        survey_outline_vertices: &[LineVertex],
        ui_vertices: &[UiVertex],
        paused: bool,
        overlay: F,
    ) -> Result<(), SurfaceFrameError>
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
        if CLOUD_ADAPTIVE_ENABLED.load(Ordering::Relaxed)
            && !paused && draw_clouds()
            && self.last_render_stats.cloud_raymarch_passes != 0
            && self.last_render_stats.gpu_sample_count != 0
            && self.last_render_stats.gpu_sample_config_id == self.last_render_stats.config_id
            && self.cloud_governor.update_cloud_cost(
            self.last_render_stats.gpu_pass_total_ms,
            self.last_render_stats.gpu_cloud_raymarch_ms,
            self.last_frame_instant.elapsed().as_secs_f32(),
        ) {
            CLOUD_AUTO_BUMP.store(self.cloud_governor.bump(), Ordering::Relaxed);
            // Cloud resolution follows measured GPU time, so two builds (or two
            // GPUs) can settle on different looks. Say so instead of changing
            // the image silently.
            eprintln!(
                "[cloud-governor] step {} effective_divisor={} (gpu pass {:.1} ms)",
                self.cloud_governor.bump(),
                cloud_scale_request(self.size.width, self.size.height) % 8,
                self.last_render_stats.gpu_pass_total_ms
            );
        }
        self.last_frame_instant = Instant::now();
        let mut preflight_trace = crate::gameplay_trace::Trace::new();
        if self.cloud_render_scale != cloud_scale_request(self.size.width, self.size.height) {
            if self.cloud_render_scale / 8 == scene_scale_percent() {
                self.resize_cloud_targets();
            } else {
                self.resize_scene_targets();
            }
        }
        preflight_trace.mark("render-target-resize");
        if self
            .gpu_profiler
            .as_mut()
            .is_some_and(|profiler| !profiler.collect_finished(&self.device))
        {
            // Drop all slots together; no failed map can be reused next frame.
            self.gpu_profiler = None;
        }
        preflight_trace.mark("render-gpu-poll");

        let acquire_start = Instant::now();
        let frame = acquire_surface_frame(&self.surface,&self.device_lost,&self.frame_pacer,&self.device)?;
        let mut frame_stats = RenderFrameStats::default();
        self.render_frame_id = self.render_frame_id.wrapping_add(1);
        frame_stats.frame_id = self.render_frame_id;
        if let Some(profiler) = self.gpu_profiler.as_mut() {
            let completed = profiler.take_sample();
            let gpu_timings = profiler.last_timings;
            if let Some(sample) = completed {
                frame_stats.gpu_sample_count = 1;
                frame_stats.gpu_sample_frame_id = sample.frame_id;
                frame_stats.gpu_sample_config_id = sample.config_id;
            }
            frame_stats.gpu_pass_total_ms = gpu_timings.total_ms;
            frame_stats.gpu_shadow_ms = gpu_timings.shadow_ms;
            frame_stats.gpu_terrain_ms = gpu_timings.terrain_ms;
            frame_stats.gpu_indirect_ms = gpu_timings.indirect_ms;
            frame_stats.gpu_cloud_raymarch_ms = gpu_timings.cloud_raymarch_ms;
            frame_stats.gpu_cloud_temporal_ms = gpu_timings.cloud_temporal_ms;
            frame_stats.gpu_sky_ms = gpu_timings.sky_ms;
            frame_stats.gpu_sky_probe_ms = gpu_timings.sky_probe_ms;
            frame_stats.gpu_cloud_composite_ms = gpu_timings.cloud_composite_ms;
            frame_stats.gpu_outline_ms = gpu_timings.outline_ms;
            frame_stats.gpu_postprocess_ms = gpu_timings.postprocess_ms;
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
            let reacquired = acquire_surface_frame(&self.surface,&self.device_lost,&self.frame_pacer,&self.device)?;
            frame_stats.surface_acquire_ms += reacquire_start.elapsed().as_secs_f32() * 1000.0;
            reacquired
        } else {
            frame
        };

        // Fingerprint the effective pass configuration, not requested quality.
        // It travels with the readback so a resize/debug/pause change cannot
        // silently relabel an older GPU measurement as the current frame.
        let distance = render_distance_settings();
        frame_stats.config_id = [
            scene_dims(self.config.width, self.config.height).0,
            scene_dims(self.config.width, self.config.height).1,
            effective_cloud_render_scale(
                scene_dims(self.config.width, self.config.height).0,
                scene_dims(self.config.width, self.config.height).1,
            ),
            self.current_debug_view.as_u32(), u32::from(paused),
            u32::from(shadow::enabled()), u32::from(indirect_lighting_enabled()),
            distance.view_distance_chunks as u32, distance.lod_start_chunks as u32,
            distance.lod_levels as u32, self.config.present_mode as u32,
            u32::from(self.fxaa_enabled), u32::from(!draw_clouds()),
            u32::from(render_isolation().skip_horizon),
        ].into_iter().fold(0xcbf29ce484222325u64, |hash, value| {
            (hash ^ value as u64).wrapping_mul(0x100000001b3)
        });

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let upload_start = Instant::now();
        let projected_outlines;
        let projected_invert_outlines;
        let projected_survey_outlines;
        let survey_outline_vertices = if let Some(frame) = &self.planet_eye {
            projected_survey_outlines = planet::outlines(survey_outline_vertices, frame, self.last_world_scale);
            projected_survey_outlines.as_slice()
        } else {
            survey_outline_vertices
        };
        let (outline_vertices, invert_outline_vertices) = if let Some(frame) = &self.planet_eye {
            projected_outlines = planet::outlines(outline_vertices, frame, self.last_world_scale);
            projected_invert_outlines = planet::outlines(invert_outline_vertices, frame, self.last_world_scale);
            (projected_outlines.as_slice(), projected_invert_outlines.as_slice())
        } else { (outline_vertices, invert_outline_vertices) };
        let has_outline = self
            .outline_stream
            .upload(&self.device, &self.queue, outline_vertices);
        let has_invert_outline =
            self.invert_outline_stream
                .upload(&self.device, &self.queue, invert_outline_vertices);
        let has_survey_outline = self.survey_outline_stream
            .upload(&self.device, &self.queue, survey_outline_vertices);
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
        self.shadow_renderer.set_horizon_source(
            self.horizon.shadow_generation(),
            self.horizon.resident_coverage(),
        );
        let redraw_shadow_map = !paused
            && shadow::enabled()
            && self
                .shadow_renderer
                .apply_update_if_needed(
                    &mut encoder,
                    chunk_cache.shadow_caster_revision,
                    chunk_cache.shadow_caster_columns_since(self.shadow_renderer.last_mesh_revision).as_deref(),
                );

        // The main pass and every shadow cascade draw in the render-origin frame.
        let origin = self.render_origin.unwrap_or(RenderOrigin { voxel: IVec3::ZERO });
        let voxel_m = f64::from(self.last_world_scale);
        let planet_eye = self.planet_eye;
        let arrival = self.terrain_arrival;
        let planet_corners = std::cell::RefCell::new(crate::fast_hash::FastMap::default());
        let planet_corners = &planet_corners;
        let chunk_transform = move |c: IVec3| {
            let mut transform = if let Some(eye) = &planet_eye {
                let eye_origin = eye.chart_m() - origin.voxel.as_dvec3() * voxel_m;
                let mut corners = planet_corners.borrow_mut();
                planet_frame::chunk_draw_with_corners(origin, c, eye, voxel_m, |corner| {
                    *corners.entry(corner).or_insert_with(|| {
                        eye.relative(corner.as_dvec3() * f64::from(crate::world::CHUNK_SIZE) * voxel_m)
                            + eye_origin
                    })
                })
            } else {
                DrawTransform::flat(origin.chunk_offset(c))
            };
            // Bit 0 selects chart bending, bits 1 through 8 hold the mesh LOD transition.
            transform.offset[3] += (arrival.phase(c.x, c.z) << 9) as f32;
            transform
        };
        self.draw_offsets.begin_frame(DrawTransform::flat(origin.absolute_offset()));
        if redraw_shadow_map {
            self.shadow_renderer.build_cascade_draw_lists(chunk_cache, self.horizon.mesh.as_ref());
            self.shadow_renderer.prepare_draws(
                &self.device,
                &self.queue,
                chunk_cache,
                &mut self.draw_offsets,
                chunk_transform,
            );
        }
        // Main-pass slots and indirect arguments are taken before the upload; the
        // offset table must be complete before any pass binds it. This work and
        // the upload count toward terrain encode time, not shadow time.
        let main_prepare_start = Instant::now();
        let mut resident_transitions = Vec::new();
        let mut transitioning = HashSet::new();
        let absolute = DrawTransform::flat(origin.absolute_offset());
        let mut horizon_transition = None;
        let mut planet_transition = None;
        if !paused {
            for key in draw_list {
                if let Some((previous, phase)) = chunk_cache.lod_transition(key.coord, main_prepare_start) {
                    let slots = self.draw_offsets.transition_slots(chunk_transform(key.coord), phase);
                    resident_transitions.push((key.coord, previous, slots));
                    transitioning.insert(key.coord);
                }
            }
            horizon_transition = self.horizon.lod_transition(main_prepare_start)
                .map(|(mesh, phase)| (mesh, self.draw_offsets.transition_slots(absolute, phase)));
            planet_transition = self.outgoing_planet_mesh.as_ref().and_then(|(mesh, started)| {
                let phase = lod_transition_phase(*started, main_prepare_start)?;
                Some((mesh, self.draw_offsets.transition_slots(absolute, phase)))
            });
        }
        let multidraw = !paused && streaming::indirect_draws::active();
        if !paused && !multidraw {
            for key in draw_list {
                self.draw_offsets.slot(key.coord, || chunk_transform(key.coord));
            }
        }
        if !paused {
            let size = scene_dims(self.config.width, self.config.height);
            for coord in self.portals.plan(&self.device, chunk_cache, [size.0, size.1]) {
                self.draw_offsets.slot(coord, || chunk_transform(coord));
            }
        }
        let multidraw_counts = if multidraw {
            let offsets = &mut self.draw_offsets;
            let coords = draw_list.iter().map(|k| k.coord).filter(|c| !transitioning.contains(c));
            self.chunk_indirect.prepare(&self.device, &self.queue, chunk_cache, coords, |c| {
                offsets.slot(c, || chunk_transform(c))
            })
        } else {
            (0, 0)
        };
        self.draw_offsets.upload(&self.device, &self.queue);
        let main_prepare_ms = main_prepare_start.elapsed().as_secs_f32() * 1000.0;
        if redraw_shadow_map && let Some(draw_offsets) = self.draw_offsets.buffer() {
            let mut shadow_timestamp_queries = [None; shadow::CASCADE_COUNT];
            if gpu_query_set.is_some() {
                for pass_index in 0..GPU_TIMESTAMP_SHADOW_PASS_COUNT {
                    if !self.shadow_renderer.renders_cascade(pass_index as usize) {
                        continue;
                    }
                    let begin = gpu_query_count;
                    let end = begin + 1;
                    gpu_query_count += 2;
                    gpu_pass_queries[pass_index as usize] = Some((begin, end));
                    shadow_timestamp_queries[pass_index as usize] = Some((begin, end));
                }
            }
            self.shadow_renderer.render(
                &mut encoder,
                chunk_cache,
                self.horizon.mesh.as_ref(),
                gpu_query_set.as_ref(),
                &shadow_timestamp_queries,
                draw_offsets,
            );
        }
        frame_stats.shadow_redraw = redraw_shadow_map;
        frame_stats.shadow_draw_calls = if redraw_shadow_map {
            self.shadow_renderer.draw_call_count()
        } else {
            0
        };
        frame_stats.shadow_encode_ms = shadow_start.elapsed().as_secs_f32() * 1000.0 - main_prepare_ms;

        let terrain_start = Instant::now();
        let probe_timestamps = allocate_pass_timestamp_writes(
            gpu_query_set.as_ref(), &mut gpu_query_count, &mut gpu_pass_queries,
            GPU_TIMESTAMP_PASS_SKY_PROBE,
        ).map(|writes| wgpu::ComputePassTimestampWrites {
            query_set: writes.query_set,
            beginning_of_pass_write_index: writes.beginning_of_pass_write_index,
            end_of_pass_write_index: writes.end_of_pass_write_index,
        });
        self.atmosphere.record_sky_probe(
            &mut encoder, &self.camera_bind_group, &self.lighting_bind_group, probe_timestamps,
        );
        if let Some(posed) = &self.posed_assets { posed.record(&mut encoder); }
        let scene_view = &self.postprocess.scene;
        let globe_drawn = self.globe.is_some() && self.globe_visible && self.globe_driver.ground_ready();
        let globe_replaces_horizon = self.globe_visible && self.globe_driver.orbit_view();
        let backstop_active = !paused && self.horizon.resident_coverage().header[3] != 0;
        let planet_backstop = self.planet_mesh.as_ref()
            .filter(|_| backstop_active && !globe_drawn)
            .and_then(|mesh| mesh.backstop.as_ref().filter(|(_, count)| *count > 0)
                .map(|backstop| (mesh, backstop)));
        let horizon_backstop = self.horizon.mesh.as_ref().filter(|mesh| {
            backstop_active && self.planet_mesh.is_none() && !render_isolation().skip_horizon
                && !globe_replaces_horizon && !mesh.pages.is_empty()
        });
        let draw_backstop = planet_backstop.is_some() || horizon_backstop.is_some();
        // One timing interval spans both passes and the depth copy between them.
        let terrain_timestamps = allocate_pass_timestamp_writes(
            gpu_query_set.as_ref(),
            &mut gpu_query_count,
            &mut gpu_pass_queries,
            GPU_TIMESTAMP_PASS_TERRAIN,
        );
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("main pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: scene_view,
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
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.indirect.receiver,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: terrain_timestamps.as_ref().map(|writes| wgpu::RenderPassTimestampWrites {
                    query_set: writes.query_set,
                    beginning_of_pass_write_index: writes.beginning_of_pass_write_index,
                    end_of_pass_write_index: if draw_backstop { None } else { writes.end_of_pass_write_index },
                }),
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
                if let Some(draw_offsets) = self.draw_offsets.buffer() {
                    pass.set_vertex_buffer(1, draw_offsets.slice(..));
                }
                if multidraw {
                    self.chunk_indirect.draw(&mut pass, chunk_cache);
                    terrain_draw_calls += multidraw_counts.0;
                    terrain_indices += multidraw_counts.1;
                } else {
                    // Keep front-to-back order and rebind only on a page change.
                    let mut bound_page = None;
                    for key in draw_list {
                        if transitioning.contains(&key.coord) { continue; }
                        let Some(slice) = chunk_cache.draw_slice(key.coord) else { continue };
                        if bound_page != Some(slice.page) {
                            pass.set_vertex_buffer(0, slice.vertices.slice(..));
                            pass.set_index_buffer(slice.indices.slice(..), wgpu::IndexFormat::Uint32);
                            bound_page = Some(slice.page);
                        }
                        terrain_indices += slice.index_range.len();
                        // Slots were taken above; this lookup never adds one.
                        let slot = self.draw_offsets.slot(key.coord, || chunk_transform(key.coord));
                        pass.draw_indexed(slice.index_range, slice.base_vertex, slot..slot + 1);
                        terrain_draw_calls += 1;
                    }
                }
                for (coord, previous, (old_slot, new_slot)) in resident_transitions {
                    for (slice, slot) in [(previous, old_slot), (chunk_cache.draw_slice(coord), new_slot)] {
                        let Some(slice) = slice else { continue; };
                        pass.set_vertex_buffer(0, slice.vertices.slice(..));
                        pass.set_index_buffer(slice.indices.slice(..), wgpu::IndexFormat::Uint32);
                        terrain_indices += slice.index_range.len();
                        pass.draw_indexed(slice.index_range, slice.base_vertex, slot..slot + 1);
                        terrain_draw_calls += 1;
                    }
                }
                // A Planet backstop protects resident/scene pixels, not farther
                // ordinary cover. Defer that cover until after the depth copy.
                if let Some(mesh) = self.planet_mesh.as_ref()
                    .filter(|_| !globe_drawn && planet_backstop.is_none()) {
                    let current_slot = planet_transition.as_ref().map_or(ABSOLUTE_SLOT, |(_, (_, slot))| *slot);
                    let previous = planet_transition.map(|(mesh, (slot, _))| (mesh, slot));
                    for (mesh, slot) in previous.into_iter().chain([(mesh, current_slot)]) {
                        pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                        pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                        let indices = mesh.index_range(self.horizon.current_collar(), self.horizon.resident_coverage());
                        terrain_indices += indices.len();
                        pass.draw_indexed(indices, 0, slot..slot + 1);
                        terrain_draw_calls += 1;
                    }
                }
                if let Some(globe) = self.globe.as_ref().filter(|_| globe_drawn) {
                    globe.draw(&mut pass);
                    // The globe replaced the pipeline and group 0; the draws below use the world's.
                    pass.set_pipeline(&self.world_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_bind_group(1, &self.lighting_bind_group, &[]);
                    pass.set_bind_group(2, self.atmosphere.bind_group(), &[]);
                    pass.set_bind_group(3, &self.world_surface_bind_group, &[]);
                }
                // The flat horizon sits at the globe's sea level and occludes all but the disc centre
                // from orbit; Task 10 replaces this with the full altitude transition.
                if let Some(mesh) = self.horizon.mesh.as_ref().filter(|_| self.planet_mesh.is_none() && !render_isolation().skip_horizon && !globe_replaces_horizon) {
                    let planes =
                        streaming::extract_planes(REVERSE_Z * self.last_rotation_view_proj);
                    let current_slot = horizon_transition.as_ref().map_or(ABSOLUTE_SLOT, |(_, (_, slot))| *slot);
                    let previous = horizon_transition.map(|(mesh, (slot, _))| (mesh, slot));
                    for (mesh, slot) in previous.into_iter().chain([(mesh, current_slot)]) {
                        for page in &mesh.pages {
                            pass.set_vertex_buffer(0, page.vertices.slice(..));
                            pass.set_index_buffer(page.indices.slice(..), wgpu::IndexFormat::Uint32);
                            let clip = self.horizon.chart_clip();
                            if mesh.cull || clip.is_some() || page.batches.iter().any(|b|b.conditional()) {
                                for batch in &page.batches {
                                    if !batch.selected(mesh.collared) || (mesh.cull && !batch.visible(self.last_camera_pos, &planes)) || clip.is_some_and(|c| !batch.within_clip(c)) {
                                        continue;
                                    }
                                    pass.draw_indexed(batch.indices.clone(), 0, slot..slot + 1);
                                    terrain_draw_calls += 1;
                                    terrain_indices += batch.indices.len();
                                }
                            } else {
                                pass.draw_indexed(0..page.count, 0, slot..slot + 1);
                                terrain_draw_calls += 1;
                                terrain_indices += page.count as usize;
                            }
                        }
                    }
                }
                frame_stats.terrain_draw_calls = terrain_draw_calls;
                if self.scene_vertices > 0
                    && let Some(stream) = &self.scene_stream {
                        pass.set_vertex_buffer(0, stream.slice::<Vertex3>(self.scene_vertices));
                        pass.draw(0..self.scene_vertices as u32, ABSOLUTE_SLOT..ABSOLUTE_SLOT + 1);
                        frame_stats.terrain_draw_calls += 1;
                    }
                if let Some((posed, count)) = self.posed_assets.as_ref().and_then(|p| p.vertices()) {
                    pass.set_vertex_buffer(0, posed);
                    pass.draw(0..count, ABSOLUTE_SLOT..ABSOLUTE_SLOT + 1);
                    frame_stats.terrain_draw_calls += 1;
                }
                frame_stats.terrain_indices = terrain_indices;
            }
        }
        if draw_backstop {
            // Planet cover is deferred: protect resident chunks (including exact
            // edit proxies), scene and posed draws before any procedural parent.
            // Flat-world horizon ordering stays unchanged, retaining its exact
            // edit geometry in the copied protection mask too.
            encoder.copy_texture_to_texture(
                self.depth.texture().as_image_copy(),
                self.terrain_backstop_depth.texture().as_image_copy(),
                self.depth.texture().size(),
            );
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("terrain backstop pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: scene_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.indirect.receiver,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: terrain_timestamps.map(|mut writes| {
                    writes.beginning_of_pass_write_index = None;
                    writes
                }),
                occlusion_query_set: None,
            });
            pass.set_pipeline(if planet_backstop.is_some() {
                &self.world_pipeline
            } else {
                &self.terrain_backstop
            });
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_bind_group(1, &self.lighting_bind_group, &[]);
            pass.set_bind_group(2, self.atmosphere.bind_group(), &[]);
            pass.set_bind_group(3, &self.world_surface_bind_group, &[]);
            if let Some(draw_offsets) = self.draw_offsets.buffer() {
                pass.set_vertex_buffer(1, draw_offsets.slice(..));
            }
            if let Some((mesh, (indices, count))) = planet_backstop {
                // Ordinary unowned/collared cover draws first. The backstop may
                // replace farther cover at a resident-owned footprint, while the
                // copied mask keeps resident terrain and scene objects untouched.
                let current_slot = planet_transition.as_ref().map_or(ABSOLUTE_SLOT, |(_, (_, slot))| *slot);
                let previous = planet_transition.map(|(mesh, (slot, _))| (mesh, slot));
                for (cover, slot) in previous.into_iter().chain([(mesh, current_slot)]) {
                    pass.set_vertex_buffer(0, cover.vertices.slice(..));
                    pass.set_index_buffer(cover.indices.slice(..), wgpu::IndexFormat::Uint32);
                    let range = cover.index_range(self.horizon.current_collar(), self.horizon.resident_coverage());
                    frame_stats.terrain_indices += range.len();
                    pass.draw_indexed(range, 0, slot..slot + 1);
                    frame_stats.terrain_draw_calls += 1;
                }
                pass.set_pipeline(&self.terrain_backstop);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*count, 0, ABSOLUTE_SLOT..ABSOLUTE_SLOT + 1);
                frame_stats.terrain_draw_calls += 1;
                frame_stats.terrain_indices += *count as usize;
            }
            if let Some(mesh) = horizon_backstop {
                let planes = streaming::extract_planes(REVERSE_Z * self.last_rotation_view_proj);
                let margin = 8 + render_distance_settings().view_distance_chunks;
                let reach = margin as f32 * CHUNK_SIZE as f32 * self.last_world_scale;
                for page in &mesh.pages {
                    pass.set_vertex_buffer(0, page.vertices.slice(..));
                    pass.set_index_buffer(page.indices.slice(..), wgpu::IndexFormat::Uint32);
                    for batch in &page.batches {
                        if !batch.selected(false)
                            || !batch.nearby(self.last_camera_pos, reach)
                            || !batch.visible(self.last_camera_pos, &planes)
                            || self.horizon.chart_clip().is_some_and(|c| !batch.within_clip(c))
                        {
                            continue;
                        }
                        pass.draw_indexed(batch.indices.clone(), 0, ABSOLUTE_SLOT..ABSOLUTE_SLOT + 1);
                        frame_stats.terrain_draw_calls += 1;
                        frame_stats.terrain_indices += batch.indices.len();
                    }
                }
            }
        }
        frame_stats.terrain_encode_ms = terrain_start.elapsed().as_secs_f32() * 1000.0 + main_prepare_ms;
        let mut lighting_trace = crate::gameplay_trace::Trace::new();
        if !paused && indirect_lighting_enabled() {
            let timestamps = allocate_pass_timestamp_writes(
                gpu_query_set.as_ref(),
                &mut gpu_query_count,
                &mut gpu_pass_queries,
                GPU_TIMESTAMP_PASS_INDIRECT,
            );
            self.indirect.render(
                &mut encoder,
                &self.camera_bind_group,
                scene_view,
                timestamps.map(|t| {
                    (
                        t.query_set,
                        t.beginning_of_pass_write_index.unwrap(),
                        t.end_of_pass_write_index.unwrap(),
                    )
                }),
            );
        }

        lighting_trace.mark("render-indirect");
        let cloud_start = Instant::now();
        if !paused {
            let cloud_extent = wgpu::Extent3d {
                width: scene_dims(self.config.width, self.config.height).0
                    .div_ceil(effective_cloud_render_scale(
                        scene_dims(self.config.width, self.config.height).0,
                        scene_dims(self.config.width, self.config.height).1,
                    ))
                .max(1),
                height: scene_dims(self.config.width, self.config.height).1
                    .div_ceil(effective_cloud_render_scale(
                        scene_dims(self.config.width, self.config.height).0,
                        scene_dims(self.config.width, self.config.height).1,
                    ))
                .max(1),
                depth_or_array_layers: 1,
            };

            // Above the top slab the globe's cloud shell replaces the march and its composite.
            let march_handoff = !(self.globe_visible && self.globe.is_some()) || self.globe_driver.march_allowed();
            let march = draw_clouds() && march_handoff && !self.current_lighting.hell && self.camera_medium == CameraMedium::Air;
            if !march {
                // History from before the handoff would blend old clouds in on descent.
                self.cloud_history_valid = false;
            }
            if march {
                let mut cloud_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    multiview_mask: None,
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
            frame_stats.cloud_raymarch_passes = usize::from(march);

            if CLOUD_TEMPORAL_ENABLED && self.cloud_history_valid && march {
                let mut temporal_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    multiview_mask: None,
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
            } else if march {
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

            if CLOUD_TEMPORAL_ENABLED && march {
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
                    multiview_mask: None,
                    label: Some("sky pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: scene_view,
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
                sky_pass.set_bind_group(3, &self.world_surface_bind_group, &[]);
                sky_pass.draw(0..3, 0..1);
            }

            if march {
                let mut composite_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    multiview_mask: None,
                    label: Some("cloud composite pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: scene_view,
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
            self.record_portals(&mut encoder, chunk_cache);
            let scene_view = &self.postprocess.scene;

            if has_outline || has_invert_outline || has_survey_outline {
                let mut outline_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    multiview_mask: None,
                    label: Some("outline pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: scene_view,
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

                if has_survey_outline {
                    outline_pass.set_pipeline(&self.survey_line_pipeline);
                    outline_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    outline_pass.set_vertex_buffer(
                        0,
                        self.survey_outline_stream.slice::<LineVertex>(survey_outline_vertices.len()),
                    );
                    outline_pass.draw(0..survey_outline_vertices.len() as u32, 0..1);
                }

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

        // Native stylised resolve. The previous art-pixel downscale was the
        // dominant source of the "cheap low-resolution" read: it discarded
        // most of the scene's material and lighting detail before presentation.
        // Keep the 16-bit float precision + native write dither, but no block
        // sampling, palette quantization, or block contour by default.
        let pixel_scale = 1.0;
        let mut resolve_trace = crate::gameplay_trace::Trace::new();
        self.postprocess.render(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            &self.depth,
            [
                // A locked review exposure is an absolute value, so the
                // art compensation applies to the time-of-day policy only.
                if crate::terrain_review::fixed_exposure().is_some() {
                    self.current_lighting.exposure.max(0.01)
                } else {
                    self.current_lighting.exposure.max(0.01)
                        * atmosphere_params::SkyArt::active().exposure_gain()
                },
                self.current_lighting.saturation,
                if self.fxaa_enabled { 1.0 } else { 0.0 },
                if matches!(self.current_debug_view, RenderDebugView::None | RenderDebugView::Unfogged) {
                    0.0
                } else {
                    1.0
                },
                pixel_scale,
                0.85, // optional palette dither; final write uses native blue noise
                0.0,  // no palette quantization: preserve the authored ramps
                0.0,  // no block contour: lighting owns the silhouettes
                if crate::terrain_review::fixed_exposure().is_some() {
                    0.0
                } else {
                    atmosphere_params::SkyArt::active().bloom_strength
                },
                0.72, // glow threshold: emissives, not ordinary daylight
                self.effect_time,
                0.06, // restrained vignette
                self.screen_magic[0],
                self.screen_magic[1],
                self.screen_magic[2],
                self.screen_magic[3],
                self.current_lighting.mood_tint[0],
                self.current_lighting.mood_tint[1],
                self.current_lighting.mood_tint[2],
                self.current_lighting.mood_tint[3],
                self.zone_grade.0[0],
                self.zone_grade.0[1],
                self.zone_grade.0[2],
                self.zone_grade.0[3],
                self.zone_grade.1[0],
                self.zone_grade.1[1],
                self.zone_grade.1[2],
                self.zone_grade.1[3],
                self.motion_boost,
                sky_volume::ssao_scale(),
                if self.current_lighting.hell { 1.0 } else { 0.0 },
                0.0,
            ],
            allocate_pass_timestamp_writes(
                gpu_query_set.as_ref(), &mut gpu_query_count, &mut gpu_pass_queries,
                GPU_TIMESTAMP_PASS_POSTPROCESS,
            ),
        );
        resolve_trace.mark("render-postprocess");
        let ui_start = Instant::now();
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
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
        // Blend egui gamma-space premultiplied colors after the scene resolve.
        let ui_view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.config.format.remove_srgb_suffix()),
            ..Default::default()
        });
        overlay(
            &self.device,
            &self.queue,
            &mut encoder,
            &ui_view,
            self.config.width,
            self.config.height,
        );
        frame_stats.overlay_encode_ms = overlay_start.elapsed().as_secs_f32() * 1000.0;

        let mut readback_trace = crate::gameplay_trace::Trace::new();
        if !self.pending_captures.is_empty() {
            let _ = self.device.poll(wgpu::PollType::Poll);
            readback_trace.mark("capture-poll");
            self.pending_captures
                .retain(|capture| !capture.finish_if_ready());
            readback_trace.mark("capture-readback");
        }
        let capture = self.capture_path.take().and_then(|path| {
            if self.pending_captures.len() >= capture::MAX_PENDING_READBACKS {
                eprintln!("[capture] dropped {}: readback queue full (capacity={})", path.display(), capture::MAX_PENDING_READBACKS);
                return None;
            }
            capture::PendingCapture::encode(
                &self.device,
                &mut encoder,
                &frame.texture,
                &self.config,
                path,
            )
        });
        readback_trace.mark("capture-encode");

        if let Some(slot_index) = gpu_timestamp_slot
            && gpu_query_count != 0 {
                self.gpu_profiler
                    .as_ref()
                    .expect("profiler slot allocated without profiler")
                    .resolve_results(&mut encoder, slot_index, gpu_query_count);
            }

        frame_pacer::debug_stall(&self.device, &mut encoder, frame_stats.frame_id);
        let submit_start = Instant::now();
        self.submit_frame(encoder);
        self.frame_pacer.track_submission(&self.queue);
        if let Some(slot_index) = gpu_timestamp_slot
            && let Some(profiler) = self.gpu_profiler.as_mut() {
                profiler.mark_submitted(slot_index, gpu_query_count, gpu_pass_queries,
                    frame_stats.frame_id, frame_stats.config_id);
            }
        frame_stats.submit_ms = submit_start.elapsed().as_secs_f32() * 1000.0;

        if let Some(mut capture) = capture {
            capture.map();
            self.pending_captures.push(capture);
        }

        let present_start = Instant::now();
        self.queue.present(frame);
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

#[cfg(test)]
#[path = "render/sky_probe_timing_tests.rs"]
mod sky_probe_timing_tests;
