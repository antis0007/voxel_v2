//! Per-frame globe driver: drains bakes, selects tiles, uploads, evicts and records stats.

use super::globe::cache::TileCache;
use super::globe::gpu::{Globe, GlobeUniform, body_view_proj};
use super::globe::scheduler::{self, Selection};
use super::globe::tile::TileId;
use super::globe::worker::{Worker, tile_bytes};
use super::globe::{self, chart};
use crate::altitude::Tier;
use engine_voxel::WorldSampleConfig;
use glam::{DVec3, Mat4, Vec3};
use std::time::{Duration, Instant};

/// Per-frame upload cap; one 2 MB frame stays under a millisecond of copy on any adapter.
const DRAIN_BYTES: usize = 2 * 1024 * 1024;
const MAX_DRAW: usize = 512;
const MAX_REQUEST: usize = 64;
const BAKE_TURN: Duration = Duration::from_millis(2);
/// 30 s at 60 Hz; frames rather than wall time keep the drop off `Instant`.
const SURFACE_DROP_FRAMES: u32 = 1_800;

/// Globe counters for the last driven frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlobeStats {
    pub resident: usize,
    pub drawn: usize,
    pub wanted: usize,
    pub upload_bytes: usize,
    pub cpu_ms: f32,
    pub dropped: usize,
}

/// Camera inputs for one globe frame, all in chart metres.
pub struct GlobeView {
    pub eye_m: DVec3,
    pub forward: Vec3,
    /// Reverse-Z projection the terrain uses.
    pub projection: Mat4,
    pub sun_dir: Vec3,
    pub focal_px: f32,
}

pub struct GlobeDriver {
    worker: Option<Worker>,
    previous_draw: Vec<TileId>,
    keep: Vec<TileId>,
    request: Vec<TileId>,
    tier: Tier,
    surface_frames: u32,
    running: bool,
    stats: GlobeStats,
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
            running: false,
            stats: GlobeStats::default(),
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

    /// Whether the globe ran this frame from the orbit tier.
    pub fn orbit_view(&self) -> bool {
        self.running && self.tier == Tier::Orbit
    }

    pub fn stats(&self) -> GlobeStats {
        self.stats
    }

    /// Drain, select, request, then evict, upload and prepare through `Globe::frame`.
    pub fn update(
        &mut self,
        globe: &mut Globe,
        queue: &wgpu::Queue,
        cfg: WorldSampleConfig,
        save_dir: &std::path::Path,
        view: &GlobeView,
    ) {
        let start = Instant::now();
        self.running = globe::active() && (self.tier != Tier::Surface || globe::forced());
        if !self.running {
            self.stats = GlobeStats::default();
            if self.tier == Tier::Surface {
                self.surface_frames = self.surface_frames.saturating_add(1);
                // Dropping the worker closes the cache; a long surface stay should not hold it.
                if self.surface_frames >= SURFACE_DROP_FRAMES {
                    self.worker = None;
                }
            }
            return;
        }
        self.surface_frames = 0;
        let worker = self.worker.get_or_insert_with(|| {
            Worker::spawn(cfg, TileCache::open(save_dir, cfg.seed, cfg.generator_version), BAKE_TURN)
        });
        let incoming = worker.drain(DRAIN_BYTES);
        let upload_bytes = incoming.iter().map(tile_bytes).sum();
        let (eye_body, view_proj) = body_view_proj(view.eye_m, view.forward, view.projection);
        // Tiles drained this frame upload before `prepare`, so they count as ready now.
        let ready = |id: TileId| globe.resident_contains(id) || incoming.iter().any(|t| t.id == id);
        let selection: Selection =
            scheduler::select(&ready, &self.previous_draw, eye_body, f64::from(view.focal_px), MAX_DRAW);
        // `wanted` holds only missing children; a drawn tile that was never baked (the roots on
        // the first frame) would otherwise never be requested and the globe would stay empty.
        self.request.clear();
        self.request.extend(selection.draw.iter().copied().filter(|id| !ready(*id)));
        self.request.extend(selection.wanted.iter().copied());
        self.request.truncate(MAX_REQUEST);
        worker.request(&self.request);
        scheduler::keep_set(&selection.draw, &mut self.keep);
        let frame = chart::frame(view.eye_m);
        let sun = (frame * view.sun_dir.as_dvec3()).normalize_or_zero().as_vec3();
        let eye = eye_body.as_vec3();
        let uniform = GlobeUniform {
            eye_body: [eye.x, eye.y, eye.z, 0.0],
            sun_dir_body: [sun.x, sun.y, sun.z, 0.0],
            radius_m: chart::RADIUS_M as f32,
            sea_level_m: 0.0,
            focal_px: view.focal_px,
            flags: 0,
            limb: [0.0; 4],
            view_proj: view_proj.to_cols_array_2d(),
        };
        let frame_stats = globe.frame(queue, incoming, &selection, &self.keep, eye_body, uniform);
        self.stats = GlobeStats {
            resident: frame_stats.resident,
            drawn: frame_stats.drawn,
            wanted: selection.wanted.len(),
            upload_bytes,
            cpu_ms: start.elapsed().as_secs_f32() * 1000.0,
            dropped: frame_stats.dropped,
        };
        self.previous_draw = selection.draw;
    }
}
