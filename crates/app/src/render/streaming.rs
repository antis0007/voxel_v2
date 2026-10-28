//! Chunk residency, prioritized terrain generation, bounded meshing, and GPU upload.
//! Job IDs isolate cancelled work from replacements, including across world resets.
mod lod_cache;
mod shadow_casters;
use super::horizon::collar;
mod mesh_budget;
pub use mesh_budget::{mesh_budget_mib, set_mesh_budget_mib};
mod gpu_budget;
mod residency;
pub(super) use gpu_budget::configure_gpu_residency;
pub use gpu_budget::{MAX_GPU_MIB, gpu_residency_limit_mib, gpu_residency_status};
use mesh_budget::{MeshBudget, MeshLease};
use residency::{ResidentChunk, VoxelAllocations, arena_budget};
pub use residency::{set_world_residency_budgets_mib, world_residency_budgets_mib};
mod columns;
mod arena;
pub(super) mod indirect_draws;
mod gpu_upload;
mod pending_gpu;
use pending_gpu::{PendingGpuLedger, PendingGpuUpload, PendingGpuLease};
mod terrain;
mod local_sources;
mod visibility;
pub mod occlusion;
mod view_missing;
mod edit_proxies;
mod quality_proxies;
#[cfg(test)]
mod pin_tests;
fn edited_mesh_is_stale(last_edit: Option<u64>, mesh_revision: u64) -> bool {
    last_edit.is_some_and(|edited| edited > mesh_revision)
}

#[cfg(test)]
mod terrain_revision_contract {
    use super::edited_mesh_is_stale;
    #[test]
    fn terrain_contract_local_edits_reject_old_results_without_global_revision_churn() {
        assert!(edited_mesh_is_stale(Some(8), 7));
        assert!(!edited_mesh_is_stale(Some(8), 8));
        assert!(!edited_mesh_is_stale(Some(8), 9));
        assert!(!edited_mesh_is_stale(None, 1));
    }
}
use super::{ChunkStreamerStats, Gpu, render_distance_settings};
use columns::ColumnPlans;
use lod_cache::LodMeshCache;
use terrain::TerrainGenerator;
pub use visibility::{build_draw_list, chunk_bounds_sphere, extract_planes};
use visibility::{
    chunk_sort_key, desired_chunk_keys, desired_chunk_keys_ring,
    desired_chunk_keys_with_snapshot_cached, lod_for_chunk, pending_priority,
};

use glam::{IVec3, Vec3};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use crate::fast_hash::{FastMap, FastSet};
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
    boundary_profile: Option<Arc<collar::Installed>>,
    /// Range in the shared arena; `None` for an empty mesh. Dropping the mesh
    /// returns the range (see `arena.rs` for when it can be reused).
    alloc: Option<arena::ArenaLease>,
    pub index_count: u32,
    pub lod: u8,
    lod_invariant: bool,
    bytes: usize,
    /// Face links of the source blocks for cave culling; `None` reads as open.
    connectivity: Option<crate::mesh::ChunkConnectivity>,
}

#[cfg(test)]
impl ChunkGpuMesh {
    /// Membership-only fixture: counts and bytes without GPU storage.
    pub(super) fn test_mesh(index_count: u32, lod: u8, lod_invariant: bool, bytes: usize) -> Self {
        Self { boundary_profile: None, alloc: None, index_count, lod, lod_invariant, bytes, connectivity: None }
    }
}

/// What a draw call needs for one resident chunk mesh.
pub struct DrawSlice<'a> {
    pub page: u32,
    pub vertices: &'a wgpu::Buffer,
    pub indices: &'a wgpu::Buffer,
    pub index_range: std::ops::Range<u32>,
    pub base_vertex: i32,
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
    pending_gpu: Arc<PendingGpuLedger>,
    gpu_world_epoch: u64,
    pub visible: Vec<ChunkKey>,
    pub visible_keys: FastSet<ChunkKey>,
    pub visible_coords: FastSet<IVec3>,
    /// Actual complete column members admitted to drawing. Desired vertical
    /// expansion cannot revoke these while its additional meshes are pending.
    pub(super) published_coords: FastSet<IVec3>,
    published_columns: FastMap<(i32,i32),Vec<IVec3>>,
    publication_dirty: bool,
    horizon_pins: FastSet<(i32,i32)>,
    horizon_pin_coords: FastSet<IVec3>,
    edit_proxies: edit_proxies::EditProxies,
    quality_proxies: quality_proxies::QualityProxies,
    seeded_sources: HashSet<IVec3>,
    pub gpu_meshes: FastMap<IVec3, ChunkGpuMesh>,
    /// Storage behind every `ChunkGpuMesh::alloc`.
    arena: arena::MeshArena,
    /// Draw/list caches track every GPU mesh change. Shadow depth has a
    /// separate revision so water-only replacements need not redraw it.
    pub gpu_mesh_revision: u64,
    /// Only validated, exactly unchanged caster inputs may retain this revision.
    pub shadow_caster_revision: u64,
    shadow_casters: shadow_casters::Cache,
    dirty_meshes: FastSet<IVec3>,
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
    /// Stable horizon handoff radius. A single chunk that is momentarily
    /// missing (remesh, GPU-budget eviction, or a just-expanded ring) must not
    /// drag the entire backdrop inward and fade the resident world to sky.
    coverage_stable_meters: f32,
    coverage_drop_since: Option<Instant>,
    coverage_last_tick: Option<Instant>,
    coverage_center: Option<IVec3>,
}

/// Reject a handful of near misses while still reducing coverage for a ring
/// that is genuinely incomplete. Distant misses clustered around the median are
/// fence posts: they are missing because the stream has not reached that ring,
/// not because the world ended. A small number of near misses is a transient
/// hole to be filled by the backdrop, and must not pull the handoff inward.
fn robust_coverage_from_missing(
    missing: &mut [f32],
    full_radius: f32,
    margin: f32,
    total_columns: usize,
) -> f32 {
    if missing.is_empty() || full_radius <= 0.0 {
        return full_radius.max(0.0);
    }
    // Four holes in a normal view are isolated. In a tiny test view they may be
    // the whole visible world, so only discount them when they are sparse.
    if missing.len() <= 4 && missing.len() * 8 < total_columns {
        return full_radius;
    }
    missing.sort_by(f32::total_cmp);
    let median = missing[missing.len() / 2];
    let near_outlier_cutoff = median * 0.70;
    // Ignore only a bounded number of isolated near misses, not an entire sector.
    let outlier_budget = (missing.len() / 20).clamp(1, 8);
    let outlier_count = missing
        .partition_point(|distance| *distance < near_outlier_cutoff)
        .min(outlier_budget);
    let representative = missing[outlier_count.min(missing.len() - 1)];
    (representative - margin).min(full_radius).max(0.0)
}

impl ChunkCache {
    /// Actual GPU-owned disc: every required vertical chunk in each column
    /// must exist. Unlike the visual handoff estimate this never ignores holes,
    /// holds an old radius, or counts requested/queued uploads as coverage.
    pub fn strict_coverage_meters(&self, eye: Vec3, view_chunks: i32, scale: f32) -> f32 {
        if !eye.is_finite() || !scale.is_finite() || scale<=0.0 {return 0.0;}
        let span=CHUNK_SIZE as f32*scale;
        let radius=view_chunks.clamp(1,64);
        let cx=(eye.x/span).floor() as i32; let cz=(eye.z/span).floor() as i32;
        let mut columns:HashMap<(i32,i32),bool>=HashMap::new();
        for q in &self.published_coords {
            let ready=self.gpu_meshes.contains_key(q);
            columns.entry((q.x,q.z)).and_modify(|v|*v &= ready).or_insert(ready);
        }
        let mut limit=radius as f32*span;
        for z in cz-radius..=cz+radius {for x in cx-radius..=cx+radius {
            if columns.get(&(x,z))==Some(&true) {continue;}
            let lo=glam::Vec2::new(x as f32*span,z as f32*span);
            let p=glam::Vec2::new(eye.x,eye.z);
            limit=limit.min(p.distance(p.clamp(lo,lo+glam::Vec2::splat(span))));
        }}
        (limit-2.0*scale).max(0.0)
    }
    pub fn resident_coverage(&self, eye: Vec3, view_chunks: i32, scale: f32) -> super::coverage::ResidentCoverage {
        self.coverage_map(eye,view_chunks,scale,true)
    }

    /// Only complete, actually drawn bundles can supply a collar. A previously
    /// selected edge is O(vertical members) to reuse; no mesh scan/allocation.
    /// Caller memoizes a failed selection until gpu_mesh_revision/coverage changes.
    pub(crate) fn collar_input(
        &self,
        previous: Option<&Arc<collar::Input>>,
        coverage: &super::coverage::ResidentCoverage,
        preferred:&[(i32,i32)],
    ) -> Option<Arc<collar::Input>> {
        let owned_edge = |column: (i32,i32), side| {
            coverage.contains_column(column.0, column.1)
                && collar::Input::outside_columns(column,side).is_some_and(|columns| columns.into_iter()
                    .all(|(x,z)| !coverage.contains_column(x,z)))
        };
        let collect_dependencies = |column, dependencies: &mut Vec<Arc<collar::Installed>>| {
            dependencies.clear();
            let Some(members) = self.published_columns.get(&column) else {return false;};
            if members.is_empty() || members.len() > 64 {return false;}
            for q in members {
                let Some(mesh) = self.gpu_meshes.get(q) else {return false;};
                let Some(installed) = mesh.boundary_profile.as_ref() else {return false;};
                if installed.scope.coord != *q || installed.scope.lod != mesh.lod
                    || installed.scope.cache_epoch != self.gpu_world_epoch {return false;}
                dependencies.push(Arc::clone(installed));
            }
            true
        };
        let mut dependencies = Vec::new();
        if let Some(previous) = previous {
            let current = self.published_columns.get(&previous.column).is_some_and(|members| {
                members.len() == previous.dependencies.len()
                    && previous.dependencies.iter().all(|dep| members.contains(&dep.scope.coord)
                        && self.gpu_meshes.get(&dep.scope.coord).is_some_and(|mesh| {
                            mesh.lod == dep.scope.lod && mesh.boundary_profile.as_ref()
                                .is_some_and(|actual| Arc::ptr_eq(actual,dep))
                        }))
            });
            if owned_edge(previous.column,previous.side)
                && (preferred.is_empty() || preferred.contains(&previous.column)) {
                if current {return Some(Arc::clone(previous));}
                // Refresh the selected edge first. An identity-only replacement
                // must not switch sides and force an unrelated parent rebuild.
                if collect_dependencies(previous.column,&mut dependencies)
                    && let Some(input) = collar::Input::new(previous.column,previous.side,&dependencies) {
                        return Some(input);
                    }
            }
        }
        // Retain only the best 64 candidates instead of sorting every column.
        // The tuple preserves the previous deterministic priority ordering.
        let mut columns = BinaryHeap::with_capacity(64);
        for &(x,z) in self.published_columns.keys() {
            if !(0..4).any(|side| owned_edge((x,z),side)) {continue;}
            let key = (!preferred.contains(&(x,z)),
                i64::from(x).abs_diff(i64::from(self.center_chunk.x))
                    .saturating_add(i64::from(z).abs_diff(i64::from(self.center_chunk.z))),x,z);
            if columns.len() < 64 {columns.push(key);}
            else if let Some(mut worst) = columns.peek_mut()
                && key < *worst {*worst=key;}
        }
        for (_,_,x,z) in columns.into_sorted_vec() {
            let column = (x,z);
            if !collect_dependencies(column,&mut dependencies) {continue;}
            for side in 0..4 {
                if !owned_edge(column,side) {continue;}
                if let Some(input) = collar::Input::new(column,side,&dependencies) {
                    return Some(input);
                }
            }
        }
        None
    }

    pub fn desired_resident_coverage(&self, eye: Vec3, view_chunks: i32, scale: f32) -> super::coverage::ResidentCoverage {
        self.coverage_map(eye,view_chunks,scale,false)
    }
    fn coverage_map(&self, eye: Vec3, view_chunks: i32, scale: f32, include_pins: bool) -> super::coverage::ResidentCoverage {
        let mut result = super::coverage::ResidentCoverage::default();
        if !eye.is_finite() || !scale.is_finite() || scale <= 0.0 { return result; }
        let radius = view_chunks.clamp(1,64);
        // Use the draw-list centre, which can differ from an interpolated
        // render eye at a chunk boundary. The mask must describe actual draws.
        let cx = self.center_chunk.x;
        let cz = self.center_chunk.z;
        result.header = [cx.saturating_sub(64), cz.saturating_sub(64), 129, 1];
        let mut columns: HashMap<(i32,i32),bool> = HashMap::new();
        for q in if include_pins { &self.published_coords } else { &self.visible_coords } {
            let ready = self.gpu_meshes.contains_key(q);
            columns.entry((q.x,q.z)).and_modify(|v| *v &= ready).or_insert(ready);
        }
        let mut sparse = Vec::with_capacity(self.horizon_pins.len());
        for ((x,z), ready) in columns {
            let dx = i64::from(x) - i64::from(cx);
            let dz = i64::from(z) - i64::from(cz);
            let r = i64::from(radius);
            let nearby = dx.abs() <= r && dz.abs() <= r && dx*dx + dz*dz <= r*r;
            if !ready || !(nearby || (include_pins && (self.horizon_pins.contains(&(x,z)) || self.edit_proxies.admitted((x,z))))) { continue; }
            if result.in_dense_window(x,z) {
                // This branch cannot exhaust sparse capacity.
                let inserted = result.set_column(x,z);
                debug_assert!(inserted);
            } else {
                sparse.push((x,z));
            }
        }
        // Linear-probe collision placement must not depend on HashMap iteration:
        // byte-identical owners keep horizon/shadow invalidation stable.
        sparse.sort_unstable();
        for (x,z) in sparse {
            if !result.set_column(x,z) {
                // Every valid outside column is one of at most 512 admitted
                // pins. A broken invariant must not publish a partial mask.
                eprintln!("[resident-coverage] rejected incomplete owner map at ({x},{z}); sparse capacity={}", super::coverage::MAX_SPARSE_COLUMNS);
                let header = result.header;
                result = super::coverage::ResidentCoverage::default();
                result.header = header; // Enabled empty: no radial fallback.
                return result;
            }
        }
        result
    }
    pub fn set_horizon_pins(&mut self, mut wanted: Vec<(i32,i32)>) {
        self.refresh_published_columns();
        // Existing GPU allocations only. Reserve at most 1 GiB/512 columns for
        // last valid edited cover; speculative uploads cannot evict this set.
        wanted.sort_unstable_by_key(|&(x,z)| (!self.horizon_pins.contains(&(x,z)),
            (i64::from(x)-i64::from(self.center_chunk.x)).abs()+(i64::from(z)-i64::from(self.center_chunk.z)).abs(),x,z));
        wanted.dedup();
        let requested=wanted.len();
        let mut pins=FastSet::default(); let mut coords=FastSet::default();
        let mut reserved=self.edit_proxies.coords.clone();
        let mut bytes=reserved.iter().filter_map(|q|self.gpu_meshes.get(q)).map(|m|m.bytes).sum::<usize>();
        let proxy_columns=self.edit_proxies.columns().count(); let mut extra_pins=0usize;
        for column in wanted {
            let extra=!self.edit_proxies.wanted(&column);
            if proxy_columns+extra_pins+usize::from(extra)>edit_proxies::COLUMN_LIMIT { continue; }
            let required:Vec<_>=self.published_coords.iter().chain(self.horizon_pin_coords.iter())
                .filter(|q|(q.x,q.z)==column).copied().collect::<HashSet<_>>().into_iter().collect();
            if required.is_empty() || required.iter().any(|q| !self.gpu_meshes.contains_key(q)) { continue; }
            let size=required.iter().filter(|q|!reserved.contains(*q)).map(|q|self.gpu_meshes[q].bytes).sum::<usize>();
            if bytes.saturating_add(size)>1024*1024*1024 { continue; }
            bytes+=size; extra_pins+=usize::from(extra); pins.insert(column); reserved.extend(required.iter().copied()); coords.extend(required);
        }
        if pins == self.horizon_pins && coords == self.horizon_pin_coords { return; }
        if pins != self.horizon_pins {
            println!("[horizon-pins] columns={} chunks={} bytes={} requested={} deferred={} reserve_limit_bytes={}",pins.len(),coords.len(),bytes,requested,requested.saturating_sub(pins.len()),1024u64*1024*1024);
        }
        self.horizon_pins=pins; self.horizon_pin_coords=coords;
        self.rebuild_visible_sets();
    }
    pub fn has_horizon_pins(&self) -> bool { !self.horizon_pins.is_empty() || self.edit_proxies.columns().next().is_some() }
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
            pending_gpu: Arc::new(PendingGpuLedger::default()),
            gpu_world_epoch: 0,
            visible,
            visible_keys,
            visible_coords,
            published_coords:FastSet::default(),
            published_columns:FastMap::default(),
            publication_dirty:true,
            horizon_pins: FastSet::default(),
            horizon_pin_coords: FastSet::default(),
            edit_proxies: edit_proxies::EditProxies::default(),
            quality_proxies: quality_proxies::QualityProxies::default(),
            seeded_sources: HashSet::new(),
            gpu_meshes: FastMap::default(),
            arena: arena::MeshArena::new(arena_budget(residency::limits().1)),
            gpu_mesh_revision: 0,
            shadow_caster_revision: 0,
            shadow_casters: shadow_casters::Cache::default(),
            dirty_meshes: FastSet::default(),
            cpu_chunks: HashMap::new(),
            center_chunk,
            column_y_cache: ColumnPlans::new(),
            simulation_seeds: Vec::new(),
            expansion_radius: 0,
            fully_expanded: false,
            spawn_ready: false,
            coverage_stable_meters: -1.0,
            coverage_drop_since: None,
            coverage_last_tick: None,
            coverage_center: None,
        }
    }

    /// Conservative radius in metres where the visible terrain columns have a
    /// published GPU mesh. Missing columns are measured by the nearest point on
    /// their XZ bounds, then reduced by a safety margin.
    ///
    /// A single remeshing chunk is not a world edge. Taking the absolute nearest
    /// miss made one transient hole collapse the handoff to that column, hiding
    /// every resident chunk beyond it and pulling the low-poly backdrop right up
    /// to the player. Sparse misses are therefore treated as holes to fill, not
    /// as evidence that the whole ring ended. Several misses at the same distance
    /// still reduce coverage immediately.
    pub fn published_coverage_meters(&self, eye: Vec3, view_chunks: i32, scale: f32) -> f32 {
        if !eye.is_finite() || !scale.is_finite() || scale <= 0.0 {
            return 0.0;
        }
        let chunk_m = CHUNK_SIZE as f32 * scale;
        let radius = view_chunks.clamp(1, 64);
        let full_radius = radius as f32 * chunk_m;
        let center_x = (eye.x / chunk_m).floor() as i32;
        let center_z = (eye.z / chunk_m).floor() as i32;
        // A column counts as covered only when its highest required chunk has a
        // published mesh. Any lower mesh would otherwise overstate surface
        // coverage and let the backdrop start too far away.
        let mut top_by_column: HashMap<(i32, i32), i32> =
            HashMap::with_capacity(self.visible_coords.len());
        for coord in &self.visible_coords {
            let top = top_by_column.entry((coord.x, coord.z)).or_insert(coord.y);
            *top = (*top).max(coord.y);
        }
        let total_columns = ((radius * 2 + 1) * (radius * 2 + 1)) as usize;
        let mut missing = Vec::with_capacity(total_columns);
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let cx = center_x + dx;
                let cz = center_z + dz;
                let covered = top_by_column
                    .get(&(cx, cz))
                    .is_some_and(|top| self.gpu_meshes.contains_key(&IVec3::new(cx, *top, cz)));
                if covered {
                    continue;
                }
                let min_x = cx as f32 * chunk_m;
                let min_z = cz as f32 * chunk_m;
                let nx = eye.x.clamp(min_x, min_x + chunk_m);
                let nz = eye.z.clamp(min_z, min_z + chunk_m);
                missing.push(((nx - eye.x).powi(2) + (nz - eye.z).powi(2)).sqrt());
            }
        }
        robust_coverage_from_missing(&mut missing, full_radius, 8.0 * scale, total_columns)
    }

    /// Quantize coverage down to a coarse step so a slowly settling stream does
    /// not force a horizon rebuild every frame. Rounding down keeps the
    /// published radius a lower bound.
    ///
    /// A drop is held for a short grace period and then eased. A hole that is
    /// repaired by the next streaming result never moves the horizon at all.
    pub fn backdrop_coverage_meters(&mut self, eye: Vec3, view_chunks: i32, scale: f32) -> f32 {
        const QUANTUM: f32 = 64.0;
        const DROP_HOLD: Duration = Duration::from_secs(1);
        const DROP_RATE: f32 = 2.5;
        let now = Instant::now();
        let full = view_chunks.clamp(1, 64) as f32 * CHUNK_SIZE as f32 * scale;
        let raw =
            (self.published_coverage_meters(eye, view_chunks, scale) / QUANTUM).floor() * QUANTUM;
        let center = if scale.is_finite() && scale > 0.0 {
            Some(IVec3::new(
                (eye.x / (CHUNK_SIZE as f32 * scale)).floor() as i32,
                self.center_chunk.y,
                (eye.z / (CHUNK_SIZE as f32 * scale)).floor() as i32,
            ))
        } else {
            None
        };
        let teleported = self.coverage_center.is_some_and(|previous| {
            center.is_some_and(|current| {
                (current.x - previous.x)
                    .abs()
                    .max((current.z - previous.z).abs())
                    > 4
            })
        });
        if self.coverage_stable_meters < 0.0 || teleported {
            self.coverage_stable_meters = raw;
            self.coverage_drop_since = None;
        } else {
            let dt = self
                .coverage_last_tick
                .map(|last| now.saturating_duration_since(last).as_secs_f32())
                .unwrap_or(0.0)
                .clamp(0.0, 0.1);
            let stable = self.coverage_stable_meters.clamp(0.0, full);
            if raw >= stable {
                self.coverage_stable_meters = raw;
                self.coverage_drop_since = None;
            } else {
                let since = *self.coverage_drop_since.get_or_insert(now);
                if now.saturating_duration_since(since) >= DROP_HOLD {
                    // Ease toward the honest radius instead of slamming the
                    // horizon inward. Recovery is immediate because a wider
                    // stable radius never hides published terrain.
                    let eased = stable + (raw - stable) * (1.0 - (-DROP_RATE * dt).exp());
                    self.coverage_stable_meters = eased.min(stable).max(raw);
                } else {
                    self.coverage_stable_meters = stable;
                }
            }
        }
        self.coverage_last_tick = Some(now);
        self.coverage_center = center;
        (self.coverage_stable_meters / QUANTUM).floor() * QUANTUM
    }

    fn rebuild_visible_sets(&mut self) {
        self.visible_coords.clear();
        self.visible_coords.extend(self.visible.iter().map(|key|key.coord));
        for &coord in &self.horizon_pin_coords {
            if !self.visible_coords.contains(&coord)
                && let Some(mesh)=self.gpu_meshes.get(&coord) {
                    self.visible.push(ChunkKey { coord,lod:mesh.lod });
                    self.visible_coords.insert(coord);
                }
        }
        for index in 0..self.visible.len() {
            let coord=self.visible[index].coord;
            self.visible[index].lod = if self.edit_proxies.contains(&coord) { 0 } else if self.horizon_pin_coords.contains(&coord) {
                self.gpu_meshes.get(&coord).map_or_else(||self.desired_lod(coord),|mesh|mesh.lod)
            } else { self.desired_lod(coord) };
        }
        // The refresh below rebuilds `visible_keys` from these same sets.
        self.publication_dirty=true;
        self.refresh_published_columns();
    }

    fn refresh_published_columns(&mut self) {
        if !self.publication_dirty { return; }
        self.publication_dirty=false;
        let radius=render_distance_settings().view_distance_chunks.clamp(1,64);
        let mut desired:FastMap<(i32,i32),Vec<IVec3>>=FastMap::default();
        for &q in &self.visible_coords {
            let dx=i64::from(q.x)-i64::from(self.center_chunk.x);let dz=i64::from(q.z)-i64::from(self.center_chunk.z);
            if dx*dx+dz*dz<=i64::from(radius)*i64::from(radius) || self.horizon_pins.contains(&(q.x,q.z)) {
                desired.entry((q.x,q.z)).or_default().push(q);
            }
        }
        for (column,members) in self.quality_proxies.desired_columns() {
            desired.insert(column,members.to_vec());
        }
        for (column, members) in self.edit_proxies.desired_columns() {
            desired.insert(column, members.to_vec());
        }
        let mut columns_changed=false;
        self.published_columns.retain(|column,members|{
            let keep=desired.contains_key(column) && members.iter().all(|q|self.gpu_meshes.contains_key(q));
            columns_changed|=!keep;
            keep
        });
        for (column,mut members) in desired {
            let quality_only=self.quality_proxies.contains_column(&column);
            if members.iter().all(|q|self.gpu_meshes.get(q).is_some_and(|mesh| !self.edit_proxies.contains(q) || mesh.lod == 0))
                && (!quality_only || members.iter().all(|q|!self.dirty_meshes.contains(q)
                    && !self.edit_proxies.water_dirty.contains(q) && !self.pending_gpu.contains(q))) {
                members.sort_unstable_by_key(|q|(q.y,q.x,q.z));
                self.edit_proxies.mark_admitted(column);
                self.quality_proxies.mark_admitted(column);
                if self.published_columns.get(&column)!=Some(&members) {
                    columns_changed=true;
                    self.published_columns.insert(column,members);
                }
            }
        }
        self.edit_proxies.set_quality_admitted(self.quality_proxies.admitted_columns());
        // Rebuilding and comparing the coordinate set costs two hash operations
        // per published chunk. Columns are disjoint, so equal counts with no
        // column change mean the set is already current.
        let published_count:usize=self.published_columns.values().map(Vec::len).sum();
        let membership_changed=if columns_changed || published_count!=self.published_coords.len() {
            let published_coords=self.published_columns.values().flatten().copied().collect();
            let changed=self.published_coords!=published_coords;
            self.published_coords=published_coords;
            changed
        } else {
            false
        };
        self.visible_keys.clear();
        self.visible_keys.extend(self.visible.iter().copied());
        self.visible_keys.extend(self.edit_proxies.coords.iter().map(|&coord| ChunkKey { coord, lod: 0 }));
        for &coord in &self.published_coords {
            let key=ChunkKey {coord,lod:self.desired_lod(coord)};
            self.visible_keys.insert(key);
        }
        if membership_changed { self.mark_mesh_set_changed(); }
    }
    fn source_interested(&self,q:&IVec3)->bool { self.visible_coords.contains(q)||self.published_coords.contains(q)||self.edit_proxies.contains(q) }

    fn desired_lod(&self, coord: IVec3) -> u8 {
        if self.edit_proxies.contains(&coord) { return 0; }
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
        self.shadow_casters.clear();
        self.gpu_world_epoch = self.gpu_world_epoch.wrapping_add(1);
        // Outstanding upload leases remain charged until the streamer drops them.
        self.edit_proxies=edit_proxies::EditProxies::default(); self.quality_proxies=quality_proxies::QualityProxies::default(); self.seeded_sources.clear();
        self.published_columns.clear();self.published_coords.clear();
        self.publication_dirty=true;
        self.horizon_pins.clear(); self.horizon_pin_coords.clear();
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
            self.mark_mesh_set_changed();
        }
        self.cpu_chunks.clear();
        self.column_y_cache.clear();
        self.simulation_seeds.clear();
        self.expansion_radius = 0;
        self.fully_expanded = false;
        self.coverage_stable_meters = -1.0;
        self.coverage_drop_since = None;
        self.coverage_last_tick = None;
        self.coverage_center = None;
    }

    pub fn reset_with_snapshot(&mut self, center_chunk: IVec3, snapshot: &WorldSnapshot) {
        self.shadow_casters.clear();
        self.gpu_world_epoch = self.gpu_world_epoch.wrapping_add(1);
        // Outstanding upload leases remain charged until the streamer drops them.
        self.edit_proxies=edit_proxies::EditProxies::default(); self.quality_proxies=quality_proxies::QualityProxies::default(); self.seeded_sources.clear();
        self.published_columns.clear(); self.published_coords.clear(); self.publication_dirty=true;
        self.horizon_pins.clear(); self.horizon_pin_coords.clear();
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
            self.mark_mesh_set_changed();
        }
        self.cpu_chunks.clear();
        self.coverage_stable_meters = -1.0;
        self.coverage_drop_since = None;
        self.coverage_last_tick = None;
        self.coverage_center = None;
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
        for (c, range) in self.column_y_cache.completed() {
            let dx = c.0 - self.center_chunk.x;
            let dz = c.1 - self.center_chunk.z;
            let current_limit = self.expansion_radius;
            if dx * dx + dz * dz > current_limit * current_limit {
                continue;
            }
            for mut key in visibility::keys_for_column(self.center_chunk, c, range, &snapshot.cfg) {
                key.lod = self.desired_lod(key.coord);
                if self.visible_coords.insert(key.coord) {
                    self.visible_keys.insert(key);
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
            if self.visible_coords.insert(key.coord) {
                self.visible_keys.insert(key);
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
        self.gpu_meshes.retain(|coord, _| is_in_range(coord) || self.horizon_pin_coords.contains(coord) || self.published_coords.contains(coord) || self.edit_proxies.contains(coord));
        self.gpu_bytes = self.gpu_meshes.values().map(|mesh| mesh.bytes).sum();
        self.lod_meshes.retain(|coord| is_in_range(&coord));
        if self.gpu_meshes.len() != mesh_count_before {
            self.mark_mesh_set_changed();
        }
        self.cpu_chunks.retain(|coord, _| is_in_range(coord) || self.horizon_pin_coords.contains(coord) || self.published_coords.contains(coord) || self.edit_proxies.contains(coord));
        self.dirty_meshes.retain(|q| is_in_range(q) || self.horizon_pin_coords.contains(q) || self.published_coords.contains(q) || self.edit_proxies.contains(q));
        self.seeded_sources.retain(|q| is_in_range(q) || self.edit_proxies.contains(q));
        self.column_y_cache
            .retain(|&(x, z), _| (x - center.x).abs().max((z - center.z).abs()) <= hard_limit || self.edit_proxies.wanted(&(x,z)));
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

    fn mark_mesh_set_changed(&mut self) {
        self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
        self.shadow_caster_revision = self.shadow_caster_revision.wrapping_add(1);
    }

    fn install_mesh(&mut self, coord: IVec3, mesh: ChunkGpuMesh) {
        self.install_mesh_with_casters(coord, mesh, shadow_casters::Prepared::default());
    }

    /// The publication transaction calls this only after GPU validation succeeds.
    /// Vertex and index buffers of one arena page.
    pub fn page_buffers(&self, page: u32) -> Option<(&wgpu::Buffer, &wgpu::Buffer)> {
        self.arena.buffers(page)
    }

    /// Buffers and ranges for drawing the resident mesh at `coord`, if it has geometry.
    pub fn draw_slice(&self, coord: IVec3) -> Option<DrawSlice<'_>> {
        let mesh = self.gpu_meshes.get(&coord)?;
        let range = *mesh.alloc.as_ref()?.range();
        let (vertices, indices) = self.arena.buffers(range.page)?;
        let first = range.first_index();
        Some(DrawSlice {
            page: range.page,
            vertices,
            indices,
            index_range: first..first + mesh.index_count,
            base_vertex: range.base_vertex(),
        })
    }

    fn install_mesh_with_casters(
        &mut self,
        coord: IVec3,
        mesh: ChunkGpuMesh,
        prepared: shadow_casters::Prepared,
    ) {
        self.install_mesh_unpublished(coord, mesh, prepared);
        self.refresh_published_columns();
    }

    /// Installs without refreshing column publication. A batch of installs
    /// ends with one `refresh_published_columns`: each refresh walks every
    /// visible chunk (about 1 ms at 4.8k), and none of the installs reads it.
    fn install_mesh_unpublished(
        &mut self,
        coord: IVec3,
        mesh: ChunkGpuMesh,
        prepared: shadow_casters::Prepared,
    ) {
        let same_lod = self.gpu_meshes.get(&coord).is_some_and(|old| old.lod == mesh.lod);
        let unchanged_casters = self.shadow_casters.install(coord, mesh.lod, same_lod, prepared);
        // Column readiness depends on buffer presence and LOD, not geometry
        // contents. Replacing an existing representation keeps the same owners.
        let ownership_changed = !same_lod;
        let edited_source = self.dirty_meshes.remove(&coord);
        drop(self.lod_meshes.take(ChunkKey {
            coord,
            lod: mesh.lod,
        }));
        self.gpu_bytes += mesh.bytes;
        let lod = mesh.lod;
        if let Some(previous) = self.gpu_meshes.insert(coord, mesh) {
            self.gpu_bytes -= previous.bytes;
            // A different LOD is reusable only while its source/halo is unchanged.
            // Dirty invalidation already removed inactive siblings; do not put
            // the old edited source back into that cache during replacement.
            if !edited_source && previous.lod != lod && !previous.lod_invariant {
                self.lod_meshes.remember(coord, previous);
            }
        }
        self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
        if !unchanged_casters {
            self.shadow_caster_revision = self.shadow_caster_revision.wrapping_add(1);
        }
        self.publication_dirty |= ownership_changed;
        self.trim_lod_budget();
    }

    fn activate_lod(&mut self, key: ChunkKey) -> bool {
        if !self.dirty_meshes.contains(&key.coord)
            && let Some(mesh) = self.gpu_meshes.get_mut(&key.coord)
                && (mesh.lod_invariant || mesh.lod == key.lod) {
                    let ownership_changed = mesh.lod != key.lod;
                    if ownership_changed {
                        mesh.boundary_profile = mesh.boundary_profile.as_ref()
                            .and_then(|profile| profile.relabel_empty_lod(key.lod));
                    }
                    mesh.lod = key.lod;
                    if ownership_changed {
                        self.publication_dirty = true;
                        self.refresh_published_columns();
                    }
                    return true;
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
    pub world_epoch: u64,
    pub cache_epoch: u64,
    pub mesh: ChunkMesh,
    shadow_casters: shadow_casters::Prepared,
    boundary_profile: collar::Prepared,
    connectivity: Option<crate::mesh::ChunkConnectivity>,
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
    world_epoch: u64,
    cache_epoch: u64,
    chunk: Arc<ResidentChunk>,
    shadow_baseline: Option<shadow_casters::Baseline>,
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
                    let world_epoch = job.world_epoch;
                    let cache_epoch = job.cache_epoch;
                    let shadow_baseline = job.shadow_baseline;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        if job.cancelled.load(Ordering::Relaxed) {
                            return None;
                        }
                        let planning = budget.begin_plan(&job.cancelled)?;
                        let mut lease = budget.acquire(PLANNING_BYTES, &job.cancelled)?;
                        if job.key.lod > 0 {
                            let water_bytes=crate::mesh::exact_water_mesh_bytes_upper_bound(job.chunk.as_ref());
                            lease.grow_to(PLANNING_BYTES.saturating_add(water_bytes), &job.cancelled)?;
                            drop(planning);
                            if job.cancelled.load(Ordering::Relaxed) {
                                return None;
                            }
                            let mut mesh =
                                crate::mesh::coarsened_mesh(job.chunk.as_ref(), job.key.lod);
                            crate::mesh::encode_water(job.chunk.as_ref(), &mut mesh);
                            let lod_invariant = mesh.vertices.is_empty();
                            drop(job.chunk);
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
                            let shadow_casters = shadow_casters::Prepared::from_mesh(
                                &mesh, shadow_baseline.as_ref(), &job.cancelled);
                            let boundary_profile = collar::Prepared::from_mesh(key.coord, &mesh, &job.cancelled);
                            return Some(CpuChunkMesh {
                                key,
                                revision,
                                job_id, world_epoch, cache_epoch,
                                mesh,
                                shadow_casters,
                                boundary_profile,
                                connectivity: None,
                                failed: false,
                                lod_invariant,
                                _lease: Some(lease),
                            });
                        }
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
                        // The flood fill feeds only cave culling; skip it while culling is off.
                        let connectivity = if occlusion::enabled() {
                            crate::mesh::ChunkConnectivity::compute(&job.chunk, &job.cancelled)
                        } else {
                            Some(crate::mesh::ChunkConnectivity::ALL_OPEN)
                        };
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
                        let shadow_casters = shadow_casters::Prepared::from_mesh(
                            &mesh, shadow_baseline.as_ref(), &job.cancelled);
                            let boundary_profile = collar::Prepared::from_mesh(key.coord, &mesh, &job.cancelled);
                        Some(CpuChunkMesh {
                            key,
                            revision,
                            job_id, world_epoch, cache_epoch,
                            mesh,
                            shadow_casters,
                            boundary_profile,
                            connectivity,
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
                            job_id, world_epoch, cache_epoch,
                            mesh: ChunkMesh::empty(),
                            shadow_casters: shadow_casters::Prepared::default(),
                            boundary_profile: collar::Prepared::default(),
                            connectivity: None,
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

// Admission uses the same cover lane and whole-column age as dispatch/upload.
// The existing radial key remains the fallback within each lane.
type PendingSourcePriority = (
    u8,
    Option<(Instant, i32, i32)>,
    (i32, i32, i32, i32, i32, i32, i32, u8),
);

pub struct ChunkStreamer {
    pending_uploads: Vec<PendingGpuUpload>,
    pending_gpu_coords: HashMap<IVec3,u64>,
    upload_epoch: u64,
    next_upload_token: u64,
    upload_retry_after: Instant,
    upload_error: Option<String>,
    worker_retry_after: Instant,
    mesh_budget: Arc<MeshBudget>,
    requested: HashSet<ChunkKey>,
    /// Coord-only set for O(1) "is any job in-flight for this coord?" checks.
    /// Kept in sync with `requested` and `meshing`.
    requested_coords: HashSet<IVec3>,
    /// Latest edit while a mesh is being built. Publish that job before retrying
    /// so continuously simulated chunks cannot starve their first visible mesh.
    dirty_revisions: HashMap<IVec3, u64>,
    source_observations: local_sources::Observations,
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
    /// Re-keyed before admission by missing ordinary cover, then oldest whole
    /// incomplete proxy column, with radial distance as the fallback.
    pending_schedule: BinaryHeap<Reverse<PendingSourcePriority>>,
    pending_keys: HashSet<ChunkKey>,
    pending_schedule_center: IVec3,
    urgent_coords: HashSet<IVec3>,
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
            pending_uploads: Vec::new(),
            pending_gpu_coords: HashMap::new(),
            upload_epoch: 0,
            next_upload_token: 0,
            dirty_revisions: HashMap::new(),
            source_observations: local_sources::Observations::default(),
            upload_retry_after: Instant::now(),
            upload_error: None,
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
            urgent_coords: HashSet::new(),
            repair_cursor: 0,
            last_stats: ChunkStreamerStats::default(),
        })
    }

    pub fn reset(&mut self) {
        self.urgent_coords.clear();
        self.dirty_revisions.clear();
        self.source_observations=local_sources::Observations::default();
        self.upload_epoch=self.upload_epoch.wrapping_add(1);
        self.pending_uploads.clear();
        self.pending_gpu_coords.clear();
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
            self.pending_schedule.push(Reverse((2, None, pending_priority(center, key, self.urgent_coords.contains(&key.coord)).0)));
        }
    }

    fn push_pending_key(&mut self, key: ChunkKey) {
        if self.pending_keys.insert(key) {
            self.pending_schedule
                .push(Reverse((2, None, pending_priority(self.pending_schedule_center, key, self.urgent_coords.contains(&key.coord)).0)));
        }
    }

    fn source_pending_priority(cache: &ChunkCache, key: ChunkKey, urgent: bool) -> Reverse<PendingSourcePriority> {
        let (lane, column, _) = Self::ready_chunk_priority(cache, key);
        Reverse((lane, column, pending_priority(cache.center_chunk, key, urgent).0))
    }

    fn reprioritize_pending_sources(&mut self, cache: &ChunkCache) {
        // Reuse the heap allocation. Membership is unchanged, and priorities
        // reflect admission/published-cover changes even at a stationary camera.
        let mut entries = std::mem::take(&mut self.pending_schedule).into_vec();
        for entry in &mut entries {
            let (_, _, (_, _, _, _, x, y, z, lod)) = entry.0;
            let key = ChunkKey { coord: IVec3::new(x, y, z), lod };
            *entry = Self::source_pending_priority(cache, key, self.urgent_coords.contains(&key.coord));
        }
        self.pending_schedule = BinaryHeap::from(entries);
    }

    fn source_admission_has_capacity(&self) -> bool {
        self.ready_chunks.len() < MAX_READY_CHUNKS
            && self.ready_meshes.len() < MAX_READY_MESHES
            && self.meshing.len() < MAX_MESH_JOBS
            && self.generating.len() < MAX_GENERATING
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

    fn ready_chunk_priority(cache: &ChunkCache, key: ChunkKey) -> (u8, Option<(Instant, i32, i32)>, (u8, u8, i32, i32, i32, i32)) {
        let pending_since = cache.edit_proxies.pending_since(&key.coord);
        let quality_since=cache.quality_proxies.pending_since(&key.coord);
        // Uncovered ordinary columns keep emergency priority. An added Y member
        // over an existing complete published bundle is repair, not lost cover.
        let missing_normal_cover = cache.ordinary_source_interested(key.coord)
            && !cache.gpu_meshes.contains_key(&key.coord)
            && !cache.published_columns.contains_key(&(key.coord.x, key.coord.z));
        let persistent_dirty=cache.edit_proxies.persistent_contains(&key.coord) && cache.dirty_meshes.contains(&key.coord);
        let ordinary_dirty=cache.ordinary_source_interested(key.coord) && cache.dirty_meshes.contains(&key.coord);
        let lane = if missing_normal_cover {0} else if pending_since.is_some() || persistent_dirty {1}
            else if ordinary_dirty {2} else if quality_since.is_some() {3} else {4};
        let mesh_state = match cache.gpu_meshes.get(&key.coord) {
            _ if cache.dirty_meshes.contains(&key.coord) => 0u8,
            _ if cache.edit_proxies.contains(&key.coord) => 1u8,
            None => 2u8,
            Some(mesh) if mesh.lod > key.lod => 3u8,
            _ => 4u8,
        };
        let base = chunk_sort_key(cache.center_chunk, key);
        // One update can create several columns at the same Instant. Keep the
        // column identity ahead of distance/Y so a tie cannot interleave bundles.
        let pending_column = pending_since.or(quality_since).map(|since| (since, key.coord.x, key.coord.z));
        (lane, pending_column, (mesh_state, base.0, base.1, base.2, base.3, base.4))
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
        for key in &cache.visible_keys {
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

    pub fn retain_visible<S: std::hash::BuildHasher>(&mut self, visible: &HashSet<ChunkKey, S>) {
        let coords: HashSet<_> = visible.iter().map(|key| key.coord).collect();
        self.dirty_revisions
            .retain(|coord, _| coords.contains(coord) || self.pending_gpu_coords.contains_key(coord));
        // Geometry detail changes do not invalidate source voxels or a reusable LOD.
        self.requested.retain(|key| coords.contains(&key.coord) || self.pending_gpu_coords.contains_key(&key.coord));
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
        self.requested.retain(|key| in_range(&key.coord) || self.pending_gpu_coords.contains_key(&key.coord));
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
            .retain(|key| !dirty_coords.contains(&key.coord) || self.pending_gpu_coords.contains_key(&key.coord));
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
            if cpu_mesh.world_epoch != self.upload_epoch || self.meshing.get(&cpu_mesh.key).map(|(id, _)| *id) != Some(cpu_mesh.job_id) {
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
            if !cache.source_interested(&job.key.coord) {
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
                world_epoch: self.upload_epoch,
                cache_epoch: cache.gpu_world_epoch,
                chunk,
                shadow_baseline: if cache.gpu_meshes.get(&job.key.coord).is_some_and(|m| m.lod == job.key.lod) {
                    cache.shadow_casters.baseline(job.key.coord, job.key.lod)
                } else { None },
                cancelled: cancelled.clone(),
            }) {
                Ok(()) => {
                    self.meshing.insert(job.key, (job_id, cancelled));
                    dispatched += 1;
                    crate::egui_ui::map::note_chunk(job.key.coord);
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
        let inflight_dirty: HashSet<_> = dirty_coords
            .iter()
            .copied()
            .filter(|coord| self.requested_coords.contains(coord))
            .collect();
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
            if !cache.source_interested(&coord) {
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
                    let deficit = snapshot.water_levels.deficit(block);
                    if deficit > 0 { chunk_data.water_deficit.insert(block,deficit); }
                    else { chunk_data.water_deficit.remove(&block); }
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
        self.invalidate_changed_sources(snapshot,cache);
        cache.sync_residency_budget();
        let water_dirty = std::mem::take(&mut cache.edit_proxies.water_dirty);
        if !water_dirty.is_empty() {
            self.rebuild_dirty(snapshot, cache, &HashSet::new(), &water_dirty, revision);
        }
        let urgent: HashSet<_> = cache.dirty_meshes.iter().chain(cache.edit_proxies.coords.iter()).copied().collect();
        if urgent != self.urgent_coords {
            self.urgent_coords = urgent;
            self.rebuild_pending_schedule(cache.center_chunk);
        }
        self.collect_finished_meshes();
        let mut terrain_to_schedule = Vec::new();
        let mut terrain_generated = 0usize;

        for (key, _job_revision, chunk_data, activity, seed_only, seed_activity) in self
            .terrain_generator
            .collect_finished(&gpu.device, snapshot)
        {
            if seed_only {
                if cache.ordinary_source_interested(key.coord) && cache.seeded_sources.insert(key.coord) {
                    cache.edit_proxies.normal_promotions += 1;
                    if let Some(mask) = activity { cache.simulation_seeds.push((key.coord, mask)); }
                }
                continue;
            }
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
            if !cache.source_interested(&key.coord) {
                self.requested.remove(&key);
                self.requested_coords.remove(&key.coord);
                continue;
            }
            if !seed_activity { cache.edit_proxies.source_only_completed += 1; }
            if seed_activity && cache.ordinary_source_interested(key.coord) && cache.seeded_sources.insert(key.coord)
                && let Some(mask) = activity { cache.simulation_seeds.push((key.coord, mask)); }
            // A uniform volume INCLUDING its halo exposes no faces at any
            // detail level. Bypass source residency, workers and mesh scratch.
            let block = chunk_data.blocks[0];
            if (block.is_empty()
                || block.is_opaque()
                || (block == crate::world::BlockId::Water && chunk_data.water_levels.is_empty()
                    && chunk_data.water_deficit.is_empty()))
                && chunk_data.blocks.iter().all(|&b| b == block)
                && self.ready_meshes.len() < MAX_READY_MESHES
            {
                self.ready_meshes.push_back(CpuChunkMesh {
                    failed: false,
                    lod_invariant: true,
                    key,
                    revision,
                    job_id: 0,
                    world_epoch: self.upload_epoch,
                    cache_epoch: cache.gpu_world_epoch,
                    mesh: ChunkMesh::empty(),
                    shadow_casters: shadow_casters::Prepared::default(),
                            boundary_profile: collar::Prepared::Empty,
                    connectivity: Some(crate::mesh::ChunkConnectivity::uniform(block)),
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
        crate::egui_ui::map::absorb(
            &snapshot.regions.root,
            snapshot.cfg.voxel_scale.meters(),
            |c| cache.cpu_chunks.get(&c).map(|s| &***s),
            engine_core::Budget::wall(Duration::from_micros(600)),
        );

        // Drain the pending queue rather than scanning all visible chunks each frame.
        // pending_schedule is populated by push_pending() when expand_visible() adds
        // new rings, so on steady-state frames it's empty and this loop is O(0).
        let schedule_budget = self.dynamic_schedule_budget();
        let mut scheduled = 0usize;
        let mut waiting_regions = Vec::new();
        // Limit how many entries we scan from the pending queue per frame so we
        // don't stall when a large ring arrives at once.
        if self.source_admission_has_capacity() && !self.pending_schedule.is_empty() {
            self.reprioritize_pending_sources(cache);
        }
        let scan_limit = self.pending_schedule.len().min(schedule_budget * 4);
        let mut scanned = 0usize;
        while scheduled < schedule_budget && scanned < scan_limit {
            if !self.source_admission_has_capacity() {
                break;
            }
            let Some(Reverse((_, _, (_, _, _, _, kx, ky, kz, klod)))) = self.pending_schedule.pop() else {
                break;
            };
            let key = ChunkKey {
                coord: IVec3::new(kx, ky, kz),
                lod: klod,
            };
            self.pending_keys.remove(&key);
            scanned += 1;
            if self.source_observations.blocked && cache.edit_proxies.contains(&key.coord) {continue;}

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
            .schedule_many(gpu, terrain_to_schedule.into_iter().map(|key| (key, cache.ordinary_source_interested(key.coord))), snapshot.cfg, revision);
        // A render-only source may already have its final GPU mesh when it
        // enters ordinary interest. Discover on workers once, without forcing
        // a remesh or merging proxy-generated activity into the simulation.
        let promotions: Vec<_> = cache.visible.iter().copied().filter(|key| {
            cache.ordinary_source_interested(key.coord)
                && cache.gpu_meshes.contains_key(&key.coord)
                && !cache.seeded_sources.contains(&key.coord)
                && !self.terrain_generator.seed_pending(key.coord)
                && !self.requested_coords.contains(&key.coord)
        }).take(self.terrain_generator.available_slots().min(4)).collect();
        for key in promotions {
            self.terrain_generator.schedule_seed(key, snapshot.cfg, revision, cache.cpu_chunks.get(&key.coord).cloned());
        }
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
        snapshot: &WorldSnapshot,
        cache: &mut ChunkCache,
        current_revision: u64,
        max_uploads: usize,
    ) {
        self.invalidate_changed_sources(snapshot,cache);
        cache.sync_residency_budget();
        let started=Instant::now();
        let (published,vertices,indices)=self.poll_pending_uploads(&gpu.device,&gpu.device_lost,cache,max_uploads,started);
        self.last_stats.meshes_uploaded+=published;
        self.last_stats.uploaded_vertices+=vertices;
        self.last_stats.uploaded_indices+=indices;
        if !gpu.device_usable() {self.upload_error=Some("GPU device loss reported".into());return;}
        // Runs between frames (never while an encoder is open), so ranges freed
        // since the last submit can be reused for this frame's writes.
        cache.arena.begin_frame();
        let mut staged=0usize;let mut staged_vertices=0usize;let mut staged_indices=0usize;
        self.collect_finished_meshes();self.reprioritize_ready_meshes(cache);
        while gpu.device_usable() && Instant::now()>=self.upload_retry_after && staged<max_uploads
            && self.pending_uploads.len()<16
            && ((staged==0 && published==0) || started.elapsed()<UPLOAD_TIME_BUDGET) {
            let Some(cpu_mesh)=self.ready_meshes.pop_front() else {break;};
            if self.pending_gpu_coords.contains_key(&cpu_mesh.key.coord) {
                self.ready_meshes.push_back(cpu_mesh);break;
            }
            let key=cpu_mesh.key;
            let mesh_vertices=cpu_mesh.mesh.vertices.len();let mesh_indices=cpu_mesh.mesh.indices.len();
            if staged+published>0 && (vertices+staged_vertices+mesh_vertices>CHUNK_UPLOAD_VERTEX_BUDGET
                || indices+staged_indices+mesh_indices>CHUNK_UPLOAD_INDEX_BUDGET) {
                self.ready_meshes.push_front(cpu_mesh);break;
            }
            if !self.cpu_world_current(cache,&cpu_mesh) || !cache.source_interested(&key.coord) {
                self.retire_upload_request(key);continue;
            }
            let _=current_revision; // unrelated edits do not stale this dependency halo
            let desired_lod=cache.desired_lod(key.coord);
            if edited_mesh_is_stale(self.dirty_revisions.get(&key.coord).copied(),cpu_mesh.revision) {
                self.retire_upload_request(key);cache.dirty_meshes.insert(key.coord);
                self.push_pending_key(ChunkKey{coord:key.coord,lod:desired_lod});continue;
            }
            if desired_lod!=key.lod && cache.gpu_meshes.get(&key.coord)
                .is_some_and(|m|m.lod==desired_lod&&!cache.dirty_meshes.contains(&key.coord)) {
                self.retire_upload_request(key);continue;
            }
            let bytes=mesh_vertices*std::mem::size_of::<crate::mesh::Vertex3>()+mesh_indices*4;
            if let Err(error)=gpu_upload::supported(&gpu.device,&cpu_mesh.mesh) {
                eprintln!("Chunk GPU upload rejected: {error}");self.upload_error=Some(error);
                cache.budget_deferred.insert(key,usize::MAX);self.retire_upload_request(key);continue;
            }
            let Some(reservation)=cache.reserve_pending_gpu(key,bytes) else {
                self.retire_upload_request(key);continue;
            };
            // A full arena is a budget reject like the reservation above, not an upload error.
            let Ok(alloc)=cache.arena.alloc_mesh(&gpu.device,&cpu_mesh.mesh) else {
                drop(reservation);self.retire_upload_request(key);continue;
            };
            let(attempt,check)=gpu_upload::start(&gpu.device,&gpu.queue,&cpu_mesh.mesh,key,
                cpu_mesh.lod_invariant,&cache.arena,alloc);
            self.begin_pending_upload(cpu_mesh,attempt,check,reservation,cache.gpu_world_epoch);
            staged+=1;staged_vertices+=mesh_vertices;staged_indices+=mesh_indices;
        }
        self.last_stats.cpu_resident_bytes =
            cache.cpu_bytes() + self.terrain_generator.buffer_bytes();
        self.last_stats.cpu_budget_bytes = cache.residency_limits.0;
        self.last_stats.gpu_budget_bytes = cache.residency_limits.1;
        self.last_stats.budget_deferred = cache.budget_deferred.len();
        self.last_stats.cpu_evictions = cache.cpu_evictions;
        self.last_stats.gpu_evictions = cache.gpu_evictions;
        self.last_stats.gpu_resident_bytes = cache.gpu_bytes + cache.lod_meshes.bytes() + cache.pending_gpu.bytes();
        gpu_budget::observe_residency(
            self.last_stats.gpu_resident_bytes,
            cache.budget_deferred.len(),
        );
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
            + self.pending_uploads.len()
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::robust_coverage_from_missing;

    #[test]
    fn isolated_near_miss_does_not_collapse_horizon_coverage() {
        // Four far fence posts justify the real handoff; one remeshing chunk
        // next to the player must not drag it inward.
        let mut missing = [40.0, 600.0, 600.0, 600.0, 600.0];
        let coverage = robust_coverage_from_missing(&mut missing, 896.0, 8.0, 169);
        assert_eq!(coverage, 592.0);
    }

    #[test]
    fn complete_ring_uses_the_published_radius() {
        let mut missing = [];
        assert_eq!(
            robust_coverage_from_missing(&mut missing, 896.0, 8.0, 169),
            896.0
        );
    }

    #[test]
    fn a_contiguous_near_sector_still_reduces_coverage() {
        let mut missing = vec![40.0; 20];
        missing.extend([600.0; 20]);
        let coverage = robust_coverage_from_missing(&mut missing, 896.0, 8.0, 169);
        assert_eq!(coverage, 32.0);
    }
}

fn apply_snapshot_edits_cubic(chunk_data: &mut GeneratedChunk, snapshot: &WorldSnapshot) {
    let min = chunk_data.origin();
    let max = min + IVec3::splat(PADDED_CHUNK_SIZE);
    chunk_data.water_deficit = snapshot.water_levels.precision.in_box(min,max).collect();
    chunk_data.water_levels = snapshot
        .water_levels
        .in_box(min, max)
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
