//! Shell face uploads and the shell and night-light cadence of the globe driver.

use super::super::globe::chart;
use super::super::globe::gpu::Globe;
use super::super::globe::shell::{ShellConfig, SHELL_REFRESH_S};
use super::super::globe::shell_worker::ShellWorker;
use super::{DRAIN_BYTES, GlobeDriver, GlobeView, due};
use glam::DVec3;

/// Clouds refresh independently of the more expensive settlement-light walk.
const LIGHTS_S: f64 = 30.0;
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
    let mut admission = engine_gpu::transfer::UploadBudget::new(budget);
    if !admission.reserve(tile_bytes) { return 0; }
    (admission.remaining() / face_bytes).min(pending)
}

impl GlobeDriver {
    /// Moves the faces that fit the frame budget into `shell_ready` and returns their bytes.
    pub(super) fn take_ready_faces(&mut self, tile_bytes: usize) -> usize {
        self.shell_ready.clear();
        // Reserve both mip chains at the supported maximum resolution.
        // Actual byte accounting below uses the old and new face resolutions.
        let face_bytes = self.shell_pending.front().map_or(0, |b|
            (engine_gpu::image::scalar_mip_bytes(512) + engine_gpu::image::scalar_mip_bytes(b.resolution)) * 4);
        // Avoid several face copies and coverage reconstructions in one frame.
        // One full-resolution RGBA pair can exceed the normal drain budget.
        // Admit that single bounded upload on a light terrain frame, or it can starve forever.
        let n = if face_bytes > DRAIN_BYTES && tile_bytes < DRAIN_BYTES / 4 {
            self.shell_pending.len().min(1)
        } else { shell_faces_within(DRAIN_BYTES, tile_bytes, face_bytes, self.shell_pending.len()).min(1) };
        self.shell_ready.extend(self.shell_pending.drain(..n));
        n * face_bytes
    }

    /// Uploads shell faces and finished lights within the frame budget, returning face bytes;
    /// every `SHELL_REFRESH_S` requests visible faces; lights retain their own cadence.
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
            self.last_shell = None;
            self.shell_in_flight = 0;
        }
        if self.shell_worker.is_none() && self.last_shell.is_none() {
            self.shell_worker = ShellWorker::spawn_for(self.built_for);
        }
        let lights = self.shell_worker.as_ref().and_then(|w| w.drain_into(&mut self.shell_pending));
        if let Some(lights) = lights {
            self.light_count = globe.upload_lights(queue, &lights);
        }
        let clouds_enabled = crate::render::draw_clouds()
            && !self.built_for.is_some_and(super::hell_planet);
        if !clouds_enabled {
            // Rendering off must also stop new field bakes and texture copies.
            // Drain jobs already in flight so re-enabling cannot leave a face
            // marked busy forever. Night lights still use this same worker.
            for face in self.shell_pending.drain(..) {
                self.shell_in_flight &= !(1 << face.face);
            }
            self.shell_ready.clear();
            self.last_shell = None;
        }
        // Keep completed bakes queued until their preceding blend has finished.
        let _reserved = if clouds_enabled && self.shell_pending.front().is_none_or(|b| globe.shell_can_upload(b.face)) {
            self.take_ready_faces(tile_bytes)
        } else { self.shell_ready.clear(); 0 };
        let uploaded = self.shell_ready.iter().map(|b| globe.shell_upload_bytes(b)).sum();
        globe.set_shell_wind(crate::world::wind_offset(&view.clouds, view.sky_time_s).to_array());
        globe.set_shell_flow(&view.clouds, view.sky_time_s);
        globe.set_shell_obstacles(self.built_for.as_ref().and_then(super::super::globe::shell_obstacles::weights));
        for face in &self.shell_ready {
            self.shell_in_flight &= !(1 << face.face);
        }
        globe.upload_shell(queue, self.shell_ready.drain(..));
        // Horizon-culling follows orbital travel, independently of weather bakes.
        let moved = self.last_light_eye.is_none_or(|old| old.distance(eye_body) > chart::RADIUS_M * 0.02);
        let lights_due = due(self.last_shell_lights, view.clock_s, LIGHTS_S)
            || (moved && due(self.last_shell_lights, view.clock_s, 1.0));
        if lights_due {
            if let (Some(worker), Some(cfg)) = (&self.shell_worker, self.built_for) {
                worker.request_lights(cfg.seed, factory_clusters(), eye_body);
                self.last_shell_lights = Some(view.clock_s);
                self.last_light_eye = Some(eye_body);
            }
        }
        if !clouds_enabled || !due(self.last_shell, view.clock_s, SHELL_REFRESH_S) {
            return uploaded;
        }
        self.last_shell = Some(view.clock_s);
        if let Some(worker) = &self.shell_worker {
            let diameter = engine_core::lod::sphere_diameter_px(chart::RADIUS_M, eye_body.length(), f64::from(view.focal_px));
            self.shell_bake_resolution = engine_core::lod::texture_resolution(diameter * 0.65, 64,
                globe.shell_resolution(), self.shell_bake_resolution);
            let cfg = ShellConfig { resolution: self.shell_bake_resolution, controls: view.clouds, weather_seed: view.weather_seed, days: view.weather_days };
            for face in (0..6u8).filter(|f| face_in_front(*f, eye_body)) {
                // At most one outstanding bake per face, including upload waits.
                if self.shell_in_flight & (1 << face) == 0 {
                    worker.request(cfg, view.sky_time_s, face);
                    self.shell_in_flight |= 1 << face;
                }
            }
        }
        uploaded
    }
}
