//! Background column survey around spawn for the loading globe.
//!
//! One worker samples real generator columns row by row; the globe shows the
//! surveyed square on its spawn face, filling in as the survey advances.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use engine_voxel::WorldSampleConfig;
use glam::IVec3;

use super::surface::{self, Surface, Texel, UNSURVEYED};

/// Columns per side of the survey grid.
pub const N: usize = 128;
/// Half-width of the surveyed square, in metres.
const RADIUS_M: f32 = 6_000.0;

pub struct Survey {
    pub key: u64,
    rows: AtomicUsize,
    texels: Mutex<Vec<Texel>>,
    cancel: AtomicBool,
}

impl Survey {
    /// Rows sampled so far, out of `N`.
    pub fn rows(&self) -> usize {
        self.rows.load(Ordering::Acquire)
    }

    /// Spawn face holding the rows sampled so far, rebuilt every 16 rows.
    /// Areas outside the sampled square remain unsurveyed.
    pub fn surface(&self) -> Option<Arc<Surface>> {
        let rows = self.rows();
        let step = if rows == N { rows } else { rows / 16 * 16 };
        let key = self.key ^ (step as u64) << 40;
        surface::cached(key, || {
            let square = self.texels.lock().ok()?.clone();
            let face: Vec<Texel> = square.into_iter().enumerate().map(|(k, t)| if k / N >= step { UNSURVEYED } else { t }).collect();
            // Preserve sampled heights and coasts; the boundary is not a shoreline.
            let mut texels = surface::faces(N, UNSURVEYED);
            texels[..N * N].copy_from_slice(&face);
            Some(Surface { key, n: N, texels, radius_km: RADIUS_M * 4.0 / std::f32::consts::PI / 1000.0 })
        })
    }
}

static CURRENT: Mutex<Option<Arc<Survey>>> = Mutex::new(None);

fn key_of(cfg: &WorldSampleConfig, center: (i32, i32)) -> u64 {
    let parts = [cfg.seed, cfg.generator_version, cfg.voxel_scale as u32, u32::from(cfg.world_type.code()), (center.0 >> 8) as u32, (center.1 >> 8) as u32];
    let h = parts.iter().fold(0x9e37_79b9_u32, |h, v| engine_core::hash::hash_u32(h ^ v));
    (u64::from(h) << 32 | u64::from(engine_core::hash::hash_u32(h))) ^ cfg.planet_fingerprint()
}

/// The survey for this world, starting its worker on first request.
pub fn request(cfg: WorldSampleConfig, center: (i32, i32)) -> Option<Arc<Survey>> {
    let key = key_of(&cfg, center);
    let mut slot = CURRENT.lock().ok()?;
    if let Some(s) = slot.as_ref().filter(|s| s.key == key) {
        return Some(Arc::clone(s));
    }
    if let Some(old) = slot.take() {
        old.cancel.store(true, Ordering::Release);
    }
    let survey = Arc::new(Survey {
        key,
        rows: AtomicUsize::new(0),
        texels: Mutex::new(vec![Texel::default(); N * N]),
        cancel: AtomicBool::new(false),
    });
    let worker = Arc::clone(&survey);
    std::thread::Builder::new()
        .name("loading-survey".into())
        .spawn(move || run(&worker, cfg, center))
        .ok()?;
    *slot = Some(Arc::clone(&survey));
    Some(survey)
}

/// Samples the whole grid on the calling thread; used by captures.
pub fn blocking(cfg: WorldSampleConfig, center: (i32, i32)) -> Arc<Survey> {
    let survey = Arc::new(Survey {
        key: key_of(&cfg, center),
        rows: AtomicUsize::new(0),
        texels: Mutex::new(vec![Texel::default(); N * N]),
        cancel: AtomicBool::new(false),
    });
    run(&survey, cfg, center);
    survey
}

fn run(s: &Survey, cfg: WorldSampleConfig, center: (i32, i32)) {
    let step = cfg.blocks_for_span(2.0 * RADIUS_M / N as f32).max(1);
    let half = (N as i32 / 2) * step;
    let water = cfg.water_level();
    let metres = cfg.voxel_meters();
    for z in 0..N {
        if s.cancel.load(Ordering::Acquire) {
            return;
        }
        let wz = center.1 - half + z as i32 * step;
        let row: Vec<Texel> = (0..N)
            .map(|x| {
                let c = engine_worldgen::sample_column(&cfg, center.0 - half + x as i32 * step, wz);
                let above_km = (c.height - water) as f32 * metres / 1000.0;
                if c.height < water {
                    Texel { rgb: surface::water_rgb(-above_km), height_km: above_km.min(-0.004), sea_ice: 0.0, glow: [0.0; 3] }
                } else {
                    // One colour per material: per-position variation reads as speckle from orbit.
                    Texel { rgb: surface::linear(c.surface.color(IVec3::ZERO)), height_km: above_km.max(0.004), sea_ice: 0.0, glow: [0.0; 3] }
                }
            })
            .collect();
        if let Ok(mut buf) = s.texels.lock() {
            buf[z * N..(z + 1) * N].copy_from_slice(&row);
        }
        s.rows.store(z + 1, Ordering::Release);
    }
}
