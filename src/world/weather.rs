use glam::{IVec2, Vec2};

use super::CloudControls;

pub const WEATHER_TEXTURE_RESOLUTION: u32 = 128;
pub const WEATHER_CLIPMAP_LEVEL_COUNT: usize = 3;
pub const WEATHER_CLIPMAP_SPANS_METERS: [f32; WEATHER_CLIPMAP_LEVEL_COUNT] =
    [32_000.0, 96_000.0, 288_000.0];
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
    last_controls: CloudControls,
    last_seed: u32,
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
    pub accumulator_s: f32,
    pub simulation_time_s: f32,
    pub field: CloudWeatherField,
}

impl WeatherRuntimeState {
    pub fn new(seed: u32) -> Self {
        Self {
            accumulator_s: 0.0,
            simulation_time_s: 0.0,
            field: CloudWeatherField::new(seed),
        }
    }

    pub fn reset(&mut self, seed: u32) {
        self.accumulator_s = 0.0;
        self.simulation_time_s = 0.0;
        self.field.reset(seed);
    }

    pub fn advance(
        &mut self,
        dt_real_s: f32,
        center_world_xz_m: Vec2,
        authoring: &CloudControls,
        weather_seed: u32,
    ) -> Option<CloudWeatherUploadManifest> {
        let dt = dt_real_s.max(0.0);
        self.accumulator_s += dt;

        const WEATHER_DT_S: f32 = 0.5;
        const MAX_STEPS_PER_FRAME: usize = 4;
        let mut steps = 0usize;
        while self.accumulator_s >= WEATHER_DT_S && steps < MAX_STEPS_PER_FRAME {
            self.accumulator_s -= WEATHER_DT_S;
            self.simulation_time_s += WEATHER_DT_S;
            steps += 1;
        }
        if steps == MAX_STEPS_PER_FRAME {
            self.accumulator_s = self.accumulator_s.min(WEATHER_DT_S);
        }

        self.field.update(
            center_world_xz_m,
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
            last_controls: CloudControls::default(),
            last_seed: seed,
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
        _time_s: f32,
        controls: CloudControls,
        weather_seed: u32,
    ) {
        let texel_size_m = self.texel_size_m();
        let snap_m = texel_size_m * 8.0;
        let min_corner = center_world_xz_m - Vec2::splat(self.region.span_m * 0.5);
        let snapped_origin = Vec2::new(
            (min_corner.x / snap_m).floor() * snap_m,
            (min_corner.y / snap_m).floor() * snap_m,
        );
        let origin_changed = self.region.origin_m != snapped_origin;
        let controls_changed = self.last_controls != controls;
        let seed_changed = self.last_seed != weather_seed;
        let full_refresh_due = controls_changed || seed_changed;
        if !origin_changed && !full_refresh_due {
            return;
        }

        if full_refresh_due {
            self.full_refresh(snapped_origin, controls, weather_seed);
        } else if origin_changed {
            let origin_delta_m = snapped_origin - self.region.origin_m;
            let texel_offset = IVec2::new(
                (origin_delta_m.x / texel_size_m).round() as i32,
                (origin_delta_m.y / texel_size_m).round() as i32,
            );
            self.scroll_to_origin(snapped_origin, texel_offset, controls, weather_seed);
        }

        self.last_controls = controls;
        self.last_seed = weather_seed;
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
        controls: CloudControls,
        weather_seed: u32,
    ) -> WeatherSample {
        let wind_angle = controls.wind_dir_angle.to_radians();
        let wind_dir = Vec2::new(wind_angle.cos(), wind_angle.sin());
        let side_dir = Vec2::new(-wind_dir.y, wind_dir.x);
        let macro_freq = 0.000016;
        let humidity_freq = 0.000024;
        let front_freq = 0.000008;
        let mesoscale_freq = 0.000042;
        let temperature_freq = 0.000012;
        let advected = world_xz_m;
        let sheared = advected + side_dir * (6_000.0 * (controls.wind_shear - 0.5));

        let macro_mass = fbm2(sheared * macro_freq, weather_seed ^ 0x1357_2468, 4);
        let humidity_noise = fbm2(
            advected * humidity_freq + Vec2::new(17.3, 5.1),
            weather_seed ^ 0x2468_1357,
            4,
        );
        let frontal = ridge(fbm2(
            (world_xz_m + side_dir * 18_000.0) * front_freq,
            weather_seed ^ 0x9E37_79B9,
            3,
        ));
        let mesoscale = fbm2(
            (world_xz_m + wind_dir * 11_000.0) * mesoscale_freq + Vec2::new(3.7, 11.5),
            weather_seed ^ 0xDEAD_BEEF,
            3,
        );
        let temperature_proxy = fbm2(
            (world_xz_m - wind_dir * 9_000.0) * temperature_freq + Vec2::new(31.0, 7.0),
            weather_seed ^ 0xCAFE_BABE,
            3,
        );

        let humidity = saturate(
            0.28 + macro_mass * 0.32
                + humidity_noise * 0.28
                + frontal * 0.18
                + controls.stratus * 0.08,
        );
        let instability = saturate(
            (temperature_proxy - 0.42) * 1.3
                + mesoscale * 0.35
                + controls.cumulus * 0.22
                + controls.cumulonimbus * 0.35
                - controls.stratus * 0.18,
        );
        let convection = saturate(
            instability * (0.48 + humidity * 0.52) + frontal * 0.12 + controls.wind_shear * 0.10,
        );
        let storm = saturate(
            (humidity * convection * (0.55 + frontal * 0.45) - 0.26)
                * (0.75 + controls.cumulonimbus * 0.95),
        );

        let coverage = saturate(
            controls.coverage * (0.22 + macro_mass * 1.02)
                + frontal * 0.14
                + humidity * 0.06
                - 0.08,
        );
        let cloud_type = saturate(
            controls.stratus * 0.24 + controls.cumulus * 0.46 + convection * 0.18 + storm * 0.26
                - controls.cirrus * 0.08,
        );

        let low_amount = saturate(
            coverage
                * (0.26 + controls.stratus * 0.34 + controls.cumulus * 0.42)
                * (0.60 + humidity * 0.40),
        );
        let mid_amount = saturate(
            (controls.altocumulus * 0.56 + controls.altostratus * 0.92)
                * (0.32 + humidity * 0.42 + frontal * 0.30 + controls.density * 0.18),
        );
        let high_amount = saturate(
            controls.cirrus
                * (0.50
                    + (1.0 - humidity) * 0.24
                    + frontal * 0.38
                    + controls.density * 0.10),
        );
        let precipitation = saturate(storm * humidity * (0.58 + controls.cumulonimbus * 0.42));
        let density_gain = 0.35 + controls.density * 1.35;
        let low_optical_depth = (coverage
            * low_amount
            * humidity
            * density_gain
            * (0.42 + convection * 0.44 + storm * 0.58))
            .clamp(0.0, 4.0);
        let mid_optical_depth = (coverage
            * mid_amount
            * density_gain
            * (0.24 + humidity * 0.34 + controls.altostratus * 0.38 + frontal * 0.18))
            .clamp(0.0, 3.0);
        let high_optical_depth = (high_amount
            * (0.10 + controls.cirrus * 0.24 + frontal * 0.16 + (1.0 - humidity) * 0.12))
            .clamp(0.0, 1.5);

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
        controls: CloudControls,
        weather_seed: u32,
    ) {
        self.region.origin_m = origin_m;
        let resolution = self.region.resolution as usize;
        for y in 0..resolution {
            for x in 0..resolution {
                let index = self.texel_index(x, y);
                let world_xz_m = self.texel_world_position(origin_m, x, y);
                let sample = self.evaluate_sample(world_xz_m, controls, weather_seed);
                self.write_sample(index, sample);
            }
        }
        self.mark_full_level_dirty();
        self.rebuild_summary();
    }

    fn scroll_to_origin(
        &mut self,
        origin_m: Vec2,
        texel_offset: IVec2,
        controls: CloudControls,
        weather_seed: u32,
    ) {
        let resolution = self.region.resolution as i32;
        if texel_offset.x.abs() >= resolution || texel_offset.y.abs() >= resolution {
            self.full_refresh(origin_m, controls, weather_seed);
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
                    let sample = self.evaluate_sample(world_xz_m, controls, weather_seed);
                    self.write_sample(index, sample);
                }
            }
        }

        self.mark_scroll_dirty_uploads(texel_offset);
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
    let frac = p.fract();
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
