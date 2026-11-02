//! Background thread that loads or bakes requested globe tiles.

use super::bake::{BakeCursor, TileData, bake_tile};
use super::cache::TileCache;
use super::tile::TileId;
use engine_core::Budget;
use engine_voxel::WorldSampleConfig;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::Duration;

/// Partial bakes kept for resumption; past this the set is dropped and preempted tiles restart.
const MAX_PARTIAL: usize = 32;

pub struct Worker {
    requests: Sender<Vec<TileId>>,
    results: Receiver<TileData>,
    pending: RefCell<VecDeque<TileData>>,
    alive: Arc<AtomicBool>,
}

fn tile_bytes(t: &TileData) -> usize {
    t.height_m.len() * 2 + t.material.len() + t.water.len() + t.climate.len() * 2
}

impl Worker {
    /// Spawns a worker that bakes with `bake_tile`, one budgeted turn per call.
    pub fn spawn(cfg: WorldSampleConfig, cache: TileCache, budget_per_turn: Duration) -> Worker {
        let mut partial: HashMap<TileId, (TileData, BakeCursor)> = HashMap::new();
        let bake = move |cfg: &WorldSampleConfig, id: TileId| {
            if !partial.contains_key(&id) && partial.len() >= MAX_PARTIAL {
                partial.clear();
            }
            let (data, cursor) =
                partial.entry(id).or_insert_with(|| (TileData::empty(id), BakeCursor::default()));
            if bake_tile(cfg, id, data, cursor, Budget::wall(budget_per_turn)) {
                partial.remove(&id).map(|(d, _)| d)
            } else {
                None
            }
        };
        Worker::spawn_with(cfg, bake, cache, budget_per_turn)
    }

    /// Spawns a worker around `bake`, which returns `None` until a tile completes.
    pub fn spawn_with(
        cfg: WorldSampleConfig,
        bake: impl FnMut(&WorldSampleConfig, TileId) -> Option<TileData> + Send + 'static,
        cache: TileCache,
        _budget_per_turn: Duration,
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
        Worker { requests, results, pending: RefCell::new(VecDeque::new()), alive }
    }

    /// Replaces the queue with `ids`, deduplicated in the caller's nearest-first order.
    pub fn request(&self, ids: &[TileId]) {
        let mut seen = HashSet::new();
        let queue: Vec<TileId> = ids.iter().copied().filter(|t| seen.insert(*t)).collect();
        let _ = self.requests.send(queue);
    }

    /// Returns finished tiles up to `max_bytes`; the rest wait for the next call.
    pub fn drain(&self, max_bytes: usize) -> Vec<TileData> {
        let mut pending = self.pending.borrow_mut();
        while let Ok(t) = self.results.try_recv() {
            pending.push_back(t);
        }
        let mut out = Vec::new();
        let mut bytes = 0;
        while let Some(t) = pending.front() {
            let size = tile_bytes(t);
            if bytes + size > max_bytes {
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
        let done = cache.load(id).or_else(|| {
            let baked = bake(cfg, id);
            if let Some(data) = &baked {
                cache.store(data);
            }
            baked
        });
        if let Some(data) = done {
            queue.pop_front();
            if results.send(data).is_err() {
                return;
            }
        }
    }
}
