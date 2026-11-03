//! Shell face uploads and the shell and night-light cadence of the globe driver.

use super::super::globe::chart;
use super::super::globe::gpu::Globe;
use super::super::globe::shell::ShellConfig;
use super::super::globe::shell_worker::ShellWorker;
use super::{DRAIN_BYTES, GlobeDriver, GlobeView, due};
use glam::DVec3;

/// Seconds between shell rebakes and night-light rebuilds.
const SHELL_S: f64 = 30.0;
/// Angle from a face normal to its corner, `acos(1 / sqrt(3))`.
const FACE_CORNER_RAD: f64 = 0.955_316_6;

/// Whether any of `face` can be in front of the limb seen from `eye_body`.
pub fn face_in_front(face: u8, eye_body: DVec3) -> bool {
    let normal = engine_core::cube_sphere::Face::ALL[usize::from(face) % 6].basis().n.as_dvec3();
    let horizon = (chart::RADIUS_M / eye_body.length().max(chart::RADIUS_M)).acos();
    eye_body.normalize_or_zero().dot(normal).clamp(-1.0, 1.0).acos() < horizon + FACE_CORNER_RAD
}

/// Faces that fit in what the tile drain left of `budget`; the rest wait a frame.
pub fn shell_faces_within(budget: usize, tile_bytes: usize, face_bytes: usize, pending: usize) -> usize {
    if face_bytes == 0 {
        return pending;
    }
    (budget.saturating_sub(tile_bytes) / face_bytes).min(pending)
}

impl GlobeDriver {
    /// Moves the faces that fit the frame budget into `shell_ready` and returns their bytes.
    pub(super) fn take_ready_faces(&mut self, tile_bytes: usize) -> usize {
        self.shell_ready.clear();
        let face_bytes = self.shell_pending.front().map_or(0, |b| b.coverage.len());
        let n = shell_faces_within(DRAIN_BYTES, tile_bytes, face_bytes, self.shell_pending.len());
        self.shell_ready.extend(self.shell_pending.drain(..n));
        self.shell_ready.iter().map(|b| b.coverage.len()).sum()
    }

    /// Uploads shell faces and finished lights within the frame budget, returning face bytes;
    /// every `SHELL_S` requests faces in front of the limb (all six the first time) and lights.
    pub(super) fn refresh_shell(
        &mut self,
        globe: &mut Globe,
        queue: &wgpu::Queue,
        view: &GlobeView,
        eye_body: DVec3,
        tile_bytes: usize,
        factory_clusters: &dyn Fn() -> Vec<(DVec3, u32)>,
    ) -> usize {
        if self.shell_worker.as_ref().is_some_and(|w| !w.alive()) {
            self.shell_worker = None;
        }
        if self.shell_worker.is_none() && self.last_shell.is_none() {
            self.shell_worker = ShellWorker::spawn();
        }
        let lights = self.shell_worker.as_ref().and_then(|w| w.drain_into(&mut self.shell_pending));
        if let Some(lights) = lights {
            self.light_count = globe.upload_lights(queue, &lights);
        }
        let uploaded = self.take_ready_faces(tile_bytes);
        globe.upload_shell(queue, &self.shell_ready);
        self.shell_ready.clear();
        if !due(self.last_shell, view.clock_s, SHELL_S) {
            return uploaded;
        }
        let first = self.last_shell.is_none();
        self.last_shell = Some(view.clock_s);
        if let Some(worker) = &self.shell_worker {
            let cfg = ShellConfig { resolution: globe.shell_resolution(), controls: view.clouds, weather_seed: view.weather_seed };
            for face in (0..6u8).filter(|f| first || face_in_front(*f, eye_body)) {
                worker.request(cfg, view.sky_time_s, face);
            }
            if let Some(seed) = self.built_for.map(|c| c.seed) {
                worker.request_lights(seed, factory_clusters(), eye_body);
            }
        }
        uploaded
    }
}
