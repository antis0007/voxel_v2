//! Evidence from the erosion bake for settlement, ruin and lore placement.
//! This regional model has effective steps, not calibrated elapsed years.
use crate::{WorldSampleConfig, WorldType};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HistorySample {
    pub effective_erosion_steps: u32,
    pub starting_height_m: f32,
    /// End of the recorded erosion stage, before any later glacial shaping.
    pub eroded_height_m: f32,
    pub current_solved_height_m: f32,
    /// Positive is net deposition; negative is net hillslope export.
    pub hillslope_net_m: f32,
    pub fluvial_incision_m: f32,
    pub fracture_damage: f32,
    /// Final wet contributing area, not volumetric river flow per second.
    pub contributing_area_m2: f32,
    pub lake_surface_m: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HistoryBudget {
    pub effective_erosion_steps: u32,
    pub hillslope_boundary_export_m3: f64,
    /// Detachment-limited removal; not simulated downstream deposition.
    pub fluvial_removal_m3: f64,
}

fn supported(cfg: &WorldSampleConfig) -> bool {
    cfg.generator_version >= super::RAVINE_GENERATOR
        && matches!(cfg.world_type, WorldType::Hills | WorldType::Ridged)
}

/// Physical metre coordinates. Returns None for unrecorded recipes/realms or
/// outside the solved face, rather than inventing history for them.
pub fn sample(cfg: &WorldSampleConfig, x_m: f64, z_m: f64) -> Option<HistorySample> {
    if !supported(cfg) || !x_m.is_finite() || !z_m.is_finite() { return None; }
    let p = super::planet_for(cfg);
    if !p.grid.contains(x_m, z_m) { return None; }
    let r = p.history.as_ref()?;
    let at = |f: &[f32]| p.grid.bilinear(f, x_m, z_m) as f32;
    let before = at(&r.before_m);
    let hillslope = at(&r.hillslope_net_m);
    let incision = at(&r.fluvial_incision_m);
    Some(HistorySample {
        effective_erosion_steps: r.steps,
        starting_height_m: before,
        eroded_height_m: before + hillslope - incision,
        current_solved_height_m: at(&p.height),
        hillslope_net_m: hillslope,
        fluvial_incision_m: incision,
        fracture_damage: at(&r.fracture_damage),
        contributing_area_m2: at(&p.discharge).max(0.0),
        lake_surface_m: super::rivers::lake(&p, x_m, z_m).map(|y| y as f32),
    })
}

pub fn budget(cfg: &WorldSampleConfig) -> Option<HistoryBudget> {
    if !supported(cfg) { return None; }
    let p = super::planet_for(cfg);
    let r = p.history.as_ref()?;
    Some(HistoryBudget { effective_erosion_steps: r.steps,
        hillslope_boundary_export_m3: r.boundary_export_m3,
        fluvial_removal_m3: r.fluvial_removal_m3 })
}

pub(super) fn checksum(record: &super::mountain::HistoryRecord, mut h: u64) -> u64 {
    for field in [&record.before_m, &record.hillslope_net_m, &record.fluvial_incision_m, &record.fracture_damage] {
        for value in field { for b in value.to_bits().to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }}
    }
    for b in record.steps.to_le_bytes().into_iter().chain(record.boundary_export_m3.to_le_bytes())
        .chain(record.fluvial_removal_m3.to_le_bytes()) {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}
