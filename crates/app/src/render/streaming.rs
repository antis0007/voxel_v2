//! Chunk residency, prioritized terrain generation, bounded meshing, and GPU upload.
//! Job IDs isolate cancelled work from replacements, including across world resets.
mod lod_cache;
mod shadow_casters;
use super::horizon::{collar, collar_set};
mod mesh_budget;
pub use mesh_budget::{mesh_budget_mib, set_mesh_budget_mib};
mod gpu_budget;
mod residency;
pub(super) use gpu_budget::configure_gpu_residency;
pub use gpu_budget::{MAX_GPU_MIB, gpu_residency_limit_mib, gpu_residency_status};
use mesh_budget::{MeshBudget, MeshLease};
pub(super) use residency::ResidentChunk;
use residency::{VoxelAllocations, arena_budget};
pub use residency::{set_world_residency_budgets_mib, world_residency_budgets_mib};
mod columns;
mod arena;
mod retirement;
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
mod player_edits;
mod quality_proxies;
#[cfg(test)]
mod pin_tests;
fn patch_source_halo(chunk_data: &mut ResidentChunk, edits: Option<&Vec<IVec3>>, snapshot: &WorldSnapshot) {
    for &block in edits.into_iter().flatten() {
        let local = block - chunk_data.origin();
        if local.min_element() >= 0 && local.max_element() < PADDED_CHUNK_SIZE {
            let before = if chunk_data.get_world(block) == crate::world::BlockId::Water {
                chunk_data.water_fill(block)
            } else { 0.0 };
            chunk_data.set_world(block, snapshot.get(block));
            if let Some(&volume) = snapshot.water_levels.get(&block) {
                chunk_data.water_levels.insert(block, volume);
            } else {
                chunk_data.water_levels.remove(&block);
            }
            let deficit = snapshot.water_levels.deficit(block);
            if deficit > 0 { chunk_data.water_deficit.insert(block,deficit); }
            else { chunk_data.water_deficit.remove(&block); }
            let after = if chunk_data.get_world(block) == crate::world::BlockId::Water {
                chunk_data.water_fill(block)
            } else { 0.0 };
            if before != after { chunk_data.water_motion.record(block, before); }
        }
    }
    chunk_data.refresh_allocation_size();
}

#[cfg(test)]
mod altitude_tests;
#[cfg(test)]
mod lod_band_tests;
/// Level to keep for a resident mesh, given the desired level and the inner neighbour's.
/// Never coarser than desired, so an approaching chunk refines on entering its ring;
/// finer survives one ring outward, so a receding chunk does not flip back and forth.
fn resident_lod(resident: u8, desired: u8, inner: u8) -> u8 {
    if resident <= desired && resident >= inner { resident } else { desired }
}
fn edited_mesh_is_stale(last_edit: Option<u64>, mesh_revision: u64) -> bool {
    last_edit.is_some_and(|edited| edited > mesh_revision)
}

fn older_than_display(cache: &ChunkCache, coord: IVec3, revision: u64) -> bool {
    cache
        .gpu_meshes
        .get(&coord)
        .is_some_and(|mesh| mesh.source_revision > revision)
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
pub use visibility::{DrawScratch, build_draw_list, build_draw_list_with_coverage, chunk_bounds_sphere, extract_planes};
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
const CHUNK_GENERATION_INTEGRATE_TIME: Duration = Duration::from_millis(2);
const CHUNK_MESH_COLLECT_BUDGET: usize = 16;
const CHUNK_UPLOAD_SCAN_BUDGET: usize = 16;
const CHUNK_UPLOAD_VERTEX_BUDGET: usize = 350_000;
const CHUNK_UPLOAD_INDEX_BUDGET: usize = 1_200_000;
const MAX_GENERATING: usize = 64;
const MAX_MESH_JOBS: usize = 64;
const MAX_READY_CHUNKS: usize = 128;
const MAX_READY_MESHES: usize = 64;
const UPLOAD_TIME_BUDGET: Duration = Duration::from_millis(3);
const SOURCE_PRIORITY_REFRESH: Duration = Duration::from_millis(100);
pub struct ChunkGpuMesh {
    observation_source: std::sync::Weak<ResidentChunk>,
    boundary_profile: Option<Arc<collar::Installed>>,
    /// Range in the shared arena; `None` for an empty mesh. Dropping the mesh
    /// returns the range (see `arena.rs` for when it can be reused).
    alloc: Option<arena::ArenaLease>,
    pub index_count: u32,
    source_revision: u64,
    pub lod: u8,
    lod_invariant: bool,
    outgoing_lod: Option<(u8, Instant, bool)>,
    bytes: usize,
    /// Face links of the source blocks for cave culling; `None` reads as open.
    connectivity: Option<crate::mesh::ChunkConnectivity>,
    /// Holds wind-swayed vertices (terrain family 34: tufts, blades, sprays).
    /// A shadow layer with none of these draws the same depth at any time.
    pub sways: bool,
}

/// Whether a new altitude cap is far enough from the applied one to rebuild.
// Altitude moves the cap a fraction of a ring per frame in the transition
// tier; a two-ring step keeps the O(r^2) rebuild off most frames.
pub(super) fn cap_changes(applied: i32, wanted: i32, view: i32) -> bool {
    wanted != applied && (applied == 0 || wanted == 0 || wanted >= view || (wanted - applied).abs() >= 2)
}

#[cfg(test)]
impl ChunkCache {
    /// Unpublished membership-only mesh, so budget eviction may take it.
    pub(super) fn install_test_mesh(&mut self, coord: IVec3, bytes: usize) {
        self.gpu_meshes.insert(coord, ChunkGpuMesh::test_mesh(3, 0, false, bytes));
        self.gpu_bytes += bytes;
        self.mark_mesh_set_changed();
    }
}

#[cfg(test)]
impl ChunkGpuMesh {
    /// Membership-only fixture: counts and bytes without GPU storage.
    pub(super) fn test_mesh(index_count: u32, lod: u8, lod_invariant: bool, bytes: usize) -> Self {
        Self { observation_source: Default::default(), boundary_profile: None, alloc: None, index_count, source_revision: 0, lod, lod_invariant, sways: false, outgoing_lod: None, bytes, connectivity: None }
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamInterestChange {
    None,
    Priority,
    Bounds,
}

pub struct ChunkCache {
    voxel_allocations: Arc<VoxelAllocations>,
    residency_limits: (usize, usize),
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
    arriving_columns: FastMap<(i32, i32), Instant>,
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
    /// Columns whose casters changed, tagged with the revision that change
    /// produced; covers `caster_log_floor + 1 ..= shadow_caster_revision`.
    caster_log: Vec<(u64, (i32, i32))>,
    caster_log_floor: u64,
    shadow_casters: shadow_casters::Cache,
    dirty_meshes: FastSet<IVec3>,
    pub cpu_chunks: HashMap<IVec3, Arc<ResidentChunk>>,
    pub center_chunk: IVec3,
    lod_lookahead: IVec3,
    /// Cached column Y-ranges to avoid repeated terrain sampling.
    /// Keyed by (cx, cz). Invalidated when world config changes.
    column_y_cache: ColumnPlans,
    simulation_seeds: Vec<(IVec3, Box<crate::world::activity::Mask>)>,
    /// How far we've expanded the visible set from center (in chunks).
    /// Starts at 0 on reset, grows each frame until it reaches view distance.
    expansion_radius: i32,
    /// Whether expansion is complete (reached view distance).
    pub fully_expanded: bool,
    /// Altitude cap on the visible disc; `None` follows the view distance.
    radius_cap: Option<i32>,
    /// While frozen nothing published or sourced is evicted: orbit keeps the
    /// last surface set because it loads nothing to replace it.
    frozen: bool,
    pub spawn_ready: bool,
    /// Stable horizon handoff radius. A single chunk that is momentarily
    /// missing (remesh, GPU-budget eviction, or a just-expanded ring) must not
    /// drag the entire backdrop inward and fade the resident world to sky.
    coverage_stable_meters: f32,
    coverage_drop_since: Option<Instant>,
    coverage_last_tick: Option<Instant>,
    coverage_center: Option<IVec3>,
    coverage_missing: Vec<f32>,
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
    let middle = missing.len() / 2;
    let (_, median, _) = missing.select_nth_unstable_by(middle, f32::total_cmp);
    let median = *median;
    let near_outlier_cutoff = median * 0.70;
    // Ignore only a bounded number of isolated near misses, not an entire sector.
    let outlier_budget = (missing.len() / 20).clamp(1, 8);
    let outlier_count = missing.iter().filter(|&&distance| distance < near_outlier_cutoff)
        .take(outlier_budget).count();
    let rank = outlier_count.min(missing.len() - 1);
    let (_, representative, _) = missing.select_nth_unstable_by(rank, f32::total_cmp);
    let representative = *representative;
    (representative - margin).min(full_radius).max(0.0)
}

impl ChunkCache {
    /// Actual GPU-owned disc: each column must have its required surface or
    /// underground bundle. Unlike the visual handoff estimate this never ignores holes,
    /// holds an old radius, or counts requested/queued uploads as coverage.
    pub fn resident_coverage_and_radius(&self, eye: Vec3, view_chunks: i32, scale: f32) -> (super::coverage::ResidentCoverage, f32) {
        if !eye.is_finite() || !scale.is_finite() || scale <= 0.0 {
            return (super::coverage::ResidentCoverage::default(), 0.0);
        }
        let span = CHUNK_SIZE as f32 * scale;
        let radius = view_chunks.clamp(1, 64);
        let cx = (eye.x / span).floor() as i32;
        let cz = (eye.z / span).floor() as i32;
        let coverage = self.coverage_from_columns(self.complete_published_columns(), radius, true);
        let p = glam::Vec2::new(eye.x, eye.z);
        let extent = glam::Vec2::splat(span);
        let margin = 2.0 * scale;
        let mut limit_sq = (radius as f32 * span).powi(2);
        let margin_sq = margin * margin;
        if !coverage.contains_column(cx, cz) {
            let lo = glam::Vec2::new(cx as f32 * span, cz as f32 * span);
            limit_sq = limit_sq.min(p.distance_squared(p.clamp(lo, lo + extent)));
            if limit_sq <= margin_sq { return (coverage, 0.0); }
        }
        for z in cz-radius..=cz+radius {
            for x in cx-radius..=cx+radius {
                if coverage.contains_column(x, z) { continue; }
                let lo = glam::Vec2::new(x as f32 * span, z as f32 * span);
                limit_sq = limit_sq.min(p.distance_squared(p.clamp(lo, lo + extent)));
                if limit_sq <= margin_sq { return (coverage, 0.0); }
            }
        }
        (coverage, (limit_sq.sqrt() - margin).max(0.0))
    }
    pub fn resident_coverage(&self, eye: Vec3, view_chunks: i32, scale: f32) -> super::coverage::ResidentCoverage {
        self.coverage_map(eye,view_chunks,scale,true)
    }

    /// Capture every exposed edge from complete installed GPU column bundles.
    /// Reuse unchanged profiles; selection never scans mesh vertices on the frame thread.
    pub(crate) fn collar_input(
        &self,
        previous: Option<&Arc<collar_set::Set>>,
        coverage: &super::coverage::ResidentCoverage,
    ) -> Option<Arc<collar_set::Set>> {
        let mut columns: Vec<_> = self.published_columns.keys().copied()
            .filter(|&(x,z)| coverage.contains_column(x,z)).collect();
        columns.sort_unstable();
        let mut edges = Vec::new();
        let mut dependencies = Vec::new();
        for column in columns {
            let exposed = |side| collar::Input::outside_columns(column,side)
                .is_some_and(|c| !coverage.contains_column(c[0].0,c[0].1));
            if !(0..4).any(exposed) { continue; }
            let members = &self.published_columns[&column];
            if members.is_empty() || members.len() > 64 { continue; }
            dependencies.clear();
            for q in members {
                let Some(mesh) = self.gpu_meshes.get(q) else { break; };
                let Some(installed) = mesh.boundary_profile.as_ref() else { break; };
                if installed.scope.coord != *q || installed.scope.lod != mesh.lod
                    || installed.scope.cache_epoch != self.gpu_world_epoch { break; }
                dependencies.push(Arc::clone(installed));
            }
            if dependencies.len() != members.len() { continue; }
            dependencies.sort_unstable_by_key(|d| (d.scope.coord.x,d.scope.coord.y,d.scope.coord.z));
            for side in 0..4 {
                if !exposed(side) { continue; }
                let old = previous.and_then(|set| set.edge(column,side));
                let unchanged = old.is_some_and(|old| old.dependencies.len() == dependencies.len()
                    && old.dependencies.iter().zip(&dependencies).all(|(a,b)| Arc::ptr_eq(a,b)));
                if unchanged { edges.push(Arc::clone(old.unwrap())); }
                else if let Some(input) = collar::Input::new(column,side,&dependencies) { edges.push(input); }
            }
        }
        if edges.is_empty() { return None; }
        let set = collar_set::Set { edges };
        if let Some(old) = previous.filter(|old| old.same_geometry(&set)) { return Some(Arc::clone(old)); }
        Some(Arc::new(set))
    }

    pub fn desired_resident_coverage(&self, eye: Vec3, view_chunks: i32, scale: f32) -> super::coverage::ResidentCoverage {
        self.coverage_map(eye,view_chunks,scale,false)
    }
    fn coverage_map(&self, eye: Vec3, view_chunks: i32, scale: f32, include_pins: bool) -> super::coverage::ResidentCoverage {
        if !eye.is_finite() || !scale.is_finite() || scale <= 0.0 {
            return super::coverage::ResidentCoverage::default();
        }
        let radius = view_chunks.clamp(1, 64);
        if include_pins {
            self.coverage_from_columns(self.complete_published_columns(), radius, true)
        } else {
            self.coverage_from_columns(self.complete_desired_columns(), radius, false)
        }
    }

    fn complete_published_columns(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.published_columns.iter().filter_map(|(&column, members)| {
            (!members.is_empty() && members.iter().all(|q| self.gpu_meshes.contains_key(q)))
                .then_some(column)
        })
    }

    fn complete_desired_columns(&self) -> impl Iterator<Item = (i32, i32)> {
        let mut columns: FastMap<(i32, i32), (bool, bool)> = FastMap::default();
        for q in &self.visible_coords {
            let column = (q.x, q.z);
            let (ready, needs_visible_members) = columns.entry(column).or_insert_with(|| {
                if let Some(mut band) = self.column_y_cache.surface_band(column, self.center_chunk.y) {
                    (band.all(|y| self.gpu_meshes.contains_key(&IVec3::new(q.x, y, q.z))), false)
                } else {
                    (true, true)
                }
            });
            if *needs_visible_members && *ready {
                *ready = self.gpu_meshes.contains_key(q);
            }
        }
        columns.into_iter().filter_map(|(column, (ready, _))| ready.then_some(column))
    }

    fn coverage_from_columns(&self, columns: impl Iterator<Item = (i32, i32)>, radius: i32, include_pins: bool) -> super::coverage::ResidentCoverage {
        let mut result = super::coverage::ResidentCoverage::default();
        // Use the draw-list centre, which can differ from an interpolated
        // render eye at a chunk boundary. The mask must describe actual draws.
        let cx = self.center_chunk.x;
        let cz = self.center_chunk.z;
        result.header = [cx.saturating_sub(64), cz.saturating_sub(64), 129, 1];
        let mut sparse = Vec::new();
        let r = i64::from(radius);
        for (x, z) in columns {
            let dx = i64::from(x) - i64::from(cx);
            let dz = i64::from(z) - i64::from(cz);
            let nearby = dx.abs() <= r && dz.abs() <= r && dx*dx + dz*dz <= r*r;
            if !(nearby || (include_pins && (self.horizon_pins.contains(&(x,z)) || self.edit_proxies.admitted((x,z))))) { continue; }
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
        if wanted.is_empty() && self.horizon_pins.is_empty() {
            return;
        }
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
        let mut previous_pins: FastMap<_, Vec<_>> = FastMap::default();
        for &coord in &self.horizon_pin_coords {
            previous_pins.entry((coord.x, coord.z)).or_default().push(coord);
        }
        let proxy_columns=self.edit_proxies.columns().count(); let mut extra_pins=0usize;
        for column in wanted {
            let extra=!self.edit_proxies.wanted(&column);
            if proxy_columns+extra_pins+usize::from(extra)>edit_proxies::COLUMN_LIMIT { continue; }
            let mut required = self.published_columns.get(&column).cloned().unwrap_or_default();
            if let Some(previous) = previous_pins.remove(&column) {
                for coord in previous {
                    if !required.contains(&coord) {
                        required.push(coord);
                    }
                }
            }
            if required.is_empty() || required.iter().any(|q| !self.gpu_meshes.contains_key(q)) { continue; }
            let size=required.iter().filter(|q|!reserved.contains(*q)).map(|q|self.gpu_meshes[q].bytes).sum::<usize>();
            if bytes.saturating_add(size)>1024*1024*1024 { continue; }
            bytes+=size; extra_pins+=usize::from(extra); pins.insert(column); reserved.extend(required.iter().copied()); coords.extend(required);
        }
        if pins == self.horizon_pins && coords == self.horizon_pin_coords { return; }
        if pins != self.horizon_pins {
            println!("[horizon-pins] columns={} chunks={} bytes={} requested={} deferred={} reserve_limit_bytes={}",pins.len(),coords.len(),bytes,requested,requested.saturating_sub(pins.len()),1024u64*1024*1024);
        }
        // Retiring a pin must also retire the visible entry it introduced.
        // Otherwise the repair scan keeps requesting distant former owners.
        let radius = i64::from(self.expansion_radius);
        let center = self.center_chunk.as_i64vec3();
        self.visible.retain(|key| {
            let d = key.coord.as_i64vec3() - center;
            !self.horizon_pin_coords.contains(&key.coord)
                || coords.contains(&key.coord)
                || d.x * d.x + d.z * d.z <= radius * radius
        });
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
            arriving_columns: FastMap::default(),
            horizon_pins: FastSet::default(),
            horizon_pin_coords: FastSet::default(),
            edit_proxies: edit_proxies::EditProxies::default(),
            quality_proxies: quality_proxies::QualityProxies::default(),
            seeded_sources: HashSet::new(),
            gpu_meshes: FastMap::default(),
            arena: arena::MeshArena::new(arena_budget(residency::limits().1)),
            gpu_mesh_revision: 0,
            shadow_caster_revision: 0,
            caster_log: Vec::new(),
            caster_log_floor: 0,
            shadow_casters: shadow_casters::Cache::default(),
            dirty_meshes: FastSet::default(),
            cpu_chunks: HashMap::new(),
            center_chunk,
            lod_lookahead: IVec3::ZERO,
            column_y_cache: ColumnPlans::new(),
            simulation_seeds: Vec::new(),
            expansion_radius: 0,
            fully_expanded: false,
            radius_cap: None,
            frozen: false,
            spawn_ready: false,
            coverage_stable_meters: -1.0,
            coverage_drop_since: None,
            coverage_last_tick: None,
            coverage_center: None,
            coverage_missing: Vec::new(),
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
    pub fn published_coverage_meters(&mut self, eye: Vec3, view_chunks: i32, scale: f32) -> f32 {
        if !eye.is_finite() || !scale.is_finite() || scale <= 0.0 {
            return 0.0;
        }
        let chunk_m = CHUNK_SIZE as f32 * scale;
        let radius = view_chunks.clamp(1, 64);
        let full_radius = radius as f32 * chunk_m;
        let center_x = (eye.x / chunk_m).floor() as i32;
        let center_z = (eye.z / chunk_m).floor() as i32;
        let total_columns = ((radius * 2 + 1) * (radius * 2 + 1)) as usize;
        let missing = &mut self.coverage_missing;
        missing.clear();
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let cx = center_x + dx;
                let cz = center_z + dz;
                if self.published_columns.contains_key(&(cx, cz)) {
                    continue;
                }
                let min_x = cx as f32 * chunk_m;
                let min_z = cz as f32 * chunk_m;
                let nx = eye.x.clamp(min_x, min_x + chunk_m);
                let nz = eye.z.clamp(min_z, min_z + chunk_m);
                missing.push(((nx - eye.x).powi(2) + (nz - eye.z).powi(2)).sqrt());
            }
        }
        robust_coverage_from_missing(missing, full_radius, 8.0 * scale, total_columns)
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
        // The refresh below rebuilds `visible_keys` from these same sets.
        self.publication_dirty=true;
        self.refresh_published_columns();
    }

    pub(super) fn terrain_arrival(
        &self, coverage: &super::coverage::ResidentCoverage,
        has_parent: impl Fn((i32, i32)) -> bool,
    ) -> super::coverage::TerrainArrival {
        let now = Instant::now();
        let mut result = super::coverage::TerrainArrival::default();
        for (&column, &started) in &self.arriving_columns {
            if let Some(phase) = super::lod_transition_phase(started, now)
                && coverage.contains_column(column.0, column.1) && has_parent(column)
            {
                result.insert(column, phase);
            }
        }
        result
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
        let mut edit_columns = FastSet::default();
        for (column, members) in self.edit_proxies.desired_columns() {
            edit_columns.insert(column);
            desired.insert(column, members.to_vec());
        }
        let mut columns_changed=false;
        let published_coords = &mut self.published_coords;
        self.published_columns.retain(|column,members|{
            let keep=desired.contains_key(column) && members.iter().all(|q|self.gpu_meshes.contains_key(q));
            if !keep {
                columns_changed = true;
                for coord in members {
                    published_coords.remove(coord);
                }
            }
            keep
        });
        let now = Instant::now();
        self.arriving_columns.retain(|column, started| {
            self.published_columns.contains_key(column) && super::lod_transition_phase(*started, now).is_some()
        });
        for (column,mut members) in desired {
            let quality_only = self.quality_proxies.contains_column(&column);
            let edit_only = edit_columns.contains(&column);
            let ready = |q: &IVec3| {
                self.gpu_meshes.get(q).is_some_and(|mesh| !self.edit_proxies.contains(q) || mesh.lod == 0)
                    && (!quality_only || (!self.dirty_meshes.contains(q)
                        && !self.edit_proxies.water_dirty.contains(q) && !self.pending_gpu.contains(q)))
            };
            let surface = (!quality_only && !edit_only && !self.horizon_pins.contains(&column))
                .then(|| self.column_y_cache.surface_band(column, self.center_chunk.y))
                .flatten();
            let complete = if let Some(band) = &surface {
                if band.clone().all(|y| {
                    let coord = IVec3::new(column.0, y, column.1);
                    self.visible_coords.contains(&coord) && ready(&coord)
                }) {
                    members.retain(|q| band.contains(&q.y) || ready(q));
                    true
                } else {
                    false
                }
            } else {
                (edit_only || quality_only || self.radius_cap == Some(0)
                    || !self.column_y_cache.is_planning(column))
                    && members.iter().all(ready)
            };
            if complete {
                let previous = self.published_columns.get(&column);
                if !quality_only && !edit_only && !self.horizon_pins.contains(&column)
                    && self.radius_cap != Some(0)
                    && let Some(previous) = previous
                {
                    // A one-chunk vertical grace band keeps the last drawn
                    // floor or roof when the player crosses a chunk boundary.
                    // Anchor it to current interest so it cannot grow over time.
                    members.extend(previous.iter().copied().filter(|q| {
                        !self.visible_coords.contains(q)
                            && (self.visible_coords.contains(&(*q - IVec3::Y))
                                || self.visible_coords.contains(&(*q + IVec3::Y)))
                    }));
                }
                members.sort_unstable_by_key(|q|(q.y,q.x,q.z));
                self.edit_proxies.mark_admitted(column);
                self.quality_proxies.mark_admitted(column);
                if self.published_columns.get(&column)!=Some(&members) {
                    columns_changed=true;
                    let previous = self.published_columns.remove(&column);
                    let newly_published = previous.is_none();
                    if let Some(previous) = previous {
                        for coord in previous {
                            published_coords.remove(&coord);
                        }
                    }
                    published_coords.extend(members.iter().copied());
                    if newly_published && surface.is_some() && self.spawn_ready
                        && self.arriving_columns.len() < 128
                    {
                        self.arriving_columns.insert(column, now);
                    }
                    self.published_columns.insert(column,members);
                }
            }
        }
        self.edit_proxies.set_quality_admitted(self.quality_proxies.admitted_columns());
        // Mesh installs can change the resident hysteresis choice. Keep the
        // repair scan and published owners on one target per coordinate.
        for index in 0..self.visible.len() {
            self.visible[index].lod = self.desired_lod(self.visible[index].coord);
        }
        self.visible_keys.clear();
        self.visible_keys.extend(self.visible.iter().copied());
        self.visible_keys.extend(self.edit_proxies.coords.iter().map(|&coord| ChunkKey { coord, lod: 0 }));
        for &coord in &self.published_coords {
            if !self.visible_coords.contains(&coord) {
                self.visible_keys.insert(ChunkKey { coord, lod: self.desired_lod(coord) });
            }
        }
        if columns_changed { self.mark_mesh_set_changed(); }
    }
    fn source_interested(&self,q:&IVec3)->bool { self.visible_coords.contains(q)||self.published_coords.contains(q)||self.edit_proxies.contains(q) }

    fn priority_center(&self, coord: IVec3) -> IVec3 {
        let projected = self.center_chunk + self.lod_lookahead;
        let distance = |center: IVec3| coord.x.abs_diff(center.x).max(coord.z.abs_diff(center.z));
        if distance(projected) < distance(self.center_chunk) {
            projected
        } else {
            self.center_chunk
        }
    }

    fn near_stream_path(&self, coord: IVec3) -> bool {
        let center = self.priority_center(coord);
        coord.x.abs_diff(center.x).max(coord.z.abs_diff(center.z)) <= 2
    }

    fn terrain_lod_settings(&self) -> super::RenderDistanceSettings {
        let mut settings = render_distance_settings();
        if settings.lod_levels > 1 {
            // Under a small GPU budget, hold more continuous surface cover
            // while keeping the player and adjacent columns at full detail.
            if self.residency_limits.1 <= 256 << 20 {
                settings.lod_start_chunks = settings.lod_start_chunks.min(2);
                settings.lod_levels = settings.lod_levels.max(3);
            } else if self.residency_limits.1 <= 512 << 20 {
                settings.lod_start_chunks = settings.lod_start_chunks.min(3);
            }
        }
        settings
    }

    fn desired_lod(&self, coord: IVec3) -> u8 {
        // In space the only requested sources are nearby authored structures.
        if self.radius_cap == Some(0) { return 0; }
        if self.edit_proxies.contains(&coord) { return 0; }
        // Retained owners keep their installed detail until they return to
        // ordinary interest. Edits can still refresh them at the same LOD.
        if ((self.horizon_pin_coords.contains(&coord) && !self.ordinary_source_interested(coord))
            || (self.published_coords.contains(&coord) && !self.visible_coords.contains(&coord)))
            && let Some(mesh) = self.gpu_meshes.get(&coord) {
                return mesh.lod;
            }
        let settings = self.terrain_lod_settings();
        let lod_at = |center, coord| visibility::lod_for_chunk_with_settings(center, coord, settings);
        let desired = lod_at(self.center_chunk, coord);
        let Some(mesh) = self.gpu_meshes.get(&coord) else {
            return desired;
        };
        if mesh.lod >= settings.lod_levels {
            return desired;
        }
        // Fill gaps at their ordinary LOD first. Existing terrain can refine
        // a chunk early on the leading edge without widening every detail band.
        let desired = if desired > 0 && self.lod_lookahead != IVec3::ZERO {
            desired.min(lod_at(self.center_chunk + self.lod_lookahead, coord))
        } else {
            desired
        };
        let delta = coord - self.center_chunk;
        let step = IVec3::new(delta.x.signum(), 0, delta.z.signum());
        resident_lod(mesh.lod, desired, lod_at(self.center_chunk, coord - step))
    }

    pub fn rebuild_visible(&mut self) {
        self.visible = desired_chunk_keys(self.center_chunk);
        self.rebuild_visible_sets();
    }

    pub fn rebuild_visible_with_snapshot(&mut self, snapshot: &WorldSnapshot) {
        if self.radius_cap == Some(0) {
            self.update_space_interest(snapshot);
            return;
        }
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
        self.arriving_columns.clear(); self.published_columns.clear();self.published_coords.clear();
        self.publication_dirty=true;
        self.horizon_pins.clear(); self.horizon_pin_coords.clear();
        self.dirty_meshes.clear();
        self.budget_deferred.clear();
        self.lod_meshes.clear();
        self.gpu_bytes = 0;
        self.spawn_ready = false;
        self.center_chunk = center_chunk;
        self.lod_lookahead = IVec3::ZERO;
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
        self.arriving_columns.clear(); self.published_columns.clear(); self.published_coords.clear(); self.publication_dirty=true;
        self.horizon_pins.clear(); self.horizon_pin_coords.clear();
        self.dirty_meshes.clear();
        self.budget_deferred.clear();
        self.lod_meshes.clear();
        self.gpu_bytes = 0;
        self.spawn_ready = false;
        self.center_chunk = center_chunk;
        self.lod_lookahead = IVec3::ZERO;
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
        let radius = render_distance_settings().view_distance_chunks;
        self.update_center_chunk_with_snapshot_radius(center_chunk, snapshot, radius)
    }

    fn radius_target(&self) -> i32 {
        let view = render_distance_settings().view_distance_chunks;
        self.radius_cap.map_or(view, |cap| cap.clamp(0, view))
    }

    pub fn freeze(&mut self, frozen: bool) {
        self.frozen = frozen;
    }

    pub fn frozen(&self) -> bool {
        self.frozen
    }

    /// Rebuilds the visible disc around `center_chunk` capped at `radius_chunks`.
    pub fn update_center_chunk_with_snapshot_radius(
        &mut self,
        center_chunk: IVec3,
        snapshot: &WorldSnapshot,
        radius_chunks: i32,
    ) -> bool {
        self.update_streaming_center(center_chunk, snapshot, radius_chunks, Vec3::ZERO)
            != StreamInterestChange::None
    }

    pub(crate) fn update_streaming_center(
        &mut self,
        center_chunk: IVec3,
        snapshot: &WorldSnapshot,
        radius_chunks: i32,
        velocity_voxels: Vec3,
    ) -> StreamInterestChange {
        let view = render_distance_settings().view_distance_chunks;
        let wanted = radius_chunks.clamp(0, view);
        let cap = match self.radius_cap {
            Some(applied) if !cap_changes(applied.clamp(0, view), wanted, view) => Some(applied),
            _ => Some(wanted),
        };
        if cap == Some(0) && self.radius_cap == Some(0) {
            // Crossing empty space must not repeatedly invalidate upload epochs
            // or clear a nearby station's meshes on every chunk boundary.
            self.center_chunk = center_chunk;
            return StreamInterestChange::None;
        }
        // Predict where the detail bands will be when mesh work can publish.
        let lead = if wanted > 0 && velocity_voxels.is_finite() {
            let horizontal = Vec3::new(velocity_voxels.x, 0.0, velocity_voxels.z);
            // At ground sprint speed, a sub-second lead rounds to zero at
            // the standard 32-metre chunk size.
            (horizontal * (3.0 / CHUNK_SIZE as f32))
                .clamp_length_max((wanted / 2).min(6) as f32)
                .round().as_ivec3()
        } else {
            IVec3::ZERO
        };
        if self.center_chunk == center_chunk && self.radius_cap == cap {
            if self.lod_lookahead == lead {
                return StreamInterestChange::None;
            }
            // A velocity change only moves the detail bands. The current
            // column plan and its chunk membership still apply.
            self.lod_lookahead = lead;
            self.publication_dirty = true;
            self.refresh_published_columns();
            return StreamInterestChange::Priority;
        }
        self.lod_lookahead = lead;
        self.column_y_cache.set_lookahead((lead.x, lead.z));
        self.center_chunk = center_chunk;
        self.radius_cap = cap;
        let target = self.radius_target();
        if target == 0 {
            // Leaving the surface releases planetary residency. Nearby edited
            // structures are admitted separately; horizon pins must not keep
            // the last ground patch loaded for the entire trip.
            self.gpu_world_epoch = self.gpu_world_epoch.wrapping_add(1);
            self.shadow_casters.clear();
            self.edit_proxies = edit_proxies::EditProxies::default();
            self.quality_proxies = quality_proxies::QualityProxies::default();
            self.horizon_pins.clear();
            self.horizon_pin_coords.clear();
            self.arriving_columns.clear(); self.published_columns.clear();
            self.published_coords.clear();
            self.gpu_meshes.clear();
            self.gpu_bytes = 0;
            self.cpu_chunks.clear();
            self.dirty_meshes.clear();
            self.budget_deferred.clear();
            self.lod_meshes.clear();
            self.seeded_sources.clear();
            self.simulation_seeds.clear();
            self.column_y_cache.clear();
            self.visible.clear();
            self.rebuild_visible_sets();
            self.mark_mesh_set_changed();
            self.coverage_stable_meters = 0.0;
            self.coverage_drop_since = None;
            self.coverage_last_tick = None;
            self.coverage_center = None;
            self.expansion_radius = 0;
            self.fully_expanded = true;
            return StreamInterestChange::Bounds;
        }
        // Expansion never contracts on its own during movement: that would
        // discard useful jobs and regenerate the edge. The altitude cap does
        // contract it, and descent regrows ring by ring, which is accepted.
        let rebuild_radius = self.expansion_radius.min(target).max(4.min(target));

        self.visible = desired_chunk_keys_with_snapshot_cached(
            center_chunk,
            snapshot,
            &mut self.column_y_cache,
            rebuild_radius,
        );
        self.rebuild_visible_sets();
        self.expansion_radius = rebuild_radius;
        self.fully_expanded = rebuild_radius >= target;
        StreamInterestChange::Bounds
    }

    /// A small three-dimensional bubble of existing edits, with no terrain
    /// column planning. Stations and player builds remain visible in space.
    fn update_space_interest(&mut self, snapshot: &WorldSnapshot) -> bool {
        const RADIUS_M: f32 = 128.0;
        let radius = (RADIUS_M / (CHUNK_SIZE as f32 * snapshot.cfg.voxel_meters())).ceil() as i32;
        let center = self.center_chunk;
        let wanted: FastSet<_> = snapshot.edited_coords_near(center, radius).filter(|q| {
            let d = q.as_i64vec3() - center.as_i64vec3();
            d.length_squared() <= i64::from(radius) * i64::from(radius)
        }).collect();
        if wanted == self.visible_coords { return false; }
        let before = self.gpu_meshes.len();
        let gpu_bytes = &mut self.gpu_bytes;
        self.gpu_meshes.retain(|q, mesh| {
            let keep = wanted.contains(q);
            if !keep { *gpu_bytes -= mesh.bytes; }
            keep
        });
        self.cpu_chunks.retain(|q, _| wanted.contains(q));
        self.dirty_meshes.retain(|q| wanted.contains(q));
        self.seeded_sources.retain(|q| wanted.contains(q));
        self.lod_meshes.retain(|q| wanted.contains(&q));
        self.budget_deferred.retain(|q, _| wanted.contains(&q.coord));
        self.visible = wanted.into_iter().map(|coord| ChunkKey { coord, lod: 0 }).collect();
        self.visible.sort_unstable_by_key(|k| (k.coord.y, k.coord.x, k.coord.z));
        self.rebuild_visible_sets();
        if before != self.gpu_meshes.len() { self.mark_mesh_set_changed(); }
        true
    }

    pub fn space_source_coords(&self) -> Vec<IVec3> {
        if self.radius_cap == Some(0) { self.visible_coords.iter().copied().collect() }
        else { Vec::new() }
    }

    /// Incrementally expand the visible set by a few chunk-rings per frame.
    /// Returns newly added keys so the caller can enqueue them for scheduling
    /// (avoids a full O(visible) scan in enqueue_missing each frame).
    pub fn expand_visible(
        &mut self,
        snapshot: &WorldSnapshot,
        queue_pressure: usize,
    ) -> Vec<ChunkKey> {
        let target = self.radius_target();
        if target == 0 {
            return Vec::new();
        }
        self.column_y_cache
            .set_interest((self.center_chunk.x, self.center_chunk.z), target + 2);
        self.column_y_cache.begin(snapshot.cfg);
        self.column_y_cache.set_lookahead((self.lod_lookahead.x, self.lod_lookahead.z));
        let mut added = Vec::new();
        let eye_underground = self.column_y_cache.eye_underground(self.center_chunk.y);
        while let Some((c, range)) = self.column_y_cache.pop_completed() {
            let dx = c.0 - self.center_chunk.x;
            let dz = c.1 - self.center_chunk.z;
            let current_limit = self.expansion_radius;
            if dx * dx + dz * dz > current_limit * current_limit {
                continue;
            }
            let keys = if eye_underground && dx == 0 && dz == 0 {
                // If the eye's column arrived after other plans, add their
                // newly eligible cavern bands along with the local column.
                desired_chunk_keys_with_snapshot_cached(
                    self.center_chunk, snapshot, &mut self.column_y_cache, current_limit,
                )
            } else {
                visibility::keys_in_view(self.center_chunk, c, range, &snapshot.cfg, eye_underground)
            };
            for mut key in keys {
                key.lod = self.desired_lod(key.coord);
                if self.visible_coords.insert(key.coord) {
                    self.visible_keys.insert(key);
                    self.visible.push(key);
                    added.push(key);
                }
            }
        }
        self.publication_dirty |= added.iter().any(|key| self.gpu_meshes.contains_key(&key.coord));
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

        self.publication_dirty |= added.iter().any(|key| self.gpu_meshes.contains_key(&key.coord));
        if new_radius >= target {
            self.fully_expanded = true;
        }
        added
    }

    pub fn take_simulation_seeds(&mut self) -> Vec<(IVec3, Box<crate::world::activity::Mask>)> {
        std::mem::take(&mut self.simulation_seeds)
    }
    pub fn retain_visible(&mut self) {
        if self.frozen {
            return;
        }
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
        let pins = &self.horizon_pin_coords;
        let published = &self.published_coords;
        let edits = &self.edit_proxies;
        let gpu_bytes = &mut self.gpu_bytes;
        self.gpu_meshes.retain(|coord, mesh| {
            let keep = is_in_range(coord) || pins.contains(coord) || published.contains(coord) || edits.contains(coord);
            if !keep { *gpu_bytes -= mesh.bytes; }
            keep
        });
        self.lod_meshes.retain(|coord| is_in_range(&coord));
        self.budget_deferred.retain(|key, _| {
            is_in_range(&key.coord) || pins.contains(&key.coord) || edits.contains(&key.coord)
        });
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

    pub(super) fn published_column_members(&self) -> impl Iterator<Item = ((i32, i32), &[IVec3])> {
        self.published_columns.iter().map(|(&column, members)| (column, members.as_slice()))
    }

    fn mark_mesh_set_changed(&mut self) {
        self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
        self.shadow_caster_revision = self.shadow_caster_revision.wrapping_add(1);
        // A membership change is not attributed to columns: every layer redraws.
        self.caster_log.clear();
        self.caster_log_floor = self.shadow_caster_revision;
    }

    fn log_caster_change(&mut self, coord: IVec3) {
        self.shadow_caster_revision = self.shadow_caster_revision.wrapping_add(1);
        const KEEP: usize = 1024;
        if self.caster_log.len() >= 2 * KEEP {
            let drop = self.caster_log.len() - KEEP;
            self.caster_log_floor = self.caster_log[drop - 1].0;
            self.caster_log.drain(..drop);
        }
        self.caster_log.push((self.shadow_caster_revision, (coord.x, coord.z)));
    }

    /// Caster columns changed after `revision`, or `None` when that span includes
    /// a change this log does not attribute to columns (bulk membership change,
    /// or older than the retained log).
    pub fn shadow_caster_columns_since(&self, revision: u64) -> Option<Vec<(i32, i32)>> {
        if revision < self.caster_log_floor || revision > self.shadow_caster_revision {
            return None;
        }
        let start = self.caster_log.partition_point(|(rev, _)| *rev <= revision);
        Some(self.caster_log[start..].iter().map(|(_, column)| *column).collect())
    }

    /// The publication transaction calls this only after GPU validation succeeds.
    /// Revision of an installed mesh belonging to a complete published column.
    pub fn published_revision(&self, coord: IVec3) -> Option<u64> {
        self.published_coords.contains(&coord).then(|| self.gpu_meshes.get(&coord).map(|m| m.source_revision)).flatten()
    }

    /// Exact opaque-voxel source of the installed near mesh. Weak identity does
    /// not pin regenerable data; replacement/mutation invalidates observation.
    pub fn observation_source(&self, coord: IVec3) -> Option<&GeneratedChunk> {
        if !self.published_coords.contains(&coord) { return None; }
        let mesh = self.gpu_meshes.get(&coord)?;
        if mesh.lod != 0 { return None; }
        let source = self.cpu_chunks.get(&coord)?;
        (mesh.observation_source.as_ptr() == Arc::as_ptr(source))
            .then_some(&***source)
    }

    /// Vertex and index buffers of one arena page.
    pub fn page_buffers(&self, page: u32) -> Option<(&wgpu::Buffer, &wgpu::Buffer)> {
        self.arena.buffers(page)
    }

    pub(crate) fn trace_gpu_memory(&self) {
        let status = gpu_budget::gpu_residency_status();
        println!(
            "[terrain-gpu-memory] arena_bytes={} arena_used_bytes={} resident_mesh_bytes={} cached_lod_bytes={} pending_mesh_bytes={} local_process_mib={} local_process_live={}",
            self.arena.capacity_bytes(), self.arena.used_bytes(), self.gpu_bytes,
            self.lod_meshes.bytes(), self.pending_gpu.bytes(),
            status.process_usage_mib, status.live,
        );
    }

    /// Buffers and ranges for drawing the resident mesh at `coord`, if it has geometry.
    pub fn draw_slice(&self, coord: IVec3) -> Option<DrawSlice<'_>> {
        self.mesh_draw_slice(self.gpu_meshes.get(&coord)?)
    }

    pub(super) fn lod_transition(&self, coord: IVec3, now: Instant) -> Option<(Option<DrawSlice<'_>>, u32)> {
        let (lod, started, reverse_pattern) = self.gpu_meshes.get(&coord)?.outgoing_lod?;
        let phase = super::lod_transition_phase(started, now)?;
        let previous = self.lod_meshes.get(ChunkKey { coord, lod })?;
        Some((self.mesh_draw_slice(previous), phase | (u32::from(reverse_pattern) << 7)))
    }

    fn mesh_draw_slice(&self, mesh: &ChunkGpuMesh) -> Option<DrawSlice<'_>> {
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

    #[cfg(test)]
    fn install_mesh(&mut self, coord: IVec3, mesh: ChunkGpuMesh) {
        self.install_mesh_unpublished(coord, mesh, shadow_casters::Prepared::default());
        self.refresh_published_columns();
    }

    /// Installs without refreshing column publication. A batch of installs
    /// ends with one `refresh_published_columns`: each refresh walks every
    /// visible chunk (about 1 ms at 4.8k), and none of the installs reads it.
    fn install_mesh_unpublished(
        &mut self,
        coord: IVec3,
        mut mesh: ChunkGpuMesh,
        prepared: shadow_casters::Prepared,
    ) {
        let same_lod = self.gpu_meshes.get(&coord).is_some_and(|old| old.lod == mesh.lod);
        let unchanged_casters = self.shadow_casters.install(coord, mesh.lod, same_lod, prepared);
        // Column readiness depends on buffer presence and LOD, not geometry
        // contents. Replacing an existing representation keeps the same owners.
        let previous_desired = (!same_lod && self.published_coords.contains(&coord))
            .then(|| self.desired_lod(coord));
        let edited_source = self.dirty_meshes.remove(&coord);
        drop(self.lod_meshes.take(ChunkKey {
            coord,
            lod: mesh.lod,
        }));
        self.gpu_bytes += mesh.bytes;
        let lod = mesh.lod;
        mesh.outgoing_lod = self.gpu_meshes.get(&coord)
            .filter(|_| !edited_source)
            .and_then(|old| {
                // A same-detail refresh can finish during a transition.
                // Keep fading from the same cached mesh in that case.
                if old.lod == lod {
                    return old.outgoing_lod;
                }
                if old.lod_invariant {
                    return None;
                }
                let now = Instant::now();
                if let Some((outgoing, started, reversed)) = old.outgoing_lod
                    && outgoing == lod
                    && let Some(phase) = super::lod_transition_phase(started, now)
                {
                    // Swap endpoints with the same per-pixel ownership on
                    // the first frame of the reversal.
                    let offset = super::LOD_TRANSITION_TIME.mul_f64(f64::from(64 - phase) / 64.0);
                    Some((old.lod, now - offset, !reversed))
                } else {
                    Some((old.lod, now, false))
                }
            });
        let fade_until = mesh.outgoing_lod.map(|(_, started, _)| started + super::LOD_TRANSITION_TIME);
        if let Some(previous) = self.gpu_meshes.insert(coord, mesh) {
            self.gpu_bytes -= previous.bytes;
            // A different LOD is reusable only while its source/halo is unchanged.
            // Dirty invalidation already removed inactive siblings; do not put
            // the old edited source back into that cache during replacement.
            if !edited_source && previous.lod != lod && !previous.lod_invariant {
                self.lod_meshes.remember(coord, previous, fade_until);
            }
        }
        self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
        if !unchanged_casters {
            self.log_caster_change(coord);
        }
        let column = (coord.x, coord.z);
        self.publication_dirty |= !same_lod && match previous_desired {
            None => true,
            Some(desired) => self.edit_proxies.contains(&coord)
                || self.quality_proxies.contains_column(&column)
                || self.desired_lod(coord) != desired,
        };
        self.trim_lod_budget();
    }

    #[cfg(test)]
    fn activate_lod(&mut self, key: ChunkKey) -> bool {
        let activated = self.activate_lod_unpublished(key);
        self.refresh_published_columns();
        activated
    }

    fn activate_lod_unpublished(&mut self, key: ChunkKey) -> bool {
        if !self.dirty_meshes.contains(&key.coord)
            && let Some(mesh) = self.gpu_meshes.get_mut(&key.coord)
                && (mesh.lod_invariant || mesh.lod == key.lod) {
                    let ownership_changed = mesh.lod != key.lod;
                    if ownership_changed {
                        if mesh.lod == 0 || key.lod == 0 {
                            self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
                        }
                        mesh.boundary_profile = mesh.boundary_profile.as_ref()
                            .and_then(|profile| profile.relabel_empty_lod(key.lod));
                    }
                    mesh.lod = key.lod;
                    if ownership_changed {
                        self.publication_dirty = true;
                    }
                    return true;
                }
        let Some(mesh) = self.lod_meshes.take(key) else {
            return false;
        };
        self.install_mesh_unpublished(key.coord, mesh, shadow_casters::Prepared::default());
        true
    }
}
pub struct CpuChunkMesh {
    observation_source: std::sync::Weak<ResidentChunk>,
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

pub struct PortalWarmup {
    pub snapshot: WorldSnapshot,
    exit: Option<(Arc<ResidentChunk>, CpuChunkMesh)>,
}

impl PortalWarmup {
    pub fn build(world: &mut crate::world::World, center: IVec3, floor: IVec3) -> Self {
        let snapshot = world.snapshot_near(center, 2);
        let cancelled = Arc::new(AtomicBool::new(false));
        let exit = if snapshot.edits_ready(floor) {
            crate::world::generate_padded(&snapshot.cfg, floor, &cancelled).and_then(|mut chunk| {
                apply_snapshot_edits_cubic(&mut chunk, &snapshot);
                let source = Arc::new(ResidentChunk::new(chunk, &Arc::new(VoxelAllocations::default())));
                let job = MeshJob {
                    key: ChunkKey { coord: floor, lod: 0 }, revision: world.revision(),
                    job_id: 0, world_epoch: 0, cache_epoch: 0, chunk: Arc::clone(&source),
                    shadow_baseline: None, cancelled,
                };
                run_mesh_job(job, &Arc::new(MeshBudget::default()))
                    .filter(|mesh| !mesh.failed)
                    .map(|mesh| (source, mesh))
            })
        } else { None };
        Self { snapshot, exit }
    }
}

fn run_mesh_job(job: MeshJob, budget: &Arc<MeshBudget>) -> Option<CpuChunkMesh> {
    let observation_source = Arc::downgrade(&job.chunk);
    let key = job.key;
    let job_id = job.job_id;
    let revision = job.revision;
    let world_epoch = job.world_epoch;
    let cache_epoch = job.cache_epoch;
    let shadow_baseline = job.shadow_baseline;
    let cancelled = Arc::clone(&job.cancelled);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if job.cancelled.load(Ordering::Relaxed) {
            return None;
        }
        // Liquid meshes keep their exact source elevation at every LOD.
        // Rounding the bed/banks up to a coarse-cell top can bury an entire
        // shallow river beneath opaque terrain.
        // Empty padded sources have no surface, even at a chunk boundary.
        // Publish their empty mesh without queueing behind the planner or
        // reserving the scratch budget used by a real geometry build.
        let all_air = job.chunk.blocks.len() == (crate::mesh::PADDED_CHUNK_SIZE as usize).pow(3)
            && job.chunk.blocks.iter().all(|&block| block == crate::world::BlockId::Air);
        let dry_coarse = !all_air && key.lod > 0 && !job.chunk.blocks.iter().any(|&block| {
            matches!(block, crate::world::BlockId::Water | crate::world::BlockId::Lava | crate::world::BlockId::SulfuricAcid)
        });
        let (mut mesh, lod_invariant, mut lease) = if all_air {
            (ChunkMesh::empty(), true, None)
        } else if dry_coarse {
            // Coarse output fits the initial reservation. Only exact jobs
            // need the planner lock to grow their reservation safely.
            let lease = budget.acquire(PLANNING_BYTES, &job.cancelled)?;
            let mesh = crate::mesh::coarsened_dry_mesh(&job.chunk, key.lod);
            // Coarsening can filter out geometry that's present at finer LODs.
            (mesh, false, Some(lease))
        } else {
            let planning = budget.begin_plan(&job.cancelled)?;
            let mut lease = budget.acquire(PLANNING_BYTES, &job.cancelled)?;
            let plan = MeshPlan::for_chunk(&job.chunk, &job.cancelled, key.lod)?;
            lease.grow_to(plan.bytes(), &job.cancelled)?;
            drop(planning);
            if job.cancelled.load(Ordering::Relaxed) {
                return None;
            }
            let mesh = if plan.vertices == 0 {
                ChunkMesh::empty()
            } else {
                plan.build_lod(&job.chunk, key.lod)
            };
            (mesh, plan.lod_invariant(), Some(lease))
        };
        // Only LOD 0 participates in the cave connectivity walk.
        let connectivity = if key.lod == 0 && occlusion::enabled() {
            if all_air {
                Some(crate::mesh::ChunkConnectivity::ALL_OPEN)
            } else {
                crate::mesh::ChunkConnectivity::compute(&job.chunk, &job.cancelled)
            }
        } else {
            None
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
        if let Some(lease) = &mut lease {
            lease.finish(bytes);
        }
        let shadow_casters = shadow_casters::Prepared::from_mesh(
            &mesh, shadow_baseline.as_ref(), &job.cancelled);
            let boundary_profile = collar::Prepared::from_mesh(key.coord, &mesh, &job.cancelled);
        Some(CpuChunkMesh {
            observation_source: observation_source.clone(),
            key,
            revision,
            job_id, world_epoch, cache_epoch,
            mesh,
            shadow_casters,
            boundary_profile,
            connectivity,
            failed: false,
            lod_invariant,
            _lease: lease,
        })
    }));
    let output = match result {
        Ok(Some(output)) => output,
        Ok(None) if cancelled.load(Ordering::Relaxed) => return None,
        // Admission rejection must reach the collector just like a failed
        // build, so its request markers are released and retry is throttled.
        // Silently dropping it would leave the coordinate meshing forever.
        Ok(None) | Err(_) => CpuChunkMesh {
            observation_source: Default::default(),
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
    Some(output)
}

fn spawn_mesh_workers(
    worker_count: usize,
    budget: &Arc<MeshBudget>,
) -> anyhow::Result<(mpsc::SyncSender<MeshJob>, mpsc::SyncSender<MeshJob>, mpsc::Receiver<CpuChunkMesh>)> {
    // Keep waiting work in the reorderable ready list when workers are busy.
    let queue_capacity = worker_count.max(1).saturating_mul(2).min(MAX_MESH_JOBS);
    let (job_tx, job_rx) = mpsc::sync_channel::<MeshJob>(queue_capacity);
    let (urgent_tx, urgent_rx) = mpsc::sync_channel::<MeshJob>(player_edits::QUEUE_CAPACITY);
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
                    let Some(output) = run_mesh_job(job, &budget) else { continue };
                    if result_tx.send(output).is_err() {
                        break;
                    }
                }
            })
            .map_err(anyhow::Error::from)?;
    }
    let urgent_budget = Arc::clone(budget);
    let urgent_results = result_tx.clone();
    thread::Builder::new().name("chunk-mesh-player".into()).spawn(move || {
        while let Ok(job) = urgent_rx.recv() {
            let Some(output) = run_mesh_job(job, &urgent_budget) else { continue };
            if urgent_results.send(output).is_err() { break; }
        }
    })?;
    drop(result_tx);
    Ok((job_tx, urgent_tx, result_rx))
}

// Preserve cover lanes and whole-column age before ranking by view and distance.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct PendingSourcePriority {
    parked: bool,
    waiting_for_edits: bool,
    lane: u8,
    column_age: Option<(Instant, i32, i32)>,
    out_of_view: bool,
    radial: (i32, i32, i32, i32, i32, i32, i32, u8),
}

struct StreamingView {
    eye_chunks: glam::DVec2,
    sides: [glam::Vec2; 2],
}

impl StreamingView {
    fn new(eye_voxel: glam::DVec3, forward: Vec3, up: Vec3, fov_degrees: f32, aspect: f32) -> Option<Self> {
        if up.y < 0.999 {
            return None;
        }
        let horizontal = glam::Vec2::new(forward.x, forward.z);
        let vertical_slope = (fov_degrees.to_radians() * 0.5).tan();
        // At steep pitches the view can cross the camera's horizontal plane.
        // Use radial admission when a single horizontal wedge cannot contain it.
        let min_forward = horizontal.length() - forward.y.abs() * vertical_slope;
        if min_forward <= 0.0 || !min_forward.is_finite() {
            return None;
        }
        let along = horizontal.normalize() * (aspect * vertical_slope / min_forward);
        let across = glam::Vec2::new(-horizontal.y, horizontal.x).normalize();
        Some(Self {
            eye_chunks: glam::DVec2::new(eye_voxel.x, eye_voxel.z) / CHUNK_SIZE as f64,
            sides: [(along + across).normalize(), (along - across).normalize()],
        })
    }

    fn sees_column(&self, (x, z): (i32, i32)) -> bool {
        let offset = (glam::DVec2::new(f64::from(x) + 0.5, f64::from(z) + 0.5)
            - self.eye_chunks).as_vec2();
        // Include the entire column and prefetch a strip along either edge.
        self.sides.iter().all(|side| side.dot(offset) >= -2.0)
    }
}

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
    source_refresh:HashSet<IVec3>,
    generating: HashSet<ChunkKey>,
    meshing: HashMap<ChunkKey, (u64, Arc<AtomicBool>)>,
    ready_chunks: VecDeque<ChunkJob>,
    ready_meshes: VecDeque<CpuChunkMesh>,
    mesh_job_tx: mpsc::SyncSender<MeshJob>,
    urgent_job_tx: mpsc::SyncSender<MeshJob>,
    player_edits: HashMap<IVec3, player_edits::Mark>,
    mesh_result_rx: mpsc::Receiver<CpuChunkMesh>,
    next_mesh_job_id: u64,
    terrain_generator: TerrainGenerator,
    /// Chunks that need to be scheduled but haven't been yet.
    /// Populated when new keys are added to the visible set, avoiding an
    /// O(visible) scan inside enqueue_missing every frame.
    /// Ranked by cover, incomplete proxy age, view and distance.
    pending_schedule: BinaryHeap<Reverse<PendingSourcePriority>>,
    pending_keys: HashSet<ChunkKey>,
    pending_unranked: Vec<ChunkKey>,
    pending_ranked_at: Option<Instant>,
    pending_schedule_center: IVec3,
    view: Option<StreamingView>,
    parked_sources: usize,
    column_view: FastMap<(i32, i32), (bool, bool)>,
    urgent_coords: HashSet<IVec3>,
    repair_cursor: usize,
    seed_cursor: usize,
    last_stats: ChunkStreamerStats,
}

impl ChunkStreamer {
    pub fn install_portal_warmup(&mut self, cache: &mut ChunkCache, mut warmup: PortalWarmup) {
        let Some((mut source, mut mesh)) = warmup.exit.take() else { return; };
        if !cache.source_interested(&mesh.key.coord)
            || cache.cpu_bytes().saturating_add(source.allocation_bytes()) > cache.source_limit() {
            return;
        }
        mesh.observation_source = Default::default();
        let Some(resident) = Arc::get_mut(&mut source) else { return; };
        resident.reaccount(&cache.voxel_allocations);
        mesh.observation_source = Arc::downgrade(&source);
        cache.cpu_chunks.insert(mesh.key.coord, source);
        mesh.world_epoch = self.upload_epoch;
        mesh.cache_epoch = cache.gpu_world_epoch;
        self.requested.insert(mesh.key);
        self.requested_coords.insert(mesh.key.coord);
        self.ready_meshes.push_front(mesh);
    }

    pub fn new(device: &wgpu::Device, worker_count: usize) -> anyhow::Result<Self> {
        let mesh_budget = Arc::new(MeshBudget::default());
        let (mesh_job_tx, urgent_job_tx, mesh_result_rx) = spawn_mesh_workers(worker_count, &mesh_budget)?;

        Ok(Self {
            worker_retry_after: Instant::now(),
            mesh_budget,
            pending_uploads: Vec::new(),
            pending_gpu_coords: HashMap::new(),
            upload_epoch: 0,
            next_upload_token: 0,
            dirty_revisions: HashMap::new(),
            source_observations: local_sources::Observations::default(),
            source_refresh:HashSet::new(),
            upload_retry_after: Instant::now(),
            upload_error: None,
            requested: HashSet::new(),
            requested_coords: HashSet::new(),
            generating: HashSet::new(),
            meshing: HashMap::new(),
            ready_chunks: VecDeque::new(),
            ready_meshes: VecDeque::new(),
            mesh_job_tx,
            urgent_job_tx,
            player_edits: HashMap::new(),
            mesh_result_rx,
            next_mesh_job_id: 0,
            terrain_generator: TerrainGenerator::new(device, worker_count),
            pending_schedule: BinaryHeap::new(),
            pending_keys: HashSet::new(),
            pending_unranked: Vec::new(),
            pending_ranked_at: None,
            pending_schedule_center: IVec3::ZERO,
            view: None,
            parked_sources: 0,
            column_view: FastMap::default(),
            urgent_coords: HashSet::new(),
            repair_cursor: 0,
            seed_cursor: 0,
            last_stats: ChunkStreamerStats::default(),
        })
    }

    pub fn reset(&mut self) {
        self.urgent_coords.clear();
        self.dirty_revisions.clear();
        self.player_edits.clear();
        self.source_observations=local_sources::Observations::default();
        self.source_refresh.clear();
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
        self.pending_unranked.clear();
        self.pending_ranked_at = None;
        self.view = None;
        self.parked_sources = 0;
        self.repair_cursor = 0;
        self.seed_cursor = 0;
        while self.mesh_result_rx.try_recv().is_ok() {}
        self.terrain_generator.reset();
        self.last_stats = ChunkStreamerStats::default();
    }

    pub fn begin_frame_stats(&mut self) {
        self.last_stats = ChunkStreamerStats::default();
    }

    pub fn set_view(&mut self, eye_voxel: glam::DVec3, forward: Vec3, up: Vec3, fov_degrees: f32, aspect: f32) {
        // Run matched native captures with the previous radial scheduler.
        static RADIAL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *RADIAL.get_or_init(|| std::env::var_os("VOXEL_STREAM_RADIAL").is_some()) {
            return;
        }
        let next = StreamingView::new(eye_voxel, forward, up, fov_degrees, aspect);
        let turned = self.view.as_ref().zip(next.as_ref()).is_some_and(|(before, after)| {
            before.sides.iter().zip(after.sides).any(|(a, b)| a.dot(b) < 0.95)
        });
        if turned || self.view.is_some() != next.is_some() {
            self.pending_ranked_at = None;
        }
        self.view = next;
    }

    /// Service nearby structures and GPU retirement even with surface streaming
    /// stopped. Empty space schedules no procedural columns or chunk rings.
    pub fn stream_space(&mut self, gpu: &Gpu, snapshot: &WorldSnapshot, cache: &mut ChunkCache, revision: u64) {
        if self.view.take().is_some() {
            self.pending_ranked_at = None;
        }
        if cache.update_space_interest(snapshot) {
            self.retain_visible(&cache.visible_keys);
            self.queue_needed_visible(cache, cache.center_chunk);
        }
        if !cache.visible_keys.is_empty() {
            self.enqueue_missing(gpu, snapshot, revision, cache);
        }
        self.upload_finished(gpu, snapshot, cache, revision, 3);
        // Authored orbital structures do not seed procedural ground simulation.
        cache.simulation_seeds.clear();
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
        self.pending_schedule.clear();
        self.pending_unranked.clear();
        self.pending_ranked_at = None;
        self.parked_sources = 0;
    }

    fn push_pending_key(&mut self, key: ChunkKey) {
        if self.pending_keys.insert(key) {
            self.pending_unranked.push(key);
        }
    }

    fn source_pending_priority(cache: &ChunkCache, key: ChunkKey, urgent: bool, player_edit: bool,
        parked: bool, waiting_for_edits: bool, out_of_view: bool) -> Reverse<PendingSourcePriority> {
        let (lane, column_age, _) = Self::ready_chunk_priority(cache, key);
        let (lane, column_age) = if player_edit { (0, None) } else { (lane + 1, column_age) };
        Reverse(PendingSourcePriority {
            parked, waiting_for_edits, lane, column_age, out_of_view,
            radial: pending_priority(cache.priority_center(key.coord), key, urgent).0,
        })
    }

    fn reprioritize_pending_sources(&mut self, cache: &ChunkCache, gpu: &Gpu, snapshot: &WorldSnapshot) {
        let now = Instant::now();
        let refresh = self.pending_ranked_at.is_none_or(|at| now.duration_since(at) >= SOURCE_PRIORITY_REFRESH);
        if !refresh && self.pending_unranked.is_empty() {
            return;
        }
        let urgent = &self.urgent_coords;
        let edits = &self.player_edits;
        let view = self.view.as_ref();
        let protect_radius = render_distance_settings().lod_start_chunks.max(4) as u32;
        let allow_parking = !gpu.horizon_globe_only && gpu.planet_mesh.is_none()
            && !gpu.globe_driver.orbit_view() && !super::render_isolation().skip_horizon
            && !crate::low_spec::active();
        let column_view = &mut self.column_view;
        column_view.clear();
        let mut rank = |key: ChunkKey| {
            let coord = key.coord;
            let column = (coord.x, coord.z);
            let urgent = urgent.contains(&coord);
            let player_edit = edits.contains_key(&coord);
            let distance = coord.x.abs_diff(cache.center_chunk.x).max(coord.z.abs_diff(cache.center_chunk.z));
            let view_eligible = distance > 2
                && !urgent && !player_edit && !cache.horizon_pins.contains(&column)
                && !cache.quality_proxies.contains_column(&column);
            let (out_of_view, parked) = if view_eligible && let Some(view) = view {
                *column_view.entry(column).or_insert_with(|| {
                    let out_of_view = !view.sees_column(column);
                    // Keep the coarse parent visible while the child waits.
                    let parked = out_of_view && cache.spawn_ready && allow_parking
                        && gpu.horizon.covers_streaming_column(snapshot.cfg, column);
                    (out_of_view, parked)
                })
            } else {
                (false, false)
            };
            Self::source_pending_priority(cache, key, urgent, player_edit,
                parked && key.lod > 0 && distance > protect_radius && !cache.near_stream_path(coord),
                !snapshot.edits_ready(coord) && !cache.cpu_chunks.contains_key(&coord)
                    && (cache.dirty_meshes.contains(&coord)
                        || (!cache.gpu_meshes.get(&coord).is_some_and(|mesh| mesh.lod == key.lod || mesh.lod_invariant)
                            && cache.lod_meshes.get(key).is_none())),
                out_of_view)
        };
        // Movement and urgent changes invalidate the ranking immediately.
        // Batch background cover/age changes so large rings don't reheap
        // thousands of waiting entries on every frame.
        if refresh {
            // A previous centre or LOD band may have left obsolete requests.
            // They must not consume the frame's bounded admission scan.
            self.pending_keys.retain(|key| cache.visible_keys.contains(key));
            self.pending_schedule.clear();
            let mut parked_sources = 0;
            self.pending_schedule.extend(self.pending_keys.iter().copied().map(rank).inspect(|priority| {
                parked_sources += usize::from(priority.0.parked);
            }));
            self.parked_sources = parked_sources;
            self.pending_unranked.clear();
            self.pending_ranked_at = Some(now);
        } else {
            for key in self.pending_unranked.drain(..) {
                if !cache.visible_keys.contains(&key) {
                    self.pending_keys.remove(&key);
                    continue;
                }
                let priority = rank(key);
                self.parked_sources += usize::from(priority.0.parked);
                self.pending_schedule.push(priority);
            }
        }
    }

    fn source_admission_has_capacity(&self) -> bool {
        self.ready_chunks.len() < MAX_READY_CHUNKS
            && self.ready_meshes.len() < MAX_READY_MESHES
            && self.meshing.len() < MAX_MESH_JOBS
    }

    /// A cached mesh already owns GPU memory. Bringing it back can cover a
    /// nearby hole without waiting for generation, meshing, or an upload.
    fn restore_near_gpu_cover(&mut self, cache: &mut ChunkCache) {
        const RESTORE_LIMIT: usize = 8;
        let radius = render_distance_settings().lod_start_chunks.max(2) as u32;
        let mut candidates: Vec<_> = cache.lod_meshes.keys().filter_map(|key| {
            let coord = key.coord;
            let distance = coord.x.abs_diff(cache.center_chunk.x).max(coord.z.abs_diff(cache.center_chunk.z));
            if distance > radius || !cache.visible_coords.contains(&coord)
                || !cache.ordinary_source_interested(coord) || cache.dirty_meshes.contains(&coord)
                || cache.gpu_meshes.contains_key(&coord)
            {
                return None;
            }
            let desired = ChunkKey { coord, lod: cache.desired_lod(coord) };
            // Let the in-flight exact result replace a coarse fallback. If
            // the cached mesh is already exact, leave the request in charge.
            if key == desired && self.requested_coords.contains(&coord) {
                return None;
            }
            Some((key, desired))
        }).collect();
        candidates.sort_unstable_by_key(|&(key, desired)| {
            (Self::ready_chunk_priority(cache, desired), key.lod != desired.lod, key.lod)
        });
        let mut restored = 0;
        for (key, desired) in candidates {
            if cache.gpu_meshes.contains_key(&key.coord) {
                continue;
            }
            if cache.activate_lod_unpublished(key) {
                self.pending_keys.remove(&key);
                self.last_stats.lod_cache_hits += 1;
                if key != desired && !self.requested_coords.contains(&key.coord) {
                    self.push_pending_key(desired);
                }
                restored += 1;
                if restored == RESTORE_LIMIT {
                    break;
                }
            }
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

    fn ready_chunk_priority(cache: &ChunkCache, key: ChunkKey) -> (u8, Option<(Instant, i32, i32)>, (u8, u8, i32, i32, i32, i32)) {
        let priority_center = cache.priority_center(key.coord);
        let base = chunk_sort_key(priority_center, key);
        // These are the exact meshes checked by spawn_progress. Prioritize
        // them at source admission, CPU meshing and GPU upload, so decorative
        // terrain cannot take the last slot needed to leave the loader.
        if !cache.spawn_ready && (key.coord - cache.center_chunk).abs().max_element() <= 1 {
            return (0, None, (0, base.0, base.1, base.2, base.3, base.4));
        }
        let pending_since = cache.edit_proxies.pending_since(&key.coord);
        let quality_since=cache.quality_proxies.pending_since(&key.coord);
        let resident = cache.gpu_meshes.get(&key.coord);
        let dirty = cache.dirty_meshes.contains(&key.coord);
        let ordinary = cache.ordinary_source_interested(key.coord);
        let refining = resident.is_some_and(|mesh| mesh.lod > key.lod);
        let leading_refinement = refining && {
            let settings = cache.terrain_lod_settings();
            key.lod < visibility::lod_for_chunk_with_settings(cache.center_chunk, key.coord, settings)
                && key.lod == visibility::lod_for_chunk_with_settings(
                    cache.center_chunk + cache.lod_lookahead, key.coord, settings,
                )
        };
        // Uncovered ordinary columns keep emergency priority. An added Y member
        // over an existing complete published bundle is repair, not lost cover.
        let missing_normal_cover = ordinary
            && resident.is_none()
            && !cache.published_columns.contains_key(&(key.coord.x, key.coord.z))
            && cache.column_y_cache.surface_band((key.coord.x, key.coord.z), cache.center_chunk.y)
                .is_none_or(|band| band.contains(&key.coord.y));
        // Keep the ground in the player's path ahead of the rest of the
        // visible ring. Radial ordering alone runs after other ranking fields
        // and can leave these columns behind distant work at every stage.
        if cache.spawn_ready && missing_normal_cover && cache.near_stream_path(key.coord) {
            return (0, None, (0, base.0, base.1, base.2, base.3, base.4));
        }
        let persistent_dirty=dirty && cache.edit_proxies.persistent_contains(&key.coord);
        let ordinary_dirty=ordinary && dirty;
        let distance = (key.coord.x - cache.center_chunk.x).abs()
            .max((key.coord.z - cache.center_chunk.z).abs());
        // Inside the full-detail ring a coarse mesh is the ground the player
        // is standing on. It has to refine ahead of distant holes, or those
        // cells stay in the surface long after arrival.
        let lod_start = render_distance_settings().lod_start_chunks;
        let close = distance <= lod_start;
        let coarse_near = distance <= lod_start + 1
            && key.lod == 0
            && refining;
        let lane = if missing_normal_cover || coarse_near {0} else if pending_since.is_some() || persistent_dirty {1}
            else if ordinary_dirty {2} else if quality_since.is_some() {3} else {4};
        let lane = lane + 1;
        let mesh_state = match resident {
            _ if dirty => 0u8,
            _ if cache.edit_proxies.contains(&key.coord) => 1u8,
            None if close => 2u8,
            _ if coarse_near || leading_refinement => 3u8,
            None => 4u8,
            Some(mesh) if mesh.lod > key.lod => 5u8,
            _ => 6u8,
        };
        // One update can create several columns at the same Instant. Keep the
        // column identity ahead of distance/Y so a tie cannot interleave bundles.
        let pending_column = pending_since.or(quality_since).map(|since| (since, key.coord.x, key.coord.z));
        (lane, pending_column, (mesh_state, base.0, base.1, base.2, base.3, base.4))
    }

    fn reprioritize_ready_chunks(&mut self, cache: &ChunkCache) {
        if self.ready_chunks.len() <= 1 {
            return;
        }
        let view = self.view.as_ref();
        self.ready_chunks
            .make_contiguous()
            .sort_by_cached_key(|job| Self::view_ready_priority(cache, view, job.key));
    }

    fn reprioritize_ready_meshes(&mut self, cache: &ChunkCache) {
        if self.ready_meshes.len() <= 1 {
            return;
        }
        let view = self.view.as_ref();
        self.ready_meshes
            .make_contiguous()
            .sort_by_cached_key(|mesh| (!self.player_edits.contains_key(&mesh.key.coord), Self::view_ready_priority(cache, view, mesh.key)));
    }

    fn view_ready_priority(cache: &ChunkCache, view: Option<&StreamingView>, key: ChunkKey)
        -> (u8, Option<(Instant, i32, i32)>, bool, (u8, u8, i32, i32, i32, i32))
    {
        let (lane, age, priority) = Self::ready_chunk_priority(cache, key);
        let q = key.coord;
        let out_of_view = q.x.abs_diff(cache.center_chunk.x).max(q.z.abs_diff(cache.center_chunk.z)) > 2
            && view.is_some_and(|view| !view.sees_column((q.x, q.z)));
        (lane, age, out_of_view, priority)
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
        self.source_refresh.retain(|q|coords.contains(q)||self.pending_gpu_coords.contains_key(q));
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
        self.terrain_generator.retain_coords(|coord| coords.contains(coord));
        self.requested_coords.clear();
        self.requested_coords.extend(self.requested.iter().map(|key| key.coord));
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
        self.source_refresh.retain(|q|in_range(q)||self.pending_gpu_coords.contains_key(q));
        self.requested.retain(|key| in_range(&key.coord) || self.pending_gpu_coords.contains_key(&key.coord));
        self.generating.retain(|key| in_range(&key.coord));
        self.retain_mesh_jobs(|key| in_range(&key.coord));
        self.ready_chunks.retain(|job| in_range(&job.key.coord));
        self.ready_meshes.retain(|mesh| in_range(&mesh.key.coord));
        self.pending_keys.retain(|key| in_range(&key.coord));
        self.rebuild_pending_schedule(center);
        self.requested_coords.clear();
        self.requested_coords.extend(self.requested.iter().map(|key| key.coord));
        // Simulation seed jobs have no mesh request. Preserve them on a
        // crossing while their coordinates remain within the grace range.
        self.terrain_generator.retain_coords(in_range);
    }

    fn cancel_coords(&mut self, dirty_coords: &HashSet<IVec3>) {
        if dirty_coords.is_empty() {
            return;
        }

        self.source_refresh.retain(|q|!dirty_coords.contains(q));
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
        while collected < CHUNK_MESH_COLLECT_BUDGET && self.ready_meshes.len() < MAX_READY_MESHES {
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
        let mut dispatched = self.dispatch_player_edits(cache);
        if dispatched >= CHUNK_MESH_DISPATCH_BUDGET || self.meshing.len() >= MAX_MESH_JOBS {
            return dispatched;
        }
        self.reprioritize_ready_chunks(cache);

        // Discarded jobs and cache hits also consume frame time.
        for _ in dispatched..CHUNK_MESH_DISPATCH_BUDGET {
            if self.meshing.len() >= MAX_MESH_JOBS {
                break;
            }
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
            if cache.activate_lod_unpublished(desired) {
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
                    job.source.water_motion.seal();
                    self.player_edit_dispatched(job.key.coord,job.revision);
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
                        Ok((tx, urgent, rx)) => {
                            self.urgent_job_tx = urgent;
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
        let restart: HashSet<_> = dirty_coords
            .iter()
            .copied()
            .filter(|coord| !self.requested_coords.contains(coord)
                || (self.needs_player_restart(coord) && !self.pending_gpu_coords.contains_key(coord)))
            .collect();
        for &coord in dirty_coords {
            self.dirty_revisions.insert(coord, revision);
        }
        if !restart.is_empty() {
            self.cancel_coords(&restart);
        }
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
        let mut ca_coords = None;
        for &coord in dirty_coords {
            let delta = coord - cache.center_chunk;
            if delta.x * delta.x + delta.z * delta.z <= radius * radius
                && !cache.visible_coords.contains(&coord)
                && (snapshot.chunks.contains_key(&coord)
                    || ca_coords.get_or_insert_with(|| snapshot.ca_chunks().collect::<HashSet<_>>()).contains(&coord))
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

            if !restart.contains(&coord) {
                let clone_bytes = cache.cpu_chunks.get(&coord)
                    .filter(|source| Arc::strong_count(source) > 1)
                    .map_or(0, |source| source.allocation_bytes());
                // Patch rather than regenerate: simulation writes reach the
                // journal exactly like player edits, and regeneration is the cost.
                if (!*crate::AB_OLD || self.player_edits.contains_key(&coord)) && snapshot.edits_ready(coord)
                    && cache.cpu_bytes().saturating_add(clone_bytes) <= cache.residency_limits.0.saturating_sub(terrain::reservation_bytes()) {
                    if let Some(source) = cache.cpu_chunks.get_mut(&coord) {
                        patch_source_halo(Arc::make_mut(source), halo_edits.get(&coord), snapshot);
                    }
                } else {
                    cache.cpu_chunks.remove(&coord);
                }
                continue;
            }

            let lod = cache.desired_lod(coord);
            if !snapshot.edits_ready(coord) {
                self.push_pending_key(ChunkKey { coord, lod });
                continue;
            }
            let clone_bytes = cache.cpu_chunks.get(&coord)
                .filter(|source| Arc::strong_count(source) > 1)
                .map_or(0, |source| source.allocation_bytes());
            if clone_bytes > 0 && (*crate::AB_OLD && !self.player_edits.contains_key(&coord) ||
                cache.cpu_bytes().saturating_add(clone_bytes) > cache.residency_limits.0.saturating_sub(terrain::reservation_bytes())) {
                cache.cpu_chunks.remove(&coord);
                self.push_pending_key(ChunkKey { coord, lod });
                continue;
            }
            let Some(chunk_data) = cache.cpu_chunks.get_mut(&coord) else {
                self.push_pending_key(ChunkKey { coord, lod });
                continue;
            };

            let chunk_data = Arc::make_mut(chunk_data);
            patch_source_halo(chunk_data, halo_edits.get(&coord), snapshot);

            let key = ChunkKey { coord, lod };
            self.requested.insert(key);
            self.requested_coords.insert(key.coord);
            self.ready_chunks.push_front(ChunkJob {
                key,
                revision,
                source: cache.cpu_chunks[&key.coord].clone(),
            });
            self.player_edit_dispatched(coord,revision);
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
        crate::gameplay_trace::frame_stage("upload-residency-done");
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

        let integration_started = Instant::now();
        for _ in 0..CHUNK_GENERATION_COLLECT_BUDGET {
            if integration_started.elapsed() >= CHUNK_GENERATION_INTEGRATE_TIME
                || self.ready_chunks.len() >= MAX_READY_CHUNKS
            {
                break;
            }
            let Ok(result) = self.terrain_generator
                .collect_one(snapshot, |coord| cache.source_interested(&coord)) else { break; };
            let Some((key, chunk_data, activity, seed_only, seed_activity)) = result else {
                continue;
            };
            if seed_only {
                if cache.ordinary_source_interested(key.coord) && cache.seeded_sources.insert(key.coord) {
                    cache.edit_proxies.normal_promotions += 1;
                    if let Some(mask) = activity { cache.simulation_seeds.push((key.coord, mask)); }
                }
                continue;
            }
            self.generating.remove(&key);
            // Drop abandoned results without overlaying edits or requeuing them.
            let interested = cache.source_interested(&key.coord);
            let Some(chunk_data) = chunk_data else {
                self.requested.remove(&key);
                self.requested_coords.remove(&key.coord);
                if interested { self.push_pending_key(key); }
                continue;
            };
            // Accept terrain data as long as the chunk is still in the visible
            // range — don't discard on revision mismatch.  A world edit (which
            // bumps revision) will re-queue via rebuild_dirty anyway.
            if !interested {
                self.requested.remove(&key);
                self.requested_coords.remove(&key.coord);
                continue;
            }
            let ordinary = cache.ordinary_source_interested(key.coord);
            if !seed_activity && !ordinary {
                cache.edit_proxies.source_only_completed += 1;
            }
            if seed_activity && ordinary && cache.seeded_sources.insert(key.coord)
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
                // These uniform volumes and their halos have no active
                // simulation cells. There is no retained source to rediscover.
                if ordinary && matches!(block, crate::world::BlockId::Air | crate::world::BlockId::Water) {
                    cache.seeded_sources.insert(key.coord);
                }
                self.ready_meshes.push_back(CpuChunkMesh {
                    observation_source: Default::default(),
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
        let mut deferred_sources = Vec::new();
        if !self.source_admission_has_capacity() || self.terrain_generator.available_slots() == 0 {
            self.restore_near_gpu_cover(cache);
        }
        // Limit how many entries we scan from the pending queue per frame so we
        // don't stall when a large ring arrives at once.
        if self.source_admission_has_capacity() && !self.pending_keys.is_empty() {
            self.reprioritize_pending_sources(cache, gpu, snapshot);
        }
        let scan_limit = self.pending_schedule.len().min(schedule_budget * 4);
        let mut scanned = 0usize;
        let mut request_budget = residency::RequestBudget::default();
        while scheduled < schedule_budget && scanned < scan_limit {
            if !self.source_admission_has_capacity() {
                break;
            }
            if self.pending_schedule.peek().is_some_and(|priority| {
                priority.0.parked || priority.0.waiting_for_edits
            }) {
                break;
            }
            let Some(Reverse(priority)) = self.pending_schedule.pop() else {
                break;
            };
            let (_, _, _, _, kx, ky, kz, klod) = priority.radial;
            let key = ChunkKey {
                coord: IVec3::new(kx, ky, kz),
                lod: klod,
            };
            self.pending_keys.remove(&key);
            scanned += 1;
            if self.source_observations.blocked && cache.edit_proxies.contains(&key.coord) {continue;}

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
            if !cache.budget_allows_request(key, &mut request_budget) {
                continue;
            }
            if cache.activate_lod_unpublished(key) {
                request_budget = residency::RequestBudget::default();
                self.last_stats.lod_cache_hits += 1;
                continue;
            }
            // A running job owns the coordinate until completion, but a
            // cached mesh above can still cover it while that job is pending.
            if self.requested_coords.contains(&key.coord) {
                continue;
            }
            if let Some(source) = cache.cpu_chunks.get(&key.coord).cloned() {
                self.requested.insert(key);
                self.requested_coords.insert(key.coord);
                self.ready_chunks.push_back(ChunkJob {
                    key,
                    revision,
                    source,
                });
                self.player_edit_dispatched(key.coord,revision);
                scheduled += 1;
                continue;
            }
            if terrain_to_schedule.len() >= self.terrain_generator.available_slots()
                || !snapshot.edits_ready(key.coord)
            {
                // A busy generator or an unloaded save region should not stop
                // later keys that can reuse CPU voxels or an existing GPU LOD.
                deferred_sources.push(key);
                continue;
            }
            self.requested.insert(key);
            self.requested_coords.insert(key.coord);
            self.generating.insert(key);
            self.player_edit_dispatched(key.coord,revision);
            terrain_to_schedule.push(key);
            scheduled += 1;
        }

        for key in deferred_sources {
            self.push_pending_key(key);
        }

        // Repair missed requests fairly with a rotating, bounded scan. Repair
        // only seeds the heap, so all jobs use the same capacity checks above.
        if self.source_admission_has_capacity() && !cache.visible.is_empty() {
            for _ in 0..cache.visible.len().min(64) {
                self.repair_cursor %= cache.visible.len();
                let key = cache.visible[self.repair_cursor];
                self.repair_cursor += 1;
                if self.pending_keys.contains(&key) || self.requested_coords.contains(&key.coord) {
                    continue;
                }
                let complete = cache.gpu_meshes.get(&key.coord).is_some_and(|mesh| {
                    mesh.lod == key.lod && !cache.dirty_meshes.contains(&key.coord)
                });
                if !complete
                    && cache.budget_allows_request(key, &mut request_budget)
                {
                    self.push_pending_key(key);
                }
            }
        }
        let terrain_scheduled = terrain_to_schedule.len();
        self.terrain_generator
            .schedule_many(gpu, terrain_to_schedule.into_iter().map(|key| {
                // Near voxels seed simulation before play. Coarse terrain can
                // publish first and use the bounded seed promotion below.
                (key, key.lod == 0 && cache.ordinary_source_interested(key.coord))
            }), snapshot.cfg);
        // A cached voxel source admitted above can start meshing this frame.
        // Sort it with completed generation results under the same dispatch budget.
        let mesh_dispatched = self.dispatch_ready_chunks(cache, revision);
        // A render-only source may already have its final GPU mesh when it
        // enters ordinary interest. Discover on workers once, without forcing
        // a remesh or merging proxy-generated activity into the simulation.
        // A full scan on every settled frame costs O(view area), and used to
        // search all pending jobs for each candidate. Rotate bounded work even
        // when the first thousands of sources have already seeded simulation.
        let seed_slots = self.terrain_generator.available_slots().min(4);
        let mut promoted = 0;
        for _ in 0..cache.visible.len().min(64) {
            if promoted >= seed_slots { break; }
            self.seed_cursor %= cache.visible.len();
            let key = cache.visible[self.seed_cursor];
            self.seed_cursor += 1;
            if cache.seeded_sources.contains(&key.coord)
                || !cache.gpu_meshes.contains_key(&key.coord)
                || self.requested_coords.contains(&key.coord)
                || !cache.ordinary_source_interested(key.coord)
                || self.terrain_generator.seed_pending(key.coord)
            { continue; }
            self.terrain_generator.schedule_seed(key, snapshot.cfg, cache.cpu_chunks.get(&key.coord).cloned());
            promoted += 1;
        }
        cache.refresh_published_columns();
        self.last_stats.terrain_generated += terrain_generated;
        self.last_stats.terrain_scheduled += terrain_scheduled;
        self.last_stats.mesh_dispatched += mesh_dispatched;
        self.last_stats.requested = self.requested.len();
        self.last_stats.generating = self.generating.len();
        self.last_stats.meshing = self.meshing.len();
        self.last_stats.ready_chunks = self.ready_chunks.len();
        self.last_stats.ready_meshes = self.ready_meshes.len();
        self.last_stats.pending_schedule = self.pending_keys.len();
        self.last_stats.parked_offscreen = self.parked_sources;
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
        // A previous redraw may have returned before submitting a frame.
        gpu.flush_chunk_uploads();
        let started=Instant::now();
        // Cancellation can empty pending_uploads before transfer callbacks run.
        // Keep nonblocking completion progress independent of publication state.
        let transfers_pending = gpu.upload_staging.jobs() > 0;
        let mut poll_error = None;
        if transfers_pending || self.has_upload_checks() {
            if let Err(error) = gpu.device.poll(wgpu::PollType::Poll) {
                if transfers_pending {
                    self.upload_error = Some(format!("Terrain transfer polling failed: {error}"));
                    return;
                }
                poll_error = Some(format!("GPU polling failed: {error}"));
            }
        }
        let (published,vertices,indices)=self.publish_pending_uploads(&gpu.device_lost,cache,max_uploads,started,poll_error);
        crate::gameplay_trace::frame_stage("upload-poll-done");
        self.last_stats.meshes_uploaded+=published;
        self.last_stats.uploaded_vertices+=vertices;
        self.last_stats.uploaded_indices+=indices;
        if !gpu.device_usable() {self.upload_error=Some("GPU device loss reported".into());return;}
        // Runs between frames (never while an encoder is open), so ranges freed
        // since the last submit can be reused for this frame's writes.
        cache.arena.begin_frame();
        crate::gameplay_trace::frame_stage("upload-retirement-done");
        let mut staged=0usize;let mut staged_vertices=0usize;let mut staged_indices=0usize;
        let mut transfer_credits = Vec::new();
        self.collect_finished_meshes();
        if self.pending_uploads.len() < 16 && Instant::now() >= self.upload_retry_after
            && (published == 0 || started.elapsed() < UPLOAD_TIME_BUDGET)
        {
            self.reprioritize_ready_meshes(cache);
        }
        crate::gameplay_trace::frame_stage("upload-ready-done");
        for attempt in 0..CHUNK_UPLOAD_SCAN_BUDGET {
            if !gpu.device_usable() || Instant::now() < self.upload_retry_after
                || self.pending_uploads.len() >= 16
                || ((attempt > 0 || published > 0) && started.elapsed() >= UPLOAD_TIME_BUDGET)
            {
                break;
            }
            // An in-flight upload owns its coordinate, but must not stall
            // publication of unrelated terrain behind it in the ready queue.
            let Some(index) = self.ready_meshes.iter().position(|mesh|
                !self.pending_gpu_coords.contains_key(&mesh.key.coord)
            ) else { break; };
            let cpu_mesh = self.ready_meshes.remove(index).expect("ready mesh index");
            let key=cpu_mesh.key;
            let mesh_vertices=cpu_mesh.mesh.vertices.len();let mesh_indices=cpu_mesh.mesh.indices.len();
            let has_geometry = mesh_vertices != 0 || mesh_indices != 0;
            if !self.cpu_world_current(cache,&cpu_mesh) || !cache.source_interested(&key.coord) {
                self.retire_upload_request(key);continue;
            }
            let _=current_revision; // unrelated edits do not stale this dependency halo
            let desired_lod=cache.desired_lod(key.coord);
            if older_than_display(cache,key.coord,cpu_mesh.revision) {
                self.retire_upload_request(key);
                if edited_mesh_is_stale(self.dirty_revisions.get(&key.coord).copied(),cpu_mesh.revision) {
                    cache.dirty_meshes.insert(key.coord);
                    self.push_pending_key(ChunkKey{coord:key.coord,lod:desired_lod});
                }
                continue;
            }
            if edited_mesh_is_stale(self.dirty_revisions.get(&key.coord).copied(),cpu_mesh.revision) {
                cache.dirty_meshes.insert(key.coord);
                self.push_pending_key(ChunkKey{coord:key.coord,lod:desired_lod});
            }
            if desired_lod!=key.lod && cache.gpu_meshes.get(&key.coord)
                .is_some_and(|m|m.lod==desired_lod&&!cache.dirty_meshes.contains(&key.coord)) {
                self.retire_upload_request(key);continue;
            }
            let bytes=mesh_vertices*std::mem::size_of::<crate::mesh::Vertex3>()+mesh_indices*4;
            // An individually oversized result cannot become admissible by
            // waiting. Do not let it permanently block the ready queue head.
            if bytes > super::upload_staging::BYTE_LIMIT {
                cache.budget_deferred.insert(key, bytes);
                self.retire_upload_request(key);
                continue;
            }
            if let Err(error)=gpu_upload::supported(&gpu.device,&cpu_mesh.mesh) {
                eprintln!("Chunk GPU upload rejected: {error}");self.upload_error=Some(error);
                cache.budget_deferred.insert(key,usize::MAX);self.retire_upload_request(key);continue;
            }
            // Obsolete and oversized results must be retired even when a
            // valid mesh of the same size would exhaust this frame's budget.
            let exceeds_geometry = vertices + staged_vertices + mesh_vertices > CHUNK_UPLOAD_VERTEX_BUDGET
                || indices + staged_indices + mesh_indices > CHUNK_UPLOAD_INDEX_BUDGET;
            let geometry_started = staged != 0 || vertices != 0 || indices != 0;
            if has_geometry && (staged >= max_uploads || (geometry_started && exceeds_geometry)) {
                self.ready_meshes.insert(index, cpu_mesh);
                break;
            }
            let Some(reservation)=cache.reserve_pending_gpu(key,bytes) else {
                self.retire_upload_request(key);continue;
            };
            if !has_geometry {
                // Complete columns still need an empty member to be published.
                // Keep its edit/epoch checks and grouping, but there is no GPU
                // allocation, copy or validation scope to wait on.
                let empty = ChunkGpuMesh {
                    boundary_profile: None,
                    observation_source: Default::default(),
                    alloc: None,
                    index_count: 0,
                    sways: false,
                    source_revision: 0,
                    lod: key.lod,
                    lod_invariant: cpu_mesh.lod_invariant,
                    outgoing_lod: None,
                    bytes: 0,
                    connectivity: None,
                };
                self.begin_pending_upload(cpu_mesh, Ok(empty), None, reservation, cache.gpu_world_epoch);
                continue;
            }
            let Some(transfer_credit) = gpu.upload_staging.reserve(bytes) else {
                drop(reservation);
                self.ready_meshes.insert(index, cpu_mesh);
                break;
            };
            // A full arena is a budget reject like the reservation above, not an upload error.
            let Ok(alloc)=cache.arena.alloc_mesh(&gpu.device,&cpu_mesh.mesh) else {
                drop(reservation);self.retire_upload_request(key);continue;
            };
            crate::gameplay_trace::frame_stage("upload-allocation-done");
            let(attempt,check)=gpu_upload::start(&gpu.device,&gpu.queue,&cpu_mesh.mesh,key,
                cpu_mesh.lod_invariant,&cache.arena,alloc);
            crate::gameplay_trace::frame_stage("upload-copy-done");
            // Even a failed second write can leave the first queued. Keep the
            // full conservative reservation until this batch completes.
            transfer_credits.push(transfer_credit);
            self.begin_pending_upload(cpu_mesh,attempt,check,reservation,cache.gpu_world_epoch);
            staged+=1;staged_vertices+=mesh_vertices;staged_indices+=mesh_indices;
        }
        crate::gameplay_trace::frame_stage("upload-submit");
        if !transfer_credits.is_empty() {
            *gpu.chunk_upload_credits.lock().unwrap() = transfer_credits;
        }
        crate::gameplay_trace::frame_stage("upload-submit-done");
        self.last_stats.upload_staging_bytes = gpu.upload_staging.bytes();
        self.last_stats.upload_staging_jobs = gpu.upload_staging.jobs();
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
        self.last_stats.pending_schedule = self.pending_keys.len();
        self.last_stats.parked_offscreen = self.parked_sources;
    }

    pub fn expansion_pressure(&self) -> usize {
        // Pending keys include work that cannot start yet. Let the bounded
        // column planner discover the next ring while those keys wait.
        self.generating.len()
            + self.meshing.len()
            + self.ready_chunks.len()
            + self.ready_meshes.len()
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
    let mut has_edits = false;
    for dy in -1..=1 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let coord = chunk_data.chunk + IVec3::new(dx, dy, dz);
                let Some(chunk) = snapshot.chunks.get(&coord) else {
                    continue;
                };
                has_edits = true;
                let origin = coord * CHUNK_SIZE;
                let local_min = (min - origin).max(IVec3::ZERO);
                let local_end = (max - origin).min(IVec3::splat(CHUNK_SIZE));
                let width = (local_end.x - local_min.x) as usize;
                for y in local_min.y..local_end.y {
                    for z in local_min.z..local_end.z {
                        let source = crate::world::ChunkData::local_index(local_min.x, y, z);
                        let local = origin + IVec3::new(local_min.x, y, z) - min;
                        let target = ((local.y * PADDED_CHUNK_SIZE + local.z)
                            * PADDED_CHUNK_SIZE + local.x) as usize;
                        chunk_data.blocks[target..target + width]
                            .copy_from_slice(&chunk.blocks[source..source + width]);
                    }
                }
            }
        }
    }
    let edited_cell = |p: IVec3| {
        has_edits && snapshot.chunks.contains_key(&crate::world::chunk_coord_from_world_block(p))
    };
    // Saved chunks own their water stocks. Strip procedural entries in the
    // edited volume, then overlay sparse snapshot data instead of looking up
    // water twice for every voxel in a saved chunk and its halo.
    if has_edits {
        let keep_default = |p: IVec3| !(p.cmpge(min).all() && p.cmplt(max).all() && edited_cell(p));
        chunk_data.water_levels.retain(|&p, _| keep_default(p));
        chunk_data.water_deficit.retain(|&p, _| keep_default(p));
    }
    chunk_data.water_deficit.extend(snapshot.water_levels.precision.in_box(min, max)
        .filter(|&(p, deficit)| deficit != 0 || !edited_cell(p)));
    chunk_data.water_levels.extend(snapshot.water_levels.in_box(min, max)
        .map(|(&p, &v)| (p, v)));
    snapshot.apply_ca_cells(chunk_data);
}

#[cfg(test)]
mod caster_log_tests {
    use super::ChunkCache;
    use glam::IVec3;

    #[test]
    fn caster_changes_are_attributed_to_columns_since_a_revision() {
        let mut cache = ChunkCache::new(IVec3::ZERO);
        let start = cache.shadow_caster_revision;
        cache.log_caster_change(IVec3::new(3, 1, -2));
        cache.log_caster_change(IVec3::new(-7, 0, 4));
        assert_eq!(cache.shadow_caster_columns_since(start).as_deref(), Some(&[(3, -2), (-7, 4)][..]));
        assert_eq!(cache.shadow_caster_columns_since(start + 1).as_deref(), Some(&[(-7, 4)][..]));
        assert_eq!(cache.shadow_caster_columns_since(start + 2).as_deref(), Some(&[][..]));
        assert!(cache.shadow_caster_columns_since(start + 3).is_none(), "a future revision is unattributed");
    }

    #[test]
    fn membership_changes_and_a_trimmed_log_are_unattributed() {
        let mut cache = ChunkCache::new(IVec3::ZERO);
        let start = cache.shadow_caster_revision;
        cache.log_caster_change(IVec3::new(1, 0, 1));
        cache.mark_mesh_set_changed();
        assert!(cache.shadow_caster_columns_since(start).is_none());
        assert!(cache.shadow_caster_columns_since(start + 1).is_none());
        let after_bulk = cache.shadow_caster_revision;
        assert_eq!(cache.shadow_caster_columns_since(after_bulk).as_deref(), Some(&[][..]));
        for i in 0..2100 {
            cache.log_caster_change(IVec3::new(i, 0, 0));
        }
        assert!(cache.shadow_caster_columns_since(after_bulk).is_none(), "older than the retained log");
        let recent = cache.shadow_caster_revision - 1;
        assert_eq!(cache.shadow_caster_columns_since(recent).as_deref(), Some(&[(2099, 0)][..]));
    }
}
