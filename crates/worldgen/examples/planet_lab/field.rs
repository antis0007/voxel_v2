//! Ground sampling shared by every lab view: one terrain query per sample,
//! reflectance from the surface block and climate, and a nested grid cache.
use engine_content::BlockId;
use engine_core::cube_sphere::atlas;
use engine_worldgen::{WorldSampleConfig, planetary};
use glam::{DVec3, Vec3};
use std::sync::OnceLock;

pub const SEA: f32 = planetary::review::lab::SEA_M as f32;

#[derive(Clone, Copy)]
pub struct Cell {
    /// Ground top in metres, unrounded.
    pub top: f32,
    /// Standing water surface, or NaN on dry ground.
    pub water: f32,
    pub albedo: Vec3,
    pub surface: BlockId,
    pub suspended: f32,
}

impl Cell {
    pub fn wet(&self) -> bool { self.water > self.top }
    /// What a ray from above meets first.
    pub fn skin(&self) -> f32 { if self.wet() { self.water } else { self.top } }
}

/// Linear reflectance per surface block. Visible-band values, darker than
/// broadband albedo tables because those include near infrared.
fn block_albedo(block: BlockId) -> Vec3 {
    static TABLE: OnceLock<Vec<Vec3>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut table = vec![Vec3::splat(0.2); 256];
        for &id in engine_content::ALL_BLOCK_IDS {
            let name = format!("{id:?}");
            let c = match name.as_str() {
                "Grass" => (0.075, 0.125, 0.040),
                "Dirt" => (0.150, 0.095, 0.058),
                "Stone" => (0.190, 0.190, 0.195),
                "Sand" => (0.500, 0.420, 0.270),
                "Sandstone" => (0.420, 0.290, 0.170),
                "SnowGrass" | "Snow" => (0.800, 0.830, 0.870),
                "Gravel" => (0.260, 0.250, 0.230),
                "Mud" => (0.110, 0.085, 0.060),
                "Clay" => (0.250, 0.170, 0.120),
                "Ice" => (0.560, 0.700, 0.800),
                "Basalt" => (0.050, 0.050, 0.058),
                "Ash" => (0.070, 0.065, 0.060),
                "Lava" => (0.900, 0.250, 0.030),
                "Limestone" => (0.480, 0.450, 0.360),
                "GroveMoss" => (0.045, 0.095, 0.032),
                "Paydirt" => (0.300, 0.240, 0.120),
                "Water" => (0.020, 0.060, 0.110),
                _ => {
                    let h = name.bytes().fold(7u32, |a, c| a.wrapping_mul(31).wrapping_add(u32::from(c))) as f32;
                    (0.25 + 0.2 * (h * 0.37).sin(), 0.25 + 0.2 * (h * 0.71).sin(), 0.25 + 0.2 * (h * 0.13).sin())
                }
            };
            table[id as u8 as usize] = Vec3::new(c.0, c.1, c.2);
        }
        table
    });
    table[block as u8 as usize]
}

/// One terrain query at chart metres `(x, z)` filtered to `footprint` metres.
pub fn sample(cfg: &WorldSampleConfig, x: f64, z: f64, footprint: f32) -> Cell {
    // Columns are addressed by voxel; address them at the finest scale when
    // samples are closer than a voxel, or neighbours collapse onto one column.
    let fine = WorldSampleConfig { voxel_scale: engine_core::VoxelScalePreset::QuarterMeter, ..*cfg };
    let cfg = if footprint < cfg.voxel_meters() { &fine } else { cfg };
    let vm = f64::from(cfg.voxel_meters());
    let (column, head, g) = planetary::orbital_surface(cfg, (x / vm).floor() as i32, (z / vm).floor() as i32, footprint);
    let mut albedo = block_albedo(column.surface);
    if matches!(column.surface, BlockId::Grass) {
        // Dry grass cures to straw; wet warm ground is a deeper green.
        let dry = ((0.32 - g.moisture) / 0.32).clamp(0.0, 1.0);
        albedo = albedo.lerp(Vec3::new(0.30, 0.25, 0.11), dry * 0.75);
        albedo = albedo.lerp(Vec3::new(0.040, 0.105, 0.045), ((g.moisture - 0.65) / 0.35).clamp(0.0, 1.0) * 0.5);
    }
    // Canopy only shows once a sample spans whole crowns.
    let canopy = column.vegetation_density.clamp(0.0, 1.0) * ((footprint - 1.5) / 6.0).clamp(0.0, 1.0);
    let leaf = Vec3::new(0.030, 0.062, 0.024).lerp(Vec3::new(0.045, 0.085, 0.028), g.temperature.clamp(0.0, 1.0));
    albedo = albedo.lerp(leaf, canopy * 0.85);
    let water = match head {
        Some(level) => level,
        None if g.top_m < SEA => SEA,
        None => f32::NAN,
    };
    Cell { top: g.top_m, water, albedo, surface: column.surface, suspended: g.suspended }
}

/// Chart metres of a body direction, as the sampler wants them.
pub fn chart_of(dir: DVec3) -> (f64, f64) {
    let p = atlas::from_body(dir * atlas::RADIUS_M);
    (p.x, p.z)
}

pub fn workers() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16)
}

/// Run `row(j)` for every row on a small pool, results in row order.
pub fn par_rows<T: Send>(rows: usize, row: impl Fn(usize) -> Vec<T> + Sync) -> Vec<T> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut parts: Vec<(usize, Vec<T>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers().min(rows.max(1))).map(|_| scope.spawn(|| {
            let mut mine = Vec::new();
            loop {
                let j = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if j >= rows { break; }
                mine.push((j, row(j)));
            }
            mine
        })).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    parts.sort_unstable_by_key(|(j, _)| *j);
    parts.into_iter().flat_map(|(_, v)| v).collect()
}

/// A square grid of cells; `x0, z0` is the centre of cell (0, 0).
pub struct Grid {
    pub n: usize,
    pub spacing: f64,
    pub x0: f64,
    pub z0: f64,
    pub cells: Vec<Cell>,
}

impl Grid {
    pub fn build(cfg: &WorldSampleConfig, cx: f64, cz: f64, spacing: f64, n: usize) -> Self {
        let half = (n as f64 - 1.0) * 0.5 * spacing;
        // Snap so a moved window resamples the same lattice.
        let x0 = ((cx - half) / spacing).round() * spacing;
        let z0 = ((cz - half) / spacing).round() * spacing;
        let cells = par_rows(n, |j| (0..n).map(|i| {
            sample(cfg, x0 + i as f64 * spacing, z0 + j as f64 * spacing, spacing as f32)
        }).collect());
        Self { n, spacing, x0, z0, cells }
    }

    pub fn at(&self, i: usize, j: usize) -> &Cell { &self.cells[j * self.n + i] }

    /// Physical slope from this grid's existing authoritative heights.
    /// Edge cells use one-sided differences; no terrain query is added.
    pub fn slope(&self, i: usize, j: usize) -> f32 {
        let (l, r) = (i.saturating_sub(1), (i + 1).min(self.n - 1));
        let (b, t) = (j.saturating_sub(1), (j + 1).min(self.n - 1));
        let dx = (self.at(r, j).top - self.at(l, j).top) / ((r - l).max(1) as f32 * self.spacing as f32);
        let dz = (self.at(i, t).top - self.at(i, b).top) / ((t - b).max(1) as f32 * self.spacing as f32);
        dx.hypot(dz)
    }

    /// Lattice coordinate of chart metres, if at least `margin` cells inside.
    pub fn locate(&self, x: f64, z: f64, margin: f64) -> Option<(f64, f64)> {
        let (u, v) = ((x - self.x0) / self.spacing, (z - self.z0) / self.spacing);
        let hi = self.n as f64 - 1.0 - margin;
        (u >= margin && v >= margin && u < hi && v < hi).then_some((u, v))
    }

    fn lerp(&self, u: f64, v: f64, f: impl Fn(&Cell) -> f32) -> f32 {
        let (i, j) = ((u.floor() as usize).min(self.n - 2), (v.floor() as usize).min(self.n - 2));
        let (fx, fz) = ((u - i as f64) as f32, (v - j as f64) as f32);
        let a = f(self.at(i, j)) * (1.0 - fx) + f(self.at(i + 1, j)) * fx;
        let b = f(self.at(i, j + 1)) * (1.0 - fx) + f(self.at(i + 1, j + 1)) * fx;
        a * (1.0 - fz) + b * fz
    }

    pub fn top(&self, u: f64, v: f64) -> f32 { self.lerp(u, v, |c| c.top) }
    pub fn skin(&self, u: f64, v: f64) -> f32 { self.lerp(u, v, Cell::skin) }

    pub fn nearest(&self, u: f64, v: f64) -> &Cell {
        self.at((u.round() as usize).min(self.n - 1), (v.round() as usize).min(self.n - 1))
    }

    /// Outward surface normal of the visible skin, y up.
    pub fn normal(&self, u: f64, v: f64) -> Vec3 {
        let e = 0.5;
        let (ul, ur) = ((u - e).max(0.0), (u + e).min(self.n as f64 - 1.001));
        let (vl, vr) = ((v - e).max(0.0), (v + e).min(self.n as f64 - 1.001));
        let dx = (self.skin(ur, v) - self.skin(ul, v)) / ((ur - ul) * self.spacing) as f32;
        let dz = (self.skin(u, vr) - self.skin(u, vl)) / ((vr - vl) * self.spacing) as f32;
        Vec3::new(-dx, 1.0, -dz).normalize()
    }
}

/// Nested grids around one centre: level k has spacing `s0 * 2^k`.
pub struct Clipmap {
    pub levels: Vec<Grid>,
}

impl Clipmap {
    pub fn build(cfg: &WorldSampleConfig, cx: f64, cz: f64, s0: f64, n: usize, levels: usize) -> Self {
        Self { levels: (0..levels).map(|k| Grid::build(cfg, cx, cz, s0 * f64::from(1u32 << k), n)).collect() }
    }

    /// Finest level at or above `min_level` that holds the point.
    pub fn find(&self, x: f64, z: f64, min_level: usize) -> Option<(usize, f64, f64)> {
        (min_level.min(self.levels.len() - 1)..self.levels.len())
            .find_map(|k| self.levels[k].locate(x, z, 1.0).map(|(u, v)| (k, u, v)))
    }
}
