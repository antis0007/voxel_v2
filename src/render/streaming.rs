//! Chunk residency, prioritized terrain generation, bounded meshing, and GPU upload.
//! Job IDs isolate cancelled work from replacements, including across world resets.
mod lod_cache;
mod mesh_budget;
pub use mesh_budget::{mesh_budget_mib, set_mesh_budget_mib};
mod residency;
mod gpu_budget;
pub use gpu_budget::{gpu_residency_limit_mib, gpu_residency_status, MAX_GPU_MIB};
pub(super) use gpu_budget::configure_gpu_residency;
use mesh_budget::{MeshBudget, MeshLease};
use residency::{ResidentChunk, VoxelAllocations};
pub use residency::{set_world_residency_budgets_mib, world_residency_budgets_mib};
mod columns;
mod gpu_upload;
mod terrain;
mod visibility;
use super::{ChunkStreamerStats, Gpu, render_distance_settings};
use columns::ColumnPlans;
use lod_cache::LodMeshCache;
use terrain::TerrainGenerator;
pub use visibility::{build_draw_list, chunk_bounds_sphere, extract_planes};
use visibility::{
    chunk_sort_key, desired_chunk_keys, desired_chunk_keys_ring,
    desired_chunk_keys_with_snapshot_cached, lod_for_chunk, pending_priority,
};

use glam::IVec3;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use crate::mesh::{ChunkMesh, GeneratedChunk, MeshPlan, PADDED_CHUNK_SIZE, PLANNING_BYTES};
use crate::world::{CHUNK_SIZE, WorldSnapshot};

const CHUNK_SCHEDULE_BUDGET: usize = 16;
const CHUNK_MESH_DISPATCH_BUDGET: usize = 16;
const CHUNK_GENERATION_COLLECT_BUDGET: usize = 16;
const CHUNK_UPLOAD_VERTEX_BUDGET: usize = 350_000;
const CHUNK_UPLOAD_INDEX_BUDGET: usize = 1_200_000;
const MAX_GENERATING: usize = 64;
const MAX_MESH_JOBS: usize = 64;
const MAX_READY_CHUNKS: usize = 128;
const MAX_READY_MESHES: usize = 64;
const UPLOAD_TIME_BUDGET: Duration = Duration::from_millis(3);
pub struct ChunkGpuMesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
    pub lod: u8,
    lod_invariant: bool,
    bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkKey {
    pub coord: IVec3,
    pub lod: u8,
}

pub struct ChunkCache {
    voxel_allocations: Arc<VoxelAllocations>,
    residency_limits: (usize, usize),
    budget_center: IVec3,
    budget_deferred: HashMap<ChunkKey, usize>,
    cpu_evictions: usize,
    gpu_evictions: usize,
    lod_meshes: LodMeshCache,
    gpu_bytes: usize,
    pub visible: Vec<ChunkKey>,
    pub visible_keys: HashSet<ChunkKey>,
    pub visible_coords: HashSet<IVec3>,
    pub gpu_meshes: HashMap<IVec3, ChunkGpuMesh>,
    /// Increments whenever the GPU mesh set changes so render passes can cache
    /// derived work, such as shadow maps, without guessing.
    pub gpu_mesh_revision: u64,
    dirty_meshes: HashSet<IVec3>,
    pub cpu_chunks: HashMap<IVec3, Arc<ResidentChunk>>,
    pub center_chunk: IVec3,
    /// Cached column Y-ranges to avoid repeated terrain sampling.
    /// Keyed by (cx, cz). Invalidated when world config changes.
    column_y_cache: ColumnPlans,
    simulation_seeds: Vec<(IVec3, Box<crate::world::activity::Mask>)>,
    /// How far we've expanded the visible set from center (in chunks).
    /// Starts at 0 on reset, grows each frame until it reaches view distance.
    expansion_radius: i32,
    /// Whether expansion is complete (reached view distance).
    pub fully_expanded: bool,
    pub spawn_ready: bool,
}

impl ChunkCache {
    pub fn new(center_chunk: IVec3) -> Self {
        let visible = desired_chunk_keys(center_chunk);
        let visible_keys = visible.iter().copied().collect();
        let visible_coords = visible.iter().map(|key| key.coord).collect();

        Self {
            voxel_allocations: Arc::default(),
            residency_limits: residency::limits(),
            budget_center: center_chunk,
            budget_deferred: HashMap::new(),
            cpu_evictions: 0,
            gpu_evictions: 0,
            lod_meshes: LodMeshCache::default(),
            gpu_bytes: 0,
            visible,
            visible_keys,
            visible_coords,
            gpu_meshes: HashMap::new(),
            gpu_mesh_revision: 0,
            dirty_meshes: HashSet::new(),
            cpu_chunks: HashMap::new(),
            center_chunk,
            column_y_cache: ColumnPlans::new(),
            simulation_seeds: Vec::new(),
            expansion_radius: 0,
            fully_expanded: false,
            spawn_ready: false,
        }
    }

    pub fn new_with_snapshot(center_chunk: IVec3, snapshot: &WorldSnapshot) -> Self {
        // Start with small local set — expand incrementally each frame.
        let mut cache = Self {
            voxel_allocations: Arc::default(),
            residency_limits: residency::limits(),
            budget_center: center_chunk,
            budget_deferred: HashMap::new(),
            cpu_evictions: 0,
            gpu_evictions: 0,
            lod_meshes: LodMeshCache::default(),
            gpu_bytes: 0,
            visible: Vec::new(),
            visible_keys: HashSet::new(),
            visible_coords: HashSet::new(),
            gpu_meshes: HashMap::new(),
            gpu_mesh_revision: 0,
            dirty_meshes: HashSet::new(),
            cpu_chunks: HashMap::new(),
            center_chunk,
            column_y_cache: ColumnPlans::new(),
            simulation_seeds: Vec::new(),
            expansion_radius: 2,
            fully_expanded: false,
            spawn_ready: false,
        };
        cache.visible = desired_chunk_keys_with_snapshot_cached(
            center_chunk,
            snapshot,
            &mut cache.column_y_cache,
            cache.expansion_radius,
        );
        cache.rebuild_visible_sets();
        cache
    }

    fn rebuild_visible_sets(&mut self) {
        for index in 0..self.visible.len() {
            self.visible[index].lod = self.desired_lod(self.visible[index].coord);
        }
        self.visible_keys = self.visible.iter().copied().collect();
        self.visible_coords = self.visible.iter().map(|key| key.coord).collect();
    }

    fn desired_lod(&self, coord: IVec3) -> u8 {
        let desired = lod_for_chunk(self.center_chunk, coord);
        let Some(mesh) = self.gpu_meshes.get(&coord) else {
            return desired;
        };
        if mesh.lod >= render_distance_settings().lod_levels {
            return desired;
        }
        let delta = coord - self.center_chunk;
        let step = IVec3::new(delta.x.signum(), 0, delta.z.signum());
        // Keep the resident level inside a one-chunk dead band on either side.
        let near = lod_for_chunk(self.center_chunk, coord - step);
        let far = lod_for_chunk(self.center_chunk, coord + step);
        if mesh.lod >= near && mesh.lod <= far {
            mesh.lod
        } else {
            desired
        }
    }

    pub fn rebuild_visible(&mut self) {
        self.visible = desired_chunk_keys(self.center_chunk);
        self.rebuild_visible_sets();
    }

    pub fn rebuild_visible_with_snapshot(&mut self, snapshot: &WorldSnapshot) {
        let target = render_distance_settings().view_distance_chunks;
        self.expansion_radius = self.expansion_radius.clamp(0, target);
        self.fully_expanded = self.expansion_radius >= target;
        self.visible = desired_chunk_keys_with_snapshot_cached(
            self.center_chunk,
            snapshot,
            &mut self.column_y_cache,
            self.expansion_radius,
        );
        self.rebuild_visible_sets();
    }

    pub fn sync_visible_to_render_settings(&mut self) {
        self.expansion_radius = render_distance_settings().view_distance_chunks;
        self.fully_expanded = true;
        self.rebuild_visible();
    }

    pub fn sync_visible_to_render_settings_with_snapshot(&mut self, snapshot: &WorldSnapshot) {
        self.rebuild_visible_with_snapshot(snapshot);
    }

    pub fn reset(&mut self, center_chunk: IVec3) {
        self.dirty_meshes.clear();
        self.budget_deferred.clear();
        self.lod_meshes.clear();
        self.gpu_bytes = 0;
        self.spawn_ready = false;
        self.center_chunk = center_chunk;
        self.visible = desired_chunk_keys(center_chunk);
        self.rebuild_visible_sets();
        if !self.gpu_meshes.is_empty() {
            self.gpu_meshes.clear();
            self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
        }
        self.cpu_chunks.clear();
        self.column_y_cache.clear();
        self.simulation_seeds.clear();
        self.expansion_radius = 0;
        self.fully_expanded = false;
    }

    pub fn reset_with_snapshot(&mut self, center_chunk: IVec3, snapshot: &WorldSnapshot) {
        self.dirty_meshes.clear();
        self.budget_deferred.clear();
        self.lod_meshes.clear();
        self.gpu_bytes = 0;
        self.spawn_ready = false;
        self.center_chunk = center_chunk;
        self.column_y_cache.clear();
        self.simulation_seeds.clear();
        self.expansion_radius = 2;
        self.fully_expanded = false;
        self.visible = desired_chunk_keys_with_snapshot_cached(
            center_chunk,
            snapshot,
            &mut self.column_y_cache,
            self.expansion_radius,
        );
        self.rebuild_visible_sets();
        if !self.gpu_meshes.is_empty() {
            self.gpu_meshes.clear();
            self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
        }
        self.cpu_chunks.clear();
    }

    pub fn update_center_chunk(&mut self, center_chunk: IVec3) -> bool {
        if self.center_chunk == center_chunk {
            return false;
        }

        let old_center = self.center_chunk;
        self.center_chunk = center_chunk;
        self.visible = desired_chunk_keys(center_chunk);
        self.rebuild_visible_sets();
        // Shrink expansion radius by movement distance, keep at least 4 rings.
        let move_dist = (center_chunk.x - old_center.x)
            .abs()
            .max((center_chunk.y - old_center.y).abs())
            .max((center_chunk.z - old_center.z).abs());
        self.expansion_radius = (self.expansion_radius - move_dist).max(4);
        self.fully_expanded = false;
        true
    }

    pub fn update_center_chunk_with_snapshot(
        &mut self,
        center_chunk: IVec3,
        snapshot: &WorldSnapshot,
    ) -> bool {
        if self.center_chunk == center_chunk {
            return false;
        }

        self.center_chunk = center_chunk;

        let settings = render_distance_settings();
        let target = settings.view_distance_chunks;
        // Keep requested coverage during movement, including vertical flight.
        // Contracting it discards useful jobs and repeatedly regenerates the edge.
        let rebuild_radius = self.expansion_radius.max(4).min(target);

        self.visible = desired_chunk_keys_with_snapshot_cached(
            center_chunk,
            snapshot,
            &mut self.column_y_cache,
            rebuild_radius,
        );
        self.rebuild_visible_sets();
        self.expansion_radius = rebuild_radius;
        self.fully_expanded = rebuild_radius >= target;
        true
    }

    /// Incrementally expand the visible set by a few chunk-rings per frame.
    /// Returns newly added keys so the caller can enqueue them for scheduling
    /// (avoids a full O(visible) scan in enqueue_missing each frame).
    pub fn expand_visible(
        &mut self,
        snapshot: &WorldSnapshot,
        queue_pressure: usize,
    ) -> Vec<ChunkKey> {
        let settings = render_distance_settings();
        let target = settings.view_distance_chunks;
        self.column_y_cache
            .set_interest((self.center_chunk.x, self.center_chunk.z), target + 2);
        self.column_y_cache.begin(snapshot.cfg);
        let mut added = Vec::new();
        let limit = target + 1;
        for (c, range) in self.column_y_cache.completed() {
            let dx = c.0 - self.center_chunk.x;
            let dz = c.1 - self.center_chunk.z;
            if dx * dx + dz * dz > limit * limit {
                continue;
            }
            for mut key in visibility::keys_for_column(self.center_chunk, c, range, &snapshot.cfg) {
                key.lod = self.desired_lod(key.coord);
                if self.visible_keys.insert(key) {
                    self.visible_coords.insert(key.coord);
                    self.visible.push(key);
                    added.push(key);
                }
            }
        }
        if self.fully_expanded {
            return added;
        }
        if self.expansion_radius >= target {
            self.fully_expanded = true;
            return added;
        }

        let old_radius = self.expansion_radius;
        // Sample one ring at a time and wait for downstream capacity.
        if queue_pressure >= MAX_READY_CHUNKS {
            return added;
        }
        let new_radius = (old_radius + 1).min(target);
        self.expansion_radius = new_radius;

        // Only generate keys for the NEW ring (old_radius..new_radius) and append
        // them to the existing visible list.  This avoids rebuilding the entire
        // O(radius²) set every frame during expansion.
        let new_keys = desired_chunk_keys_ring(
            self.center_chunk,
            snapshot,
            &mut self.column_y_cache,
            old_radius,
            new_radius,
        );
        added.reserve(new_keys.len());
        for mut key in new_keys {
            key.lod = self.desired_lod(key.coord);
            if self.visible_keys.insert(key) {
                self.visible_coords.insert(key.coord);
                self.visible.push(key);
                added.push(key);
            }
        }

        if new_radius >= target {
            self.fully_expanded = true;
        }
        added
    }

    pub fn take_simulation_seeds(&mut self) -> Vec<(IVec3, Box<crate::world::activity::Mask>)> {
        std::mem::take(&mut self.simulation_seeds)
    }
    pub fn retain_visible(&mut self) {
        // Evict chunks that are genuinely too far away rather than anything
        // outside the current (possibly mid-expansion) visible_coords.  Using a
        // hard distance cap prevents "blank world" flicker when the player crosses
        // a chunk boundary and visible_coords briefly collapses to the small
        // 5×5×3 seed set before expand_visible() has had a chance to grow it.
        let settings = render_distance_settings();
        let hard_limit = settings.view_distance_chunks + 2; // 2-chunk grace buffer
        let center = self.center_chunk;
        let is_in_range = |coord: &IVec3| {
            let dx = (coord.x - center.x).abs();
            let dz = (coord.z - center.z).abs();
            dx.max(dz) <= hard_limit
        };
        let mesh_count_before = self.gpu_meshes.len();
        self.gpu_meshes.retain(|coord, _| is_in_range(coord));
        self.gpu_bytes = self.gpu_meshes.values().map(|mesh| mesh.bytes).sum();
        self.lod_meshes.retain(|coord| is_in_range(&coord));
        if self.gpu_meshes.len() != mesh_count_before {
            self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
        }
        self.cpu_chunks.retain(|coord, _| is_in_range(coord));
        self.dirty_meshes.retain(is_in_range);
        self.column_y_cache
            .retain(|&(x, z), _| (x - center.x).abs().max((z - center.z).abs()) <= hard_limit);
    }

    pub fn spawn_progress(&mut self) -> (usize, usize) {
        let mut ready = 0;
        for z in -1..=1 {
            for y in -1..=1 {
                for x in -1..=1 {
                    if self
                        .gpu_meshes
                        .contains_key(&(self.center_chunk + IVec3::new(x, y, z)))
                    {
                        ready += 1;
                    }
                }
            }
        }
        self.spawn_ready |= ready == 27;
        (ready, 27)
    }

    fn install_mesh(&mut self, coord: IVec3, mesh: ChunkGpuMesh) {
        self.dirty_meshes.remove(&coord);
        drop(self.lod_meshes.take(ChunkKey {
            coord,
            lod: mesh.lod,
        }));
        self.gpu_bytes += mesh.bytes;
        let lod = mesh.lod;
        if let Some(previous) = self.gpu_meshes.insert(coord, mesh) {
            self.gpu_bytes -= previous.bytes;
            if previous.lod != lod && !previous.lod_invariant {
                self.lod_meshes.remember(coord, previous);
            }
        }
        self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
        self.trim_lod_budget();
    }

    fn activate_lod(&mut self, key: ChunkKey) -> bool {
        if !self.dirty_meshes.contains(&key.coord) {
            if let Some(mesh) = self.gpu_meshes.get_mut(&key.coord) {
                if mesh.lod_invariant || mesh.lod == key.lod {
                    mesh.lod = key.lod;
                    return true;
                }
            }
        }
        let Some(mesh) = self.lod_meshes.take(key) else {
            return false;
        };
        self.install_mesh(key.coord, mesh);
        true
    }
}
pub struct CpuChunkMesh {
    failed: bool,
    lod_invariant: bool,
    pub key: ChunkKey,
    pub revision: u64,
    pub job_id: u64,
    pub mesh: ChunkMesh,
    // Rust drops fields in declaration order: release the byte reservation
    // only after the mesh vectors have actually been freed.
    _lease: Option<MeshLease>,
}

#[derive(Clone)]
struct ChunkJob {
    key: ChunkKey,
    revision: u64,
    source: Arc<ResidentChunk>,
}

struct MeshJob {
    key: ChunkKey,
    revision: u64,
    job_id: u64,
    chunk: Arc<ResidentChunk>,
    cancelled: Arc<AtomicBool>,
}

fn spawn_mesh_workers(
    worker_count: usize,
    budget: &Arc<MeshBudget>,
) -> anyhow::Result<(mpsc::SyncSender<MeshJob>, mpsc::Receiver<CpuChunkMesh>)> {
    let (job_tx, job_rx) = mpsc::sync_channel::<MeshJob>(MAX_MESH_JOBS);
    let (result_tx, result_rx) = mpsc::sync_channel::<CpuChunkMesh>(MAX_READY_MESHES);
    let shared_rx = Arc::new(Mutex::new(job_rx));
    for worker_index in 0..worker_count.max(1) {
        let budget = Arc::clone(budget);
        let shared_rx = Arc::clone(&shared_rx);
        let result_tx = result_tx.clone();
        thread::Builder::new()
            .name(format!("chunk-mesh-{worker_index}"))
            .spawn(move || {
                loop {
                    let job = match shared_rx.lock().unwrap_or_else(|e| e.into_inner()).recv() {
                        Ok(job) => job,
                        Err(_) => break,
                    };
                    let key = job.key;
                    let job_id = job.job_id;
                    let revision = job.revision;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        if job.cancelled.load(Ordering::Relaxed) {
                            return None;
                        }
                        let planning = budget.begin_plan(&job.cancelled)?;
                        let mut lease = budget.acquire(PLANNING_BYTES, &job.cancelled)?;
                        let plan = MeshPlan::for_chunk(job.chunk.as_ref(), &job.cancelled)?;
                        lease.grow_to(plan.bytes(), &job.cancelled)?;
                        drop(planning);
                        if job.cancelled.load(Ordering::Relaxed) {
                            return None;
                        }
                        let mut mesh = if plan.vertices == 0 {
                            ChunkMesh::empty()
                        } else {
                            plan.build_lod(job.chunk.as_ref(), job.key.lod)
                        };
                        let lod_invariant = plan.lod_invariant();
                        drop(plan);
                        drop(job.chunk);
                        if job.cancelled.load(Ordering::Relaxed) {
                            return None;
                        }
                        // Builder scratch has been released. Reuse its reserved
                        // allowance for a bounded compaction peak; far greedy
                        // meshes otherwise retain their unit-face upper bounds.
                        if mesh.vertices.len() * std::mem::size_of::<crate::mesh::Vertex3>()
                            <= 4 * 1024 * 1024
                            && mesh.indices.len() * 4 <= 4 * 1024 * 1024
                        {
                            mesh.vertices.shrink_to_fit();
                            mesh.indices.shrink_to_fit();
                        }
                        let bytes = mesh.vertices.capacity()
                            * std::mem::size_of::<crate::mesh::Vertex3>()
                            + mesh.indices.capacity() * 4;
                        lease.finish(bytes);
                        Some(CpuChunkMesh {
                            key,
                            revision,
                            job_id,
                            mesh,
                            failed: false,
                            lod_invariant,
                            _lease: Some(lease),
                        })
                    }));
                    let output = match result {
                        Ok(Some(output)) => output,
                        Ok(None) => continue,
                        Err(_) => CpuChunkMesh {
                            key,
                            revision,
                            job_id,
                            mesh: ChunkMesh::empty(),
                            failed: true,
                            lod_invariant: true,
                            _lease: None,
                        },
                    };
                    if result_tx.send(output).is_err() {
                        break;
                    }
                }
            })
            .map_err(anyhow::Error::from)?;
    }
    drop(result_tx);
    Ok((job_tx, result_rx))
}

pub struct ChunkStreamer {
    upload_checks: Vec<gpu_upload::Check>,
    upload_retry_after: Instant,
    upload_error: Option<String>,
    worker_retry_after: Instant,
    mesh_budget: Arc<MeshBudget>,
    empty_vertex: wgpu::Buffer,
    empty_index: wgpu::Buffer,
    requested: HashSet<ChunkKey>,
    /// Coord-only set for O(1) "is any job in-flight for this coord?" checks.
    /// Kept in sync with `requested` and `meshing`.
    requested_coords: HashSet<IVec3>,
    /// Latest edit while a mesh is being built. Publish that job before retrying
    /// so continuously simulated chunks cannot starve their first visible mesh.
    dirty_revisions: HashMap<IVec3, u64>,
    generating: HashSet<ChunkKey>,
    meshing: HashMap<ChunkKey, (u64, Arc<AtomicBool>)>,
    ready_chunks: VecDeque<ChunkJob>,
    ready_meshes: VecDeque<CpuChunkMesh>,
    mesh_job_tx: mpsc::SyncSender<MeshJob>,
    mesh_result_rx: mpsc::Receiver<CpuChunkMesh>,
    next_mesh_job_id: u64,
    terrain_generator: TerrainGenerator,
    /// Chunks that need to be scheduled but haven't been yet.
    /// Populated when new keys are added to the visible set, avoiding an
    /// O(visible) scan inside enqueue_missing every frame.
    /// Stored as a min-heap keyed by radial XZ distance so near chunks are
    /// scheduled before distant ones.
    pending_schedule: BinaryHeap<Reverse<(i32, i32, i32, i32, i32, i32, i32, u8)>>,
    pending_keys: HashSet<ChunkKey>,
    pending_schedule_center: IVec3,
    repair_cursor: usize,
    last_stats: ChunkStreamerStats,
}

impl ChunkStreamer {
    pub fn new(device: &wgpu::Device, worker_count: usize) -> anyhow::Result<Self> {
        let mesh_budget = Arc::new(MeshBudget::default());
        let (mesh_job_tx, mesh_result_rx) = spawn_mesh_workers(worker_count, &mesh_budget)?;

        Ok(Self {
            worker_retry_after: Instant::now(),
            mesh_budget,
            upload_checks: Vec::new(),
            dirty_revisions: HashMap::new(),
            upload_retry_after: Instant::now(),
            upload_error: None,
            empty_vertex: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shared empty chunk vertex buffer"),
                size: std::mem::size_of::<crate::mesh::Vertex3>() as u64,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            }),
            empty_index: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shared empty chunk index buffer"),
                size: 4,
                usage: wgpu::BufferUsages::INDEX,
                mapped_at_creation: false,
            }),
            requested: HashSet::new(),
            requested_coords: HashSet::new(),
            generating: HashSet::new(),
            meshing: HashMap::new(),
            ready_chunks: VecDeque::new(),
            ready_meshes: VecDeque::new(),
            mesh_job_tx,
            mesh_result_rx,
            next_mesh_job_id: 0,
            terrain_generator: TerrainGenerator::new(device),
            pending_schedule: BinaryHeap::new(),
            pending_keys: HashSet::new(),
            pending_schedule_center: IVec3::ZERO,
            repair_cursor: 0,
            last_stats: ChunkStreamerStats::default(),
        })
    }

    pub fn reset(&mut self) {
        self.dirty_revisions.clear();
        self.upload_checks.clear();
        self.upload_retry_after = Instant::now();
        self.upload_error = None;
        self.requested.clear();
        self.requested_coords.clear();
        self.generating.clear();
        self.retain_mesh_jobs(|_| false);
        self.ready_chunks.clear();
        self.ready_meshes.clear();
        self.pending_schedule.clear();
        self.pending_keys.clear();
        self.repair_cursor = 0;
        while self.mesh_result_rx.try_recv().is_ok() {}
        self.terrain_generator.reset();
        self.last_stats = ChunkStreamerStats::default();
    }

    pub fn begin_frame_stats(&mut self) {
        self.last_stats = ChunkStreamerStats::default();
    }

    pub fn last_stats(&self) -> ChunkStreamerStats {
        self.last_stats
    }

    fn retain_mesh_jobs(&mut self, keep: impl Fn(&ChunkKey) -> bool) {
        self.meshing.retain(|key, (_, cancelled)| {
            let retained = keep(key);
            if !retained {
                cancelled.store(true, Ordering::Relaxed);
            }
            retained
        });
    }

    fn rebuild_pending_schedule(&mut self, center: IVec3) {
        self.pending_schedule_center = center;
        let pending_keys: Vec<_> = self.pending_keys.iter().copied().collect();
        self.pending_schedule.clear();
        for key in pending_keys {
            self.pending_schedule.push(pending_priority(center, key));
        }
    }

    fn push_pending_key(&mut self, key: ChunkKey) {
        if self.pending_keys.insert(key) {
            self.pending_schedule
                .push(pending_priority(self.pending_schedule_center, key));
        }
    }

    fn dynamic_schedule_budget(&self) -> usize {
        let queue_pressure = self.generating.len()
            + self.meshing.len()
            + self.ready_chunks.len()
            + self.ready_meshes.len();
        if queue_pressure >= 192 {
            4
        } else if queue_pressure >= 128 {
            8
        } else {
            CHUNK_SCHEDULE_BUDGET
        }
    }

    fn ready_chunk_priority(cache: &ChunkCache, key: ChunkKey) -> (u8, u8, i32, i32, i32, i32) {
        let mesh_state = match cache.gpu_meshes.get(&key.coord) {
            None => 0u8,
            Some(mesh) if mesh.lod > key.lod => 1u8,
            _ => 2u8,
        };
        let base = chunk_sort_key(cache.center_chunk, key);
        (mesh_state, base.0, base.1, base.2, base.3, base.4)
    }

    fn reprioritize_ready_chunks(&mut self, cache: &ChunkCache) {
        if self.ready_chunks.len() <= 1 {
            return;
        }
        self.ready_chunks
            .make_contiguous()
            .sort_unstable_by_key(|job| Self::ready_chunk_priority(cache, job.key));
    }

    fn reprioritize_ready_meshes(&mut self, cache: &ChunkCache) {
        if self.ready_meshes.len() <= 1 {
            return;
        }
        self.ready_meshes
            .make_contiguous()
            .sort_unstable_by_key(|mesh| Self::ready_chunk_priority(cache, mesh.key));
    }

    pub fn queue_needed_visible(&mut self, cache: &ChunkCache, center: IVec3) {
        if self.pending_schedule_center != center {
            self.rebuild_pending_schedule(center);
        }
        for key in &cache.visible {
            if cache
                .gpu_meshes
                .get(&key.coord)
                .is_some_and(|mesh| mesh.lod == key.lod && !cache.dirty_meshes.contains(&key.coord))
            {
                continue;
            }
            self.push_pending_key(*key);
        }
    }

    pub fn retain_visible(&mut self, visible: &HashSet<ChunkKey>) {
        let coords: HashSet<_> = visible.iter().map(|key| key.coord).collect();
        self.dirty_revisions.retain(|coord, _| coords.contains(coord));
        // Geometry detail changes do not invalidate source voxels or a reusable LOD.
        self.requested.retain(|key| coords.contains(&key.coord));
        self.generating.retain(|key| coords.contains(&key.coord));
        self.retain_mesh_jobs(|key| coords.contains(&key.coord));
        self.ready_chunks
            .retain(|job| coords.contains(&job.key.coord));
        self.ready_meshes
            .retain(|mesh| coords.contains(&mesh.key.coord));
        self.pending_keys.retain(|key| visible.contains(key));
        self.rebuild_pending_schedule(self.pending_schedule_center);
        self.terrain_generator.retain_visible(visible);
        self.requested_coords = self.requested.iter().map(|k| k.coord).collect();
    }

    /// Looser eviction used after a player chunk-crossing: cancel work only for
    /// chunks beyond the hard distance limit, so in-flight jobs for still-valid
    /// chunks aren't thrown away and immediately re-requested.
    pub fn retain_within_range(&mut self, center: IVec3) {
        let settings = render_distance_settings();
        let hard_limit = settings.view_distance_chunks + 2;
        let in_range = |coord: &IVec3| -> bool {
            let dx = (coord.x - center.x).abs();
            let dz = (coord.z - center.z).abs();
            dx.max(dz) <= hard_limit
        };
        self.requested.retain(|key| in_range(&key.coord));
        self.generating.retain(|key| in_range(&key.coord));
        self.retain_mesh_jobs(|key| in_range(&key.coord));
        self.ready_chunks.retain(|job| in_range(&job.key.coord));
        self.ready_meshes.retain(|mesh| in_range(&mesh.key.coord));
        self.pending_keys.retain(|key| in_range(&key.coord));
        self.rebuild_pending_schedule(center);
        let terrain_keys: HashSet<ChunkKey> = self
            .requested
            .iter()
            .chain(self.generating.iter())
            .copied()
            .collect();
        self.terrain_generator.retain_visible(&terrain_keys);
        self.requested_coords = self.requested.iter().map(|k| k.coord).collect();
    }

    fn cancel_coords(&mut self, dirty_coords: &HashSet<IVec3>) {
        if dirty_coords.is_empty() {
            return;
        }

        self.requested
            .retain(|key| !dirty_coords.contains(&key.coord));
        self.generating
            .retain(|key| !dirty_coords.contains(&key.coord));
        self.retain_mesh_jobs(|key| !dirty_coords.contains(&key.coord));
        self.ready_chunks
            .retain(|job| !dirty_coords.contains(&job.key.coord));
        self.ready_meshes
            .retain(|mesh| !dirty_coords.contains(&mesh.key.coord));
        self.pending_keys
            .retain(|key| !dirty_coords.contains(&key.coord));
        self.rebuild_pending_schedule(self.pending_schedule_center);
        self.terrain_generator.invalidate_coords(dirty_coords);
        self.requested_coords
            .retain(|coord| !dirty_coords.contains(coord));
    }

    fn collect_finished_meshes(&mut self) {
        let mut collected = 0usize;
        while collected < MAX_READY_MESHES && self.ready_meshes.len() < MAX_READY_MESHES {
            let Ok(cpu_mesh) = self.mesh_result_rx.try_recv() else {
                break;
            };
            collected += 1;
            if self.meshing.get(&cpu_mesh.key).map(|(id, _)| *id) != Some(cpu_mesh.job_id) {
                continue;
            }
            if cpu_mesh.failed {
                self.meshing.remove(&cpu_mesh.key);
                self.requested.remove(&cpu_mesh.key);
                self.requested_coords.remove(&cpu_mesh.key.coord);
                self.push_pending_key(cpu_mesh.key);
                self.worker_retry_after = Instant::now() + Duration::from_secs(1);
                eprintln!(
                    "Mesh construction failed at {:?}; reservation released and request deferred",
                    cpu_mesh.key.coord
                );
                continue;
            }
            self.meshing.remove(&cpu_mesh.key);
            self.ready_meshes.push_back(cpu_mesh);
        }
    }

    fn dispatch_ready_chunks(&mut self, cache: &mut ChunkCache, current_revision: u64) -> usize {
        if Instant::now() < self.worker_retry_after {
            return 0;
        }
        let mut dispatched = 0usize;
        self.reprioritize_ready_chunks(cache);

        while dispatched < CHUNK_MESH_DISPATCH_BUDGET && self.meshing.len() < MAX_MESH_JOBS {
            let Some(mut job) = self.ready_chunks.pop_front() else {
                break;
            };

            // Only skip if the chunk is no longer in the visible set.
            // A stale revision means a world edit happened; rebuild_dirty() will
            // re-queue affected chunks so we don't need to drop work here.
            if !cache.visible_coords.contains(&job.key.coord) {
                self.requested.remove(&job.key);
                self.requested_coords.remove(&job.key.coord);
                continue;
            }
            let _ = current_revision;
            let desired = ChunkKey {
                coord: job.key.coord,
                lod: cache.desired_lod(job.key.coord),
            };
            if job.key != desired {
                self.requested.remove(&job.key);
                self.requested.insert(desired);
                job.key = desired;
            }
            if cache.activate_lod(desired) {
                self.last_stats.lod_cache_hits += 1;
                self.requested.remove(&desired);
                self.requested_coords.remove(&desired.coord);
                continue;
            }

            if self.meshing.contains_key(&job.key) {
                continue;
            }

            // A queued mesh owns its source independently of the regenerable cache.
            let chunk = job.source.clone();

            let job_id = self.next_mesh_job_id;
            self.next_mesh_job_id = self.next_mesh_job_id.wrapping_add(1);
            let cancelled = Arc::new(AtomicBool::new(false));

            match self.mesh_job_tx.try_send(MeshJob {
                key: job.key,
                revision: job.revision,
                job_id,
                chunk,
                cancelled: cancelled.clone(),
            }) {
                Ok(()) => {
                    self.meshing.insert(job.key, (job_id, cancelled));
                    dispatched += 1;
                }
                Err(mpsc::TrySendError::Full(_)) => {
                    self.ready_chunks.push_front(job);
                    break;
                }
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    // Never run an unbudgeted mesh build on the event thread.
                    self.ready_chunks.push_front(job);
                    let retry: HashSet<_> = self
                        .meshing
                        .keys()
                        .copied()
                        .chain(self.ready_chunks.iter().map(|job| job.key))
                        .chain(self.ready_meshes.iter().map(|mesh| mesh.key))
                        .collect();
                    self.retain_mesh_jobs(|_| false);
                    self.requested.retain(|key| self.generating.contains(key));
                    self.requested_coords = self.requested.iter().map(|key| key.coord).collect();
                    self.ready_chunks.clear();
                    self.ready_meshes.clear();
                    for key in retry {
                        self.push_pending_key(key);
                    }
                    self.worker_retry_after = Instant::now() + Duration::from_secs(1);
                    match spawn_mesh_workers(1, &self.mesh_budget) {
                        Ok((tx, rx)) => {
                            self.mesh_job_tx = tx;
                            self.mesh_result_rx = rx;
                        }
                        Err(e) => eprintln!("Cannot restart mesh worker: {e:#}"),
                    }
                    eprintln!("Mesh worker pool stopped; background recovery attempted");
                    break;
                }
            }
        }
        dispatched
    }

    pub fn rebuild_dirty(
        &mut self,
        snapshot: &WorldSnapshot,
        cache: &mut ChunkCache,
        dirty_blocks: &HashSet<IVec3>,
        dirty_coords: &HashSet<IVec3>,
        revision: u64,
    ) {
        if dirty_coords.is_empty() {
            return;
        }

        // Index changed cells by the source halo they intersect. Remote activity
        // no longer makes each visible dirty chunk scan the entire edit batch.
        let mut halo_edits: HashMap<IVec3, Vec<IVec3>> = HashMap::new();
        for &p in dirty_blocks {
            let lo = crate::world::chunk_coord_from_world_block(p - IVec3::ONE);
            let hi = crate::world::chunk_coord_from_world_block(p + IVec3::ONE);
            for y in lo.y..=hi.y {
                for z in lo.z..=hi.z {
                    for x in lo.x..=hi.x {
                        halo_edits.entry(IVec3::new(x, y, z)).or_default().push(p);
                    }
                }
            }
        }
        // Preserve work already in flight. Repeated water/fire edits used to
        // cancel generation, meshing and even completed results every tick.
        // Keep only one job per coordinate and repair its revision on publish.
        let inflight_dirty: HashSet<_> = dirty_coords.iter().copied()
            .filter(|coord| self.requested_coords.contains(coord)).collect();
        let restart: HashSet<_> = dirty_coords.difference(&inflight_dirty).copied().collect();
        for &coord in dirty_coords {
            self.dirty_revisions.insert(coord, revision);
        }
        self.cancel_coords(&restart);
        cache.dirty_meshes.extend(dirty_coords.iter().copied());
        cache
            .budget_deferred
            .retain(|key, _| !dirty_coords.contains(&key.coord));
        cache
            .lod_meshes
            .retain(|coord| !dirty_coords.contains(&coord));

        // Construction outside procedural bounds must appear immediately,
        // without requiring movement to another chunk or another expansion ring.
        let radius = render_distance_settings().view_distance_chunks;
        for &coord in dirty_coords {
            let delta = coord - cache.center_chunk;
            if snapshot.chunks.contains_key(&coord)
                && delta.x * delta.x + delta.z * delta.z <= radius * radius
                && cache.visible_coords.insert(coord)
            {
                let key = ChunkKey {
                    coord,
                    lod: cache.desired_lod(coord),
                };
                cache.visible_keys.insert(key);
                cache.visible.push(key);
            }
        }

        for &coord in dirty_coords {
            if !cache.visible_coords.contains(&coord) {
                continue;
            }

            if inflight_dirty.contains(&coord) {
                // The worker retains its immutable source. The follow-up must
                // regenerate from the latest snapshot, including halo changes.
                cache.cpu_chunks.remove(&coord);
                continue;
            }

            let lod = cache.desired_lod(coord);
            if !snapshot.edits_ready(coord) {
                self.push_pending_key(ChunkKey { coord, lod });
                continue;
            }
            let Some(chunk_data) = cache.cpu_chunks.get_mut(&coord) else {
                self.push_pending_key(ChunkKey { coord, lod });
                continue;
            };

            // A cancelled worker may still hold this source. Regenerate rather
            // than clone a pinned array outside the residency reservation.
            let Some(chunk_data) = Arc::get_mut(chunk_data) else {
                cache.cpu_chunks.remove(&coord);
                self.push_pending_key(ChunkKey { coord, lod });
                continue;
            };
            for &block in halo_edits.get(&coord).into_iter().flatten() {
                let local = block - chunk_data.origin();
                if local.min_element() >= 0 && local.max_element() < PADDED_CHUNK_SIZE {
                    chunk_data.set_world(block, snapshot.get(block));
                    if let Some(&volume) = snapshot.water_levels.get(&block) {
                        chunk_data.water_levels.insert(block, volume);
                    } else {
                        chunk_data.water_levels.remove(&block);
                    }
                }
            }
            chunk_data.refresh_allocation_size();

            let key = ChunkKey { coord, lod };
            self.requested.insert(key);
            self.requested_coords.insert(key.coord);
            self.ready_chunks.push_front(ChunkJob {
                key,
                revision,
                source: cache.cpu_chunks[&key.coord].clone(),
            });
        }
    }

    /// Push newly visible keys into the pending schedule queue.
    /// Called from the outside when new chunks enter the visible set so that
    /// enqueue_missing doesn't have to scan all visible chunks every frame.
    /// Keys are stored with radial XZ distance so near chunks are scheduled first.
    pub fn push_pending(&mut self, keys: Vec<ChunkKey>, center: IVec3) {
        if self.pending_schedule_center != center {
            self.rebuild_pending_schedule(center);
        }
        for key in keys {
            self.push_pending_key(key);
        }
    }

    pub fn enqueue_missing(
        &mut self,
        gpu: &Gpu,
        snapshot: &WorldSnapshot,
        revision: u64,
        cache: &mut ChunkCache,
    ) {
        cache.sync_residency_budget();
        self.collect_finished_meshes();
        let mut terrain_to_schedule = Vec::new();
        let mut terrain_generated = 0usize;

        for (key, _job_revision, chunk_data, activity) in self
            .terrain_generator
            .collect_finished(&gpu.device, snapshot)
        {
            self.generating.remove(&key);
            let Some(chunk_data) = chunk_data else {
                self.requested.remove(&key);
                self.requested_coords.remove(&key.coord);
                self.push_pending_key(key);
                continue;
            };
            // Accept terrain data as long as the chunk is still in the visible
            // range — don't discard on revision mismatch.  A world edit (which
            // bumps revision) will re-queue via rebuild_dirty anyway.
            if !cache.visible_coords.contains(&key.coord) {
                self.requested.remove(&key);
                self.requested_coords.remove(&key.coord);
                continue;
            }
            if let Some(mask) = activity {
                cache.simulation_seeds.push((key.coord, mask));
            }
            // A uniform volume INCLUDING its halo exposes no faces at any
            // detail level. Bypass source residency, workers and mesh scratch.
            let block = chunk_data.blocks[0];
            if (block.is_empty()
                || block.is_opaque()
                || (block == crate::world::BlockId::Water && chunk_data.water_levels.is_empty()))
                && chunk_data.blocks.iter().all(|&b| b == block)
                && self.ready_meshes.len() < MAX_READY_MESHES
            {
                self.ready_meshes.push_back(CpuChunkMesh {
                    failed: false,
                    lod_invariant: true,
                    key,
                    revision,
                    job_id: 0,
                    mesh: ChunkMesh::empty(),
                    _lease: None,
                });
                terrain_generated += 1;
                continue;
            }
            if !cache.admit_source(key.coord, chunk_data) {
                self.requested.remove(&key);
                self.requested_coords.remove(&key.coord);
                self.push_pending_key(key);
                continue;
            }
            self.ready_chunks.push_back(ChunkJob {
                key,
                revision, // use current revision so downstream checks pass
                source: cache.cpu_chunks[&key.coord].clone(),
            });
            terrain_generated += 1;
        }

        let mesh_dispatched = self.dispatch_ready_chunks(cache, revision);

        // Drain the pending queue rather than scanning all visible chunks each frame.
        // pending_schedule is populated by push_pending() when expand_visible() adds
        // new rings, so on steady-state frames it's empty and this loop is O(0).
        let schedule_budget = self.dynamic_schedule_budget();
        let mut scheduled = 0usize;
        let mut waiting_regions = Vec::new();
        // Limit how many entries we scan from the pending queue per frame so we
        // don't stall when a large ring arrives at once.
        let scan_limit = self.pending_schedule.len().min(schedule_budget * 4);
        let mut scanned = 0usize;
        while scheduled < schedule_budget && scanned < scan_limit {
            if self.ready_chunks.len() >= MAX_READY_CHUNKS
                || self.ready_meshes.len() >= MAX_READY_MESHES
                || self.meshing.len() >= MAX_MESH_JOBS
                || self.generating.len() >= MAX_GENERATING
            {
                break;
            }
            let Some(Reverse((_, _, _, _, kx, ky, kz, klod))) = self.pending_schedule.pop() else {
                break;
            };
            let key = ChunkKey {
                coord: IVec3::new(kx, ky, kz),
                lod: klod,
            };
            self.pending_keys.remove(&key);
            scanned += 1;

            if !snapshot.edits_ready(key.coord) && !cache.cpu_chunks.contains_key(&key.coord) {
                waiting_regions.push(key);
                continue;
            }
            // Skip if no longer visible.
            if !cache.visible_keys.contains(&key) {
                continue;
            }
            // Already have the correct LOD mesh — nothing to do.
            if cache
                .gpu_meshes
                .get(&key.coord)
                .is_some_and(|mesh| mesh.lod == key.lod && !cache.dirty_meshes.contains(&key.coord))
            {
                continue;
            }
            if !cache.budget_allows_request(key) {
                continue;
            }
            if cache.activate_lod(key) {
                self.last_stats.lod_cache_hits += 1;
                continue;
            }
            // The running job owns this coordinate. Completion explicitly queues
            // a changed desired LOD; do not keep popping the same busy heap entry.
            if self.requested_coords.contains(&key.coord) {
                continue;
            }

            if cache.cpu_chunks.contains_key(&key.coord) {
                self.requested.insert(key);
                self.requested_coords.insert(key.coord);
                self.ready_chunks.push_back(ChunkJob {
                    key,
                    revision,
                    source: cache.cpu_chunks[&key.coord].clone(),
                });
                scheduled += 1;
                continue;
            }
            if terrain_to_schedule.len() >= self.terrain_generator.available_slots() {
                self.push_pending_key(key);
                break;
            }
            self.requested.insert(key);
            self.requested_coords.insert(key.coord);
            self.generating.insert(key);
            terrain_to_schedule.push(key);
            scheduled += 1;
        }

        for key in waiting_regions {
            self.push_pending_key(key);
        }

        // Repair missed requests fairly with a rotating, bounded scan. Repair
        // only seeds the heap, so all jobs use the same capacity checks above.
        if self.pending_schedule.is_empty() && !cache.visible.is_empty() {
            for _ in 0..cache.visible.len().min(64) {
                self.repair_cursor %= cache.visible.len();
                let key = cache.visible[self.repair_cursor];
                self.repair_cursor += 1;
                let complete = cache.gpu_meshes.get(&key.coord).is_some_and(|mesh| {
                    mesh.lod == key.lod && !cache.dirty_meshes.contains(&key.coord)
                });
                if !complete
                    && cache.budget_allows_request(key)
                    && !self.requested_coords.contains(&key.coord)
                {
                    self.push_pending_key(key);
                }
            }
        }
        let terrain_scheduled = terrain_to_schedule.len();
        self.terrain_generator
            .schedule_many(gpu, terrain_to_schedule, snapshot.cfg, revision);
        self.last_stats.terrain_generated += terrain_generated;
        self.last_stats.terrain_scheduled += terrain_scheduled;
        self.last_stats.mesh_dispatched += mesh_dispatched;
        self.last_stats.requested = self.requested.len();
        self.last_stats.generating = self.generating.len();
        self.last_stats.meshing = self.meshing.len();
        self.last_stats.ready_chunks = self.ready_chunks.len();
        self.last_stats.ready_meshes = self.ready_meshes.len();
        self.last_stats.pending_schedule = self.pending_schedule.len();
    }

    pub fn upload_error(&self) -> Option<&str> {
        self.upload_error.as_deref()
    }
    pub fn upload_finished(
        &mut self,
        gpu: &Gpu,
        cache: &mut ChunkCache,
        current_revision: u64,
        max_uploads: usize,
    ) {
        cache.sync_residency_budget();
        let mut index = 0;
        while index < self.upload_checks.len() {
            match gpu_upload::poll(&mut self.upload_checks[index]) {
                std::task::Poll::Pending => index += 1,
                std::task::Poll::Ready(error) => {
                    drop(self.upload_checks.swap_remove(index));
                    if let Some(error) = error {
                        eprintln!("Chunk GPU upload rejected: {error}");
                        self.upload_error = Some(error);
                        self.upload_retry_after = Instant::now() + Duration::from_secs(2);
                    }
                }
            }
        }
        if Instant::now() < self.upload_retry_after || self.upload_checks.len() >= 16 {
            return;
        }
        let mut uploaded = 0usize;
        let mut uploaded_vertices = 0usize;
        let mut uploaded_indices = 0usize;
        let started = Instant::now();
        self.collect_finished_meshes();
        self.reprioritize_ready_meshes(cache);

        while uploaded < max_uploads
            && self.upload_checks.len() < 16
            && (uploaded == 0 || started.elapsed() < UPLOAD_TIME_BUDGET)
        {
            let Some(cpu_mesh) = self.ready_meshes.pop_front() else {
                break;
            };
            let mesh_vertices = cpu_mesh.mesh.vertices.len();
            let mesh_indices = cpu_mesh.mesh.indices.len();
            let would_exceed_budget = uploaded > 0
                && (uploaded_vertices + mesh_vertices > CHUNK_UPLOAD_VERTEX_BUDGET
                    || uploaded_indices + mesh_indices > CHUNK_UPLOAD_INDEX_BUDGET);
            if would_exceed_budget {
                self.ready_meshes.push_front(cpu_mesh);
                break;
            }

            self.requested.remove(&cpu_mesh.key);
            // requested_coords tracks any in-flight key for this coord.
            // Since we enforce at most one key per coord in requested, removing
            // unconditionally here is correct and O(1) instead of O(N).
            self.requested_coords.remove(&cpu_mesh.key.coord);

            // Only discard if the chunk is no longer visible.  Stale revision just
            // means the world was edited while this mesh was in-flight; we still
            // upload it — a subsequent rebuild_dirty() will re-mesh the affected
            // chunks with the latest data.  Discarding here was the main cause of
            // wasted work and blank-chunk flicker on every player chunk crossing.
            if !cache.visible_coords.contains(&cpu_mesh.key.coord) {
                continue;
            }
            // If the world was edited after this mesh started, the chunk will be
            // re-queued by rebuild_dirty.  Upload what we have now so there's no gap.
            let _ = current_revision;

            let desired_lod = cache.desired_lod(cpu_mesh.key.coord);
            // Avoid allocating an obsolete result when a current mesh is ready.
            if desired_lod != cpu_mesh.key.lod
                && cache
                    .gpu_meshes
                    .get(&cpu_mesh.key.coord)
                    .is_some_and(|mesh| {
                        mesh.lod == desired_lod && !cache.dirty_meshes.contains(&cpu_mesh.key.coord)
                    })
            {
                continue;
            }
            let bytes =
                mesh_vertices * std::mem::size_of::<crate::mesh::Vertex3>() + mesh_indices * 4;
            if let Err(error) = gpu_upload::supported(&gpu.device, &cpu_mesh.mesh) {
                self.upload_error = Some(error.clone());
                eprintln!("Chunk GPU upload rejected: {error}");
                cache.budget_deferred.insert(cpu_mesh.key, usize::MAX);
                continue;
            }
            if !cache.admit_mesh(cpu_mesh.key, bytes) {
                continue;
            }
            let (attempt, check) = gpu_upload::start(
                &gpu.device,
                &gpu.queue,
                &cpu_mesh.mesh,
                cpu_mesh.key,
                cpu_mesh.lod_invariant,
                &self.empty_vertex,
                &self.empty_index,
            );
            self.upload_checks.push(check);
            let gpu_mesh = match attempt {
                Ok(mesh) => mesh,
                Err(error) => {
                    eprintln!("Chunk GPU upload delayed: {error}");
                    self.upload_error = Some(error);
                    self.upload_retry_after = Instant::now() + Duration::from_secs(2);
                    self.ready_meshes.push_front(cpu_mesh);
                    break;
                }
            };
            if desired_lod == cpu_mesh.key.lod
                || !cache.gpu_meshes.contains_key(&cpu_mesh.key.coord)
            {
                cache.install_mesh(cpu_mesh.key.coord, gpu_mesh);
            } else {
                // A useful completed old LOD must not replace the desired active one.
                cache.lod_meshes.remember(cpu_mesh.key.coord, gpu_mesh);
                cache.trim_lod_budget();
            }
            if desired_lod != cpu_mesh.key.lod
                && cache
                    .gpu_meshes
                    .get(&cpu_mesh.key.coord)
                    .is_none_or(|mesh| mesh.lod != desired_lod)
            {
                let desired = ChunkKey {
                    coord: cpu_mesh.key.coord,
                    lod: desired_lod,
                };
                if cache.activate_lod(desired) {
                    self.last_stats.lod_cache_hits += 1;
                } else {
                    self.push_pending_key(desired);
                }
            }
            if self.dirty_revisions.get(&cpu_mesh.key.coord)
                .is_some_and(|&edited| edited > cpu_mesh.revision)
            {
                // install_mesh clears the dirty flag. Restore it when this is
                // only a fallback, and never reuse a stale source or LOD.
                cache.dirty_meshes.insert(cpu_mesh.key.coord);
                cache.cpu_chunks.remove(&cpu_mesh.key.coord);
                cache.lod_meshes.retain(|coord| coord != cpu_mesh.key.coord);
                self.push_pending_key(ChunkKey { coord: cpu_mesh.key.coord, lod: desired_lod });
            } else {
                self.dirty_revisions.remove(&cpu_mesh.key.coord);
            }
            self.upload_error = None;
            uploaded += 1;
            uploaded_vertices += mesh_vertices;
            uploaded_indices += mesh_indices;
        }

        self.last_stats.meshes_uploaded += uploaded;
        self.last_stats.uploaded_vertices += uploaded_vertices;
        self.last_stats.uploaded_indices += uploaded_indices;
        self.last_stats.cpu_resident_bytes =
            cache.cpu_bytes() + self.terrain_generator.buffer_bytes();
        self.last_stats.cpu_budget_bytes = cache.residency_limits.0;
        self.last_stats.gpu_budget_bytes = cache.residency_limits.1;
        self.last_stats.budget_deferred = cache.budget_deferred.len();
        self.last_stats.cpu_evictions = cache.cpu_evictions;
        self.last_stats.gpu_evictions = cache.gpu_evictions;
        self.last_stats.gpu_resident_bytes = cache.gpu_bytes + cache.lod_meshes.bytes();
        gpu_budget::observe_residency(self.last_stats.gpu_resident_bytes, cache.budget_deferred.len());
        self.last_stats.lod_cache_bytes = cache.lod_meshes.bytes();
        self.last_stats.terrain_buffer_bytes = self.terrain_generator.buffer_bytes();
        self.last_stats.queued_mesh_bytes = self.mesh_budget.used();
        self.last_stats.requested = self.requested.len();
        self.last_stats.generating = self.generating.len();
        self.last_stats.meshing = self.meshing.len();
        self.last_stats.ready_chunks = self.ready_chunks.len();
        self.last_stats.ready_meshes = self.ready_meshes.len();
        self.last_stats.pending_schedule = self.pending_schedule.len();
    }

    pub fn queued_jobs(&self) -> usize {
        self.generating.len()
            + self.meshing.len()
            + self.ready_chunks.len()
            + self.ready_meshes.len()
            + self.pending_keys.len()
    }
}

fn apply_snapshot_edits_cubic(chunk_data: &mut GeneratedChunk, snapshot: &WorldSnapshot) {
    let min = chunk_data.origin();
    let max = min + IVec3::splat(PADDED_CHUNK_SIZE);
    chunk_data.water_levels = snapshot
        .water_levels
        .iter()
        .filter(|(p, _)| p.cmpge(min).all() && p.cmplt(max).all())
        .map(|(&p, &v)| (p, v))
        .collect();
    if snapshot.chunks.is_empty() {
        return;
    }
    for dy in -1..=1 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let coord = chunk_data.chunk + IVec3::new(dx, dy, dz);
                let Some(chunk) = snapshot.chunks.get(&coord) else {
                    continue;
                };
                let ox = coord.x * CHUNK_SIZE;
                let oy = coord.y * CHUNK_SIZE;
                let oz = coord.z * CHUNK_SIZE;
                let padded_min = chunk_data.chunk * CHUNK_SIZE - IVec3::ONE;
                let padded_max = padded_min + IVec3::splat(PADDED_CHUNK_SIZE - 1);
                let local_min = (padded_min - IVec3::new(ox, oy, oz)).max(IVec3::ZERO);
                let local_max =
                    (padded_max - IVec3::new(ox, oy, oz)).min(IVec3::splat(CHUNK_SIZE - 1));
                for z in local_min.z..=local_max.z {
                    for y in local_min.y..=local_max.y {
                        for x in local_min.x..=local_max.x {
                            let idx = crate::world::ChunkData::local_index(x, y, z);
                            let block = chunk.blocks[idx];
                            let pos = IVec3::new(ox + x, oy + y, oz + z);
                            let local = pos - chunk_data.origin();
                            let index = ((local.y * PADDED_CHUNK_SIZE + local.z)
                                * PADDED_CHUNK_SIZE
                                + local.x) as usize;
                            chunk_data.blocks[index] = block;
                        }
                    }
                }
            }
        }
    }
}
