//! Kinematic plate history on a shared cube sphere. The plate boundaries
//! move with finite Euler rotations. Crust is backtraced and resampled before
//! boundary deformation is applied, so ranges and basins can leave a margin
//! and survive in a plate interior.
use std::sync::OnceLock;

use glam::DVec3;

use super::{SEA, tectonics::Plates, topology::Mesh};
use crate::geo::Progress;
mod cadence;
mod deep_time;
mod climate_cycle;
mod ice;
mod coupled;
mod episodes;
pub(super) mod history;

pub const DRIFT_MYR: u16 = 240;
const STEP_MYR: u16 = 5;
const SNAPSHOT_MYR: u16 = 10;
const SIDE: usize = 56;
const RIFTED_SIDE: usize = 84;
const RADIANS_PER_MYR: f64 = 0.0046;
pub(super) const PLATE_REFERENCE_CM_PER_YEAR: f64 = 2.5;
pub(super) const PLATE_MAX_CM_PER_YEAR: f64 = 5.0;
pub(super) const PLATE_ACCELERATION_CM_PER_YEAR_PER_MYR: f64 = 0.006;
/// 2.5 cm/year on a 6371 km reference planet, converted to radians/Myr.
/// Preserve angular motion on the compact planet, so plates do not lap its
/// smaller circumference at an unscaled Earth metre rate.
pub(super) fn motion_radians_per_myr(generator: u32) -> f64 {
    if super::resolved_history(generator) { PLATE_REFERENCE_CM_PER_YEAR * 10_000.0 / 6_371_000.0 }
    else { RADIANS_PER_MYR }
}
pub(super) const MOTION_CHANGE_AGE_MYR: u16 = STEP_MYR;

pub struct DriftFrame {
    pub duration_myr: u16,
    pub myr: u16,
    pub n: usize,
    /// Six faces, in Face::ALL order. Edge and corner samples are shared.
    pub texels: Vec<DriftTexel>,
    pub budget: Option<HistoryBudget>,
    pub sea_m: f32,
    pub annual_range_c: Option<[f32; 2]>,
    /// Bounded selection of actual threshold crossings since the previous frame.
    pub events: Vec<HistoryEvent>,
    /// Per current plate, the scaled-axis Euler rotation (radians) that carried
    /// its crust from the previous published frame to this one. Empty when the
    /// history records no motion stages; playback then crossfades.
    pub motion: Vec<[f32; 3]>,
}

#[derive(Clone, Copy)]
pub struct HistoryEvent {
    pub myr: f64,
    pub direction: [f32; 3],
    pub metres: f32,
    pub eruption: bool,
}

#[derive(Clone, Copy, Default)]
pub struct HistoryBudget {
    pub continental_km3: f64,
    pub continental_error_km3: f64,
    pub continental_eroded_km3: f64,
    pub continental_created_km3: f64,
    pub eroded_m3: f64,
    pub sediment_m3: f64,
    pub sediment_error_m3: f64,
    pub alongshore_m3: f64,
    pub plates: usize,
}

#[derive(Clone, Copy)]
pub struct DriftTexel {
    pub height_m: f32,
    /// Convergence, spreading, and transform shear, respectively.
    pub stress: [f32; 3],
    pub continental_km: f32,
    pub ocean_age_myr: f32,
    pub sediment_m: f32,
    pub runoff_m2: f32,
    /// Dominant original continental plate, with zero for oceanic crust.
    pub lineage: u16,
    /// Moisture and annual temperature from the simulated climate, when available.
    pub climate: Option<[f32; 2]>,
    pub winter_c: Option<f32>,
    pub rupture_m: f32,
    pub eruption_m: f32,
    pub weathered_myr: f32,
    /// Plate that owns this texel at the frame's time; indexes `DriftFrame::motion`.
    pub plate: u16,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Crust {
    pub continental: f64,
    pub relief_m: f64,
    pub(super) ocean_age_myr: f64,
    pub continental_km: f64,
    pub sediment_m: f64,
    pub strain: f64,
    pub slip_m: f64,
    pub lineage: u16,
    pub carbonate: f64,
}

pub(super) struct Drift {
    mesh: &'static Mesh,
    crust: Vec<Crust>,
    history: Option<history::Field>,
    sea_m: f64,
    ice: Option<ice::Ice>,
    deep: Option<deep_time::Evolution>,
}

impl Drift {
    pub(super) fn solve(plates: &mut Plates, progress: &Progress) -> Self {
        if super::coupled_history(plates.config().generator_version) {
            return coupled::solve(plates, progress);
        }
        static MESH: OnceLock<Mesh> = OnceLock::new();
        static RIFTED_MESH: OnceLock<Mesh> = OnceLock::new();
        let rifted = plates.rifted_margins();
        let mesh = if rifted {
            RIFTED_MESH.get_or_init(|| Mesh::with_side(RIFTED_SIDE))
        } else {
            MESH.get_or_init(|| Mesh::with_side(SIDE))
        };
        let crust = mesh
            .directions
            .iter()
            .map(|&dir| Crust {
                continental: plates.initial_crust(dir),
                ocean_age_myr: 100.0,
                relief_m: 0.0,
                ..Crust::default()
            })
            .collect();
        let mut field = Self { mesh, crust, history: None, sea_m: SEA, ice: None, deep: None };
        // The seeded warp depends on location, not plate positions or time.
        let warped: Vec<_> = mesh.directions.iter().map(|&dir| plates.warp(dir)).collect();
        progress.publish_drift(field.frame(plates, 0, &warped));
        let mut next = Vec::with_capacity(field.crust.len());
        for myr in (STEP_MYR..=DRIFT_MYR).step_by(usize::from(STEP_MYR)) {
            if matches!(myr, 60 | 140) {
                plates.split(u32::from(myr));
            }
            if myr == DRIFT_MYR - MOTION_CHANGE_AGE_MYR + STEP_MYR {
                plates.reorganize();
            }
            let step_radians = RADIANS_PER_MYR * f64::from(STEP_MYR);
            let inverse = plates.advance(step_radians);
            next.extend(mesh.directions.iter().zip(&warped).map(|(&dir, &q)| {
                let edge = plates.warped_edge(q);
                let [convergence, spreading, shear] = edge.stresses().map(|v| v.min(1.0));
                let old = field.at(inverse[edge.owner] * dir);
                let continental = if rifted {
                    // Extend a moving crust column across the rift belt. The
                    // inherited thickness falls as its footprint stretches.
                    old.continental * (-spreading * step_radians / super::tectonics::RIFT_WIDTH).exp()
                } else {
                    old.continental * (1.0 - spreading * 0.24)
                };
                let ocean = 1.0 - continental;
                let age = (old.ocean_age_myr + f64::from(STEP_MYR)).min(160.0)
                    * (1.0 - spreading * ocean * 0.82);
                let other = if convergence > 0.01 {
                    let across = (dir + edge.normal * (edge.boundary + 0.06)).normalize();
                    field.at(inverse[edge.neighbour] * across)
                } else {
                    Crust { continental, ..old }
                };
                let orogeny = continental * (5.0 + 15.0 * other.continental);
                let contrast = continental - other.continental;
                let age_myr = if contrast > 0.0 { other.ocean_age_myr } else { age };
                let subduction = if plates.faults() {
                    let plate_bend = (age_myr / 160.0).sqrt();
                    contrast * (7.0 + 7.0 * plate_bend)
                } else { 10.0 * contrast };
                let opening = if rifted {
                    // Thinning lowers the buoyancy-supported surface in the
                    // province solve. Young oceanic crust adds ridge relief.
                    spreading * 2.0 * ocean
                } else {
                    spreading * (2.0 * ocean - 12.0 * old.continental)
                };
                let fault_relief = shear * (2.0 * continental - 1.0);
                // Young seafloor cools and subsides approximately with the
                // square root of its age. Only the age change affects relief.
                let cooling = ocean * 3.5 * (old.ocean_age_myr.sqrt() - age.sqrt());
                let relief_m = (old.relief_m * 0.94
                    + convergence * (orogeny + subduction)
                    + opening
                    + fault_relief
                    + cooling)
                    .clamp(-85.0, 180.0);
                Crust {
                    continental,
                    relief_m,
                    ocean_age_myr: age,
                    ..Crust::default()
                }
            }));
            std::mem::swap(&mut field.crust, &mut next);
            next.clear();
            if myr % SNAPSHOT_MYR == 0 {
                progress.publish_drift(field.frame(plates, myr, &warped));
            }
        }
        field
    }

    pub(super) fn history_column(&self, dir: DVec3) -> Option<(history::Column, f64)> {
        self.history.as_ref().map(|h| h.column(self.mesh, dir))
    }

    pub(super) fn former_ice(&self, dir: DVec3) -> f64 {
        self.ice.as_ref().map_or(0.0, |ice| ice.at(self.mesh, dir))
    }

    pub(super) fn climate_anomaly(&self, dir: DVec3) -> f64 {
        self.deep.as_ref().and_then(|d| d.climate.as_ref()).map_or(0.0, |c| c.anomaly(dir.y))
    }

    pub(super) fn history_checksum(&self, hash: u64) -> u64 {
        self.history.as_ref().map_or(hash, |h| h.checksum(hash))
    }

    pub(super) fn at(&self, dir: DVec3) -> Crust {
        let (ids, weights) = self.mesh.stencil(dir);
        let mut sample = Crust::default();
        let mut strongest = 0.0;
        for (i, w) in ids.into_iter().zip(weights) {
            let v = self.crust[i];
            sample.continental += v.continental * w;
            sample.relief_m += v.relief_m * w;
            sample.ocean_age_myr += v.ocean_age_myr * w;
            sample.continental_km += v.continental_km * w;
            sample.sediment_m += v.sediment_m * w;
            sample.strain += v.strain * w;
            sample.slip_m += v.slip_m * w;
            sample.carbonate += v.carbonate * w;
            if v.continental_km * w > strongest {
                strongest = v.continental_km * w;
                sample.lineage = v.lineage;
            }
        }
        sample
    }

    fn frame(&self, plates: &Plates, myr: u16, warped: &[DVec3]) -> DriftFrame {
        if super::fine_history(plates.config().generator_version) {
            return cadence::frame(self, plates, myr, warped);
        }
        let vertices: Vec<_> = self
            .mesh
            .directions
            .iter()
            .zip(warped)
            .enumerate()
            .map(|(i, (&dir, &q))| {
                let edge = plates.warped_edge(q);
                let height = plates.evolved_at(dir, &edge, |d| self.at(d)).height;
                DriftTexel {
                    height_m: (height - self.sea_m) as f32,
                    stress: edge.stresses().map(|v| v.min(1.0) as f32),
                    continental_km: self.crust[i].continental_km as f32,
                    ocean_age_myr: self.crust[i].ocean_age_myr as f32,
                    sediment_m: self.crust[i].sediment_m as f32,
                    runoff_m2: 0.0,
                    lineage: self.crust[i].lineage,
                    climate: self.history.as_ref().map(|h| [h.records[i].wet as f32, h.records[i].temperature as f32]),
                    winter_c: self.history.as_ref().map(|h| h.records[i].winter_c as f32),
                    rupture_m: self.history.as_ref().map_or(0.0, |h| h.records[i].quake_m as f32),
                    eruption_m: self.history.as_ref().map_or(0.0, |h| h.records[i].eruption_m as f32),
                    weathered_myr: self.history.as_ref().map_or(0.0, |h| h.records[i].weathered_myr as f32),
                    plate: edge.owner as u16,
                }
            })
            .collect();
        DriftFrame {
            duration_myr: super::history_duration_myr(&plates.config()),
            myr,
            sea_m: self.sea_m as f32,
            annual_range_c: self.history.as_ref().map(|history| {
                history.records.iter().fold([f32::INFINITY, f32::NEG_INFINITY], |[low, high], record| {
                    let t = record.temperature as f32;
                    [low.min(t), high.max(t)]
                })
            }),
            n: if plates.rifted_margins() { RIFTED_SIDE + 1 } else { SIDE + 1 },
            texels: self
                .mesh
                .face_vertices()
                .iter()
                .map(|&i| vertices[i])
                .collect(),
            budget: None,
            events: Vec::new(),
            motion: Vec::new(),
        }
    }
}

mod snapshot;
