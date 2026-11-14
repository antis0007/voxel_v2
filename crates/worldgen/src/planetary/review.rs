//! Read-only review coordinates from the authoritative receiver graph.
use super::*;

/// The cross-section relation used by the coastal bake, available to field comparisons.
pub fn relative_conveyance(area_m2: f64, width_m: f64) -> f64 {
    morphodynamics::conveyance(area_m2, width_m)
}

/// Centre of the largest sediment-fed coastal bake.
pub fn coastal_origin(cfg: &WorldSampleConfig) -> Option<DVec3> {
    if !active(cfg) { return None; }
    get(cfg).alluvium.as_ref()?.coast_origin().map(atlas::from_body)
}

/// Start of the walking approach, viewing lip and open-water target in atlas metres.
pub fn coastal_outlook(cfg: &WorldSampleConfig) -> Option<[DVec3; 3]> {
    if !active(cfg) { return None; }
    Some(get(cfg).alluvium.as_ref()?.outlook_points()?.map(atlas::from_body))
}

pub mod catchment;
pub mod history;
pub mod lab;

pub struct WatershedSweep {
    pub side: usize,
    pub cell_m: f64,
    pub heights: Vec<Vec<f32>>,
    pub wet: Vec<f32>,
}

/// Screen regional terrain with a fixed landscape and runoff. Each triple
/// scales uplift, incision rate and the contributing area for channel onset.
pub fn watershed_sweep(cfg: &WorldSampleConfig, x: f64, z: f64, parameters: &[[f64; 3]]) -> Option<WatershedSweep> {
    if !active(cfg) || !evolved_watersheds(cfg.generator_version) || !x.is_finite() || !z.is_finite()
        || parameters.is_empty() || parameters.iter().flatten().any(|p| !(0.05..=4.0).contains(p))
    {
        return None;
    }
    Some(slope_relief::watershed::sweep(&get(cfg), cfg, direction(cfg, x, z), parameters))
}

/// Main-stem reach immediately below a junction of two wet tributaries.
pub fn confluence(cfg: &WorldSampleConfig) -> Option<(DVec3, DVec3, f64)> {
    if !active(cfg) { return None; }
    let world = get(cfg);
    let mut incoming = vec![0u8; world.height.len()];
    for (i, &r) in world.flow.receiver.iter().enumerate() {
        if i != r && world.height[i] > SEA && world.flow.discharge[i] >= world.source_area() {
            incoming[r] += 1;
        }
    }
    let spawn = spawn_point(cfg);
    let point = |i: usize| atlas::from_body(world.river_nodes[i]);
    let i = (0..incoming.len()).filter(|&i| {
        let r = world.flow.receiver[i];
        incoming[i] >= 2 && i != r && world.height[r] > SEA + 4.0
            && world.flow.spill[i] <= world.height[i] + 0.75
            && atlas::split(point(i)).0 == atlas::split(spawn).0
            && atlas::split(point(r)).0 == atlas::split(spawn).0
    }).min_by(|&a, &b| (point(a) - spawn).length_squared()
        .total_cmp(&(point(b) - spawn).length_squared()).then(a.cmp(&b)))?;
    Some((point(i), point(world.flow.receiver[i]), world.flow.discharge[i]))
}
/// Compare the spherical solve with and without the shelf/trench profile.
/// Both runs keep tectonics, faulting, runoff and coastal sediment transport.
pub fn bathymetry_audit(cfg: &crate::WorldSampleConfig) -> BathymetryAudit {
    use super::{SEA, build};
    let shaped = super::get(cfg);
    let unshaped = build(cfg, &crate::geo::Progress::default(), false);
    let mesh = &shaped.mesh;
    let area: f64 = mesh.area.iter().sum();
    let mut result = BathymetryAudit::default();
    let mut shifts = Vec::new();
    let crossing = |a: f64, b: f64| ((a > SEA) != (b > SEA)).then(|| (SEA - a) / (b - a));
    for (i, (&a, &b)) in unshaped.height.iter().zip(&shaped.height).enumerate() {
        let weight = mesh.area[i] / area * 100.0;
        match (a > SEA, b > SEA) {
            (true, false) => result.drowned_percent += weight,
            (false, true) => result.emerged_percent += weight,
            _ => {}
        }
        if a > SEA { result.land_without_percent += weight; }
        if b > SEA { result.land_with_percent += weight; }
        for &(j, metres) in &mesh.neighbours[i] {
            if j <= i { continue; }
            if let (Some(before), Some(after)) = (
                crossing(a, unshaped.height[j]), crossing(b, shaped.height[j]),
            ) {
                shifts.push((before - after).abs() * metres);
            }
        }
    }
    shifts.sort_unstable_by(f64::total_cmp);
    result.shared_crossings = shifts.len();
    if let Some(&max) = shifts.last() {
        result.crossing_max_m = max;
        result.crossing_p95_m = shifts[(shifts.len() - 1) * 95 / 100];
    }
    // Confirm the final, offshore-only profile affects real sampled beds and
    // leaves their sea classification unchanged, beyond the routing mesh.
    let frames = super::slot(cfg).entry.progress.drift_frames();
    result.sea_range_m = frames.first().map(|first| {
        frames.iter().fold([first.sea_m; 2], |[low, high], f| [low.min(f.sea_m), high.max(f.sea_m)])
    });
    if super::changing_seas(cfg.generator_version) {
        for &dir in mesh.directions.iter().step_by(8) {
            let a = unshaped.ground(cfg, dir, 512.0).top_m;
            let b = shaped.ground(cfg, dir, 512.0).top_m;
            result.resolved_samples += 1;
            result.resolved_land_flips += usize::from((a > SEA as f32) != (b > SEA as f32));
            result.resolved_bed_changes += usize::from((a - b).abs() > 0.01);
        }
    }
    result
}

#[derive(Debug, Default)]
pub struct BathymetryAudit {
    pub land_without_percent: f64,
    pub land_with_percent: f64,
    pub drowned_percent: f64,
    pub emerged_percent: f64,
    pub shared_crossings: usize,
    pub crossing_p95_m: f64,
    pub crossing_max_m: f64,
    pub resolved_samples: usize,
    pub resolved_land_flips: usize,
    pub resolved_bed_changes: usize,
    pub sea_range_m: Option<[f32; 2]>,
}
