//! Per-frame globe driver: drains bakes, selects tiles, uploads, evicts and records stats.

use super::globe::cache::TileCache;
use super::globe::gpu::{Globe, GlobeUniform, body_view_proj};
use super::globe::scheduler::{self, Selection};
use super::globe::tile::TileId;
use super::globe::shell::{self, Light, ShellConfig};
use super::globe::shell_worker::ShellWorker;
use super::globe::worker::{Worker, tile_bytes};
use super::globe::{self, chart};
use crate::altitude::Tier;
use engine_voxel::WorldSampleConfig;
use glam::{DVec3, Mat4, Vec3};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Per-frame upload cap; one 2 MB frame stays under a millisecond of copy on any adapter.
const DRAIN_BYTES: usize = 2 * 1024 * 1024;
/// Draw cap and shell face edge: halved on the lean tier to fit an iGPU frame.
pub fn lean_limits(lean: bool) -> (usize, u32) {
    if lean { (256, 256) } else { (512, 512) }
}
const MAX_REQUEST: usize = 64;
const BAKE_TURN: Duration = Duration::from_millis(2);
/// 30 s assuming 60 Hz; frames rather than wall time keep the drop off `Instant`.
const SURFACE_DROP_FRAMES: u32 = 1_800;
/// Shell and night-light refresh in frames: 30 s of game time at 60 Hz.
const SHELL_FRAMES: u64 = 1_800;
/// Angle from a face normal to its corner, `acos(1 / sqrt(3))`.
const FACE_CORNER_RAD: f64 = 0.955_316_6;
/// A dead worker is respawned at most this often, so a bake that panics every time cannot spin.
const RESPAWN_FRAMES: u64 = 600;

/// Globe counters for the last driven frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlobeStats {
    pub resident: usize,
    pub drawn: usize,
    pub wanted: usize,
    /// Bytes of tiles that reached the GPU this frame.
    pub upload_bytes: usize,
    pub cpu_ms: f32,
    pub dropped: usize,
    /// Worker bake wall time accumulated since the previous frame.
    pub bake_ms: f32,
}

/// Camera inputs for one globe frame, all in chart metres.
pub struct GlobeView {
    pub eye_m: DVec3,
    pub forward: Vec3,
    /// Reverse-Z projection the terrain uses.
    pub projection: Mat4,
    pub sun_dir: Vec3,
    pub focal_px: f32,
    /// Time base of the cloud march's wind offset.
    pub sky_time_s: f32,
    pub clouds: crate::world::CloudControls,
    pub weather_seed: u32,
}

/// Whether any of `face` can be in front of the limb seen from `eye_body`.
pub fn face_in_front(face: u8, eye_body: DVec3) -> bool {
    let normal = engine_core::cube_sphere::Face::ALL[usize::from(face) % 6].basis().n.as_dvec3();
    let horizon = (chart::RADIUS_M / eye_body.length().max(chart::RADIUS_M)).acos();
    eye_body.normalize_or_zero().dot(normal).clamp(-1.0, 1.0).acos() < horizon + FACE_CORNER_RAD
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Run,
    Skip,
    /// Skip and drop the worker so it stops holding the cache open.
    DropWorker,
}

/// What the driver does this frame; `surface_frames` counts this frame too.
pub fn driver_step(tier: Tier, forced: bool, surface_frames: u32) -> Step {
    match tier {
        Tier::Surface if !forced && surface_frames >= SURFACE_DROP_FRAMES => Step::DropWorker,
        Tier::Surface if !forced => Step::Skip,
        _ => Step::Run,
    }
}

/// Whether the globe replaces the flat horizon: only from orbit, and only once it has tiles.
pub fn hides_horizon(running: bool, tier: Tier, resident: usize) -> bool {
    running && tier == Tier::Orbit && resident > 0
}

/// Unready drawn tiles first (a never-baked root would otherwise never be requested), then
/// `wanted`, deduplicated and cut to `MAX_REQUEST`.
pub fn request_order(draw: &[TileId], wanted: &[TileId], ready: &dyn Fn(TileId) -> bool, out: &mut Vec<TileId>) {
    out.clear();
    for id in draw.iter().copied().filter(|id| !ready(*id)).chain(wanted.iter().copied()) {
        if out.len() == MAX_REQUEST {
            break;
        }
        if !out.contains(&id) {
            out.push(id);
        }
    }
}

pub struct GlobeDriver {
    worker: Option<Worker>,
    previous_draw: Vec<TileId>,
    keep: Vec<TileId>,
    request: Vec<TileId>,
    tier: Tier,
    surface_frames: u32,
    frame: u64,
    last_spawn: Option<u64>,
    running: bool,
    stats: GlobeStats,
    shell_worker: Option<ShellWorker>,
    last_shell: Option<u64>,
    light_count: u32,
    march: bool,
    /// Finished faces waiting for upload budget.
    shell_pending: std::collections::VecDeque<super::globe::shell_worker::ShellBake>,
}

impl Default for GlobeDriver {
    fn default() -> Self {
        GlobeDriver {
            worker: None,
            previous_draw: Vec::new(),
            keep: Vec::new(),
            request: Vec::new(),
            tier: Tier::Surface,
            surface_frames: 0,
            frame: 0,
            last_spawn: None,
            running: false,
            stats: GlobeStats::default(),
            shell_worker: None,
            last_shell: None,
            light_count: 0,
            march: true,
            shell_pending: std::collections::VecDeque::new(),
        }
    }
}

impl GlobeDriver {
    pub fn set_tier(&mut self, tier: Tier) {
        self.tier = tier;
    }

    /// Whether the last frame prepared the globe; a skipped frame must not draw stale instances.
    pub fn running(&self) -> bool {
        self.running
    }

    pub fn orbit_view(&self) -> bool {
        hides_horizon(self.running, self.tier, self.stats.resident)
    }

    /// Whether the volumetric cloud march and its composite run this frame.
    pub fn march_allowed(&self) -> bool {
        self.march
    }

    pub fn stats(&self) -> GlobeStats {
        self.stats
    }

    /// Drain, select, request, then evict, upload and prepare through `Globe::frame`.
    ///
    /// `save_dir` is called only when the worker is spawned.
    pub fn update(
        &mut self,
        globe: &mut Globe,
        queue: &wgpu::Queue,
        cfg: WorldSampleConfig,
        save_dir: &dyn Fn() -> PathBuf,
        view: &GlobeView,
        lights: &dyn Fn(DVec3) -> Vec<Light>,
    ) {
        let start = Instant::now();
        self.frame += 1;
        self.surface_frames = if self.tier == Tier::Surface { self.surface_frames.saturating_add(1) } else { 0 };
        let step = if globe::active() { driver_step(self.tier, globe::forced(), self.surface_frames) } else { Step::Skip };
        self.running = step == Step::Run;
        self.march = !self.running || shell::volumetric_clouds_enabled(view.eye_m.y, shell::cloud_top_slab_m(&view.clouds));
        if !self.running {
            self.stats = GlobeStats::default();
            if step == Step::DropWorker {
                self.worker = None;
                // The next running frame respawns it and rebakes all six faces.
                self.shell_worker = None;
                self.last_shell = None;
                self.shell_pending.clear();
            }
            return;
        }
        if self.worker.as_ref().is_some_and(|w| !w.alive()) {
            self.worker = None;
        }
        let may_spawn = self.last_spawn.is_none_or(|f| self.frame - f >= RESPAWN_FRAMES);
        if self.worker.is_none() && may_spawn {
            self.last_spawn = Some(self.frame);
            let cache = TileCache::open(&save_dir(), cfg.seed, cfg.generator_version);
            self.worker = Some(Worker::spawn(cfg, cache, BAKE_TURN));
        }
        let incoming = self.worker.as_ref().map(|w| w.drain(DRAIN_BYTES)).unwrap_or_default();
        let bake_nanos = self.worker.as_ref().map_or(0, Worker::take_bake_nanos);
        let incoming_bytes: usize = incoming.iter().map(tile_bytes).sum();
        let incoming_count = incoming.len();
        let (eye_body, view_proj) = body_view_proj(view.eye_m, view.forward, view.projection);
        // Tiles drained this frame upload before `prepare`, so they count as ready now.
        let ready = |id: TileId| globe.resident_contains(id) || incoming.iter().any(|t| t.id == id);
        let selection: Selection =
            scheduler::select(&ready, &self.previous_draw, eye_body, f64::from(view.focal_px), lean_limits(crate::render::lean_shading()).0);
        request_order(&selection.draw, &selection.wanted, &ready, &mut self.request);
        if let Some(worker) = &self.worker {
            worker.request(&self.request);
        }
        scheduler::keep_set(&selection.draw, &mut self.keep);
        let frame = chart::frame(view.eye_m);
        let sun = (frame * view.sun_dir.as_dvec3()).normalize_or_zero().as_vec3();
        let shell_bytes = self.refresh_shell(globe, queue, view, eye_body, incoming_bytes, lights);
        let eye = eye_body.as_vec3();
        let uniform = GlobeUniform {
            eye_body: [eye.x, eye.y, eye.z, self.light_count as f32],
            sun_dir_body: [sun.x, sun.y, sun.z, 0.0],
            radius_m: chart::RADIUS_M as f32,
            sea_level_m: 0.0,
            focal_px: view.focal_px,
            flags: u32::from(crate::render::lean_shading()) | if globe.shell_ready() { 2 } else { 0 } | if self.light_count > 0 { 4 } else { 0 },
            limb: [0.35, 0.55, 0.95, 1.0],
            view_proj: view_proj.to_cols_array_2d(),
        };
        let frame_stats = globe.frame(queue, incoming, &selection, &self.keep, eye_body, uniform);
        // Every tile has the same fixed size, so dropped bytes are a whole number of tiles.
        let per_tile = incoming_bytes.checked_div(incoming_count).unwrap_or(0);
        self.stats = GlobeStats {
            resident: frame_stats.resident,
            drawn: frame_stats.drawn,
            wanted: selection.wanted.len(),
            upload_bytes: incoming_bytes - per_tile * frame_stats.dropped + shell_bytes,
            cpu_ms: start.elapsed().as_secs_f32() * 1000.0,
            dropped: frame_stats.dropped,
            bake_ms: bake_nanos as f32 / 1.0e6,
        };
        self.previous_draw = selection.draw;
    }

    /// Uploads shell faces within the frame budget, returning their bytes; every `SHELL_FRAMES`
    /// requests faces in front of the limb (all six the first time) and rebuilds the lights.
    fn refresh_shell(
        &mut self,
        globe: &mut Globe,
        queue: &wgpu::Queue,
        view: &GlobeView,
        eye_body: DVec3,
        tile_bytes: usize,
        lights: &dyn Fn(DVec3) -> Vec<Light>,
    ) -> usize {
        if self.shell_worker.as_ref().is_some_and(|w| !w.alive()) {
            self.shell_worker = None;
        }
        if self.shell_worker.is_none() && self.last_shell.is_none() {
            self.shell_worker = ShellWorker::spawn();
        }
        if let Some(worker) = &self.shell_worker {
            self.shell_pending.extend(worker.drain());
        }
        let face_bytes = self.shell_pending.front().map_or(0, |b| b.coverage.len());
        let n = shell_faces_within(DRAIN_BYTES, tile_bytes, face_bytes, self.shell_pending.len());
        let ready: Vec<_> = self.shell_pending.drain(..n).collect();
        globe.upload_shell(queue, &ready);
        let uploaded = ready.iter().map(|b| b.coverage.len()).sum();
        // Frames, not game seconds: 1,800 frames is 30 s only at 60 Hz.
        if self.last_shell.is_some_and(|f| self.frame - f < SHELL_FRAMES) {
            return uploaded;
        }
        let first = self.last_shell.is_none();
        self.last_shell = Some(self.frame);
        if let Some(worker) = &self.shell_worker {
            let cfg = ShellConfig { resolution: globe.shell_resolution(), controls: view.clouds, weather_seed: view.weather_seed };
            for face in (0..6u8).filter(|f| first || face_in_front(*f, eye_body)) {
                worker.request(cfg, view.sky_time_s, face);
            }
        }
        self.light_count = globe.upload_lights(queue, &lights(eye_body));
        uploaded
    }
}

/// Faces that fit in what the tile drain left of `budget`; the rest wait a frame.
pub fn shell_faces_within(budget: usize, tile_bytes: usize, face_bytes: usize, pending: usize) -> usize {
    if face_bytes == 0 {
        return pending;
    }
    (budget.saturating_sub(tile_bytes) / face_bytes).min(pending)
}

#[cfg(test)]
#[path = "globe_driver_tests.rs"]
mod tests;
