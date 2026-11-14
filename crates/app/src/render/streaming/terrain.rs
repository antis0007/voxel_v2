//! Authoritative terrain generation shared with collision, picking and edits.
//! Bounded workers avoid a divergent GPU world generator.
use super::{
    ChunkKey, Gpu, MAX_GENERATING, apply_snapshot_edits_cubic,
};
use crate::mesh::GeneratedChunk;
use crate::world::{WorldSampleConfig, WorldSnapshot};
use glam::IVec3;
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};

struct Job {
    id: u64,
    key: ChunkKey,
    cfg: WorldSampleConfig,
    cancelled: Arc<AtomicBool>,
    seed_activity: bool,
    source: Option<Arc<super::ResidentChunk>>,
}
struct Pending {
    key: ChunkKey,
    cancelled: Arc<AtomicBool>,
    seed_activity: bool,
    seed_only: bool,
    cached_seed_source: bool,
}
// Pending jobs include running arrays and completed results awaiting collection.
// Reserve this window from the CPU source budget before admission, rather than
// incorrectly counting CPU terrain workers as GPU residency (or zero bytes).
fn job_bytes() -> usize {
    (crate::mesh::PADDED_CHUNK_SIZE as usize).pow(3) * std::mem::size_of::<crate::world::BlockId>()
        + (crate::mesh::PADDED_CHUNK_SIZE as usize).pow(2) * std::mem::size_of::<i32>()
        + std::mem::size_of::<crate::world::activity::Mask>()
        + 1024
}
// Keep the FIFO shallow so newly urgent chunks can enter in distance order.
const QUEUED_PER_WORKER: usize = 2;
fn admission_room(window: usize, pending: usize, queued: usize, workers: usize) -> usize {
    window.saturating_sub(pending)
        .min((workers * QUEUED_PER_WORKER).saturating_sub(queued))
}
pub(super) fn reservation_bytes() -> usize {
    (super::residency::limits().0 / 4).min(MAX_GENERATING * job_bytes())
}
pub(super) struct TerrainGenerator {
    sender: mpsc::SyncSender<Job>,
    receiver: mpsc::Receiver<(
        u64,
        Option<GeneratedChunk>,
        Option<Box<crate::world::activity::Mask>>,
    )>,
    pending: HashMap<u64, Pending>,
    seed_jobs: HashMap<IVec3, u64>,
    next_id: u64,
    queued: Arc<AtomicUsize>,
    workers: usize,
}
impl TerrainGenerator {
    pub(super) fn new(_device: &wgpu::Device, mesh_workers: usize) -> Self {
        let (sender, jobs) = mpsc::sync_channel::<Job>(MAX_GENERATING);
        let (completed, receiver) = mpsc::sync_channel(MAX_GENERATING);
        let jobs = Arc::new(Mutex::new(jobs));
        let queued = Arc::new(AtomicUsize::new(0));
        // Account for the mesh pool and leave headroom for the frame thread
        // and simulation. The byte reservation also bounds queued results.
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get());
        let automatic = threads.saturating_sub(mesh_workers + (threads / 6).max(2)).max(1);
        let count = std::env::var("VOXEL_TERRAIN_WORKERS").ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|&count| count > 0)
            .unwrap_or(automatic)
            .min(threads.min(16))
            .min((reservation_bytes() / job_bytes()).max(1));
        println!("[stream-workers] terrain={count} mesh={mesh_workers} logical={threads}");
        for index in 0..count {
            let jobs = jobs.clone();
            let completed = completed.clone();
            let queued = queued.clone();
            std::thread::Builder::new()
                .name(format!("landscape-{index}"))
                .spawn(move || {
                    loop {
                        let job = jobs.lock().unwrap_or_else(|e| e.into_inner()).recv();
                        let Ok(job) = job else {
                            break;
                        };
                        queued.fetch_sub(1, Ordering::Relaxed);
                        let chunk = if job.source.is_some() { None } else { std::panic::catch_unwind(|| {
                            crate::world::generate_padded(&job.cfg, job.key.coord, &job.cancelled)
                        })
                        .ok()
                        .flatten() };
                        let activity = if job.seed_activity && !job.cancelled.load(Ordering::Relaxed) {
                            job.source.as_ref().map(|source| &***source)
                                .or(chunk.as_ref()).and_then(crate::simulation::discover)
                        } else { None };
                        if completed.send((job.id, chunk, activity)).is_err() {
                            break;
                        }
                    }
                })
                .expect("cannot start landscape worker");
        }
        Self {
            sender,
            receiver,
            pending: HashMap::new(),
            seed_jobs: HashMap::new(),
            next_id: 0,
            queued,
            workers: count,
        }
    }
    pub(super) fn available_slots(&self) -> usize {
        admission_room(
            (reservation_bytes() / job_bytes()).min(MAX_GENERATING),
            self.pending.len(),
            self.queued.load(Ordering::Relaxed),
            self.workers,
        )
    }
    pub(super) fn buffer_bytes(&self) -> usize {
        self.pending.len() * job_bytes()
    }
    pub(super) fn reset(&mut self) {
        self.seed_jobs.clear();
        for job in self.pending.values() {
            job.cancelled.store(true, Ordering::Relaxed);
        }
    }
    pub(super) fn retain_coords(&mut self, keep: impl Fn(&IVec3) -> bool) {
        for job in self.pending.values() {
            if !keep(&job.key.coord) {
                job.cancelled.store(true, Ordering::Relaxed);
            }
        }
    }
    pub(super) fn invalidate_coords(&mut self, dirty: &HashSet<IVec3>) {
        for job in self.pending.values() {
            if dirty.contains(&job.key.coord) {
                job.cancelled.store(true, Ordering::Relaxed);
            }
        }
    }
    pub(super) fn schedule_many<I: IntoIterator<Item = (ChunkKey, bool)>>(
        &mut self,
        _gpu: &Gpu,
        keys: I,
        cfg: WorldSampleConfig,
    ) {
        for (key, seed_activity) in keys {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1);
            let cancelled = Arc::new(AtomicBool::new(false));
            self.queued.fetch_add(1, Ordering::Relaxed);
            self.sender
                .try_send(Job {
                    id,
                    key,
                    cfg,
                    cancelled: cancelled.clone(),
                    seed_activity, source: None,
                })
                .unwrap_or_else(|_| panic!("terrain admission exceeded worker capacity"));
            self.pending.insert(
                id,
                Pending {
                    key,
                    cancelled,
                    seed_activity, seed_only: false, cached_seed_source: false,
                },
            );
        }
    }
    pub(super) fn seed_pending(&self, coord: IVec3) -> bool {
        self.seed_jobs.get(&coord).and_then(|id| self.pending.get(id))
            .is_some_and(|job| !job.cancelled.load(Ordering::Relaxed))
    }
    pub(super) fn schedule_seed(&mut self, key: ChunkKey, cfg: WorldSampleConfig, source: Option<Arc<super::ResidentChunk>>) {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let cached_seed_source = source.is_some();
        self.queued.fetch_add(1, Ordering::Relaxed);
        self.sender.try_send(Job { id, key, cfg, cancelled: cancelled.clone(), seed_activity: true, source })
            .unwrap_or_else(|_| panic!("seed admission exceeded terrain capacity"));
        self.pending.insert(id, Pending { key, cancelled, seed_activity: true, seed_only: true, cached_seed_source });
        self.seed_jobs.insert(key.coord, id);
    }
    pub(super) fn collect_one(
        &mut self,
        snapshot: &WorldSnapshot,
        interested: impl Fn(IVec3) -> bool,
    ) -> Result<Option<(
        ChunkKey,
        Option<GeneratedChunk>,
        Option<Box<crate::world::activity::Mask>>,
        bool,
        bool,
    )>, mpsc::TryRecvError> {
        let (id, chunk, activity) = self.receiver.try_recv()?;
        let Some(job) = self.pending.remove(&id) else {
            return Ok(None);
        };
        // An old cancelled job may complete after a reset or a replacement.
        if self.seed_jobs.get(&job.key.coord) == Some(&id) {
            self.seed_jobs.remove(&job.key.coord);
        }
        if job.cancelled.load(Ordering::Relaxed) {
            return Ok(None);
        }
        if job.seed_only {
            return Ok((chunk.is_some() || job.cached_seed_source)
                .then_some((job.key, None, activity, true, job.seed_activity)));
        }
        let chunk = chunk
            .filter(|_| interested(job.key.coord) && snapshot.edits_ready(job.key.coord))
            .map(|mut chunk| {
                apply_snapshot_edits_cubic(&mut chunk, snapshot);
                chunk
            });
        Ok(Some((job.key, chunk, activity, false, job.seed_activity)))
    }
}
impl Drop for TerrainGenerator {
    fn drop(&mut self) {
        self.reset();
    }
}

#[cfg(test)]
mod admission_tests {
    use super::admission_room;

    #[test]
    fn running_jobs_do_not_allow_a_deep_fifo() {
        // Eight workers are busy, with their two queued jobs each already full.
        assert_eq!(admission_room(64, 24, 16, 8), 0);
        // A worker starts one queued job: admit exactly one replacement.
        assert_eq!(admission_room(64, 24, 15, 8), 1);
    }

    #[test]
    fn uncollected_results_still_consume_the_memory_window() {
        assert_eq!(admission_room(16, 16, 0, 8), 0);
        assert_eq!(admission_room(16, 15, 0, 8), 1);
        assert_eq!(admission_room(16, 20, 20, 8), 0);
    }
}
