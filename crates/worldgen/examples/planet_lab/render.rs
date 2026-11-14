//! CPU reference views of the generated planet: top-down patch, eye-level
//! perspective and whole-globe. They show generated ground (relief, cover,
//! water), not the game's renderer; trees and props are not drawn.
use crate::field::{self, Cell, Clipmap, Grid, SEA};
use engine_core::cube_sphere::atlas;
use engine_worldgen::{WorldSampleConfig, planetary};
use glam::{DVec3, Vec3};

pub struct Light {
    /// Unit vector toward the sun; x east (+chart x), y up, z +chart z.
    pub sun: Vec3,
    pub sun_colour: Vec3,
    pub sky: Vec3,
}

impl Light {
    pub fn new(azimuth_deg: f32, elevation_deg: f32) -> Self {
        let (az, el) = (azimuth_deg.to_radians(), elevation_deg.to_radians());
        let sun = Vec3::new(az.sin() * el.cos(), el.sin(), az.cos() * el.cos());
        // Low sun is redder and weaker; the sky fill keeps shadows readable.
        let low = (1.0 - el.sin()).powi(3);
        Self { sun, sun_colour: Vec3::new(1.0, 0.96 - 0.25 * low, 0.90 - 0.55 * low) * 3.2,
            sky: Vec3::new(0.20, 0.30, 0.48) * 0.55 }
    }
}

/// Coastal-water absorption per metre, red first (after Pope and Fry 1997,
/// raised for dissolved matter so depth colour saturates within the shelf).
const WATER_ABSORB: Vec3 = Vec3::new(0.40, 0.085, 0.030);
const DEEP_WATER: Vec3 = Vec3::new(0.004, 0.020, 0.045);

fn land(cell: &Cell, n: Vec3, shadow: f32, light: &Light) -> Vec3 {
    let ndl = n.dot(light.sun).max(0.0);
    cell.albedo * (light.sun_colour * ndl * shadow + light.sky * (0.35 + 0.65 * n.y.max(0.0)))
}

fn water(cell: &Cell, view: Vec3, shadow: f32, light: &Light, sky_reflection: Vec3) -> Vec3 {
    let depth = (cell.water - cell.top).max(0.0);
    let ndv = view.y.clamp(0.02, 1.0);
    let ndl = light.sun.y.max(0.0);
    // Refraction bounds the slant path inside the water.
    let path = depth * (1.0 / (0.66 + 0.34 * ndv) + 1.0 / (0.66 + 0.34 * ndl));
    let through = (-WATER_ABSORB * path).exp();
    let mut body = cell.albedo * through + DEEP_WATER * (Vec3::ONE - through);
    body = body.lerp(Vec3::new(0.13, 0.11, 0.075), (cell.suspended * 0.7).clamp(0.0, 0.7));
    let fresnel = 0.02 + 0.98 * (1.0 - ndv).powi(5);
    let half = (view + light.sun).normalize_or_zero();
    let ndh = half.y.clamp(0.0, 1.0);
    // Cox-Munk slope variance near 0.02 (7 m/s wind) as a GGX lobe.
    let a2 = 0.04_f32;
    let d = a2 / (std::f32::consts::PI * (ndh * ndh * (a2 - 1.0) + 1.0).powi(2));
    let glint = d * (0.02 + 0.98 * (1.0 - view.dot(half).clamp(0.0, 1.0)).powi(5)) * 0.25 / ndv.max(0.15);
    body * (light.sun_colour * ndl * shadow + light.sky) * (1.0 - fresnel)
        + sky_reflection * fresnel + light.sun_colour * glint * shadow
}

fn drop_m(horizontal: f64) -> f32 { (horizontal * horizontal / (2.0 * atlas::RADIUS_M)) as f32 }

/// Soft sun visibility from `(x, z, h)` across the nested grids.
fn sun_shadow(clip: &Clipmap, x: f64, z: f64, h: f32, level: usize, light: &Light) -> f32 {
    let horizontal = (light.sun.x * light.sun.x + light.sun.z * light.sun.z).sqrt();
    if horizontal < 1e-3 { return 1.0; }
    let (dx, dz, rise) = (f64::from(light.sun.x / horizontal), f64::from(light.sun.z / horizontal), light.sun.y / horizontal);
    let (mut travelled, mut lit) = (0.0_f64, 1.0_f32);
    let mut lvl = level;
    for _ in 0..96 {
        let step = clip.levels[lvl].spacing;
        travelled += step;
        let Some((k, u, v)) = clip.find(x + dx * travelled, z + dz * travelled, lvl) else { break; };
        lvl = k;
        let clearance = h + rise * travelled as f32 + 0.05 - clip.levels[k].skin(u, v);
        lit = lit.min(1.0 + 10.0 * clearance / travelled as f32);
        if lit <= 0.0 { return 0.0; }
        // Past this height nothing on the planet can occlude.
        if h + rise * travelled as f32 > 520.0 { break; }
    }
    lit.clamp(0.0, 1.0)
}

fn sky(d: Vec3, light: &Light) -> Vec3 {
    let up = d.y.max(0.0);
    let base = Vec3::new(0.62, 0.74, 0.90).lerp(Vec3::new(0.10, 0.24, 0.58), up.powf(0.45)) * 1.6;
    let toward = d.dot(light.sun).max(0.0);
    base + light.sun_colour * (0.06 * toward.powi(8) + 6.0 * toward.powi(2000))
}

fn shade(clip: &Clipmap, level: usize, u: f64, v: f64, x: f64, z: f64, view: Vec3, light: &Light) -> Vec3 {
    let grid = &clip.levels[level];
    let cell = grid.nearest(u, v);
    let h = grid.skin(u, v);
    let shadow = sun_shadow(clip, x, z, h, level, light);
    if cell.wet() && grid.top(u, v) < grid.skin(u, v) {
        let mirror = Vec3::new(-view.x, view.y.abs().max(0.03), -view.z).normalize();
        water(cell, view, shadow, light, sky(mirror, light) * 0.8)
    } else {
        land(cell, grid.normal(u, v), shadow, light)
    }
}

pub enum Layer { Shaded, Cover, Slope, Height, Detail }

impl Layer {
    pub fn parse(name: &str) -> anyhow::Result<Self> {
        Ok(match name {
            "shaded" => Self::Shaded, "cover" => Self::Cover, "slope" => Self::Slope,
            "height" => Self::Height, "detail" => Self::Detail,
            other => anyhow::bail!("unknown layer {other}: shaded|cover|slope|height|detail"),
        })
    }
}

fn hypsometric(h: f32) -> Vec3 {
    if h < SEA { return Vec3::new(0.02, 0.07, 0.16).lerp(Vec3::new(0.10, 0.30, 0.42), ((h - SEA + 58.0) / 58.0).clamp(0.0, 1.0)); }
    let stops = [(0.0, Vec3::new(0.10, 0.25, 0.08)), (60.0, Vec3::new(0.30, 0.34, 0.10)), (160.0, Vec3::new(0.40, 0.28, 0.14)),
        (300.0, Vec3::new(0.35, 0.30, 0.28)), (500.0, Vec3::new(0.85, 0.85, 0.88))];
    let e = h - SEA;
    for pair in stops.windows(2) {
        if e <= pair[1].0 { return pair[0].1.lerp(pair[1].1, ((e - pair[0].0) / (pair[1].0 - pair[0].0)).clamp(0.0, 1.0)); }
    }
    stops[4].1
}

/// Top-down window, north (+z) up, one grid cell per pixel.
pub fn patch(grid: Grid, layer: &Layer, light: &Light) -> Vec<[f32; 3]> {
    let n = grid.n;
    let clip = Clipmap { levels: vec![grid] };
    let grid = &clip.levels[0];
    // Box blur of the top for the detail layer: what survives is fine relief.
    let blurred: Vec<f32> = if matches!(layer, Layer::Detail) {
        let r = ((48.0 / grid.spacing).round() as usize).clamp(2, n / 4);
        let mut sums = vec![0.0f64; (n + 1) * (n + 1)];
        for j in 0..n { for i in 0..n {
            sums[(j + 1) * (n + 1) + i + 1] = f64::from(grid.at(i, j).top) + sums[j * (n + 1) + i + 1]
                + sums[(j + 1) * (n + 1) + i] - sums[j * (n + 1) + i];
        }}
        (0..n * n).map(|k| {
            let (i, j) = (k % n, k / n);
            let (i0, i1, j0, j1) = (i.saturating_sub(r), (i + r + 1).min(n), j.saturating_sub(r), (j + r + 1).min(n));
            let total = sums[j1 * (n + 1) + i1] - sums[j0 * (n + 1) + i1] - sums[j1 * (n + 1) + i0] + sums[j0 * (n + 1) + i0];
            (total / ((i1 - i0) * (j1 - j0)) as f64) as f32
        }).collect()
    } else { Vec::new() };
    field::par_rows(n, |row| {
        let j = n - 1 - row;
        (0..n).map(|i| {
            let cell = grid.at(i, j);
            let (u, v) = (i as f64, j as f64);
            let colour = match layer {
                Layer::Shaded => shade(&clip, 0, u, v, grid.x0 + u * grid.spacing, grid.z0 + v * grid.spacing, Vec3::Y, light),
                Layer::Cover => cell.albedo * 2.2,
                Layer::Slope => Vec3::splat((grid.slope(i, j) / 1.2).clamp(0.0, 1.0).sqrt() * 1.4),
                Layer::Height if cell.wet() => hypsometric(cell.top.min(SEA - 0.01)) * 1.3,
                Layer::Height => {
                    let h = cell.skin();
                    let contour = ((h / 10.0).fract() - 0.5).abs() * 2.0;
                    let line = if cell.wet() { 1.0 } else { 0.55 + 0.45 * (1.0 - contour).min(0.12 * 10.0 / (grid.slope(i, j) * grid.spacing as f32 + 1e-3)).clamp(0.0, 1.0) };
                    hypsometric(h) * line * (0.6 + 0.8 * grid.normal(u, v).dot(light.sun).max(0.0)) * 1.3
                }
                Layer::Detail => {
                    let d = (cell.top - blurred[j * n + i]) / 6.0;
                    let warm = Vec3::new(0.9, 0.35, 0.1) * d.clamp(0.0, 1.0);
                    let cool = Vec3::new(0.1, 0.4, 0.9) * (-d).clamp(0.0, 1.0);
                    Vec3::splat(0.18) + warm + cool
                }
            };
            colour.to_array()
        }).collect()
    })
}

pub struct Camera {
    pub x: f64,
    pub z: f64,
    /// Height above the ground or water under the eye, metres.
    pub above_m: f32,
    /// Radians; yaw 0 looks along +z (north), positive turns toward +x (east).
    pub yaw: f32,
    pub pitch: f32,
    pub fov_deg: f32,
}

/// Perspective view marched against nested height grids. Detail matches the
/// pixel footprint: the finest grid has `2 px` cells at the nearest ground.
pub fn view(cfg: &WorldSampleConfig, cam: &Camera, width: usize, height: usize, light: &Light) -> Vec<[f32; 3]> {
    let half = (cam.fov_deg.to_radians() * 0.5).tan();
    let pixel = 2.0 * half / height as f32;
    let step0 = cfg.voxel_meters().min(0.5);
    let wanted = f64::from(cam.above_m * pixel * 1.5).max(f64::from(step0));
    let s0 = f64::from(step0) * 2f64.powi((wanted / f64::from(step0)).log2().floor() as i32);
    let n = 320;
    let levels = ((12_000.0 / (s0 * n as f64 * 0.5)).log2().ceil() as usize).clamp(3, 12) + 1;
    let clip = Clipmap::build(cfg, cam.x, cam.z, s0, n, levels);
    let Some((k, u, v)) = clip.find(cam.x, cam.z, 0) else { return vec![[0.0; 3]; width * height]; };
    let eye_y = clip.levels[k].skin(u, v) + cam.above_m;
    let forward = Vec3::new(cam.yaw.sin() * cam.pitch.cos(), cam.pitch.sin(), cam.yaw.cos() * cam.pitch.cos());
    let right = Vec3::new(cam.yaw.cos(), 0.0, -cam.yaw.sin());
    let up = forward.cross(right).normalize();
    let far = clip.levels[levels - 1].spacing * (n as f64 * 0.5 - 2.0);
    let horizon_haze = Vec3::new(0.62, 0.74, 0.90) * 1.6;
    field::par_rows(height, |py| (0..width).map(|px| {
        let sx = ((px as f32 + 0.5) / width as f32 * 2.0 - 1.0) * half * width as f32 / height as f32;
        let sy = (1.0 - (py as f32 + 0.5) / height as f32 * 2.0) * half;
        let d = (forward + right * sx + up * sy).normalize();
        let horizontal = f64::from((d.x * d.x + d.z * d.z).sqrt());
        let gap_at = |t: f64| -> Option<(f32, usize, f64, f64)> {
            let (x, z) = (cam.x + f64::from(d.x) * t, cam.z + f64::from(d.z) * t);
            let min_level = ((t * f64::from(pixel) * 1.2 / s0).max(1.0).log2().floor() as usize).min(levels - 1);
            let (k, u, v) = clip.find(x, z, min_level)?;
            let ground = clip.levels[k].skin(u, v) - drop_m(t * horizontal);
            Some((eye_y + d.y * t as f32 - ground, k, u, v))
        };
        let (mut t, mut before) = (0.0_f64, 0.0_f64);
        let mut hit = None;
        while t < far {
            let Some((gap, k, _, _)) = gap_at(t) else { break; };
            if gap < 0.0 {
                let (mut lo, mut hi) = (before, t);
                for _ in 0..10 {
                    let mid = (lo + hi) * 0.5;
                    if gap_at(mid).is_some_and(|g| g.0 < 0.0) { hi = mid; } else { lo = mid; }
                }
                hit = gap_at(hi).map(|(_, k, u, v)| (hi, k, u, v));
                break;
            }
            before = t;
            t += (clip.levels[k].spacing * 0.5).max(f64::from(gap) * 0.35);
        }
        let colour = match hit {
            Some((t, k, u, v)) => {
                let (x, z) = (cam.x + f64::from(d.x) * t, cam.z + f64::from(d.z) * t);
                let lit = shade(&clip, k, u, v, x, z, -d, light);
                let air = (-(t as f32) / 9000.0).exp();
                lit * air + horizon_haze * (1.0 - air)
            }
            None => sky(d, light),
        };
        colour.to_array()
    }).collect())
}

pub enum GlobeLayer { Shaded, Height, Tectonics, Flow, Sediment, Climate }

impl GlobeLayer {
    pub fn parse(name: &str) -> anyhow::Result<Self> {
        Ok(match name {
            "shaded" => Self::Shaded, "height" => Self::Height, "tectonics" => Self::Tectonics,
            "flow" => Self::Flow, "sediment" => Self::Sediment, "climate" => Self::Climate,
            other => anyhow::bail!("unknown globe layer {other}: shaded|height|tectonics|flow|sediment|climate"),
        })
    }
}

/// Orthographic whole-planet view centred on `(lon, lat)` degrees.
pub fn globe(cfg: &WorldSampleConfig, lon_deg: f64, lat_deg: f64, size: usize, layer: &GlobeLayer, sun_phase_deg: f32) -> Vec<[f32; 3]> {
    let (lon, lat) = (lon_deg.to_radians(), lat_deg.to_radians());
    let centre = DVec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin());
    let east = DVec3::new(-lon.sin(), 0.0, lon.cos());
    let north = east.cross(centre).normalize();
    let radius = size as f64 * 0.47;
    let footprint = (atlas::RADIUS_M / radius) as f32;
    let dir_of = |px: usize, py: usize| -> Option<DVec3> {
        let (a, b) = ((px as f64 + 0.5 - size as f64 * 0.5) / radius, (size as f64 * 0.5 - py as f64 - 0.5) / radius);
        let c2 = 1.0 - a * a - b * b;
        (c2 > 0.0).then(|| (east * a + north * b + centre * c2.sqrt()).normalize())
    };
    let cells: Vec<Option<(Cell, DVec3)>> = field::par_rows(size, |py| (0..size).map(|px| {
        dir_of(px, py).map(|dir| {
            let (x, z) = field::chart_of(dir);
            (field::sample(cfg, x, z, footprint), dir)
        })
    }).collect());
    let phase = f64::from(sun_phase_deg.to_radians());
    let sun = (centre * phase.cos() + east * phase.sin() * 0.92 + north * phase.sin() * 0.39).normalize();
    let skin = |px: usize, py: usize| cells[py * size + px].as_ref().map(|(c, _)| f64::from(c.skin()));
    field::par_rows(size, |py| (0..size).map(|px| {
        let Some((cell, dir)) = &cells[py * size + px] else { return [0.0, 0.0, 0.0]; };
        let mu = dir.dot(centre);
        let colour = match layer {
            GlobeLayer::Shaded => {
                // Screen-space height gradient tilts the radial normal.
                let h = f64::from(cell.skin());
                let gx = (skin((px + 1).min(size - 1), py).unwrap_or(h) - skin(px.saturating_sub(1), py).unwrap_or(h)) / (2.0 * f64::from(footprint));
                let gy = (skin(px, py.saturating_sub(1)).unwrap_or(h) - skin(px, (py + 1).min(size - 1)).unwrap_or(h)) / (2.0 * f64::from(footprint));
                let n = (*dir - (east * gx + north * gy) * 1.5).normalize();
                let ndl = n.dot(sun).max(0.0) as f32;
                let day = (dir.dot(sun) as f32 * 6.0 + 0.5).clamp(0.0, 1.0);
                let lit = Vec3::new(1.0, 0.96, 0.90) * 3.0 * ndl * day + Vec3::new(0.20, 0.30, 0.48) * 0.45 * day;
                let surface = if cell.wet() {
                    let through = (-WATER_ABSORB * (cell.water - cell.top).max(0.0) * 2.2).exp();
                    let body = cell.albedo * through + DEEP_WATER * (Vec3::ONE - through);
                    body.lerp(Vec3::new(0.13, 0.11, 0.075), (cell.suspended * 0.7).clamp(0.0, 0.7))
                        * (Vec3::new(1.0, 0.96, 0.90) * 3.0 * dir.dot(sun).max(0.0) as f32 + Vec3::new(0.20, 0.30, 0.48) * 0.45 * day)
                } else { cell.albedo * lit };
                // Thin limb haze so the disc edge reads as air, not a cut-out.
                let haze = ((1.0 - mu as f32).powi(4) * 0.6 * day).clamp(0.0, 0.8);
                surface * (1.0 - haze) + Vec3::new(0.25, 0.42, 0.80) * haze
            }
            GlobeLayer::Height => hypsometric(if cell.wet() { cell.top.min(SEA - 0.01) } else { cell.top }) * 1.6,
            other => {
                let p = planetary::review::lab::probe(cfg, *dir).unwrap_or_default();
                let land = if cell.wet() && cell.top < SEA { 0.35 } else { 1.0 };
                let c = match other {
                    GlobeLayer::Tectonics => Vec3::new(p.collision as f32, (1.0 - (p.boundary_m / 6000.0).min(1.0)) as f32 * 0.8, p.rift as f32)
                        + Vec3::splat(p.fault_damage as f32 * 0.3),
                    GlobeLayer::Flow => {
                        let river = ((p.channel.max(1e-3)).log10() / 3.0).clamp(0.0, 1.0) as f32;
                        hypsometric(cell.top) * 0.5 + Vec3::new(0.1, 0.5, 1.0) * river * 2.0
                    }
                    GlobeLayer::Sediment => Vec3::new(0.9, 0.7, 0.3) * (p.sediment_m / 40.0).clamp(0.0, 1.0) as f32
                        + Vec3::new(0.2, 0.5, 0.9) * p.former_ice.clamp(0.0, 1.0) as f32 + Vec3::new(0.6, 0.6, 0.5) * p.carbonate.clamp(0.0, 1.0) as f32 * 0.5,
                    _ => Vec3::new(((p.annual_c + 15.0) / 45.0).clamp(0.0, 1.0) as f32, 0.25, p.rain.clamp(0.0, 1.0) as f32),
                };
                c * land * 1.4
            }
        };
        colour.to_array()
    }).collect())
}
