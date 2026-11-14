//! Read-only solve fields for the headless planet lab (`examples/planet_lab`).
//!
//! Nothing here feeds generation. The lab renders maps of the solved sphere
//! and compares generators, so it needs the coarse fields beside the composed
//! ground that `planetary::orbital_surface` already returns.
use super::*;

/// Coarse solve state interpolated at one direction.
#[derive(Clone, Copy, Debug, Default)]
pub struct Probe {
    /// Solved surface before any fine relief, metres.
    pub solved_m: f64,
    /// Lake or sea spill level routed through the cell, metres.
    pub spill_m: f64,
    /// Upstream runoff routed through the cell, in solver area units.
    pub discharge: f64,
    /// Discharge divided by the channel-onset threshold; rivers start near 1.
    pub channel: f64,
    pub sediment_m: f64,
    pub collision: f64,
    pub rift: f64,
    pub carbonate: f64,
    pub former_ice: f64,
    pub fault_damage: f64,
    /// Rain 0..1 and mean annual temperature in Celsius.
    pub rain: f64,
    pub annual_c: f64,
    /// Distance to the nearest plate boundary, metres, and its convergence rate.
    pub boundary_m: f64,
    pub convergence: f64,
}

pub fn probe(cfg: &WorldSampleConfig, dir: DVec3) -> Option<Probe> {
    if !active(cfg) { return None; }
    let w = get(cfg);
    let s = |field: &[f64]| if field.is_empty() { 0.0 } else { w.mesh.sample(field, dir) };
    let province = w.plates.at(dir);
    let discharge = s(&w.flow.discharge);
    Some(Probe {
        solved_m: s(&w.height),
        spill_m: s(&w.flow.spill),
        discharge,
        channel: discharge / w.source_area(),
        sediment_m: s(&w.sediment_cover),
        collision: s(&w.collision),
        rift: s(&w.rift),
        carbonate: s(&w.carbonate),
        former_ice: s(&w.former_ice),
        fault_damage: s(&w.fault_damage),
        rain: s(&w.rain),
        annual_c: s(&w.temperature),
        boundary_m: province.boundary_m,
        convergence: province.convergence,
    })
}

/// Solve identity: node count and the checksum the geology cache keys on.
pub fn solve_identity(cfg: &WorldSampleConfig) -> Option<(usize, u64)> {
    if !active(cfg) { return None; }
    let w = get(cfg);
    Some((w.height.len(), w.checksum))
}

pub const SEA_M: f64 = SEA;

/// Uphill gradient of the refined surface at chart metres `(x, z)`, as the
/// erosion tiles report it at an 8 m footprint: rise per metre along +x and
/// along +z, and the tile exposure weight. The carve stage follows this
/// vector; the lab checks it against a finite difference of sampled heights.
pub fn drainage_gradient(cfg: &WorldSampleConfig, x: f64, z: f64) -> Option<([f64; 2], f64)> {
    if !active(cfg) { return None; }
    let w = get(cfg);
    let dir = direction(cfg, x, z);
    let point = w.surface_query(dir);
    let mut ground = point.ground_base(cfg, 8.0, w.volcano_shape(dir));
    let (weight, gradient) = w.slope_tiles.apply(&w, cfg, dir, 8.0, &mut ground)?;
    let (face, local) = atlas::split(DVec3::new(x, 0.0, z));
    let (east, north) = Projection::Tangent.tangents(face, local.x / atlas::HALF_M, local.z / atlas::HALF_M);
    Some(([gradient.dot(east.normalize()), gradient.dot(north.normalize())], weight))
}
