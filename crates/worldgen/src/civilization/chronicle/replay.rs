use super::{Chronicle, MODEL_REVISION, STEP_YEARS, YEARS, simulation};
use crate::civilization::{CHRONICLE_GENERATOR, Site, WorldSampleConfig, encode};
use glam::{IVec2, Vec2};

impl Chronicle {
    /// Replays recorded inputs without sampling terrain or mutating live settlements.
    pub fn replay(&self) -> Result<Self, &'static str> {
        let p = self.provenance;
        if p.model_revision != MODEL_REVISION
            || p.generator < CHRONICLE_GENERATOR
            || p.end_year != YEARS
            || p.step_years != STEP_YEARS
            || !(-64..960).contains(&p.region[0])
            || !(-64..64).contains(&p.region[1])
        {
            return Err("Unsupported history recipe");
        }
        let key = IVec2::from_array(p.region);
        for (i, s) in self.sites.iter().enumerate() {
            if let Some(s) = s
                && (s.id != encode(key, i, 0)
                    || s.history.founded == 0
                    || s.history.founded > YEARS
                    || !s.history.founded.is_multiple_of(STEP_YEARS)
                    || s.landscape.flood_risk_ppm > 1_000_000
                    || !(0..=7).contains(&s.shelter))
            {
                return Err("Invalid history inputs");
            }
        }
        let mut sites = self.sites.map(|s| {
            s.map(|s| {
                let mut history = s.history;
                history.peak_year = history.founded;
                history.peak_households = 0;
                history.households = 0;
                history.abandoned = None;
                history.flood_year = None;
                history.rockfall_year = None;
                history.refugees_received = 0;
                history.neighbor = None;
                history.burial = s.burial_candidate;
                Site {
                    center: Vec2::new(s.center_mm[0] as f32, s.center_mm[2] as f32) / 1000.0,
                    datum: s.center_mm[1] as f32 / 1000.0,
                    history,
                    shelter: s.shelter,
                    flood_risk: s.landscape.flood_risk_ppm as f32 / 1_000_000.0,
                    landscape: s.landscape,
                    burial_candidate: s.burial_candidate,
                }
            })
        });
        let cfg = WorldSampleConfig {
            seed: p.seed,
            generator_version: p.generator,
            world_type: crate::WorldType::Planet,
            voxel_scale: engine_core::VoxelScalePreset::OneMeter,
            planet: None,
        };
        let events = simulation::run(&cfg, key, &mut sites, self.linked);
        Ok(Self {
            provenance: p,
            linked: self.linked,
            sites: Self::records(key, &sites),
            events,
        })
    }
}
