//! Isolated history audit, without regional refinement or persistent caches.
use crate::{WorldSampleConfig, geo::Progress, planetary::DriftFrame};
use std::sync::Arc;

/// Run the production plate and surface-history solver on its physical mesh.
pub fn frames(cfg: &WorldSampleConfig) -> Vec<Arc<DriftFrame>> {
    let progress = Progress::default();
    let mut plates = super::super::tectonics::Plates::new(cfg);
    plates.evolve(&progress);
    progress.drift_frames()
}
