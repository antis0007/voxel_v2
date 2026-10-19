//! Authoritative terrain generation shared with collision, picking and edits.
//! Bounded workers avoid a divergent GPU world generator.
use super::{
    CHUNK_GENERATION_COLLECT_BUDGET, ChunkKey, Gpu, MAX_GENERATING, apply_snapshot_edits_cubic,
};
use crate::mesh::GeneratedChunk;
use crate::world::{WorldSampleConfig, WorldSnapshot};
use glam::IVec3;
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct Job {
    id: u64,
    key: ChunkKey,
    cfg: WorldSampleConfig,
    cancelled: Arc<AtomicBool>,
}
struct Pending {
    key: ChunkKey,
    revision: u64,
    cancelled: Arc<AtomicBool>,
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
    next_id: u64,
}
impl TerrainGenerator {
    pub(super) fn new(_device: &wgpu::Device) -> Self {
        let (sender, jobs) = mpsc::sync_channel::<Job>(MAX_GENERATING);
        let (completed, receiver) = mpsc::sync_channel(MAX_GENERATING);
        let jobs = Arc::new(Mutex::new(jobs));
        // Leave cores for rendering, meshing and simulation, but don't throttle
        // a many-core machine to four generators while thousands wait. The
        // existing byte reservation still bounds running and completed arrays.
        let count = std::thread::available_parallelism()
            .map_or(2, |n| n.get().saturating_sub(4).clamp(1, 8))
            .min((reservation_bytes() / job_bytes()).max(1));
        println!("[stream-workers] terrain={count}");
        for index in 0..count {
            let jobs = jobs.clone();
            let completed = completed.clone();
            std::thread::Builder::new()
                .name(format!("landscape-{index}"))
                .spawn(move || {
                    loop {
                        let job = jobs.lock().unwrap_or_else(|e| e.into_inner()).recv();
                        let Ok(job) = job else {
                            break;
                        };
                        let chunk = std::panic::catch_unwind(|| {
                            crate::world::generate_padded(&job.cfg, job.key.coord, &job.cancelled)
                        })
                        .ok()
                        .flatten();
                        let activity = chunk.as_ref().and_then(crate::simulation::discover);
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
            next_id: 0,
        }
    }
    pub(super) fn available_slots(&self) -> usize {
        (reservation_bytes() / job_bytes())
            .min(MAX_GENERATING)
            .saturating_sub(self.pending.len())
    }
    pub(super) fn buffer_bytes(&self) -> usize {
        self.pending.len() * job_bytes()
    }
    pub(super) fn reset(&mut self) {
        for job in self.pending.values() {
            job.cancelled.store(true, Ordering::Relaxed);
        }
    }
    pub(super) fn retain_visible(&mut self, visible: &HashSet<ChunkKey>) {
        let coords: HashSet<_> = visible.iter().map(|key| key.coord).collect();
        for job in self.pending.values() {
            if !coords.contains(&job.key.coord) {
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
    pub(super) fn schedule_many<I: IntoIterator<Item = ChunkKey>>(
        &mut self,
        _gpu: &Gpu,
        keys: I,
        cfg: WorldSampleConfig,
        revision: u64,
    ) {
        for key in keys {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1);
            let cancelled = Arc::new(AtomicBool::new(false));
            self.sender
                .try_send(Job {
                    id,
                    key,
                    cfg,
                    cancelled: cancelled.clone(),
                })
                .expect("terrain admission exceeded worker capacity");
            self.pending.insert(
                id,
                Pending {
                    key,
                    revision,
                    cancelled,
                },
            );
        }
    }
    pub(super) fn collect_finished(
        &mut self,
        _device: &wgpu::Device,
        snapshot: &WorldSnapshot,
    ) -> Vec<(
        ChunkKey,
        u64,
        Option<GeneratedChunk>,
        Option<Box<crate::world::activity::Mask>>,
    )> {
        let start = Instant::now();
        let mut output = Vec::new();
        for _ in 0..CHUNK_GENERATION_COLLECT_BUDGET {
            if start.elapsed() >= Duration::from_millis(2) {
                break;
            }
            let Ok((id, chunk, activity)) = self.receiver.try_recv() else {
                break;
            };
            let Some(job) = self.pending.remove(&id) else {
                continue;
            };
            if job.cancelled.load(Ordering::Relaxed) {
                continue;
            }
            let chunk = chunk
                .filter(|_| snapshot.edits_ready(job.key.coord))
                .map(|mut chunk| {
                    apply_snapshot_edits_cubic(&mut chunk, snapshot);
                    chunk
                });
            output.push((job.key, job.revision, chunk, activity));
        }
        output
    }
}
impl Drop for TerrainGenerator {
    fn drop(&mut self) {
        self.reset();
    }
}
