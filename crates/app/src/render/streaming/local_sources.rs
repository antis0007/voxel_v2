//! Local source changes invalidate actual request ownership, including at equal
//! simulation revisions. Pure procedural terrain is overlaid only at collection.
use super::*;
use crate::world::{ChunkData, WorldSampleConfig};
use std::{path::PathBuf, sync::Weak};

struct Dependency {
    coord: IVec3,
    chunk: Option<Weak<ChunkData>>,
    record: Option<(u64, u64, u32, u64, u64)>,
}
impl Dependency {
    fn capture(s: &WorldSnapshot, coord: IVec3) -> Self {
        Self {
            coord,
            chunk: s.chunks.get(&coord).map(Arc::downgrade),
            record: s
                .regions
                .records
                .get(&coord)
                .map(|r| (r.generation, r.offset, r.len, r.checksum, r.version)),
        }
    }
    fn matches(&self, s: &WorldSnapshot, chunks_changed: bool, records_changed: bool) -> bool {
        let same = !chunks_changed
            || match (&self.chunk, s.chunks.get(&self.coord)) {
                (Some(a), Some(b)) => Weak::ptr_eq(a, &Arc::downgrade(b)),
                (None, None) => true,
                _ => false,
            };
        same && (!records_changed
            || self.record
                == s.regions
                    .records
                    .get(&self.coord)
                    .map(|r| (r.generation, r.offset, r.len, r.checksum, r.version)))
    }
}
fn refresh_dependencies(deps: &mut Vec<Dependency>, s: &WorldSnapshot, q: IVec3) {
    deps.clear();
    deps.reserve(27);
    for y in -1..=1 {
        for z in -1..=1 {
            for x in -1..=1 {
                deps.push(Dependency::capture(s, q + IVec3::new(x, y, z)));
            }
        }
    }
}
#[derive(Default)]
pub(super) struct Observations {
    cfg: Option<WorldSampleConfig>,
    root: Option<Arc<PathBuf>>,
    sources: HashMap<IVec3, Vec<Dependency>>,
    // Reuse policy-tick scratch storage within the existing observation charge.
    tracked: HashSet<IVec3>,
    changed_water: HashSet<IVec3>,
    water: Option<Arc<crate::world::WaterSnapshot>>,
    chunks: Option<Weak<crate::fast_hash::FastMap<IVec3, Arc<ChunkData>>>>,
    records: Option<crate::world::regions::RegionStorageIdentity>,
    allocations: Option<Arc<VoxelAllocations>>,
    charged: usize,
    pub(super) blocked: bool,
}
impl Drop for Observations {
    fn drop(&mut self) {
        if let Some(a) = &self.allocations {
            a.0.fetch_sub(self.charged, Ordering::Relaxed);
        }
    }
}
impl Observations {
    fn changed(&mut self, s: &WorldSnapshot, cache: &ChunkCache) -> HashSet<IVec3> {
        // Ordinary streaming has no exact-source observations to invalidate.
        // Enrollment captures fresh dependencies when a proxy first appears.
        if self.sources.is_empty() && cache.edit_proxies.coords.is_empty() {
            return HashSet::new();
        }
        let root_changed = self
            .root
            .as_ref()
            .is_some_and(|r| !Arc::ptr_eq(r, &s.regions.root));
        let cfg_changed = self.cfg.is_some_and(|c| c != s.cfg);
        let chunks_changed = self
            .chunks
            .as_ref()
            .is_none_or(|old| !Weak::ptr_eq(old, &Arc::downgrade(&s.chunks)));
        let records_changed = self
            .records
            .as_ref()
            .is_none_or(|old| !s.regions.records.matches_storage_identity(old));
        if chunks_changed {
            self.chunks = Some(Arc::downgrade(&s.chunks));
        }
        if records_changed {
            self.records = Some(s.regions.records.storage_identity());
        }
        self.changed_water.clear();
        if let Some(old) = self
            .water
            .as_ref()
            .filter(|old| !Arc::ptr_eq(old, &s.water_levels))
        {
            self.changed_water.extend(
                s.water_levels
                    .changed_chunks(old)
                    .map(|(q, _, _)| q)
                    .take(4097),
            );
        }
        let broad_water_change = self.changed_water.len() > 4096;
        self.cfg = Some(s.cfg);
        self.root = Some(s.regions.root.clone());
        self.water = Some(s.water_levels.clone());
        // Promotion keeps combined exact-source membership. Retain observations
        // for an ordinary owner after optional quality interest has retired.
        self.sources.retain(|q, _| cache.source_interested(q));
        self.tracked.clear();
        self.tracked.extend(self.sources.keys().copied());
        for q in &cache.edit_proxies.coords {
            for y in -1..=1 {
                for z in -1..=1 {
                    for x in -1..=1 {
                        let target = *q + IVec3::new(x, y, z);
                        if cache.source_interested(&target) {
                            self.tracked.insert(target);
                        }
                    }
                }
            }
        }
        // Conservative fixed reservation includes HashMap slack, weak record
        // stamps, vectors and bounded temporary sets. It is charged to the same
        // source allocation ledger as worker-held voxels, not invisible memory.
        const LIMIT: usize = 4096;
        const RESERVATION: usize = 16 * 1024 * 1024;
        let was_blocked = self.blocked;
        self.blocked = self.tracked.len() > LIMIT;
        if self.allocations.is_none() && !self.tracked.is_empty() && !self.blocked {
            if cache.cpu_bytes().saturating_add(RESERVATION) > residency::limits().0 {
                self.blocked = true;
            } else {
                cache
                    .voxel_allocations
                    .0
                    .fetch_add(RESERVATION, Ordering::Relaxed);
                self.charged = RESERVATION;
                self.allocations = Some(cache.voxel_allocations.clone());
            }
        }
        if self.blocked {
            // Fail closed: invalidate old optional/persistent results, defer new
            // exact-source admissions until bounded observation is available.
            // An over-limit scratch set must not remain as uncharged storage.
            self.changed_water = HashSet::new();
            return std::mem::take(&mut self.tracked);
        }
        let mut changed = HashSet::new();
        for &q in &self.tracked {
            let stale = was_blocked
                || broad_water_change
                || root_changed
                || cfg_changed
                || self.sources.get(&q).is_none_or(|deps| {
                    (chunks_changed || records_changed || !self.changed_water.is_empty())
                        && deps.iter().any(|d| {
                            !d.matches(s, chunks_changed, records_changed)
                                || self.changed_water.contains(&d.coord)
                        })
                });
            if stale {
                changed.insert(q);
                refresh_dependencies(self.sources.entry(q).or_default(), s, q);
            }
        }
        changed
    }
}
impl ChunkStreamer {
    /// Must run on the current snapshot before any source/mesh admission or GPU
    /// completion polling. No revision arithmetic and no global epoch changes.
    pub fn invalidate_changed_sources(
        &mut self,
        s: &WorldSnapshot,
        cache: &mut ChunkCache,
    ) -> HashSet<IVec3> {
        let changed = self.source_observations.changed(s, cache);
        if changed.is_empty() {
            return changed;
        }
        // Drop destination handles/leases and release ONLY owned upload tokens
        // before cancellation clears per-coordinate scheduling markers.
        self.cancel_pending_sources(&changed);
        self.cancel_coords(&changed);
        cache.cpu_chunks.retain(|q, _| !changed.contains(q));
        cache.lod_meshes.retain(|q| !changed.contains(&q));
        cache
            .budget_deferred
            .retain(|k, _| !changed.contains(&k.coord));
        cache.dirty_meshes.extend(changed.iter().copied());
        // Keep the last validated GPU cover while fresh sources are built.
        // Existing mesh-result job-ID comparison rejects cancelled worker output.
        for &coord in &changed {
            if cache.source_interested(&coord) {
                self.push_pending_key(ChunkKey {
                    coord,
                    lod: cache.desired_lod(coord),
                });
            }
        }
        cache.publication_dirty = true;
        changed
    }
}
