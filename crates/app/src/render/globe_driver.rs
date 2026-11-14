//! Per-frame globe driver: drains bakes, selects tiles, uploads, evicts and records stats.

use super::globe::cache::TileCache;
use super::globe::gpu::{Globe, GlobeUniform, body_view_proj};
use super::globe::scheduler::{self, SelectScratch, Selection};
use super::globe::shell;
use super::globe::shell_worker::{ShellBake, ShellWorker};
use super::globe::tile::TileId;
use super::globe::worker::{Worker, tile_bytes};
use super::globe::{self, chart};
use crate::altitude::Tier;
use engine_voxel::WorldSampleConfig;
use glam::{DVec3, Mat4, Vec3, Vec4};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn hell_planet(cfg: WorldSampleConfig) -> bool {
    cfg.is_hell()
}

#[path = "globe_driver_shell.rs"]
mod shell_refresh;
pub use shell_refresh::{face_in_front, shell_faces_within};

/// Per-frame upload cap; one 2 MB frame stays under a millisecond of copy on any adapter.
const DRAIN_BYTES: usize = 2 * 1024 * 1024;
/// Draw cap and shell face edge: halved on the lean tier to fit an iGPU frame.
pub fn lean_limits(lean: bool) -> (usize, u32) {
    if lean { (256, 256) } else { (512, 512) }
}
const MAX_REQUEST: usize = 64;
const BAKE_TURN: Duration = Duration::from_millis(2);
/// Seconds on the surface tier before the worker is dropped.
const SURFACE_DROP_S: f64 = 30.0;
/// A dead worker is respawned at most this often, so a bake that panics every time cannot spin.
const RESPAWN_S: f64 = 10.0;

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
    pub cloud_bake_ms: f32,
    pub cloud_pending: u32,
    pub cloud_resolution: u32,
    pub night_lights: u32,
}

/// Camera inputs for one globe frame, all in chart metres.
pub struct GlobeView {
    pub eye_m: DVec3,
    pub forward: Vec3,
    /// Reverse-Z projection the terrain uses.
    pub projection: Mat4,
    pub sun_dir: Vec3,
    pub moon_dir: Vec3,
    /// `LightingState::moon_intensity`; planets hold 0.05 while the moon is up.
    pub moon_intensity: f32,
    pub focal_px: f32,
    /// Time base of the cloud march's wind offset.
    pub sky_time_s: f32,
    pub clouds: crate::world::CloudControls,
    pub weather_seed: u32,
    /// Synced weather day, `synoptic::days`.
    pub weather_days: f64,
    /// Render-thread wall seconds; the sky time can be frozen by reviews, this cannot.
    pub clock_s: f64,
    /// `RenderDebugView::as_u32`; the globe answers the surface views it shares with the terrain pass.
    pub debug_view: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Run,
    Skip,
    /// Skip and drop the worker so it stops holding the cache open.
    DropWorker,
}

/// What the driver does this frame, `surface_s` seconds after the surface tier began.
pub fn driver_step(tier: Tier, forced: bool, surface_s: f64) -> Step {
    match tier {
        Tier::Surface if !forced && surface_s >= SURFACE_DROP_S => Step::DropWorker,
        Tier::Surface if !forced => Step::Skip,
        _ => Step::Run,
    }
}

/// Whether the globe was built for another world and must drop everything it holds.
pub fn config_changed(built_for: Option<WorldSampleConfig>, cfg: WorldSampleConfig) -> bool {
    built_for.is_some_and(|old| old != cfg)
}

/// Whether `period` seconds have passed since `last`; a clock that steps back counts as due.
pub(crate) fn due(last: Option<f64>, now: f64, period: f64) -> bool {
    last.is_none_or(|t| now - t >= period || now < t)
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

#[derive(Default)]
pub struct GlobeDriver {
    worker: Option<Worker>,
    /// The world the worker, cache and resident tiles belong to.
    built_for: Option<WorldSampleConfig>,
    /// Holds last frame's draw list until `select_into` replaces it.
    selection: Selection,
    select_scratch: SelectScratch,
    keep: Vec<TileId>,
    keep_for_draw: Vec<TileId>,
    request: Vec<TileId>,
    /// The list the worker last received; an identical list is not resent.
    sent: Vec<TileId>,
    tier: Tier,
    surface_since: Option<f64>,
    last_spawn: Option<f64>,
    running: bool,
    ground_ready: bool,
    stats: GlobeStats,
    shell_worker: Option<ShellWorker>,
    last_shell: Option<f64>,
    last_shell_lights: Option<f64>,
    last_light_eye: Option<DVec3>,
    shell_in_flight: u8,
    shell_bake_resolution: u32,
    light_count: u32,
    /// Whether the volumetric march may run; meaningful only while `running`.
    march: bool,
    /// Finished faces waiting for upload budget.
    shell_pending: VecDeque<ShellBake>,
    shell_ready: Vec<ShellBake>,
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
        hides_horizon(self.ground_ready(), self.tier, self.stats.resident)
    }

    pub fn ground_ready(&self) -> bool {
        self.running && self.ground_ready
    }

    pub fn ground_ready_for(&self, cfg: WorldSampleConfig) -> bool {
        self.built_for == Some(cfg) && self.ground_ready()
    }

    /// Whether the volumetric cloud march and its composite run this frame.
    pub fn march_allowed(&self) -> bool {
        !self.running || self.march
    }

    pub fn stats(&self) -> GlobeStats {
        self.stats
    }

    fn drop_workers(&mut self) {
        self.worker = None;
        // The next running frame respawns it and rebakes all six faces.
        self.shell_worker = None;
        self.last_shell = None;
        self.last_shell_lights = None;
        self.last_light_eye = None;
        self.shell_in_flight = 0;
        self.shell_bake_resolution = 0;
        self.shell_pending.clear();
        self.sent.clear();
    }

    /// Forgets everything built for the previous world: workers, queues and resident tiles.
    fn reset(&mut self, globe: &mut Globe) {
        self.drop_workers();
        globe.clear();
        self.selection.draw.clear();
        self.selection.wanted.clear();
        self.last_spawn = None;
        self.light_count = 0;
        self.stats = GlobeStats::default();
        self.ground_ready = false;
    }

    /// Selects tiles, orders requests and fills the keep set; true when the request list changed.
    fn plan(&mut self, ready: &dyn Fn(TileId) -> bool, eye_body: DVec3, focal_px: f64, max_draw: usize) -> bool {
        self.plan_with_maturity(ready,&|_| true,eye_body,focal_px,&[],max_draw)
    }

    fn plan_with_maturity(&mut self, ready: &dyn Fn(TileId) -> bool, mature: &dyn Fn(TileId) -> bool,
        eye_body: DVec3, focal_px: f64, planes: &[Vec4], max_draw: usize) -> bool {
        scheduler::select_into_with_maturity(ready,mature,eye_body,focal_px,planes,max_draw,&mut self.select_scratch,&mut self.selection);
        request_order(&self.selection.draw, &self.selection.wanted, ready, &mut self.request);
        if !planes.is_empty() {
            // Keep a cheap fallback for a camera turn onto a previously unseen face.
            for id in (0..6).map(TileId::root) {
                if !ready(id) && !self.request.contains(&id) {
                    if self.request.len() == MAX_REQUEST { self.request.pop(); }
                    self.request.insert(0, id);
                }
            }
        }
        if self.keep.is_empty() || self.keep_for_draw != self.selection.draw {
            scheduler::keep_set(&self.selection.draw, &mut self.keep);
            self.keep_for_draw.clone_from(&self.selection.draw);
        }
        if self.request == self.sent {
            return false;
        }
        self.sent.clear();
        self.sent.extend_from_slice(&self.request);
        true
    }

    /// Drain, select, request, then evict, upload and prepare through `Globe::frame`.
    ///
    /// `save_dir` is called only when the worker is spawned, `factory_clusters` only on a
    /// light refresh.
    pub fn update(
        &mut self,
        globe: &mut Globe,
        queue: &wgpu::Queue,
        cfg: WorldSampleConfig,
        save_dir: &dyn Fn() -> PathBuf,
        view: &GlobeView,
        factory_clusters: &dyn Fn() -> Vec<(DVec3, u32)>,
    ) {
        let start = Instant::now();
        let now = view.clock_s;
        if config_changed(self.built_for, cfg) {
            self.reset(globe);
        }
        self.built_for = Some(cfg);
        let surface_s = if self.tier == Tier::Surface {
            now - *self.surface_since.get_or_insert(now)
        } else {
            self.surface_since = None;
            0.0
        };
        let step = if globe::active() { driver_step(self.tier, globe::forced(), surface_s) } else { Step::Skip };
        self.running = step == Step::Run;
        self.march = if cfg.is_planet() {
            // Keep the volume through the complete optical-depth handoff;
            // stopping at the compressed cirrus top would cut it mid-fade.
            view.eye_m.y < shell::HANDOFF_M.1
        } else {
            shell::volumetric_clouds_enabled(view.eye_m.y, shell::cloud_top_slab_m(&view.clouds))
        };
        if !self.running {
            self.stats = GlobeStats::default();
            if step == Step::DropWorker {
                self.drop_workers();
            }
            return;
        }
        if self.worker.as_ref().is_some_and(|w| !w.alive()) {
            self.worker = None;
        }
        if self.worker.is_none() && due(self.last_spawn, now, RESPAWN_S) {
            self.last_spawn = Some(now);
            let cache = TileCache::open(&save_dir(), &cfg);
            self.worker = Some(Worker::spawn(cfg, cache, BAKE_TURN));
            self.sent.clear();
        }
        let incoming = self.worker.as_ref().map(|w| w.drain(DRAIN_BYTES)).unwrap_or_default();
        let bake_nanos = self.worker.as_ref().map_or(0, Worker::take_bake_nanos);
        let incoming_bytes: usize = incoming.iter().map(tile_bytes).sum();
        let incoming_count = incoming.len();
        let (eye_body, view_proj) = body_view_proj(view.eye_m, view.forward, view.projection);
        // Tiles drained this frame upload before `prepare`, so they count as ready now.
        let ready = |id: TileId| globe.resident_contains(id) || incoming.iter().any(|t| t.id == id);
        let max_draw = lean_limits(crate::render::lean_shading()).0;
        let planes = super::streaming::extract_planes(view_proj);
        let surface_mode = if cfg.is_planet() { globe::surface_mode() } else { 0 };
        let sample_scale = if surface_mode == 1 { 2.0 } else { 1.0 };
        if self.plan_with_maturity(&ready, &|id| globe.can_refine(id), eye_body, f64::from(view.focal_px) * sample_scale, &planes[..4], max_draw)
            && let Some(worker) = &self.worker
        {
            worker.request(&self.sent);
        }
        let frame = chart::frame(view.eye_m);
        let sun = (frame * view.sun_dir.as_dvec3()).normalize_or_zero().as_vec3();
        let moon = (frame * view.moon_dir.as_dvec3()).normalize_or_zero().as_vec3();
        let shell_bytes = self.refresh_shell(globe, queue, view, eye_body, incoming_bytes, factory_clusters);
        let eye = eye_body.as_vec3();
        let uniform = GlobeUniform {
            eye_body: [eye.x, eye.y, eye.z, self.light_count as f32],
            // The sky and disc must animate their shared emission field on the
            // same clock; the worker refresh cadence still uses wall time.
            sun_dir_body: [sun.x, sun.y, sun.z, view.sky_time_s.rem_euclid(3600.0)],
            radius_m: chart::RADIUS_M as f32,
            // Baked heights are world metres, so the sea surface sits at the water level, not 0.
            sea_level_m: cfg.world_y(cfg.water_level()),
            focal_px: view.focal_px,
            flags: u32::from(crate::render::lean_shading()) | if globe.shell_ready() && crate::render::draw_clouds() && !hell_planet(cfg) { 2 } else { 0 } | if self.light_count > 0 { 4 } else { 0 }
                | if hell_planet(cfg) { 8 } else { 0 }
                | if engine_worldgen::planetary::tidal_coasts(&cfg) { 16 } else { 0 }
                | if globe::reference_cloud_air() { 32 } else { 0 }
                | if cfg.is_planet() { 64 } else { 0 }
                | ((view.debug_view & 0xFF) << 8) | (surface_mode << 16),
            moon_body: [moon.x, moon.y, moon.z, (view.moon_intensity * 20.0).min(1.0)],
            view_proj: view_proj.to_cols_array_2d(),
        };
        let frame_stats = globe.frame(queue, incoming, &self.selection, &self.keep, eye_body, uniform);
        if !self.ground_ready {
            // The keep set protects roots until the next world reset.
            self.ground_ready = (0..6).map(TileId::root).all(|id| globe.resident_contains(id));
        }
        if frame_stats.dropped > 0 {
            // A dropped tile stays unready while the list may not change, so resend next frame.
            self.sent.clear();
        }
        // Every tile has the same fixed size, so dropped bytes are a whole number of tiles.
        let per_tile = incoming_bytes.checked_div(incoming_count).unwrap_or(0);
        self.stats = GlobeStats {
            resident: frame_stats.resident,
            drawn: frame_stats.drawn,
            wanted: self.selection.wanted.len(),
            upload_bytes: incoming_bytes - per_tile * frame_stats.dropped + shell_bytes,
            cpu_ms: start.elapsed().as_secs_f32() * 1000.0,
            dropped: frame_stats.dropped,
            bake_ms: bake_nanos as f32 / 1.0e6,
            cloud_bake_ms: self.shell_worker.as_ref().map_or(0, ShellWorker::take_bake_nanos) as f32 / 1.0e6,
            cloud_pending: self.shell_in_flight.count_ones(),
            cloud_resolution: self.shell_bake_resolution,
            night_lights: self.light_count,
        };
    }
}

#[cfg(test)]
#[path = "globe_driver_tests.rs"]
mod tests;
