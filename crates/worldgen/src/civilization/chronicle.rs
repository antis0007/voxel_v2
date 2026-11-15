//! Causal records from bounded settlement history on solved geography.
mod digest;
mod replay;
mod simulation;

use super::{B, History, Region, WorldSampleConfig};
use glam::IVec2;

pub const MODEL_REVISION: u32 = 1;
pub const YEARS: u32 = 1000;
pub const STEP_YEARS: u32 = 20;
pub const MAX_EVENTS: usize = (YEARS / STEP_YEARS) as usize * 2;

/// Surveyed physical inputs, independent of the voxel representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Landscape {
    pub top_mm: i32,
    pub slope_ppm: u32,
    pub moisture_ppm: u32,
    pub sediment_ppm: u32,
    pub flood_risk_ppm: u32,
    pub scree_mm: u32,
    pub bedrock: B,
}

impl Landscape {
    pub(super) fn surveyed(g: crate::geo::Ground, flood_risk: f32, bedrock: B) -> Self {
        let unit = |v: f32| (v.clamp(0.0, 1.0) * 1_000_000.0).round() as u32;
        Self {
            top_mm: (g.top_m * 1000.0).round() as i32,
            slope_ppm: unit(g.slope),
            moisture_ppm: unit(g.moisture),
            sediment_ppm: unit(g.sediment),
            flood_risk_ppm: unit(flood_risk),
            scree_mm: (g.scree_m.max(0.0) * 1000.0).round() as u32,
            bedrock,
        }
    }
}

/// The immutable recipe and solved landscape behind one regional record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Provenance {
    pub seed: u32,
    pub generator: u32,
    pub planet_recipe: u64,
    pub landscape_checksum: u64,
    pub model_revision: u32,
    pub region: [i32; 2],
    pub deep_time_myr: u16,
    pub end_year: u32,
    pub step_years: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Cause {
    Ordinary,
    Drought,
    Flood,
    Rockfall,
}

impl Cause {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ordinary => "harvest",
            Self::Drought => "drought",
            Self::Flood => "flood",
            Self::Rockfall => "rockfall",
        }
    }
}

/// One site's state transition; paired transfers share year and peer identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub id: u64,
    pub site: u32,
    pub year: u32,
    pub cause: Cause,
    pub peer: Option<u32>,
    pub founded: bool,
    pub households_before: u32,
    pub households_after: u32,
    pub stores_before: i32,
    pub stores_after: i32,
    pub harvest: i32,
    pub demand: i32,
    pub surplus_discarded: i32,
    pub unmet_demand: i32,
    pub aid_received: i32,
    pub aid_sent: i32,
    pub migrants_in: u32,
    pub migrants_out: u32,
    pub shortage_departures: u32,
    pub growth: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SiteRecord {
    pub id: u32,
    pub center_mm: [i32; 3],
    pub landscape: Landscape,
    pub history: History,
    pub shelter: i32,
    pub burial_candidate: super::Burial,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chronicle {
    pub provenance: Provenance,
    pub linked: bool,
    pub sites: [Option<SiteRecord>; 2],
    pub events: Vec<Event>,
}

impl Chronicle {
    pub(super) fn simulate(
        cfg: &WorldSampleConfig,
        key: IVec2,
        sites: &mut Region,
        linked: bool,
    ) -> Self {
        let provenance = Provenance {
            seed: cfg.seed,
            generator: cfg.generator_version,
            planet_recipe: cfg.planet_fingerprint(),
            landscape_checksum: crate::geo::planet_for(cfg).checksum,
            model_revision: MODEL_REVISION,
            region: [key.x, key.y],
            deep_time_myr: crate::planetary::DRIFT_MYR,
            end_year: YEARS,
            step_years: STEP_YEARS,
        };
        let events = simulation::run(cfg, key, sites, linked);
        Self {
            provenance,
            linked,
            sites: Self::records(key, sites),
            events,
        }
    }

    pub(super) fn records(key: IVec2, sites: &Region) -> [Option<SiteRecord>; 2] {
        std::array::from_fn(|slot| {
            sites[slot].map(|s| SiteRecord {
                id: super::encode(key, slot, 0),
                landscape: s.landscape,
                history: s.history,
                shelter: s.shelter,
                burial_candidate: s.burial_candidate,
                center_mm: [
                    (s.center.x * 1000.0).round() as i32,
                    (s.datum * 1000.0).round() as i32,
                    (s.center.y * 1000.0).round() as i32,
                ],
            })
        })
    }

    /// Recorded shortages, assistance and displacement, never inferred catastrophes.
    pub fn archive_text(&self, site: u32) -> Option<String> {
        let record = self.sites.iter().flatten().find(|s| s.id == site)?;
        let h = record.history;
        let mut text = format!(
            "{} ledger. Years {}-{}. Peak: {} households; remaining: {}.",
            h.livelihood.name(),
            h.founded,
            self.provenance.end_year,
            h.peak_households,
            h.households
        );
        let event = self
            .events
            .iter()
            .rev()
            .filter(|e| e.site == site)
            .find(|e| e.shortage_departures + e.migrants_out > 0)
            .or_else(|| {
                self.events
                    .iter()
                    .rev()
                    .find(|e| e.site == site && e.cause != Cause::Ordinary)
            });
        if let Some(e) = event {
            text.push_str(&format!(
                " Year {}: {}. {} households left after shortages; {} moved to the linked hold.",
                e.year,
                e.cause.label(),
                e.shortage_departures,
                e.migrants_out
            ));
            if e.aid_received > 0 {
                text.push_str(" Food arrived from the linked hold.");
            }
        }
        if let Some(year) = h.abandoned {
            text.push_str(&format!(" Abandoned in year {year}."));
        }
        // Neither the survey nor a hazard step dates the deposit itself.
        let sealing = match h.burial {
            super::Burial::Alluvium => Some(Cause::Flood),
            super::Burial::Scree => Some(Cause::Rockfall),
            super::Burial::Bedrock => None,
        };
        if let Some(cause) = sealing
            && let Some(e) = self
                .events
                .iter()
                .find(|e| e.site == site && e.cause == cause)
        {
            text.push_str(&format!(
                " First recorded {}: year {}.",
                cause.label(),
                e.year
            ));
            text.push_str(match h.burial {
                super::Burial::Alluvium => " Sediment-rich ground at the surveyed site.",
                _ => " Talus at the surveyed site.",
            });
        } else {
            text.push(' ');
            text.push_str(h.burial.description());
        }
        Some(text)
    }
}
