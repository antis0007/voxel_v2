//! Background thread that loads or bakes requested globe tiles.

use super::bake::{BakeCursor, TileData, bake_tile};
use super::cache::TileCache;
use super::tile::TileId;
use engine_core::Budget;
use engine_voxel::WorldSampleConfig;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;

/// Partial bakes kept for resumption; evict one least-complete tile at the cap.
const MAX_PARTIAL: usize = 32;
/// A real tile needs about 200 turns; a bake past this is stuck and would starve the queue.
const MAX_TURNS: u32 = 4_096;

pub struct Worker {
    requests: Sender<Vec<TileId>>,
    results: Receiver<TileData>,
    pending: RefCell<VecDeque<TileData>>,
    alive: Arc<AtomicBool>,
    bake_nanos: Arc<AtomicU64>,
}

pub(crate) fn tile_bytes(t: &TileData) -> usize {
    t.height_m.len() * 2 + t.material.len() + t.water.len() + t.climate.len() * 4 + t.normals.len() * 2
}

impl Worker {
    /// Spawns a worker that bakes with `bake_tile`, one budgeted turn per call.
    pub fn spawn(cfg: WorldSampleConfig, cache: TileCache, budget_per_turn: Duration) -> Worker {
        let mut partial: HashMap<TileId, (TileData, BakeCursor)> = HashMap::new();
        let bake_nanos = Arc::new(AtomicU64::new(0));
        let counter = bake_nanos.clone();
        let bake = move |cfg: &WorldSampleConfig, id: TileId| {
            if !partial.contains_key(&id) && partial.len() >= MAX_PARTIAL {
                // Camera turns must not throw away every nearly finished tile.
                if let Some(victim) = partial.iter().min_by_key(|(key, (_, cursor))| (cursor.next_sample, **key)).map(|(key, _)| *key) {
                    partial.remove(&victim);
                }
            }
            let (data, cursor) =
                partial.entry(id).or_insert_with(|| (TileData::empty(id), BakeCursor::default()));
            // Wall time on the worker thread only; it feeds the bake_ms stat, never a decision.
            let start = std::time::Instant::now();
            let done = bake_tile(cfg, id, data, cursor, Budget::wall(budget_per_turn));
            counter.fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
            if done { partial.remove(&id).map(|(d, _)| d) } else { None }
        };
        let mut worker = Worker::spawn_with(cfg, bake, cache);
        worker.bake_nanos = bake_nanos;
        worker
    }

    /// Returns and clears the bake wall time accumulated since the last call.
    pub fn take_bake_nanos(&self) -> u64 {
        self.bake_nanos.swap(0, Ordering::Relaxed)
    }

    /// Spawns a worker around `bake`, which returns `None` until a tile completes.
    pub fn spawn_with(
        cfg: WorldSampleConfig,
        bake: impl FnMut(&WorldSampleConfig, TileId) -> Option<TileData> + Send + 'static,
        cache: TileCache,
    ) -> Worker {
        let (requests, request_rx) = channel::<Vec<TileId>>();
        let (result_tx, results) = channel();
        let alive = Arc::new(AtomicBool::new(true));
        let flag = alive.clone();
        let spawned = std::thread::Builder::new().name("globe-bake".into()).spawn(move || {
            let mut bake = bake;
            // A panicking bake must not take the game down; the globe just stops refining.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run(&cfg, &mut bake, &cache, &request_rx, &result_tx)
            }));
            flag.store(false, Ordering::Release);
        });
        if spawned.is_err() {
            alive.store(false, Ordering::Release);
        }
        Worker { requests, results, pending: RefCell::new(VecDeque::new()), alive, bake_nanos: Arc::new(AtomicU64::new(0)) }
    }

    /// Replaces the queue with `ids`, deduplicated in the caller's nearest-first order.
    pub fn request(&self, ids: &[TileId]) {
        let mut seen = HashSet::new();
        let queue: Vec<TileId> = ids.iter().copied().filter(|t| seen.insert(*t)).collect();
        let _ = self.requests.send(queue);
    }

    /// Returns finished tiles up to `max_bytes`, at least one if any are ready.
    pub fn drain(&self, max_bytes: usize) -> Vec<TileData> {
        let mut pending = self.pending.borrow_mut();
        while let Ok(t) = self.results.try_recv() {
            pending.push_back(t);
        }
        let mut out = Vec::new();
        let mut bytes = 0;
        while let Some(t) = pending.front() {
            let size = tile_bytes(t);
            // Always pass one tile so a cap below a tile's size cannot stall delivery.
            if !out.is_empty() && bytes + size > max_bytes {
                break;
            }
            bytes += size;
            out.extend(pending.pop_front());
        }
        out
    }

    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
}

fn run(
    cfg: &WorldSampleConfig,
    bake: &mut dyn FnMut(&WorldSampleConfig, TileId) -> Option<TileData>,
    cache: &TileCache,
    requests: &Receiver<Vec<TileId>>,
    results: &Sender<TileData>,
) {
    let mut queue: VecDeque<TileId> = VecDeque::new();
    // Front tile already known to miss the cache, with its turn count.
    let mut current: Option<(TileId, u32)> = None;
    loop {
        // Newest selection wins; checking between turns lets it preempt a half-baked tile.
        loop {
            match requests.try_recv() {
                Ok(q) => queue = q.into(),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        let Some(&id) = queue.front() else {
            match requests.recv() {
                Ok(q) => queue = q.into(),
                Err(_) => return,
            }
            continue;
        };
        let turns = match current {
            Some((cur, turns)) if cur == id => turns,
            _ => {
                if let Some(data) = cache.load(id) {
                    queue.pop_front();
                    if results.send(data).is_err() {
                        return;
                    }
                    continue;
                }
                0
            }
        };
        if turns >= MAX_TURNS {
            eprintln!("[globe] bake gave up on {id:?}");
            queue.pop_front();
            current = None;
            continue;
        }
        current = Some((id, turns + 1));
        if let Some(data) = bake(cfg, id) {
            cache.store(&data);
            queue.pop_front();
            current = None;
            if results.send(data).is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
