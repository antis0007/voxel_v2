//! Physical process cadence is independent of retained preview resolution.
use super::{DriftFrame, STEP_MYR};

pub(super) struct Clock {
    pub plate_myr: u16,
    pub surface_steps: usize,
    pub coast_steps: usize,
}

impl Clock {
    pub fn for_generator(generator: u32) -> Self {
        if crate::planetary::fine_history(generator) {
            Self { plate_myr: 1, surface_steps: 4, coast_steps: 4 }
        } else {
            Self { plate_myr: STEP_MYR, surface_steps: 20, coast_steps: 5 }
        }
    }

    pub fn relative_plate_step(&self) -> f64 {
        f64::from(self.plate_myr) / f64::from(STEP_MYR)
    }

    pub fn coast_fraction(&self) -> f64 { self.coast_steps as f64 / 20.0 }
}

/// Sample only retained display vertices, leaving the physical mesh unchanged.
pub(super) fn frame_indices(mesh: &super::Mesh) -> impl Iterator<Item = usize> + '_ {
    let old_n = super::RIFTED_SIDE + 1;
    let n = super::RIFTED_SIDE / 3 + 1;
    (0..6 * n * n).map(move |index| {
        let face = index / (n * n);
        let z = index / n % n;
        let x = index % n;
        mesh.face_vertices()[(face * old_n + z * 3) * old_n + x * 3]
    })
}

pub(super) fn frame(field: &super::Drift, plates: &super::Plates, myr: u16,
    warped: &[glam::DVec3]) -> DriftFrame
{
    let history = field.history.as_ref();
    let texels = frame_indices(field.mesh).map(|i| {
        let dir = field.mesh.directions[i];
        let edge = plates.warped_edge(warped[i]);
        let height = plates.evolved_at(dir, &edge, |d| field.at(d)).height;
        let crust = field.crust[i];
        super::DriftTexel {
            height_m: (height - field.sea_m) as f32,
            stress: edge.stresses().map(|v| v.min(1.0) as f32),
            continental_km: crust.continental_km as f32,
            ocean_age_myr: crust.ocean_age_myr as f32,
            sediment_m: crust.sediment_m as f32,
            runoff_m2: 0.0, lineage: crust.lineage,
            climate: history.map(|h| [h.records[i].wet as f32, h.records[i].temperature as f32]),
            winter_c: history.map(|h| h.records[i].winter_c as f32),
            rupture_m: history.map_or(0.0, |h| h.records[i].quake_m as f32),
            eruption_m: history.map_or(0.0, |h| h.records[i].eruption_m as f32),
            weathered_myr: history.map_or(0.0, |h| h.records[i].weathered_myr as f32),
            plate: edge.owner as u16,
        }
    }).collect();
    DriftFrame {
        duration_myr: crate::planetary::history_duration_myr(&plates.config()),
        myr, n: super::RIFTED_SIDE / 3 + 1, texels, budget: None, events: Vec::new(), motion: Vec::new(),
        sea_m: field.sea_m as f32,
        annual_range_c: history.map(|h| h.records.iter().fold(
            [f32::INFINITY, f32::NEG_INFINITY], |[low, high], r| {
                [low.min(r.temperature as f32), high.max(r.temperature as f32)]
            })),
    }
}
