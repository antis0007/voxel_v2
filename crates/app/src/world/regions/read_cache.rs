//! Byte-bounded LRU of decoded stored chunks shared by every `RegionView` clone.
//!
//! Evicted edited chunks are read voxel by voxel from simulation, meshing and
//! AI threads. Lookups are a hash probe plus an ordered-stamp update, and a
//! chunk being decoded by one thread is waited on rather than decoded again.
use super::{DiskChunk, disk_bytes, edited_budget_mib};
use glam::IVec3;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Condvar, Mutex, MutexGuard};

pub(super) const MAX_ENTRIES: usize = 256;

/// A quarter of the resident edit budget: 16 MiB, about 250 chunks, by default.
pub(super) fn byte_limit() -> usize {
    (edited_budget_mib() * 1024 * 1024 / 4).max(1024 * 1024)
}

struct Entry {
    version: u64,
    stamp: u64,
    bytes: usize,
    data: DiskChunk,
}

#[derive(Default)]
pub(super) struct ReadCache {
    entries: HashMap<IVec3, Entry>,
    order: BTreeMap<u64, IVec3>,
    bytes: usize,
    clock: u64,
    loading: HashSet<IVec3>,
}

impl ReadCache {
    pub(super) fn get(&mut self, coord: IVec3, version: u64) -> Option<DiskChunk> {
        let entry = self.entries.get_mut(&coord).filter(|e| e.version == version)?;
        self.order.remove(&entry.stamp);
        self.clock += 1;
        entry.stamp = self.clock;
        self.order.insert(self.clock, coord);
        Some(entry.data.clone())
    }

    pub(super) fn insert(&mut self, coord: IVec3, version: u64, data: DiskChunk, limit: usize) {
        self.remove(coord);
        let bytes = disk_bytes(&data);
        while self.entries.len() >= MAX_ENTRIES
            || (!self.entries.is_empty() && self.bytes + bytes > limit)
        {
            let Some((_, &oldest)) = self.order.first_key_value() else {
                break;
            };
            self.remove(oldest);
        }
        self.clock += 1;
        self.order.insert(self.clock, coord);
        self.bytes += bytes;
        self.entries.insert(coord, Entry { version, stamp: self.clock, bytes, data });
    }

    fn remove(&mut self, coord: IVec3) {
        if let Some(old) = self.entries.remove(&coord) {
            self.order.remove(&old.stamp);
            self.bytes -= old.bytes;
        }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(super) fn bytes(&self) -> usize {
        self.bytes
    }
}

/// Cache plus the wake-up for threads waiting on another thread's decode.
#[derive(Default)]
pub(super) struct SharedCache {
    cache: Mutex<ReadCache>,
    loaded: Condvar,
}

pub(super) enum Lookup<'a> {
    Hit(DiskChunk),
    /// The caller decodes; dropping the claim wakes waiting readers.
    Claimed(Claim<'a>),
}

pub(super) struct Claim<'a> {
    shared: &'a SharedCache,
    coord: IVec3,
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        self.shared.lock().loading.remove(&self.coord);
        self.shared.loaded.notify_all();
    }
}

impl SharedCache {
    pub(super) fn lock(&self) -> MutexGuard<'_, ReadCache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Return a cached chunk, or claim its decode once no other thread holds it.
    pub(super) fn lookup(&self, coord: IVec3, version: u64) -> Lookup<'_> {
        let mut cache = self.lock();
        loop {
            if let Some(hit) = cache.get(coord, version) {
                return Lookup::Hit(hit);
            }
            if cache.loading.insert(coord) {
                return Lookup::Claimed(Claim { shared: self, coord });
            }
            cache = self.loaded.wait(cache).unwrap_or_else(|e| e.into_inner());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::VoxelScalePreset;
    use crate::world::{World, WorldType};
    use std::sync::Arc;

    fn sample() -> DiskChunk {
        let cfg = World::with_seed(WorldType::Superflat, VoxelScalePreset::OneMeter, 7).sample_config();
        DiskChunk {
            chunk: Arc::new(engine_worldgen::generate(&cfg, IVec3::ZERO)),
            water: Arc::default(),
        }
    }

    #[test]
    fn read_cache_keeps_recent_entries_within_count_and_bytes() {
        let data = sample();
        let mut cache = ReadCache::default();
        for x in 0..300 {
            cache.insert(IVec3::new(x, 0, 0), 1, data.clone(), usize::MAX);
            // Touching the first chunk every step keeps it the most recent.
            assert!(cache.get(IVec3::ZERO, 1).is_some(), "hot entry evicted at {x}");
        }
        assert_eq!(cache.len(), MAX_ENTRIES);
        assert!(cache.get(IVec3::new(1, 0, 0), 1).is_none(), "oldest cold entry must go first");
        assert!(cache.get(IVec3::new(299, 0, 0), 1).is_some());
        assert_eq!(cache.bytes(), MAX_ENTRIES * disk_bytes(&data));

        let limit = disk_bytes(&data) * 10;
        let mut small = ReadCache::default();
        for x in 0..40 {
            small.insert(IVec3::new(x, 0, 0), 1, data.clone(), limit);
        }
        assert_eq!(small.len(), 10);
        assert!(small.bytes() <= limit);
    }

    #[test]
    fn read_cache_replaces_stale_versions() {
        let data = sample();
        let mut cache = ReadCache::default();
        cache.insert(IVec3::ZERO, 1, data.clone(), usize::MAX);
        assert!(cache.get(IVec3::ZERO, 2).is_none(), "a newer record must not hit an old decode");
        cache.insert(IVec3::ZERO, 2, data.clone(), usize::MAX);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.bytes(), disk_bytes(&data));
        assert!(cache.get(IVec3::ZERO, 2).is_some());
    }

    #[test]
    fn concurrent_readers_wait_for_one_decode() {
        let shared = Arc::new(SharedCache::default());
        let claim = match shared.lookup(IVec3::ZERO, 1) {
            Lookup::Claimed(claim) => claim,
            Lookup::Hit(_) => panic!("empty cache cannot hit"),
        };
        let waiter = {
            let shared = Arc::clone(&shared);
            std::thread::spawn(move || matches!(shared.lookup(IVec3::ZERO, 1), Lookup::Hit(_)))
        };
        std::thread::sleep(std::time::Duration::from_millis(20));
        shared.lock().insert(IVec3::ZERO, 1, sample(), usize::MAX);
        drop(claim);
        assert!(waiter.join().unwrap_or(false), "the waiting reader must reuse the decode");
    }
}
