//! Plate tectonics and volcanoes on the planet sphere.
//!
//! Everything is sampled from a unit direction and body metres, so plates and
//! cones continue across cube faces.
use crate::BlockId;
use engine_core::cube_sphere::atlas;
use glam::DVec3;

/// Volcanoes, plate boundaries, fiery wastes and enchanted woods. Dormant until the live version reaches it.
pub const VERSION: u32 = 124;

const PLATES: u32 = 14;
/// Vent lattice pitch in body metres; at most one vent per cell.
const VENT_CELL_M: f64 = 2_600.0;
/// Vents keep this far from the spawn point, plus their own footprint.
const SPAWN_CLEAR_M: f64 = 4_500.0;

fn hash(a: i64, b: i64, c: i64, seed: u32) -> u32 {
    let mut h = seed ^ (a as u32).wrapping_mul(0x9e37_79b9) ^ (b as u32).wrapping_mul(0x85eb_ca6b) ^ (c as u32).wrapping_mul(0xc2b2_ae35);
    h = (h ^ (h >> 16)).wrapping_mul(0x7feb_352d);
    h = (h ^ (h >> 15)).wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}

fn unit(h: u32) -> f64 {
    f64::from(h >> 8) / 16_777_215.0
}

fn plate(seed: u32, i: u32) -> (DVec3, DVec3) {
    let r = |k: i64| unit(hash(i64::from(i), k, 7, seed)) * 2.0 - 1.0;
    let centre = DVec3::new(r(0), r(1), r(2)).try_normalize().unwrap_or(DVec3::Y);
    // Angular velocity about an Euler pole; magnitude spread so some plates stall.
    let pole = DVec3::new(r(3), r(4), r(5)).try_normalize().unwrap_or(DVec3::X);
    (centre, pole * (0.3 + 0.7 * unit(hash(i64::from(i), 6, 7, seed))))
}

/// Plate-boundary state at one point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tectonics {
    /// Metres to the nearest plate boundary along the surface.
    pub boundary_m: f64,
    /// Closing speed across that boundary, -1 spreading to 1 colliding.
    pub convergence: f64,
}

pub fn tectonics(seed: u32, dir: DVec3) -> Tectonics {
    // A slow warp bends Voronoi edges into ragged sutures.
    let warp = DVec3::new(
        crate::planet::noise_at(dir * 4.0, seed.wrapping_add(601)),
        crate::planet::noise_at(dir * 4.0, seed.wrapping_add(607)),
        crate::planet::noise_at(dir * 4.0, seed.wrapping_add(613)),
    ) - 0.5;
    let p = (dir + warp * 0.22).normalize();
    let (mut best, mut second) = ((f64::MIN, 0), (f64::MIN, 0));
    for i in 0..PLATES {
        let d = plate(seed, i).0.dot(p);
        if d > best.0 { second = best; best = (d, i); } else if d > second.0 { second = (d, i); }
    }
    let (a, b) = (plate(seed, best.1), plate(seed, second.1));
    let boundary_m = (best.0.clamp(-1.0, 1.0).acos() - second.0.clamp(-1.0, 1.0).acos()).abs() * 0.5 * atlas::RADIUS_M;
    let normal = (b.0 - a.0 - p * p.dot(b.0 - a.0)).try_normalize().unwrap_or(DVec3::X);
    let relative = b.1.cross(p) - a.1.cross(p);
    Tectonics { boundary_m, convergence: (-relative.dot(normal)).clamp(-1.0, 1.0) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Steep concave cone on a collision arc.
    Strato,
    /// Broad low dome over a rift or hotspot.
    Shield,
    /// Small loose cone on the flank of a larger field.
    Cinder,
    /// Collapsed ring with a sunken lake floor.
    Caldera,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vent {
    pub dir: DVec3,
    pub kind: Kind,
    pub active: bool,
    pub radius_m: f64,
    pub height_m: f64,
}

impl Vent {
    pub fn crater_radius_m(&self) -> f64 {
        let share = match self.kind { Kind::Strato => 0.09, Kind::Shield => 0.05, Kind::Cinder => 0.24, Kind::Caldera => 0.48 };
        share * self.radius_m
    }
}

fn vent_in_cell(seed: u32, cell: [i64; 3]) -> Option<Vent> {
    vent_in_cell_with(seed, cell, true, |d| tectonics(seed, d))
}

pub(crate) fn vent_in_cell_with(seed: u32, cell: [i64; 3], scattered_hotspots: bool, tectonic: impl Fn(DVec3) -> Tectonics) -> Option<Vent> {
    let h = |k: i64| unit(hash(cell[0] * 31 + k, cell[1], cell[2], seed.wrapping_add(9_001)));
    if h(0) > 0.45 { return None; }
    let jitter = DVec3::new(h(1), h(2), h(3));
    let body = (DVec3::new(cell[0] as f64, cell[1] as f64, cell[2] as f64) + jitter) * VENT_CELL_M;
    let dir = body.try_normalize()?;
    // Only the cell that holds the projected surface point owns the vent.
    let surface = (dir * atlas::RADIUS_M / VENT_CELL_M).floor();
    if surface.as_i64vec3().to_array() != cell { return None; }
    // New players arrive at the spawn face centre; no cone or lava within walking range.
    let spawn = (atlas::to_body(DVec3::ZERO) / atlas::RADIUS_M).normalize();
    if (dir - spawn).length() * atlas::RADIUS_M < SPAWN_CLEAR_M { return None; }
    let t = tectonic(dir);
    let near = (-(t.boundary_m / 900.0).powi(2)).exp();
    let arc = near * t.convergence.max(0.0);
    let rift = near * (-t.convergence).max(0.0);
    let hotspot = f64::from(u8::from(scattered_hotspots && h(4) < 0.06));
    if h(5) > (arc * 1.6 + rift * 1.2 + hotspot).min(0.95) { return None; }
    let pick = h(6);
    let kind = if arc >= rift.max(hotspot * 0.5) {
        if pick < 0.55 { Kind::Strato } else if pick < 0.75 { Kind::Caldera } else { Kind::Cinder }
    } else if pick < 0.6 { Kind::Shield } else { Kind::Cinder };
    let size = 0.75 + 0.5 * h(7);
    let (radius_m, height_m) = match kind {
        Kind::Strato => (820.0, 260.0),
        Kind::Shield => (1_500.0, 110.0),
        Kind::Cinder => (170.0, 48.0),
        Kind::Caldera => (1_150.0, 120.0),
    };
    Some(Vent { dir, kind, active: h(8) < if kind == Kind::Caldera { 0.25 } else { 0.55 }, radius_m: radius_m * size, height_m: height_m * size })
}

/// Vents whose cone could reach within `reach_m` of `dir`.
pub fn vents_near(seed: u32, dir: DVec3, reach_m: f64) -> Vec<Vent> {
    let body = dir * atlas::RADIUS_M;
    let span = ((reach_m + 1_800.0) / VENT_CELL_M).ceil() as i64;
    let c = (body / VENT_CELL_M).floor().as_i64vec3();
    let mut out = Vec::new();
    for x in -span..=span {
        for y in -span..=span {
            for z in -span..=span {
                if let Some(v) = vent_in_cell(seed, [c.x + x, c.y + y, c.z + z])
                    && (v.dir - dir).length() * atlas::RADIUS_M < reach_m + v.radius_m * 1.6
                {
                    out.push(v);
                }
            }
        }
    }
    out
}

/// Height offset of one cone at surface distance `d` from its vent, and its slope.
fn profile(v: &Vent, d: f64) -> (f64, f64) {
    let (r, h) = (v.radius_m, v.height_m);
    let x = d / r;
    match v.kind {
        Kind::Strato if x < 1.0 => (h * (1.0 - x).powf(1.7), 1.7 * h / r * (1.0 - x).powf(0.7)),
        Kind::Shield if x < 1.0 => (h * (1.0 - x * x).powf(1.5), 3.0 * h / r * x * (1.0 - x * x).sqrt()),
        Kind::Cinder if x < 1.0 => (h * (1.0 - x), h / r),
        Kind::Caldera => {
            let ring = (-((x - 0.55) / 0.2).powi(2)).exp();
            (h * ring, h / r * ring * 2.0)
        }
        _ => (0.0, 0.0),
    }
}

/// How one column looks after volcanoes and plate boundaries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shaped {
    pub metres: f64,
    /// Flat lake top and its fluid, inside a crater.
    pub lake: Option<(f64, BlockId)>,
    /// Steepest cone slope here, rise over run.
    pub slope: f64,
    /// 0..1 closeness to an active vent; drives the wastes.
    pub heat: f64,
    /// Exposed lava crust, tephra or rock on a sculpted volcanic landform.
    pub(crate) crust: Option<BlockId>,
    pub(crate) active_caldera: bool,
}

/// Adds every nearby cone to `metres`. `base_at` is the bare terrain under a vent.
/// `plates` adds rift troughs and collision welts; the solved face has its own.
pub fn shape(seed: u32, dir: DVec3, metres: f64, plates: bool, base_at: impl Fn(DVec3) -> f64) -> Shaped {
    let out = shape_from(dir, metres, vents_near(seed, dir, 0.0).iter(), base_at, None);
    if !plates { return out; }
    let mut out = out;
    let t = tectonics(seed, dir);
    let band = (-(t.boundary_m / 260.0).powi(2)).exp();
    out.metres += band * if t.convergence < 0.0 { 34.0 * t.convergence } else { 22.0 * t.convergence };
    out.heat = out.heat.max(band * (-t.convergence).max(0.0) * 0.7);
    out
}

pub(crate) fn shape_from<'a>(dir: DVec3, metres: f64, vents: impl Iterator<Item = &'a Vent>, base_at: impl Fn(DVec3) -> f64, asymmetry: Option<u32>) -> Shaped {
    let mut out = Shaped { metres, lake: None, slope: 0.0, heat: 0.0, crust: None, active_caldera: false };
    for v in vents {
        let d = (v.dir - dir).length() * atlas::RADIUS_M;
        let rc = v.crater_radius_m();
        let mut profile_distance = d;
        if let Some(seed) = asymmetry.filter(|_| d > rc && d < v.radius_m * 2.2) {
            // Keep a closed crater rim while breaking up the flanks and apron.
            let p = dir * (atlas::RADIUS_M / v.radius_m);
            let broad = crate::planet::noise_at(p * 1.4, seed.wrapping_add(9701)) - 0.5;
            let gullies = crate::planet::noise_at(p * 3.5, seed.wrapping_add(9703)) - 0.5;
            let flank = ((d - rc) / (v.radius_m * 0.35)).clamp(0.0, 1.0);
            profile_distance *= 1.0 + flank * (0.68 * broad + 0.24 * gullies);
        }
        let (lift, slope) = profile(v, profile_distance);
        if lift == 0.0 && d > v.radius_m * 2.2 { continue; }
        out.metres += lift;
        out.slope = out.slope.max(slope);
        if v.active {
            out.heat = out.heat.max((-(d / (v.radius_m * 2.2)).powi(2)).exp());
        }
        if d < rc {
            let vent_base = base_at(v.dir);
            let depth = match v.kind { Kind::Caldera => v.height_m * 1.1, Kind::Cinder => v.height_m * 0.35, _ => v.height_m * 0.16 };
            let rim = vent_base + profile(&v, rc).0;
            out.metres = out.metres.min(rim - depth * (1.0 - (d / rc).powi(2)));
            let fluid = if v.active && v.kind != Kind::Caldera { BlockId::Lava } else { BlockId::Water };
            let level = rim - depth * 0.55;
            if out.metres < level {
                out.lake = Some((level, fluid));
            }
        }
    }
    out
}

/// Versioned callers use the same plates as terrain and drainage.
pub fn tectonics_for(cfg: &crate::WorldSampleConfig, dir: DVec3) -> Tectonics {
    if crate::planetary::active(cfg) { crate::planetary::tectonics_at(cfg, dir) } else { tectonics(cfg.seed, dir) }
}

pub fn vents_near_for(cfg: &crate::WorldSampleConfig, dir: DVec3, reach_m: f64) -> Vec<Vent> {
    if crate::planetary::active(cfg) { crate::planetary::vents_near(cfg, dir, reach_m) } else { vents_near(cfg.seed, dir, reach_m) }
}

/// Surface cover for volcanic ground, or `None` to keep the climate cover.
pub fn cover(s: &Shaped, patch: f64) -> Option<(BlockId, BlockId, f32)> {
    if let Some((_, fluid)) = s.lake {
        return Some((fluid, fluid, 0.0));
    }
    if let Some(surface) = s.crust {
        return Some((surface, BlockId::Basalt, 0.0));
    }
    if s.slope > 0.32 {
        return Some((BlockId::Basalt, BlockId::Basalt, 0.0));
    }
    if s.heat > 0.55 {
        // Ash is a powder: only on gentle ground, or a slope pours away when touched.
        let surface = if patch > 0.62 { BlockId::Ash } else if patch > 0.35 { BlockId::Basalt } else { BlockId::Gravel };
        return Some((surface, BlockId::Basalt, 0.0));
    }
    if s.slope > 0.12 {
        return Some((BlockId::Basalt, BlockId::Stone, 0.03));
    }
    None
}

#[cfg(test)]
#[path = "volcano_tests.rs"]
mod tests;
