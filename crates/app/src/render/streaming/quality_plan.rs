//! Invalidates optional column plans when any immutable source root changes.
use crate::fast_hash::FastMap;
use crate::world::{ChunkData, WaterSnapshot, WorldSampleConfig, WorldSnapshot};
use glam::IVec3;
use std::{
    path::PathBuf,
    sync::{Arc, Weak},
};

struct SnapshotIdentity {
    cfg: WorldSampleConfig,
    root: Weak<PathBuf>,
    chunks: Weak<FastMap<IVec3, Arc<ChunkData>>>,
    records: crate::world::regions::RegionStorageIdentity,
    water: Weak<WaterSnapshot>,
}

impl SnapshotIdentity {
    fn capture(snapshot: &WorldSnapshot) -> Self {
        Self {
            cfg: snapshot.cfg,
            root: Arc::downgrade(&snapshot.regions.root),
            chunks: Arc::downgrade(&snapshot.chunks),
            records: snapshot.regions.records.storage_identity(),
            water: Arc::downgrade(&snapshot.water_levels),
        }
    }

    fn matches(&self, snapshot: &WorldSnapshot) -> bool {
        self.cfg == snapshot.cfg
            && self.root.as_ptr() == Arc::as_ptr(&snapshot.regions.root)
            && self.chunks.as_ptr() == Arc::as_ptr(&snapshot.chunks)
            && self.water.as_ptr() == Arc::as_ptr(&snapshot.water_levels)
            && snapshot
                .regions
                .records
                .matches_storage_identity(&self.records)
    }
}

#[derive(Default)]
pub(super) struct Inputs {
    snapshot: Option<SnapshotIdentity>,
    revision: u64,
}

impl Inputs {
    pub(super) fn observe(&mut self, snapshot: &WorldSnapshot) -> u64 {
        if self
            .snapshot
            .as_ref()
            .is_none_or(|old| !old.matches(snapshot))
        {
            self.snapshot = Some(SnapshotIdentity::capture(snapshot));
            self.revision = self.revision.wrapping_add(1);
        }
        self.revision
    }
}

#[cfg(test)]
#[path = "quality_plan_tests.rs"]
mod tests;
