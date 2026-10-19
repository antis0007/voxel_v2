use glam::{IVec2, Vec2};

use super::CloudControls;

pub const WEATHER_TEXTURE_RESOLUTION: u32 = 128;
pub const WEATHER_CLIPMAP_LEVEL_COUNT: usize = 3;
// Clipmap spans scaled for a 512m-tall voxel world where clouds are at 80-550m.
// Level 0: high detail near player. Level 2: coverage to the visible horizon.
pub const WEATHER_CLIPMAP_SPANS_METERS: [f32; WEATHER_CLIPMAP_LEVEL_COUNT] =
    [1_024.0, 4_096.0, 65_536.0];
const WEATHER_TEXTURE_TEXEL_BYTES: u32 = std::mem::size_of::<[f32; 4]>() as u32;

#[derive(Clone, Copy, Debug, Default)]
pub struct WeatherRegion {
    pub origin_m: Vec2,
    pub span_m: f32,
    pub resolution: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WeatherFieldSummary {
    pub avg_coverage: f32,
    pub avg_storm: f32,
    pub avg_precipitation: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct WeatherClipmapDescriptor {
    pub level: usize,
    pub span_m: f32,
    pub resolution: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeatherUploadRect {
    pub texel_x: u32,
    pub texel_y: u32,
    pub texel_width: u32,
    pub texel_height: u32,
}

impl WeatherUploadRect {
    fn full_level(resolution: u32) -> Self {
        Self {
            texel_x: 0,
            texel_y: 0,
            texel_width: resolution,
            texel_height: resolution,
        }
    }

    fn is_empty(self) -> bool {
        self.texel_width == 0 || self.texel_height == 0
    }

    pub fn packed_bytes_per_row(self) -> u32 {
        self.texel_width * WEATHER_TEXTURE_TEXEL_BYTES
    }

    pub fn rows_per_image(self) -> u32 {
        self.texel_height
    }
}

#[derive(Clone, Debug, Default)]
pub struct WeatherClipmapUploadBatch {
    pub clipmap_level: usize,
    pub invalidate_history: bool,
    dirty_rects: Vec<WeatherUploadRect>,
}

impl WeatherClipmapUploadBatch {
    pub fn array_layer(&self) -> u32 {
        self.clipmap_level as u32
    }

    pub fn dirty_rects(&self) -> &[WeatherUploadRect] {
        &self.dirty_rects
    }
}

#[derive(Clone, Debug, Default)]
pub struct CloudWeatherUploadManifest {
    clipmap_uploads: Vec<WeatherClipmapUploadBatch>,
}

impl CloudWeatherUploadManifest {
    pub fn clipmap_uploads(&self) -> &[WeatherClipmapUploadBatch] {
        &self.clipmap_uploads
    }

    pub fn is_empty(&self) -> bool {
        self.clipmap_uploads.is_empty()
    }
}

#[derive(Debug)]
pub struct WeatherClipmapLevel {
    descriptor: WeatherClipmapDescriptor,
    region: WeatherRegion,
    summary: WeatherFieldSummary,
    primary_rgba: Vec<[f32; 4]>,
    secondary_rgba: Vec<[f32; 4]>,
    shadow_rgba: Vec<[f32; 4]>,
    pending_gpu_uploads: Vec<WeatherUploadRect>,
    pending_history_reset: bool,
    last_controls: CloudControls,
    last_seed: u32,
    last_time_s: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WeatherClipmapGpuMeta {
    pub origin_x: f32,
    pub origin_z: f32,
    pub span_m: f32,
    pub resolution: f32,
    pub avg_coverage: f32,
    pub avg_storm: f32,
    pub avg_precipitation: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct WeatherSample {
    primary: [f32; 4],
    secondary: [f32; 4],
    shadow: [f32; 4],
}

#[derive(Debug)]
pub struct CloudWeatherField {
    levels: [WeatherClipmapLevel; WEATHER_CLIPMAP_LEVEL_COUNT],
}

pub struct WeatherRuntimeState {
    pub simulation_time_s: f32,
    pub field: CloudWeatherField,
}

impl WeatherRuntimeState {
    pub fn new(seed: u32) -> Self {
        Self {
            simulation_time_s: 0.0,
            field: CloudWeatherField::new(seed),
        }
    }

    pub fn reset(&mut self, seed: u32) {
        self.simulation_time_s = 0.0;
        self.field.reset(seed);
    }

    pub fn advance(
        &mut self,
        dt_real_s: f32,
        center_world_xz_m: Vec2,
        authoring: &CloudControls,
        weather_seed: u32,
        sky_time_s: f32,
    ) -> Option<CloudWeatherUploadManifest> {
        // Accumulate real time for future slow weather evolution (storm buildup etc).
        // The 0.5 s fixed-step gate was removed: time no longer rebuilds the texture
        // (advection was decoupled to the shader), so gating was just adding latency.
        self.simulation_time_s += dt_real_s.max(0.0);

        let wind_angle = authoring.wind_dir_angle.to_radians();
        let wind = Vec2::new(wind_angle.cos(), wind_angle.sin());
        let advection = wind * sky_time_s * authoring.advection * authoring.wind_speed * 8.0;
        self.field.update(
            center_world_xz_m + advection,
            self.simulation_time_s,
            *authoring,
            weather_seed,
        );
        self.field.take_gpu_upload_manifest()
    }
}

impl WeatherClipmapLevel {
    fn new(level: usize, span_m: f32, resolution: u32, seed: u32) -> Self {
        let texel_count = (resolution * resolution) as usize;
        let mut level = Self {
            descriptor: WeatherClipmapDescriptor {
                level,
                span_m,
                resolution,
            },
            region: WeatherRegion {
                origin_m: Vec2::ZERO,
                span_m,
                resolution,
            },
            summary: WeatherFieldSummary::default(),
            primary_rgba: vec![[0.0; 4]; texel_count],
            secondary_rgba: vec![[0.0; 4]; texel_count],
            shadow_rgba: vec![[0.0; 4]; texel_count],
            pending_gpu_uploads: Vec::new(),
            pending_history_reset: true,
            last_controls: CloudControls::default(),
            last_seed: seed,
            last_time_s: -1.0,
        };
        level.update(Vec2::ZERO, 0.0, CloudControls::default(), seed);
        level
    }

    fn descriptor(&self) -> WeatherClipmapDescriptor {
        self.descriptor
    }

    fn region(&self) -> WeatherRegion {
        self.region
    }

    fn summary(&self) -> WeatherFieldSummary {
        self.summary
    }

    fn gpu_meta(&self) -> WeatherClipmapGpuMeta {
        WeatherClipmapGpuMeta {
            origin_x: self.region.origin_m.x,
            origin_z: self.region.origin_m.y,
            span_m: self.region.span_m.max(1.0),
            resolution: self.region.resolution.max(1) as f32,
            avg_coverage: self.summary.avg_coverage,
            avg_storm: self.summary.avg_storm,
            avg_precipitation: self.summary.avg_precipitation,
        }
    }

    fn update(
        &mut self,
        center_world_xz_m: Vec2,
        time_s: f32,
        controls: CloudControls,
        weather_seed: u32,
    ) {
        let texel_size_m = self.texel_size_m();

        // Keep clipmaps snapped, but much less coarsely than before so the field
        // stays world-anchored without feeling like large rectangular slabs.
        let snap_m = texel_size_m;
        let min_corner = center_world_xz_m - Vec2::splat(self.region.span_m * 0.5);
        let snapped_origin = Vec2::new(
            (min_corner.x / snap_m).floor() * snap_m,
            (min_corner.y / snap_m).floor() * snap_m,
        );

        let origin_changed = self.region.origin_m != snapped_origin;
        let seed_changed = self.last_seed != weather_seed;
        let controls_changed = self.last_controls != controls;
        self.pending_history_reset |= seed_changed || controls_changed;
        // Time no longer drives weather values (advection removed from evaluate_sample),
        // so time changes do not require a texture rebuild.

        if !(origin_changed || seed_changed || controls_changed) {
            return;
        }

        if origin_changed && !seed_changed && !controls_changed {
            // Reuse CPU samples, but upload the shifted interior too: the GPU
            // texture has no toroidal addressing or copy of the old interior.
            let texel_offset = IVec2::new(
                ((snapped_origin.x - self.region.origin_m.x) / texel_size_m).round() as i32,
                ((snapped_origin.y - self.region.origin_m.y) / texel_size_m).round() as i32,
            );
            self.scroll_to_origin(snapped_origin, time_s, texel_offset, controls, weather_seed);
        } else {
            // Seed or controls changed — full rebuild required.
            self.full_refresh(snapped_origin, time_s, controls, weather_seed);
        }

        self.last_controls = controls;
        self.last_seed = weather_seed;
        self.last_time_s = time_s;
    }

    fn texel_size_m(&self) -> f32 {
        self.region.span_m / self.region.resolution as f32
    }

    fn texel_index(&self, x: usize, y: usize) -> usize {
        y * self.region.resolution as usize + x
    }

    fn texel_world_position(&self, origin_m: Vec2, x: usize, y: usize) -> Vec2 {
        let texel_size_m = self.texel_size_m();
        origin_m + Vec2::new(x as f32 + 0.5, y as f32 + 0.5) * texel_size_m
    }

    fn evaluate_sample(
        &self,
        world_xz_m: Vec2,
        _time_s: f32, // reserved for future slow weather evolution; advection is in-shader
        controls: CloudControls,
        weather_seed: u32,
    ) -> WeatherSample {
        let wind_angle = controls.wind_dir_angle.to_radians();
        let wind_dir = Vec2::new(wind_angle.cos(), wind_angle.sin()).normalize_or_zero();
        let side_dir = Vec2::new(-wind_dir.y, wind_dir.x);

        // Tuned for a 512m-tall voxel world.  Cloud features should be
        // 60-300m across so they're visible and varied from the ground.
        let cloud_size_m = 60.0 + controls.noise_scale * 240.0;

        let macro_freq = 1.0 / (cloud_size_m * 8.0);
        let humidity_freq = 1.0 / (cloud_size_m * 5.5);
        let front_freq = 1.0 / (cloud_size_m * 18.0);
        let mesoscale_freq = 1.0 / (cloud_size_m * 2.5);
        let temperature_freq = 1.0 / (cloud_size_m * 12.0);

        // Weather texture is a static spatial field — NO time-based advection here.
        // Cloud movement is handled entirely by the shader's wx_offset_march term
        // (p.xz + wind * sky_time * wind_speed * layer_mult). Baking advection into
        // the CPU texture caused two problems:
        //   1. The CPU advection rate differed from the shader rate → coverage pattern
        //      drifted relative to cloud shapes → visible misalignment seams.
        //   2. Discrete 2Hz texture rebuilds made clouds jump every 0.5 s.
        // Now the texture encodes stable world-space weather structure (fronts, humid
        // regions). The shader scrolls through it continuously with sky_time.

        let shear_offset = side_dir * ((controls.wind_shear - 0.5) * 80.0);
        let base_domain = world_xz_m + shear_offset;

        // Domain warp: break up regularity with large-scale displacement.
        // Scale inversely with cloud_size_m so warp features match weather scale.
        let warp_freq = 1.0 / (cloud_size_m * 6.0);
        let warp_amplitude = cloud_size_m * 0.6;
        let macro_warp = Vec2::new(
            (fbm2(
                base_domain * warp_freq + Vec2::new(13.1, 7.3),
                weather_seed ^ 0x1357_2468,
                4,
            ) - 0.5)
                * warp_amplitude,
            (fbm2(
                base_domain * warp_freq + Vec2::new(-9.4, 21.7),
                weather_seed ^ 0x2468_1357,
                4,
            ) - 0.5)
                * warp_amplitude,
        );

        let system_domain = base_domain + macro_warp;
        let front_domain = system_domain + side_dir * 200.0;
        let detail_domain = world_xz_m + macro_warp * 0.45;

        let macro_mass = fbm2(system_domain * macro_freq, weather_seed ^ 0x9E37_79B9, 5);
        let humidity_noise = fbm2(
            system_domain * humidity_freq + Vec2::new(17.3, 5.1),
            weather_seed ^ 0xDEAD_BEEF,
            4,
        );
        let frontal = ridge(fbm2(
            front_domain * front_freq + Vec2::new(41.0, -18.0),
            weather_seed ^ 0xCAFE_BABE,
            4,
        ));
        let mesoscale = fbm2(
            detail_domain * mesoscale_freq + Vec2::new(3.7, 11.5),
            weather_seed ^ 0xBADC_0FFE,
            4,
        );
        let breakup = fbm2(
            (detail_domain + Vec2::new(91.0, -47.0)) * (mesoscale_freq * 1.85),
            weather_seed ^ 0x3141_5926,
            3,
        );
        let temperature_proxy = fbm2(
            (system_domain - wind_dir * 1_400.0) * temperature_freq + Vec2::new(31.0, 7.0),
            weather_seed ^ 0x0D15_EA5E,
            4,
        );

        let humidity = saturate(
            0.22
                + macro_mass * 0.34
                + humidity_noise * 0.30
                + frontal * 0.14
                + controls.stratus * 0.08,
        );

        let instability = saturate(
            (temperature_proxy - 0.40) * 1.25
                + mesoscale * 0.34
                + controls.cumulus * 0.22
                + controls.cumulonimbus * 0.40
                - controls.stratus * 0.18,
        );

        let convection = saturate(
            instability * (0.46 + humidity * 0.54)
                + frontal * 0.10
                + controls.wind_shear * 0.10,
        );

        // The storm preset must actually build storm cells. The previous
        // subtractive threshold cancelled most authored storms before upload.
        let storm = saturate(
            controls.cumulonimbus
                * (0.35 + humidity * 0.35 + convection * 0.30)
                * smoothstep(0.12, 0.55, humidity * convection + frontal * 0.15),
        );

        let coverage = saturate(
            controls.coverage * (0.18 + macro_mass * 1.08)
                + humidity * 0.12
                + frontal * 0.18
                - 0.16,
        );

        let type_weight = controls.stratus + controls.cumulus + controls.cumulonimbus;
        let type_accum =
            controls.stratus * 0.0 + controls.cumulus * 0.66 + controls.cumulonimbus * 1.0;
        let base_type = if type_weight > 0.0 {
            type_accum / type_weight
        } else {
            0.33 + convection * 0.33
        };
        let cloud_type = saturate(base_type + (mesoscale - 0.5) * 0.18);

        let low_mask = smoothstep(0.28, 0.72, breakup * 0.55 + mesoscale * 0.45);

        let low_amount = saturate(
            coverage
                * (0.55 + low_mask * 0.45)
                * (0.45
                    + controls.stratus * 0.25
                    + controls.cumulus * 0.35
                    + controls.cumulonimbus * 0.35)
                * (0.75 + humidity * 0.25),
        );

        let mid_amount = saturate(
            (controls.altocumulus * 0.52 + controls.altostratus * 0.88)
                * (0.26 + humidity * 0.38 + frontal * 0.26 + controls.density * 0.16),
        );

        let high_amount = saturate(
            controls.cirrus
                * (0.44 + (1.0 - humidity) * 0.22 + frontal * 0.36 + controls.density * 0.08),
        );

        let precipitation = saturate(storm * humidity * (0.54 + controls.cumulonimbus * 0.46));
        let density_gain = 0.25 + controls.density * 1.05;

        let low_optical_depth = (low_amount
            * density_gain
            * controls.layer_thickness
            * (1.0 + controls.stratus * 0.80 + storm * 2.0) * 2.8)
            .clamp(0.0, 4.0);

        let mid_optical_depth = (coverage
            * mid_amount
            * density_gain
            * (0.16 + humidity * 0.24 + controls.altostratus * 0.26 + frontal * 0.12))
            .clamp(0.0, 1.6);

        let high_optical_depth = (high_amount
            * (0.06 + controls.cirrus * 0.16 + frontal * 0.12 + (1.0 - humidity) * 0.08))
            .clamp(0.0, 0.75);

        WeatherSample {
            primary: [coverage, cloud_type, humidity, convection],
            secondary: [storm, low_amount, mid_amount, high_amount],
            shadow: [
                low_optical_depth,
                mid_optical_depth,
                high_optical_depth,
                precipitation,
            ],
        }
    }

    fn write_sample(&mut self, index: usize, sample: WeatherSample) {
        self.primary_rgba[index] = sample.primary;
        self.secondary_rgba[index] = sample.secondary;
        self.shadow_rgba[index] = sample.shadow;
    }

    fn mark_full_level_dirty(&mut self) {
        self.pending_gpu_uploads.clear();
        self.pending_gpu_uploads
            .push(WeatherUploadRect::full_level(self.region.resolution));
    }

    fn push_dirty_rect(&mut self, rect: WeatherUploadRect) {
        if rect.is_empty() {
            return;
        }

        let full_level_rect = WeatherUploadRect::full_level(self.region.resolution);
        if self.pending_gpu_uploads.first() == Some(&full_level_rect) {
            return;
        }
        if rect == full_level_rect {
            self.mark_full_level_dirty();
            return;
        }
        if self
            .pending_gpu_uploads
            .iter()
            .any(|existing| *existing == rect)
        {
            return;
        }
        self.pending_gpu_uploads.push(rect);
    }

    fn mark_scroll_dirty_uploads(&mut self, texel_offset: IVec2) {
        let resolution = self.region.resolution;
        let dirty_width = texel_offset.x.abs() as u32;
        let dirty_height = texel_offset.y.abs() as u32;

        if dirty_width > 0 {
            let texel_x = if texel_offset.x > 0 {
                resolution - dirty_width
            } else {
                0
            };
            self.push_dirty_rect(WeatherUploadRect {
                texel_x,
                texel_y: 0,
                texel_width: dirty_width,
                texel_height: resolution,
            });
        }

        if dirty_height > 0 {
            // Avoid duplicating the newly exposed corner when the clipmap scrolls diagonally.
            let texel_x = if texel_offset.x < 0 { dirty_width } else { 0 };
            let texel_width = resolution.saturating_sub(dirty_width);
            let texel_y = if texel_offset.y > 0 {
                resolution - dirty_height
            } else {
                0
            };
            self.push_dirty_rect(WeatherUploadRect {
                texel_x,
                texel_y,
                texel_width,
                texel_height: dirty_height,
            });
        }
    }

    fn take_gpu_upload_batch(&mut self) -> Option<WeatherClipmapUploadBatch> {
        if self.pending_gpu_uploads.is_empty() {
            return None;
        }
        Some(WeatherClipmapUploadBatch {
            clipmap_level: self.descriptor.level,
            invalidate_history: std::mem::take(&mut self.pending_history_reset),
            dirty_rects: std::mem::take(&mut self.pending_gpu_uploads),
        })
    }

    fn pack_upload_bytes(
        &self,
        bytes: &[[f32; 4]],
        rect: WeatherUploadRect,
        packed_bytes: &mut Vec<u8>,
    ) {
        packed_bytes.clear();
        debug_assert!(
            rect.texel_x + rect.texel_width <= self.region.resolution
                && rect.texel_y + rect.texel_height <= self.region.resolution
        );
        if rect.is_empty() {
            return;
        }

        let resolution = self.region.resolution as usize;
        let texel_x = rect.texel_x as usize;
        let texel_y = rect.texel_y as usize;
        let texel_width = rect.texel_width as usize;
        let texel_height = rect.texel_height as usize;
        let packed_row_bytes = rect.packed_bytes_per_row() as usize;
        packed_bytes.reserve(packed_row_bytes * texel_height);

        for row in 0..texel_height {
            let row_start = (texel_y + row) * resolution + texel_x;
            let row_end = row_start + texel_width;
            packed_bytes.extend_from_slice(bytemuck::cast_slice(&bytes[row_start..row_end]));
        }
    }

    fn pack_primary_upload_bytes(&self, rect: WeatherUploadRect, packed_bytes: &mut Vec<u8>) {
        self.pack_upload_bytes(&self.primary_rgba, rect, packed_bytes);
    }

    fn pack_secondary_upload_bytes(&self, rect: WeatherUploadRect, packed_bytes: &mut Vec<u8>) {
        self.pack_upload_bytes(&self.secondary_rgba, rect, packed_bytes);
    }

    fn pack_shadow_upload_bytes(&self, rect: WeatherUploadRect, packed_bytes: &mut Vec<u8>) {
        self.pack_upload_bytes(&self.shadow_rgba, rect, packed_bytes);
    }

    fn full_refresh(
        &mut self,
        origin_m: Vec2,
        time_s: f32,
        controls: CloudControls,
        weather_seed: u32,
    ) {
        self.region.origin_m = origin_m;
        let resolution = self.region.resolution as usize;
        for y in 0..resolution {
            for x in 0..resolution {
                let index = self.texel_index(x, y);
                let world_xz_m = self.texel_world_position(origin_m, x, y);
                let sample = self.evaluate_sample(world_xz_m, time_s, controls, weather_seed);
                self.write_sample(index, sample);
            }
        }
        self.mark_full_level_dirty();
        self.rebuild_summary();
    }

    fn scroll_to_origin(
        &mut self,
        origin_m: Vec2,
        time_s: f32,
        texel_offset: IVec2,
        controls: CloudControls,
        weather_seed: u32,
    ) {
        let resolution = self.region.resolution as i32;
        if texel_offset.x.abs() >= resolution || texel_offset.y.abs() >= resolution {
            self.full_refresh(origin_m, time_s, controls, weather_seed);
            return;
        }

        let previous_primary = self.primary_rgba.clone();
        let previous_secondary = self.secondary_rgba.clone();
        let previous_shadow = self.shadow_rgba.clone();
        self.region.origin_m = origin_m;

        for y in 0..resolution {
            for x in 0..resolution {
                let index = self.texel_index(x as usize, y as usize);
                let source_x = x + texel_offset.x;
                let source_y = y + texel_offset.y;
                if source_x >= 0 && source_x < resolution && source_y >= 0 && source_y < resolution
                {
                    let source_index = self.texel_index(source_x as usize, source_y as usize);
                    self.primary_rgba[index] = previous_primary[source_index];
                    self.secondary_rgba[index] = previous_secondary[source_index];
                    self.shadow_rgba[index] = previous_shadow[source_index];
                } else {
                    let world_xz_m = self.texel_world_position(origin_m, x as usize, y as usize);
                    let sample = self.evaluate_sample(world_xz_m, time_s, controls, weather_seed);
                    self.write_sample(index, sample);
                }
            }
        }

        self.mark_full_level_dirty();
        self.rebuild_summary();
    }

    fn rebuild_summary(&mut self) {
        let texel_count = self.primary_rgba.len().max(1) as f32;
        let mut sum_coverage = 0.0;
        let mut sum_storm = 0.0;
        let mut sum_precipitation = 0.0;

        for texel_index in 0..self.primary_rgba.len() {
            sum_coverage += self.primary_rgba[texel_index][0];
            sum_storm += self.secondary_rgba[texel_index][0];
            sum_precipitation += self.shadow_rgba[texel_index][3];
        }

        self.summary = WeatherFieldSummary {
            avg_coverage: sum_coverage / texel_count,
            avg_storm: sum_storm / texel_count,
            avg_precipitation: sum_precipitation / texel_count,
        };
    }
}

impl CloudWeatherField {
    pub fn new(seed: u32) -> Self {
        Self {
            levels: std::array::from_fn(|level| {
                WeatherClipmapLevel::new(
                    level,
                    WEATHER_CLIPMAP_SPANS_METERS[level],
                    WEATHER_TEXTURE_RESOLUTION,
                    seed,
                )
            }),
        }
    }

    pub fn update(
        &mut self,
        center_world_xz_m: Vec2,
        time_s: f32,
        controls: CloudControls,
        weather_seed: u32,
    ) {
        for level in &mut self.levels {
            level.update(center_world_xz_m, time_s, controls, weather_seed);
        }
    }

    pub fn reset(&mut self, seed: u32) {
        *self = Self::new(seed);
    }

    pub fn clipmap_count(&self) -> usize {
        self.levels.len()
    }

    pub fn descriptor(&self, level: usize) -> WeatherClipmapDescriptor {
        self.levels[level].descriptor()
    }

    pub fn region(&self, level: usize) -> WeatherRegion {
        self.levels[level].region()
    }

    pub fn summary(&self, level: usize) -> WeatherFieldSummary {
        self.levels[level].summary()
    }

    pub fn gpu_meta(&self) -> [WeatherClipmapGpuMeta; WEATHER_CLIPMAP_LEVEL_COUNT] {
        std::array::from_fn(|level| self.levels[level].gpu_meta())
    }

    pub fn take_gpu_upload_manifest(&mut self) -> Option<CloudWeatherUploadManifest> {
        let mut clipmap_uploads = Vec::new();
        for level in &mut self.levels {
            if let Some(upload_batch) = level.take_gpu_upload_batch() {
                clipmap_uploads.push(upload_batch);
            }
        }
        let manifest = CloudWeatherUploadManifest { clipmap_uploads };
        if manifest.is_empty() {
            None
        } else {
            Some(manifest)
        }
    }

    pub fn pack_primary_upload_bytes(
        &self,
        level: usize,
        rect: WeatherUploadRect,
        packed_bytes: &mut Vec<u8>,
    ) {
        self.levels[level].pack_primary_upload_bytes(rect, packed_bytes);
    }

    pub fn pack_secondary_upload_bytes(
        &self,
        level: usize,
        rect: WeatherUploadRect,
        packed_bytes: &mut Vec<u8>,
    ) {
        self.levels[level].pack_secondary_upload_bytes(rect, packed_bytes);
    }

    pub fn pack_shadow_upload_bytes(
        &self,
        level: usize,
        rect: WeatherUploadRect,
        packed_bytes: &mut Vec<u8>,
    ) {
        self.levels[level].pack_shadow_upload_bytes(rect, packed_bytes);
    }
}

fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

fn ridge(x: f32) -> f32 {
    1.0 - (x * 2.0 - 1.0).abs()
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn fbm2(p: Vec2, seed: u32, octaves: usize) -> f32 {
    let mut value = 0.0;
    let mut amp = 0.55;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for octave in 0..octaves {
        let octave_seed = seed ^ ((octave as u32 + 1).wrapping_mul(0x9E37_79B9));
        value += value_noise2(p * freq, octave_seed) * amp;
        norm += amp;
        freq *= 2.07;
        amp *= 0.5;
    }
    if norm <= 0.0 { 0.0 } else { value / norm }
}

fn value_noise2(p: Vec2, seed: u32) -> f32 {
    let cell = p.floor();
    // Rust/glam fract() subtracts trunc(), so it is negative below zero.
    // Lattice interpolation requires a [0, 1) fraction on both sides of origin.
    let frac = p - cell;
    let u = frac * frac * (Vec2::splat(3.0) - 2.0 * frac);

    let c00 = hash2(cell.x as i32, cell.y as i32, seed);
    let c10 = hash2(cell.x as i32 + 1, cell.y as i32, seed);
    let c01 = hash2(cell.x as i32, cell.y as i32 + 1, seed);
    let c11 = hash2(cell.x as i32 + 1, cell.y as i32 + 1, seed);

    let x0 = c00 + (c10 - c00) * u.x;
    let x1 = c01 + (c11 - c01) * u.x;
    x0 + (x1 - x0) * u.y
}

fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ seed;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    h as f32 / u32::MAX as f32
}
