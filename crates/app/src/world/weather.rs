use glam::{IVec2, Vec2};

use super::CloudControls;
mod scroll;
pub mod synoptic;

/// Rows of one clipmap level re-sampled per update, so synoptic motion reaches the texture.
const REFRESH_ROWS: usize = 2;
const REFRESH_INTERVAL_S: f32 = 1.0 / 60.0;
const MAX_REFRESH_TICKS: usize = 4;
/// `CLOUD_ADVECTION_SPEED_SCALE * cloud_layer_speed_mult(low)` in cloud_shader.wgsl.
const LOW_LAYER_WIND_MULT: f32 = 8.0 * 1.65;

/// Time inputs of the shared weather state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WeatherClock {
    /// How far the low deck has drifted, `wind_offset` at the sky time.
    pub drift_m: Vec2,
    /// Synced weather day, `synoptic::days`.
    pub days: f64,
    pub(crate) planet: Option<engine_voxel::WorldSampleConfig>,
}

impl WeatherClock {
    pub fn new(controls: &CloudControls, sky_time_s: f32, days: f64) -> Self {
        Self { drift_m: wind_offset(controls, sky_time_s), days, planet: None }
    }

    pub fn with_planet(mut self, cfg: engine_voxel::WorldSampleConfig) -> Self {
        self.planet = engine_worldgen::planetary::active(&cfg).then_some(cfg);
        self
    }
}

/// World-space offset the low cloud deck has drifted by at `time_s`, as the march applies it.
pub fn wind_offset(controls: &CloudControls, time_s: f32) -> Vec2 {
    let angle = controls.wind_dir_angle.to_radians();
    Vec2::new(angle.cos(), angle.sin()) * time_s * controls.advection * controls.wind_speed.max(0.0) * LOW_LAYER_WIND_MULT
}

/// Solver model time the orbital shell starts publishing at.
pub(crate) const WEATHER_SPIN_UP: f64 = 40.0;
/// Model units per weather day. At the default 480 s day this keeps the old
/// orbital pace of 0.04 units per real second (one front map about every 25 s).
pub(crate) const WEATHER_UNITS_PER_DAY: f64 = 0.04 * super::DEFAULT_DAY_LENGTH_SECONDS as f64;

/// Planetary weather model time at a synced weather day (`synoptic::days`), before spin-up.
pub(crate) fn weather_model_time(days: f64) -> f64 { days.max(0.0) * WEATHER_UNITS_PER_DAY }

/// Solver parameters of a world's planetary weather: the one source for the
/// orbital shell solver and the ground wind.
pub(crate) fn weather_params(seed: u32, planet: Option<engine_voxel::WorldSampleConfig>) -> engine_worldgen::sphere::WeatherParams {
    use engine_core::planet_frame::PlanetAxis;
    let settings = planet.and_then(|cfg| cfg.planet_settings());
    engine_worldgen::sphere::WeatherParams {
        seed,
        axis: PlanetAxis::new(glam::DVec3::NEG_Z, settings.map_or(23.4, |s| 47.0 * s.unit(6)).to_radians()),
        year_fraction: f64::from(engine_core::hash::hash01(seed ^ 0x7EA7_0001)),
        spin: 1.0,
        wetness: settings.map_or(0.5, |s| s.unit(5)),
    }
}

/// Planetary flow (zonal belts plus storm templates) at a weather day, cached per
/// small day step: the same `CloudSky` the orbital shell bakes and advects with.
pub(crate) fn planet_sky(seed: u32, planet: Option<engine_voxel::WorldSampleConfig>, days: f64) -> std::sync::Arc<engine_worldgen::sphere::CloudSky> {
    use std::sync::{Arc, Mutex};
    type Key = (u32, u64, u64);
    static CACHE: Mutex<Option<(Key, Arc<engine_worldgen::sphere::CloudSky>)>> = Mutex::new(None);
    let step = (days.max(0.0) * 1024.0).floor();
    let params = weather_params(seed, planet);
    let key = (seed, params.axis.obliquity.to_bits(), step.to_bits());
    if let Ok(cache) = CACHE.lock() && let Some((k, sky)) = cache.as_ref() && *k == key {
        return sky.clone();
    }
    let time = WEATHER_SPIN_UP + weather_model_time(step / 1024.0);
    let sky = Arc::new(engine_worldgen::sphere::CloudSky::at(seed, params.axis, params.year_fraction, time));
    if let Ok(mut cache) = CACHE.lock() { *cache = Some((key, sky.clone())); }
    sky
}

/// Ground wind in world xz metres per second at `xz_m`: the one wind truth for
/// sim and presentation, in real m/s. Off-planet the day's authored dial sets a
/// breeze (`ground.day_wind_ms` per unit), stiffened under cumulonimbus. On a
/// planetary world it is the planet sky's flow at that point, including every
/// cyclone's Holland vortex and tornado core, converted with `flow_per_ms`; the
/// day's breeze is the floor. Gusts are a fixed positional pattern, never
/// wall-clock noise. Dials: `ground` in `assets/climate.ron`.
pub(crate) fn wind_at(day: &super::DayNightCycle, planet: Option<engine_voxel::WorldSampleConfig>, xz_m: glam::DVec2) -> Vec2 {
    let dials = engine_content::climate::storm_dials();
    let g = &dials.ground;
    let days = synoptic::days(day);
    let seed = day.weather_seed();
    let planet = planet.filter(engine_worldgen::planetary::active);
    let controls = synoptic::controls_at(day.cloud_controls(), planet, xz_m, days, seed);
    let calm = controls.wind_speed.max(0.0) * g.day_wind_ms as f32
        * (1.0 + g.storm_gust as f32 * controls.cumulonimbus.clamp(0.0, 1.0));
    let (z, x) = controls.wind_dir_angle.to_radians().sin_cos();
    let mut wind = Vec2::new(x, z) * calm;
    if let Some(planet) = planet {
        let dir = synoptic::body_dir(xz_m);
        let flow = planet_sky(seed, Some(planet), days).wind(dir);
        // Chart tangents at this point carry the body-frame flow into world xz.
        let east = synoptic::body_dir(xz_m + glam::DVec2::X) - dir;
        let south = synoptic::body_dir(xz_m + glam::DVec2::Y) - dir;
        let local = Vec2::new(
            (flow.dot(east) / east.length_squared().max(1e-30)) as f32,
            (flow.dot(south) / south.length_squared().max(1e-30)) as f32,
        ) * (east.length() as f32);
        let flow_ms = local * (g.planet_gain / dials.flow_per_ms) as f32;
        wind = flow_ms.normalize_or(Vec2::new(x, z)) * flow_ms.length().max(calm);
    }
    let range = g.gust_range as f32;
    let gust = 1.0 - range + 2.0 * range * value_noise2((xz_m / g.gust_cell_m.max(1.0)).as_vec2(), seed ^ 0x6057_0001);
    (wind * gust).clamp_length_max(g.max_ms as f32)
}

/// Rising air at `xz_m` in m/s: a tornado's updraft, zero elsewhere.
pub(crate) fn updraft_at(day: &super::DayNightCycle, planet: Option<engine_voxel::WorldSampleConfig>, xz_m: glam::DVec2) -> f32 {
    let lift = engine_content::climate::storm_dials().ground.tornado_lift_ms as f32;
    let near = synoptic::tornado_at(planet, xz_m, synoptic::days(day), day.weather_seed());
    lift * near * near
}

/// A review pose on the strongest tropical cyclone (`VOXEL_REVIEW_CYCLONE`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct CycloneShot {
    pub day_index: u32,
    pub time_of_day: f32,
    /// `--benchmark-terrain-camera` pose: x, z, clearance, yaw, pitch.
    pub camera: [f32; 5],
    pub max_wind_ms: f32,
    pub category: u8,
}

/// Searches the first `days` weather days of a planet world for the strongest
/// tropical cyclone in local daylight and frames it: `orbit` looks straight
/// down from 9 km, `eyewall` stands 6 m over the sea just outside the radius of
/// maximum wind, facing the eye. Pure in (seed, planet, mode).
pub(crate) fn cyclone_review(seed: u32, planet: engine_voxel::WorldSampleConfig, mode: &str, days: f64) -> Option<CycloneShot> {
    use engine_core::cube_sphere::atlas;
    use engine_worldgen::sphere::StormKind as Kind;
    if !engine_worldgen::planetary::active(&planet) {
        return None;
    }
    let chart = |dir: glam::DVec3| atlas::from_body(dir.normalize() * atlas::RADIUS_M);
    let mut best: Option<(f64, f64, glam::DVec3, glam::DVec3, f64, u8)> = None;
    let mut t = 0.5;
    while t < days {
        let sky = planet_sky(seed, Some(planet), t);
        for s in sky.storms().iter().filter(|s| s.kind == Kind::Tropical) {
            let at = chart(s.centre);
            let solar = crate::planet::solar_time(at, (t * 480.0 * 120.0) as u64);
            if (0.38..0.62).contains(&solar) && best.is_none_or(|b| s.max_wind_ms > b.1) {
                best = Some((t, s.max_wind_ms, s.centre, s.east, s.rmw, s.category()));
            }
        }
        t += 0.05;
    }
    let (t, wind, centre, east, rmw, category) = best?;
    let eye = chart(centre);
    let camera = if mode == "eyewall" {
        let stand = chart(centre + east * rmw * 1.3);
        let to_eye = eye - stand;
        let sea_m = planet.water_level() as f32 * planet.voxel_scale.meters();
        [stand.x as f32, stand.z as f32, -(500.0 + sea_m + 6.0), (to_eye.x as f32).atan2(-to_eye.z as f32), 0.12]
    } else {
        [eye.x as f32, eye.z as f32, 9_000.0, 0.0, -1.55]
    };
    Some(CycloneShot { day_index: t.floor() as u32, time_of_day: t.fract() as f32, camera, max_wind_ms: wind as f32, category })
}

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
    clock: WeatherClock,
    refresh_row: usize,
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
    /// Level whose rows the next update re-samples.
    refresh_level: usize,
    next_refresh_s: f32,
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
        weather_days: f64,
        cfg: engine_voxel::WorldSampleConfig,
    ) -> Option<CloudWeatherUploadManifest> {
        self.simulation_time_s += dt_real_s.max(0.0);
        // Centred where the march samples the player's column: the low-deck offset.
        let clock = WeatherClock::new(authoring, sky_time_s, weather_days).with_planet(cfg);
        self.field.update_at(
            center_world_xz_m + clock.drift_m,
            self.simulation_time_s,
            *authoring,
            weather_seed,
            clock,
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
            clock: WeatherClock::default(),
            refresh_row: 0,
        };
        level.update(Vec2::ZERO, 0.0, CloudControls::default(), seed, WeatherClock::default(), 0);
        level
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
        clock: WeatherClock,
        refresh_rows: usize,
    ) {
        self.clock = clock;
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
            self.refresh_rows(refresh_rows, controls, weather_seed);
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

    fn sample_texel(&self, world_xz_m: Vec2, controls: CloudControls, weather_seed: u32) -> WeatherSample {
        canonical_sample(world_xz_m, self.clock, controls, weather_seed)
    }

    /// Re-samples the next `rows` rows in place and queues them for upload.
    fn refresh_rows(&mut self, rows: usize, controls: CloudControls, weather_seed: u32) {
        let resolution = self.region.resolution as usize;
        if rows == 0 || resolution == 0 {
            return;
        }
        let rows = rows.min(resolution);
        let first = self.refresh_row % resolution;
        for k in 0..rows {
            let y = (first + k) % resolution;
            for x in 0..resolution {
                let world_xz_m = self.texel_world_position(self.region.origin_m, x, y);
                let sample = self.sample_texel(world_xz_m, controls, weather_seed);
                self.write_sample(self.texel_index(x, y), sample);
            }
        }
        self.refresh_row = (first + rows) % resolution;
        let full = WeatherUploadRect::full_level(self.region.resolution);
        if !self.pending_gpu_uploads.contains(&full) {
            // A wrapped band is two rects; each stays inside the level.
            let tail = rows.min(resolution - first);
            for (y, h) in [(first, tail), (0, rows - tail)] {
                if h > 0 {
                    self.pending_gpu_uploads.push(WeatherUploadRect {
                        texel_x: 0,
                        texel_y: y as u32,
                        texel_width: self.region.resolution,
                        texel_height: h as u32,
                    });
                }
            }
        }
        self.rebuild_summary();
    }

    fn evaluate_sample(
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
            0.22 + macro_mass * 0.34
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
            instability * (0.46 + humidity * 0.54) + frontal * 0.10 + controls.wind_shear * 0.10,
        );

        // The storm preset must actually build storm cells. The previous
        // subtractive threshold cancelled most authored storms before upload.
        let storm = saturate(
            controls.cumulonimbus
                * (0.35 + humidity * 0.35 + convection * 0.30)
                * smoothstep(0.12, 0.55, humidity * convection + frontal * 0.15),
        );

        let coverage = saturate(
            controls.coverage * (0.18 + macro_mass * 1.08) + humidity * 0.12 + frontal * 0.18
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
            * (1.0 + controls.stratus * 0.80 + storm * 2.0)
            * 2.8)
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
        _time_s: f32,
        controls: CloudControls,
        weather_seed: u32,
    ) {
        self.region.origin_m = origin_m;
        let resolution = self.region.resolution as usize;
        for y in 0..resolution {
            for x in 0..resolution {
                let index = self.texel_index(x, y);
                let world_xz_m = self.texel_world_position(origin_m, x, y);
                let sample = self.sample_texel(world_xz_m, controls, weather_seed);
                self.write_sample(index, sample);
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
            refresh_level: 0,
            next_refresh_s: 0.0,
        }
    }

    /// `update_at` with the clock at its origin: no drift, weather day zero.
    pub fn update(
        &mut self,
        center_world_xz_m: Vec2,
        time_s: f32,
        controls: CloudControls,
        weather_seed: u32,
    ) {
        self.update_at(center_world_xz_m, time_s, controls, weather_seed, WeatherClock::default());
    }

    /// Moves the clipmaps and refreshes weather rows at a bounded 60 Hz cadence.
    pub fn update_at(
        &mut self,
        center_world_xz_m: Vec2,
        time_s: f32,
        controls: CloudControls,
        weather_seed: u32,
        clock: WeatherClock,
    ) {
        let ticks = if time_s >= self.next_refresh_s {
            let elapsed = ((time_s - self.next_refresh_s) / REFRESH_INTERVAL_S).floor() + 1.0;
            // Drop excess catch-up after stalls instead of stalling the next frame too.
            self.next_refresh_s += elapsed * REFRESH_INTERVAL_S;
            (elapsed as usize).min(MAX_REFRESH_TICKS)
        } else { 0 };
        let refreshing = self.refresh_level;
        for (i, level) in self.levels.iter_mut().enumerate() {
            let rows = if i == refreshing { REFRESH_ROWS * ticks } else { 0 };
            level.update(center_world_xz_m, time_s, controls, weather_seed, clock, rows);
        }
        if ticks > 0 {
            self.refresh_level = (refreshing + 1) % WEATHER_CLIPMAP_LEVEL_COUNT;
        }
    }

    pub fn reset(&mut self, seed: u32) {
        *self = Self::new(seed);
    }

    /// World position and stored coverage of one clipmap texel.
    #[cfg(test)]
    pub fn stored_coverage(&self, level: usize, x: usize, y: usize) -> (Vec2, f32) {
        let l = &self.levels[level];
        (l.texel_world_position(l.region.origin_m, x, y), l.primary_rgba[l.texel_index(x, y)][0])
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

/// The one weather sample both cloud renderers store: the local field at
/// advected position `q` under the controls the synoptic state gives the
/// ground point beneath it, `q - clock.drift_m`.
fn canonical_sample(q: Vec2, clock: WeatherClock, controls: CloudControls, weather_seed: u32) -> WeatherSample {
    let mut sample = canonical_sample_raw(q, clock, controls, weather_seed);
    // On planets the march stores the shell's front coverage, so descending
    // through the handoff band keeps one deck instead of two weather maps.
    if clock.planet.is_some() && let Some(cover) = fronts(weather_seed) {
        let dir = synoptic::body_dir(q.as_dvec2() - clock.drift_m.as_dvec2());
        let previous_cover = sample.primary[0];
        sample.primary[0] = crate::render::globe::shell::front_coverage(sample.primary[0],
            sample.secondary[0], cover(dir), controls.coverage, true);
        // Keep cloud amount and its shadow attached to the same front. Merely
        // replacing coverage left the old random deck active in clear slots.
        let amount = sample.primary[0] / previous_cover.max(0.02);
        sample.secondary[1] = (sample.secondary[1] * amount).clamp(0.0, 1.0);
        sample.shadow[0] = (sample.shadow[0] * amount).clamp(0.0, 4.0);
        sample.shadow[1] = (sample.shadow[1] * amount).clamp(0.0, 1.6);
    }
    sample
}

/// Pressure-solver cover the orbital shell bakes from, per weather seed.
pub(crate) type FrontCover = std::sync::Arc<dyn Fn(glam::DVec3) -> f32 + Send + Sync>;
static FRONTS: std::sync::RwLock<Option<(u32, FrontCover)>> = std::sync::RwLock::new(None);

/// Makes the shell's newest solver snapshot the march's fronts too.
pub(crate) fn publish_fronts(weather_seed: u32, cover: FrontCover) {
    if let Ok(mut fronts) = FRONTS.write() { *fronts = Some((weather_seed, cover)); }
}

fn fronts(weather_seed: u32) -> Option<FrontCover> {
    FRONTS.read().ok()?.as_ref().filter(|(seed, _)| *seed == weather_seed).map(|(_, cover)| cover.clone())
}

fn canonical_sample_raw(q: Vec2, clock: WeatherClock, controls: CloudControls, weather_seed: u32) -> WeatherSample {
    use engine_core::cube_sphere::{Face, atlas};
    let ground = q.as_dvec2() - clock.drift_m.as_dvec2();
    let dir = synoptic::body_dir(ground);
    let mut local = synoptic::apply(controls, synoptic::regional_at(clock.planet, dir, clock.days, weather_seed));
    if let Some(site) = clock.planet.as_ref().and_then(|cfg| engine_worldgen::planetary::ready_weather_site(cfg, dir)) {
        // The prevailing zonal wind is in body space, so lee and windward
        // sides remain consistent through chart edges and camera motion.
        let lat = dir.y.abs() as f32;
        let wind = planet_sky(weather_seed, clock.planet, clock.days).wind(dir).normalize_or_zero();
        let lift = (site.slope.dot(wind) as f32 * 2.0).clamp(-1.0, 1.0);
        let humid = (site.moisture - 0.5) * site.land;
        let coast = 4.0 * site.land * (1.0 - site.land);
        // Terrain modulates the authored weather, with more headroom in a
        // partly cloudy sky than in a clear or already overcast preset.
        let room = 4.0 * local.coverage * (1.0 - local.coverage);
        let change = (0.15 * lift + 0.14 * humid + 0.035 * coast) * room;
        local.coverage = (local.coverage + change).max(local.cumulonimbus * 0.9).clamp(0.02, 0.98);
        local.stratus = (local.stratus + (0.12 * coast + 0.18 * lift.max(0.0) * site.relief) * room).clamp(0.0, 1.0);
        local.cumulus = (local.cumulus + 0.20 * humid * (1.0 - lat) * room).clamp(0.0, 1.0);
    }
    let largest = dir.abs().max_element();
    let mut result = WeatherSample::default();
    let mut total = 0.0;
    // Blend overlapping face projections by body direction. A shared edge has
    // the same contributors from either chart, including at three-face corners.
    for face in Face::ALL {
        let axes = face.basis();
        let normal = dir.dot(axes.n.as_dvec3());
        let weight = smoothstep(0.85, 1.0, (normal / largest) as f32);
        if weight <= 0.0 { continue; }
        let chart = Vec2::new(
            (f64::from(face as u8) * atlas::PITCH_M
                + (dir.dot(axes.u.as_dvec3()) / normal).atan()
                    / std::f64::consts::FRAC_PI_4 * atlas::HALF_M) as f32,
            ((dir.dot(axes.v.as_dvec3()) / normal).atan()
                / std::f64::consts::FRAC_PI_4 * atlas::HALF_M) as f32,
        );
        let sample = WeatherClipmapLevel::evaluate_sample(chart + clock.drift_m, 0.0, local, weather_seed);
        for channel in 0..4 {
            result.primary[channel] += sample.primary[channel] * weight;
            result.secondary[channel] += sample.secondary[channel] * weight;
            result.shadow[channel] += sample.shadow[channel] * weight;
        }
        total += weight;
    }
    for channel in 0..4 {
        result.primary[channel] /= total;
        result.secondary[channel] /= total;
        result.shadow[channel] /= total;
    }
    result
}

/// Coverage the clipmap stores at advected position `q`: the globe shell bakes from it.
pub fn coverage_at(q: Vec2, clock: WeatherClock, controls: CloudControls, weather_seed: u32) -> f32 {
    canonical_sample(q, clock, controls, weather_seed).primary[0]
}

/// Local coverage and storm before the fronts; the shell applies its own snapshot.
pub(crate) fn cloud_shell_sample_raw(q: Vec2, clock: WeatherClock, controls: CloudControls, weather_seed: u32) -> (f32, f32) {
    let sample = canonical_sample_raw(q, clock, controls, weather_seed);
    (sample.primary[0], sample.secondary[0])
}

/// Precipitation the clipmap stores at advected position `q`, 0..1.
pub fn precipitation_at(q: Vec2, clock: WeatherClock, controls: CloudControls, weather_seed: u32) -> f32 {
    canonical_sample(q, clock, controls, weather_seed).shadow[3]
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
