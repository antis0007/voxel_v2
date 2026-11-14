//! Cube-face texels for the loading globe: linear albedo and signed height.
//!
//! Every map source is resampled onto the six faces of the cube sphere, so the
//! globe has no pole to pinch and no longitude seam. Texel `(i, j)` of a face
//! sits at face coordinates `(grid(i), grid(j))`; edge texels of neighbouring
//! faces sample the same directions, which hides the face seams.
use std::sync::{Arc, Mutex};


/// Linear albedo and height above sea in km; negative is water depth.
#[derive(Clone, Copy, Default)]
pub struct Texel {
    pub rgb: [f32; 3],
    pub height_km: f32,
    pub sea_ice: f32,
    /// Linear emission (plate seams), seen on the night side; 0..=1 of `GLOW_MAX`.
    pub glow: [f32; 3],
}

pub struct Surface {
    pub key: u64,
    /// Texels per face edge.
    pub n: usize,
    /// Six faces in `Face::ALL` order, rows along `t`.
    pub texels: Vec<Texel>,
    /// Radius the globe stands for, which scales the relief normals.
    pub radius_km: f32,
}

/// Face coordinate of texel `i` on an `n`-texel edge.
pub fn grid(i: usize, n: usize) -> f64 {
    -1.0 + 2.0 * i as f64 / (n - 1) as f64
}

pub fn linear(srgb: [f32; 3]) -> [f32; 3] {
    srgb.map(|c| c.clamp(0.0, 1.0).powf(2.2))
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t)
}

fn byte(c: [u8; 3]) -> [f32; 3] {
    linear(c.map(|v| f32::from(v) / 255.0))
}

/// Open ocean far from any coast.
pub const DEEP: Texel = Texel { rgb: [0.004, 0.016, 0.045], height_km: -3.2, sea_ice: 0.0, glow: [0.0; 3] };
/// Face not sampled yet: dark, matte and flat.
pub const UNSURVEYED: Texel = Texel { rgb: [0.012, 0.016, 0.024], height_km: 0.0, sea_ice: 0.0, glow: [0.0; 3] };

/// Land colour from relative elevation, rainfall in [0, 1], mean annual
/// temperature in degrees C and fresh-water cover in [0, 1]. Without climate
/// the ground shows bare rock, so each solve stage reads on the globe.
pub fn land_rgb(elevation: f32, wet: Option<f32>, temp_c: Option<f32>, fresh: f32) -> [f32; 3] {
    let rock = mix(byte([118, 110, 98]), byte([150, 142, 128]), elevation * 1.4);
    let Some(w) = wet else {
        return mix(rock, byte([40, 78, 104]), fresh);
    };
    let dry = mix(byte([196, 168, 116]), byte([150, 142, 92]), w * 2.5);
    let green = mix(byte([104, 126, 68]), byte([44, 78, 44]), (w - 0.45) * 2.2);
    let mut c = mix(dry, green, (w - 0.2) * 3.0);
    c = mix(c, rock, (elevation - 0.55) * 2.5);
    if let Some(t) = temp_c {
        c = mix(c, byte([124, 120, 104]), (4.0 - t) / 5.0);
        c = mix(c, byte([214, 220, 228]), (-1.0 - t) / 6.0);
    }
    mix(c, byte([40, 78, 104]), fresh)
}

/// Ocean depth from distance to the nearest land texel: a shelf at the coast
/// that drops to the abyss within a few tens of km.
pub fn ocean_depths(land: &[bool], n: usize, texel_km: f32) -> Vec<f32> {
    const FAR: f32 = 1.0e6;
    let mut d: Vec<f32> = land.iter().map(|&l| if l { 0.0 } else { FAR }).collect();
    let diag = std::f32::consts::SQRT_2;
    let at = |i: isize, j: isize| (i >= 0 && j >= 0 && (i as usize) < n && (j as usize) < n).then(|| j as usize * n + i as usize);
    for pass in 0..2 {
        let order: Box<dyn Iterator<Item = usize>> = if pass == 0 { Box::new(0..n * n) } else { Box::new((0..n * n).rev()) };
        let sign: isize = if pass == 0 { -1 } else { 1 };
        for k in order {
            let (i, j) = ((k % n) as isize, (k / n) as isize);
            for (di, dj, w) in [(sign, 0, 1.0), (0, sign, 1.0), (sign, sign, diag), (-sign, sign, diag)] {
                if let Some(o) = at(i + di, j + dj) {
                    d[k] = d[k].min(d[o] + w);
                }
            }
        }
    }
    d.into_iter().map(|t| 0.03 + 3.2 * (1.0 - (-t * texel_km / 22.0).exp())).collect()
}

/// Ocean colour by depth; the shader adds reflection and glint.
pub fn water_rgb(depth_km: f32) -> [f32; 3] {
    mix([0.02, 0.10, 0.12], DEEP.rgb, (depth_km / 1.2).sqrt())
}

/// Gives the water of one face a shelf from its coasts; with `open_edge` it
/// also falls to open ocean at the face edge, where the neighbours are `DEEP`.
pub fn settle_sea(face: &mut [Texel], n: usize, texel_km: f32, open_edge: bool) {
    let land: Vec<bool> = face.iter().map(|t| t.height_km > 0.0).collect();
    let depths = ocean_depths(&land, n, texel_km);
    for (k, t) in face.iter_mut().enumerate() {
        // Height exactly zero marks an unsurveyed texel.
        if land[k] || t.height_km == 0.0 {
            continue;
        }
        let edge = grid(k % n, n).abs().max(grid(k / n, n).abs()) as f32;
        let open = if open_edge { ((edge - 0.72) / 0.26).clamp(0.0, 1.0) } else { 0.0 };
        let depth = (-t.height_km).max(depths[k]);
        let depth = depth + (-DEEP.height_km - depth).max(0.0) * open * open * (3.0 - 2.0 * open);
        // Keep the sub-texel coast crossing so shorelines stay smooth.
        *t = Texel { rgb: water_rgb(depth), height_km: t.height_km.min(-depth), sea_ice: 0.0, glow: [0.0; 3] };
    }
}

/// Lightly blend land colours without averaging away surveyed relief.
pub fn soften_land(face: &mut [Texel], n: usize) {
    let src = face.to_vec();
    for j in 0..n {
        for i in 0..n {
            let k = j * n + i;
            if src[k].height_km <= 0.0 {
                continue;
            }
            let (mut rgb, mut w) = ([0.0_f32; 3], 0.0);
            for (di, dj) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (0, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                let (x, y) = (i as isize + di, j as isize + dj);
                if x < 0 || y < 0 || x >= n as isize || y >= n as isize {
                    continue;
                }
                let o = src[y as usize * n + x as usize];
                if o.height_km <= 0.0 {
                    continue;
                }
                let wt = if di == 0 && dj == 0 { 2.0 } else { 1.0 };
                for c in 0..3 {
                    rgb[c] += o.rgb[c] * wt;
                }
                w += wt;
            }
            face[k].rgb = mix(src[k].rgb, rgb.map(|c| c / w), 0.32);
        }
    }
}

/// Six faces filled with `fill`, ready for one face to be written.
pub fn faces(n: usize, fill: Texel) -> Vec<Texel> {
    vec![fill; 6 * n * n]
}

/// Half-float bits for the GPU; inputs stay well inside the normal range.
fn half(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    if exp <= 0 {
        return sign;
    }
    if exp >= 31 {
        return sign | 0x7bff;
    }
    // Round to nearest on the dropped mantissa bits.
    let mant = (bits & 0x7f_ffff) + 0x1000;
    if mant & 0x80_0000 != 0 {
        return sign | (((exp + 1) as u16) << 10).min(0x7bff);
    }
    sign | ((exp as u16) << 10) | (mant >> 13) as u16
}

impl Surface {
    /// RGBA16F rows for the six array layers: albedo, then height.
    pub fn rgba16(&self) -> Vec<u16> {
        self.texels.iter().flat_map(|t| [half(t.rgb[0]), half(t.rgb[1]), half(t.rgb[2]), half(t.height_km)]).collect()
    }
}

#[derive(Default)]
struct SurfaceCache {
    // History interpolation needs both endpoints resident, not every epoch.
    entries: [Option<(u64, Arc<Surface>)>; 2],
}

impl SurfaceCache {
    fn get(&mut self, key: u64, build: impl FnOnce() -> Option<Surface>) -> Option<Arc<Surface>> {
        if let Some(index) = self.entries.iter().position(|s| s.as_ref().is_some_and(|(k, _)| *k == key)) {
            self.entries.swap(0, index);
            return self.entries[0].as_ref().map(|(_, s)| Arc::clone(s));
        }
        let s = Arc::new(build()?);
        self.entries[1] = self.entries[0].take();
        self.entries[0] = Some((key, Arc::clone(&s)));
        Some(s)
    }
}

static HISTORY_CACHE: Mutex<SurfaceCache> = Mutex::new(SurfaceCache { entries: [None, None] });

/// Keeps the two interpolation endpoints; later frames only clone their Arcs.
pub fn history_cached(key: u64, build: impl FnOnce() -> Option<Surface>) -> Option<Arc<Surface>> {
    HISTORY_CACHE.lock().ok()?.get(key, build)
}

static CACHE: Mutex<Option<(u64, Arc<Surface>)>> = Mutex::new(None);

/// Builds a surface once per source key; later frames reuse it.
pub fn cached(key: u64, build: impl FnOnce() -> Option<Surface>) -> Option<Arc<Surface>> {
    let mut slot = CACHE.lock().ok()?;
    if let Some((k, s)) = slot.as_ref()
        && *k == key
    {
        return Some(Arc::clone(s));
    }
    let s = Arc::new(build()?);
    *slot = Some((key, Arc::clone(&s)));
    Some(s)
}

#[derive(Default)]
struct Worker {
    done: Option<(u64, Arc<Surface>)>,
    building: bool,
}

static WORKER: Mutex<Option<Worker>> = Mutex::new(None);

/// Newest finished surface; starts a background build when `key` is new and
/// none is running. Until it lands, the previous surface stays on screen.
pub fn latest(key: u64, build: impl FnOnce() -> Surface + Send + 'static) -> Option<Arc<Surface>> {
    let mut slot = WORKER.lock().ok()?;
    let w = slot.get_or_insert_with(Worker::default);
    if w.done.as_ref().is_none_or(|(k, _)| *k != key) && !w.building {
        w.building = true;
        let spawned = std::thread::Builder::new().name("loading-globe-map".into()).spawn(move || {
            let s = Arc::new(build());
            if let Ok(mut slot) = WORKER.lock()
                && let Some(w) = slot.as_mut()
            {
                w.done = Some((key, s));
                w.building = false;
            }
        });
        w.building = spawned.is_ok();
    }
    w.done.as_ref().map(|(_, s)| Arc::clone(s))
}

/// Stores a finished surface, as `latest` would once its build lands.
pub fn put(key: u64, s: Surface) {
    if let Ok(mut slot) = WORKER.lock() {
        slot.get_or_insert_with(Worker::default).done = Some((key, Arc::new(s)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::cube_sphere::{Face, Projection};
    use glam::Vec3;

    fn sample(key: u64) -> Surface {
        Surface { key, n: 2, texels: faces(2, DEEP), radius_km: 20.0 }
    }

    #[test]
    fn history_endpoints_build_once_per_epoch() {
        let mut cache = SurfaceCache::default();
        let mut builds = 0;
        for epoch in 1..25 {
            for _ in 0..60 {
                for key in [epoch - 1, epoch] {
                    let surface = cache.get(key, || { builds += 1; Some(sample(key)) }).unwrap();
                    assert_eq!(surface.key, key);
                }
            }
        }
        assert_eq!(builds, 25);
        assert_eq!(cache.entries.iter().flatten().count(), 2);
    }

    #[test]
    fn cache_reuses_endpoints_and_releases_old_worlds() {
        let mut cache = SurfaceCache::default();
        let first = cache.get(1, || Some(sample(1))).unwrap();
        cache.get(2, || Some(sample(2))).unwrap();
        let again = cache.get(1, || panic!("cached endpoint rebuilt")).unwrap();
        assert!(Arc::ptr_eq(&first, &again));
        assert!(cache.get(3, || None).is_none());
        assert!(Arc::ptr_eq(&first, &cache.get(1, || None).unwrap()));
        drop(again);
        cache.get(100, || Some(sample(100))).unwrap();
        cache.get(101, || Some(sample(101))).unwrap();
        assert_eq!(Arc::strong_count(&first), 1);
        assert_eq!(cache.entries.iter().flatten().count(), 2);
    }

    #[test]
    fn half_floats_round_trip_globe_values() {
        for v in [0.0_f32, 0.004, 0.25, 1.0, -3.2, 0.52] {
            let h = half(v);
            let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
            let exp = ((h >> 10) & 0x1f) as i32;
            let back = if exp == 0 { 0.0 } else { sign * (1.0 + f32::from(h & 0x3ff) / 1024.0) * 2f32.powi(exp - 15) };
            assert!((back - v).abs() <= v.abs() * 1.0e-3 + 1.0e-4, "{v} -> {back}");
        }
    }

    fn dir(face: Face, i: usize, j: usize, n: usize) -> Vec3 {
        Projection::Tangent.to_sphere(face, grid(i, n), grid(j, n)).as_vec3()
    }

    #[test]
    fn shared_edges_sample_one_direction() {
        let n = 9;
        // PosX row t = +1 meets PosZ: both edges hold the same directions.
        let a: Vec<Vec3> = (0..n).map(|i| dir(Face::PosX, i, n - 1, n)).collect();
        let b: Vec<Vec3> = (0..n).flat_map(|i| (0..n).map(move |j| dir(Face::PosZ, i, j, n))).collect();
        for d in a {
            assert!(b.iter().any(|e| e.distance(d) < 1.0e-5), "{d} has no twin on PosZ");
        }
    }

    #[test]
    fn depth_grows_away_from_the_coast() {
        let n = 32;
        let land: Vec<bool> = (0..n * n).map(|k| k % n < 4).collect();
        let d = ocean_depths(&land, n, 1.0);
        assert!(d[10] < d[20] && d[20] < d[31], "{} {} {}", d[10], d[20], d[31]);
    }
}
