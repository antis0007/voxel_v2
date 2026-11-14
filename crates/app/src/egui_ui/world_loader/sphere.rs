//! Cube-sphere survey of a planet world for the loading globe.
//!
//! Every texel samples the generator along its own direction, so there is no
//! longitude seam or pole to pinch. The worker publishes the sphere only after
//! all six faces are ready; the face counter is used for loading progress.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use engine_core::cube_sphere::{Face, Projection, atlas};
use engine_voxel::WorldSampleConfig;
use glam::{DVec3, IVec3, Vec3};

use super::surface::{self, Surface, Texel, UNSURVEYED};

/// Texels per face edge.
const PLANET_N: usize = 192;
const LEGACY_N: usize = 96;
const FACES: usize = Face::ALL.len();

pub struct SphereSurvey {
    pub key: u64,
    n: usize,
    faces: AtomicUsize,
    texels: Mutex<Vec<Texel>>,
    pub spawn: Vec3,
    cancel: AtomicBool,
}

impl SphereSurvey {
    /// Faces sampled so far, out of six.
    pub fn faces(&self) -> usize {
        self.faces.load(Ordering::Acquire)
    }
}

static CURRENT: Mutex<Option<Arc<SphereSurvey>>> = Mutex::new(None);

fn key_of(cfg: &WorldSampleConfig) -> u64 {
    let parts = [cfg.seed, cfg.generator_version, cfg.voxel_scale as u32, u32::from(cfg.world_type.code()), 0x5be7e];
    let h = parts.iter().fold(0x9e37_79b9_u32, |h, v| engine_core::hash::hash_u32(h ^ v));
    (u64::from(h) << 32 | u64::from(engine_core::hash::hash_u32(h))) ^ cfg.planet_fingerprint()
}

/// Spawn direction on the planet sphere.
pub fn spawn_dir(cfg: &WorldSampleConfig, center: (i32, i32)) -> Vec3 {
    let m = f64::from(cfg.voxel_meters());
    let (face, local) = atlas::split(DVec3::new(f64::from(center.0) * m, 0.0, f64::from(center.1) * m));
    let s = (local.x / atlas::HALF_M).clamp(-1.0, 1.0);
    let t = (local.z / atlas::HALF_M).clamp(-1.0, 1.0);
    Projection::Tangent.to_sphere(face, s, t).as_vec3()
}

fn survey(cfg: &WorldSampleConfig, center: (i32, i32)) -> SphereSurvey {
    let n = if engine_worldgen::planetary::active(cfg) { PLANET_N } else { LEGACY_N };
    SphereSurvey {
        key: key_of(cfg),
        n,
        faces: AtomicUsize::new(0),
        texels: Mutex::new(surface::faces(n, UNSURVEYED)),
        spawn: spawn_dir(cfg, center),
        cancel: AtomicBool::new(false),
    }
}

/// The survey for this planet, starting its worker on first request.
pub fn request(cfg: WorldSampleConfig, center: (i32, i32)) -> Option<Arc<SphereSurvey>> {
    let key = key_of(&cfg);
    let mut slot = CURRENT.lock().ok()?;
    if let Some(s) = slot.as_ref().filter(|s| s.key == key) {
        return Some(Arc::clone(s));
    }
    if let Some(old) = slot.take() {
        old.cancel.store(true, Ordering::Release);
    }
    let s = Arc::new(survey(&cfg, center));
    let worker = Arc::clone(&s);
    std::thread::Builder::new().name("loading-sphere".into()).spawn(move || run(&worker, cfg)).ok()?;
    *slot = Some(Arc::clone(&s));
    Some(s)
}

/// Samples all six faces on the calling thread; used by captures.
pub fn blocking(cfg: WorldSampleConfig, center: (i32, i32)) -> Arc<SphereSurvey> {
    let s = Arc::new(survey(&cfg, center));
    run(&s, cfg);
    s
}

fn run(s: &SphereSurvey, cfg: WorldSampleConfig) {
    let next_face = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get().saturating_sub(2).clamp(1, 3));
    std::thread::scope(|scope| {
        for _ in 1..workers {
            let next_face = &next_face;
            scope.spawn(move || run_faces(s, cfg, next_face));
        }
        run_faces(s, cfg, &next_face);
    });
}

fn run_faces(s: &SphereSurvey, cfg: WorldSampleConfig, next_face: &AtomicUsize) {
    let n = s.n;
    let per_face = n * n;
    let m = f64::from(cfg.voxel_meters());
    let footprint = 2.0 * atlas::HALF_M / (n - 1) as f64;
    let sea = cfg.world_y(cfg.water_level());
    let spherical = engine_worldgen::planetary::active(&cfg);
    let palette = AtlasPalette::new();
    loop {
        let fi = next_face.fetch_add(1, Ordering::Relaxed);
        if fi >= FACES { break; }
        let face = Face::ALL[fi];
        let mut out = vec![Texel::default(); per_face];
        for j in 0..n {
            if s.cancel.load(Ordering::Acquire) {
                return;
            }
            for i in 0..n {
                let local = DVec3::new(surface::grid(i, n) * atlas::HALF_M, 0.0, surface::grid(j, n) * atlas::HALF_M);
                let chart = atlas::join(face, local);
                if spherical {
                    let ground = engine_worldgen::planetary::ground(
                        &cfg, chart.x as f32, chart.z as f32, footprint as f32,
                    );
                    let inland_water = ground.water_m.filter(|&head| head > sea && head > ground.top_m);
                    let signed_m = inland_water.unwrap_or(ground.top_m) - sea;
                    let sea_ice = if signed_m < 0.0 {
                        let direction = Projection::Tangent.to_sphere(face, local.x / atlas::HALF_M, local.z / atlas::HALF_M);
                        let winter = engine_worldgen::planetary::climate_at(&cfg, direction).winter_c;
                        sea_ice_cover(winter as f32)
                    } else { 0.0 };
                    let rgb = if signed_m < 0.0 {
                        palette.ocean(-signed_m / 1000.0, sea_ice)
                    } else {
                        let land = palette.land(&ground, sea);
                        if let Some(head) = inland_water {
                            // Shade the lake at its actual elevation. Encoding
                            // lake depth as a negative terrain height would
                            // put its surface at sea level in the relief pass.
                            let depth = head - ground.top_m;
                            blend(land, surface::water_rgb(depth / 1000.0), ramp(depth, 0.0, 0.8))
                        } else { land }
                    };
                    // Keep signed height continuous through the shoreline;
                    // sea ice changes colour without changing land coverage.
                    out[j * n + i] = Texel { rgb, height_km: signed_m / 1000.0, sea_ice, glow: [0.0; 3] };
                    continue;
                }
                let (x, z) = ((chart.x / m).floor() as i32, (chart.z / m).floor() as i32);
                let (height, material) = crate::render::globe::bake::surface_at(&cfg, x, z, footprint);
                let elevation = (cfg.world_y(height) - sea) / 1000.0;
                let frozen = !cfg.is_hell() && elevation < 0.0
                    && engine_worldgen::planet::sea_ice_at(&cfg, x, z);
                out[j * n + i] = if !cfg.is_hell() && elevation < 0.0 && !frozen {
                    Texel { rgb: surface::water_rgb(-elevation), height_km: elevation, sea_ice: 0.0, glow: [0.0; 3] }
                } else {
                    let material = if frozen { engine_content::BlockId::Ice } else { material };
                    let relief = if cfg.is_hell() { cfg.world_y(height) / 1000.0 } else { elevation };
                    Texel { rgb: surface::linear(material.color(IVec3::ZERO)), height_km: relief.max(0.001), sea_ice: 0.0, glow: [0.0; 3] }
                };
            }
        }
        if !spherical && !cfg.is_hell() {
            surface::soften_land(&mut out, n);
        }
        if let Ok(mut all) = s.texels.lock() {
            all[fi * per_face..(fi + 1) * per_face].copy_from_slice(&out);
        }
        s.faces.fetch_add(1, Ordering::Release);
    }
}

fn ramp(value: f32, lo: f32, hi: f32) -> f32 {
    let t = ((value - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn sea_ice_cover(winter_c: f32) -> f32 {
    ramp(-8.0 - winter_c, 0.0, 6.0)
}

fn blend(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// Game material colours blended across climate and exposure at map scale.
pub(super) struct AtlasPalette {
    soil: [f32; 3], grass: [f32; 3], rock: [f32; 3],
    sand: [f32; 3], snow: [f32; 3], ice: [f32; 3], mud: [f32; 3],
    leaves: [f32; 3], needles: [f32; 3],
}

impl AtlasPalette {
    pub(super) fn new() -> Self {
        use engine_content::BlockId::*;
        let color = |b: engine_content::BlockId| surface::linear(b.color(IVec3::ZERO));
        Self { soil: color(Dirt), grass: color(Grass), rock: color(Stone),
            sand: color(Sand), snow: color(SnowGrass), ice: color(Ice), mud: color(Mud),
            leaves: color(Leaves), needles: color(PineLeaves) }
    }

    pub(super) fn ocean(&self, depth_km: f32, sea_ice: f32) -> [f32; 3] {
        blend(surface::water_rgb(depth_km), self.ice, 0.90 * sea_ice)
    }

    fn land(&self, g: &engine_worldgen::geo::Ground, sea: f32) -> [f32; 3] {
        let cover = self.biome(g.top_m - sea, g.moisture, g.maat_c, g.slope);
        blend(cover, blend(self.sand, self.mud, g.delta_mud), g.delta * 0.70)
    }

    pub(super) fn biome(&self, height: f32, moisture: f32, annual: f32, slope: f32) -> [f32; 3] {
        let wet = ramp(moisture, 0.10, 0.53);
        let growth = wet * ramp(annual, -6.0, 5.0);
        let soil = blend(self.sand, self.soil, ramp(moisture, 0.06, 0.37));
        let ground = blend(soil, self.grass, growth);
        let canopy = engine_worldgen::planetary::ecoregion::potential(annual, moisture, slope);
        let foliage = blend(self.needles, self.leaves, ramp(annual, 2.0, 17.0));
        let ground = blend(ground, foliage, canopy * 0.62);
        // Height alone does not make a wet, warm plateau bare rock.
        let exposed = ramp(slope, 0.26, 0.82)
            .max(ramp(height, 260.0, 480.0) * (1.0 - growth) * 0.30);
        let rock = blend(ground, self.rock, exposed);
        let snow = ramp(-annual, 1.0, 13.0) * ramp(moisture, 0.15, 0.52);
        blend(rock, self.snow, snow)
    }
}

pub(super) fn from_solve(atlas: &engine_worldgen::planetary::SolveAtlas, key: u64) -> Surface {
    let palette = AtlasPalette::new();
    let texels = atlas.texels.iter().map(|t| {
        let height = t.height_m + t.inland_depth_m;
        let sea_ice = if height < 0.0 { sea_ice_cover(t.winter_c) } else { 0.0 };
        let rgb = if height < 0.0 {
            palette.ocean(-height / 1000.0, sea_ice)
        } else {
            let land = palette.biome(t.height_m, t.wet, t.annual_c, t.slope);
            blend(land, surface::water_rgb(t.inland_depth_m / 1000.0), ramp(t.inland_depth_m, 0.0, 0.8))
        };
        Texel { rgb, height_km: height / 1000.0, sea_ice, glow: [0.0; 3] }
    }).collect();
    Surface { key, n: atlas.n, texels, radius_km: (atlas::RADIUS_M / 1000.0) as f32 }
}

impl SphereSurvey {
    /// Publish all faces together so no shaded face can border an empty tile.
    pub fn surface(&self) -> Option<Arc<Surface>> {
        if self.faces() != FACES { return None; }
        surface::cached(self.key, || {
            let texels = self.texels.lock().ok()?.clone();
            Some(Surface { key: self.key, n: self.n, texels, radius_km: (atlas::RADIUS_M / 1000.0) as f32 })
        })
    }
}
