//! Explicit budgets for regenerable world data. Worker-held voxel allocations
//! remain charged after cache eviction. Edited world data is never evicted here.
use crate::mesh::GeneratedChunk;
use crate::world::{BlockId, CHUNK_SIZE};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, OnceLock};
use std::sync::atomic::{AtomicI8, AtomicUsize, Ordering};

const MIB: usize = 1024 * 1024;
const WEATHER_COLUMNS: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;
const WEATHER_ROOF_BYTES: usize = WEATHER_COLUMNS * std::mem::size_of::<AtomicI8>();
const UNKNOWN_ROOF: i8 = -2;
static CPU_MIB: AtomicUsize = AtomicUsize::new(256);
// Leave bounded room for an incoming mesh while its old buffers remain drawable.
pub(super) const GPU_STREAMING_RESERVE: usize = 16 * MIB;

pub fn world_residency_budgets_mib() -> (usize, usize) {
    (
        CPU_MIB.load(Ordering::Relaxed),
        super::gpu_budget::effective_mib(),
    )
}

pub fn set_world_residency_budgets_mib(cpu: usize, gpu: usize) {
    CPU_MIB.store(cpu.clamp(32, 2048), Ordering::Relaxed);
    super::gpu_budget::set_limit_mib(gpu);
}

pub(super) fn limits() -> (usize, usize) {
    let (cpu, gpu) = world_residency_budgets_mib();
    (cpu * MIB, gpu * MIB)
}

/// Mesh arena pages: the GPU mesh limit plus the LOD cache, with a quarter
/// extra for fragmentation. Pages are kept once made, so this caps VRAM.
pub(super) fn arena_budget(gpu_limit: usize) -> usize {
    (gpu_limit + super::lod_cache::MAX_BYTES) / 4 * 5
}

#[derive(Default)]
pub(super) struct VoxelAllocations(pub AtomicUsize);

/// Dropped only when the cache AND every worker release the source allocation.
pub struct ResidentChunk {
    data: GeneratedChunk,
    allocations: Arc<VoxelAllocations>,
    bytes: usize,
    /// Torch cells, scanned on first light query and dropped on any mutable access.
    torches: OnceLock<Vec<(IVec3, BlockId)>>,
    weather_roofs: OnceLock<Box<[AtomicI8]>>,
}

impl ResidentChunk {
    pub(super) fn reaccount(&mut self, allocations: &Arc<VoxelAllocations>) {
        let bytes = self.bytes + self.weather_roof_bytes();
        self.allocations.0.fetch_sub(bytes, Ordering::Relaxed);
        allocations.0.fetch_add(bytes, Ordering::Relaxed);
        self.allocations = Arc::clone(allocations);
    }
    pub(super) fn new(data: GeneratedChunk, allocations: &Arc<VoxelAllocations>) -> Self {
        let bytes = data.allocation_bytes();
        allocations.0.fetch_add(bytes, Ordering::Relaxed);
        Self {
            data,
            allocations: Arc::clone(allocations),
            bytes,
            torches: OnceLock::new(),
            weather_roofs: OnceLock::new(),
        }
    }

    fn append_torch_cells(&self, out: &mut Vec<(IVec3, BlockId)>) {
        out.extend_from_slice(self.torches.get_or_init(|| self.data.torch_cells()));
    }

    fn weather_roof_bytes(&self) -> usize {
        self.weather_roofs.get().map_or(0, |_| WEATHER_ROOF_BYTES)
    }

    /// Highest solid or liquid cell in this column, or one below the chunk.
    pub(crate) fn weather_roof(&self, p: IVec3) -> i32 {
        let roofs = self.weather_roofs.get_or_init(|| {
            let roofs = (0..WEATHER_COLUMNS).map(|_| AtomicI8::new(UNKNOWN_ROOF)).collect();
            self.allocations.0.fetch_add(WEATHER_ROOF_BYTES, Ordering::Relaxed);
            roofs
        });
        let index = (p.z.rem_euclid(CHUNK_SIZE) * CHUNK_SIZE + p.x.rem_euclid(CHUNK_SIZE)) as usize;
        let bottom = self.data.chunk.y * CHUNK_SIZE;
        let mut roof = roofs[index].load(Ordering::Relaxed);
        if roof == UNKNOWN_ROOF {
            roof = (0..CHUNK_SIZE)
                .rev()
                .find(|&y| {
                    let block = self.data.get_world(IVec3::new(p.x, bottom + y, p.z));
                    block.is_solid() || matches!(block, BlockId::Water | BlockId::Lava)
                })
                .map_or(-1, |y| y as i8);
            roofs[index].store(roof, Ordering::Relaxed);
        }
        bottom + i32::from(roof)
    }

    pub(super) fn refresh_allocation_size(&mut self) {
        let bytes = self.data.allocation_bytes();
        if bytes >= self.bytes {
            self.allocations
                .0
                .fetch_add(bytes - self.bytes, Ordering::Relaxed);
        } else {
            self.allocations
                .0
                .fetch_sub(self.bytes - bytes, Ordering::Relaxed);
        }
        self.bytes = bytes;
    }
}

// A player edit may copy a worker-held immutable source within the source
// budget. Each copy owns its own allocation charge and releases it on drop.
impl Clone for ResidentChunk {
    fn clone(&self) -> Self { Self::new(self.data.clone(), &self.allocations) }
}

impl Deref for ResidentChunk {
    type Target = GeneratedChunk;
    fn deref(&self) -> &GeneratedChunk {
        &self.data
    }
}
impl DerefMut for ResidentChunk {
    fn deref_mut(&mut self) -> &mut GeneratedChunk {
        self.torches.take();
        if self.weather_roofs.take().is_some() {
            self.allocations.0.fetch_sub(WEATHER_ROOF_BYTES, Ordering::Relaxed);
        }
        &mut self.data
    }
}
impl Drop for ResidentChunk {
    fn drop(&mut self) {
        // Free the actual array before making its budget available again.
        drop(std::mem::take(&mut self.data.blocks));
        drop(std::mem::take(&mut self.data.water_levels));
        drop(std::mem::take(&mut self.data.water_deficit));
        drop(std::mem::take(&mut self.data.water_motion));
        self.allocations.0.fetch_sub(self.bytes + self.weather_roof_bytes(), Ordering::Relaxed);
    }
}

use super::{ChunkCache, ChunkKey};
use glam::IVec3;

const MAX_COVER_RADIUS: i32 = 8;
const COVER_RANKS: usize = (MAX_COVER_RADIUS * MAX_COVER_RADIUS + 1) as usize;

// The scheduling and repair loops share one view of reclaimable GPU memory.
#[derive(Default)]
pub(super) struct RequestBudget {
    pending_growth: Option<usize>,
    cover: Option<[usize; COVER_RANKS]>,
}

impl ChunkCache {
    /// Torch cells of every resident source within `chunk_radius` chunks of `center`.
    pub(crate) fn append_torch_cells_within(
        &self, center: IVec3, chunk_radius: i32,
        include_chunk: impl Fn(IVec3) -> bool, out: &mut Vec<(IVec3, BlockId)>,
    ) {
        for (coord, source) in &self.cpu_chunks {
            if (*coord - center).abs().max_element() <= chunk_radius && include_chunk(*coord) {
                source.append_torch_cells(out);
            }
        }
    }

    pub(super) fn mesh_limit(&self) -> usize {
        // Keep bounded growth room for nearby edited meshes. Ordinary
        // streaming must not consume the last bytes then protect every
        // displayed column from eviction, permanently blocking replacement.
        let reserve=(self.residency_limits.1/8).max(GPU_STREAMING_RESERVE).min(64*MIB);
        self.residency_limits.1.saturating_sub(reserve)
    }

    fn replacement_limit(&self,coord:IVec3)->usize {
        if (coord-self.center_chunk).abs().max_element()<=1 && self.gpu_meshes.contains_key(&coord) {
            self.residency_limits.1.saturating_sub(GPU_STREAMING_RESERVE)
        } else {self.mesh_limit()}
    }

    pub(super) fn trim_lod_budget(&mut self) {
        if self.frozen {
            return;
        }
        self.lod_meshes
            .trim_to(self.mesh_limit().saturating_sub(self.gpu_bytes).saturating_sub(self.pending_gpu.bytes()));
    }

    // Total ordering prevents equal-distance chunks from evicting each other.
    pub(super) fn residency_rank(&self, coord: IVec3) -> (i32, i64, i32, i32, i32) {
        let d = (coord - self.center_chunk).as_i64vec3();
        (
            (d.abs().max_element() > 1) as i32,
            d.x * d.x + d.z * d.z,
            coord.y,
            coord.z,
            coord.x,
        )
    }

    pub(super) fn source_limit(&self) -> usize {
        self.residency_limits
            .0
            .saturating_sub(super::terrain::reservation_bytes())
    }

    pub(super) fn cpu_bytes(&self) -> usize {
        self.voxel_allocations.0.load(Ordering::Relaxed)
    }

    /// Sources are regenerable meshing inputs, so a published column keeps
    /// its GPU meshes, never its sources: at view 28 the published sources
    /// alone exceed the budget, and protecting them left nothing to evict,
    /// so admission failed and the last ring regenerated forever.
    fn trim_sources_to(&mut self, limit: usize) {
        if self.frozen || self.cpu_bytes() <= limit {
            return;
        }
        let mut victims: Vec<_> = self
            .cpu_chunks
            .iter()
            .filter(|(q, source)| Arc::strong_count(source) == 1
                && !self.horizon_pin_coords.contains(*q)
                && !self.edit_proxies.contains(q)
                && !self.quality_proxies.coords.contains(*q))
            .map(|(&coord, _)| (self.residency_rank(coord), coord))
            .collect();
        victims.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        for (_, coord) in victims {
            if self.cpu_bytes() <= limit {
                break;
            }
            self.cpu_chunks.remove(&coord);
            self.cpu_evictions += 1;
        }
    }

    pub(super) fn admit_source(&mut self, coord: IVec3, data: GeneratedChunk) -> bool {
        let bytes = data.allocation_bytes();
        // This cache is a current meshing source, not a published GPU snapshot.
        // A rejected regenerated source must not leave an older cache entry
        // which enqueue_missing could stamp with the current revision.
        self.cpu_chunks.remove(&coord);
        if self.frozen && self.cpu_bytes().saturating_add(bytes) > self.source_limit() {
            // Deferred, not evicted for: the caller regenerates it after thaw.
            return false;
        }
        if self.cpu_bytes().saturating_add(bytes) > self.source_limit() {
            // Evict a batch to a low-water mark: sorting once avoids repeatedly
            // scanning the whole cache per source, or quadratic slider changes.
            self.trim_sources_to((self.source_limit() * 7 / 8).saturating_sub(bytes));
        }
        if self.cpu_bytes().saturating_add(bytes) > self.source_limit() {
            return false;
        }
        self.cpu_chunks.insert(
            coord,
            Arc::new(ResidentChunk::new(data, &self.voxel_allocations)),
        );
        true
    }

    pub(super) fn reserve_pending_gpu(&mut self,key:ChunkKey,bytes:usize) -> Option<super::PendingGpuLease> {
        if self.pending_gpu.contains(&key.coord) {return None;}
        if !self.admit_mesh(key,bytes) {return None;}
        let reservation=self.pending_gpu.reserve(key.coord,bytes);
        if reservation.is_none(){self.budget_deferred.insert(key,bytes);}
        reservation
    }
    /// Reservation already includes this destination; do not charge it twice.
    pub(super) fn pending_publication_fits(&self,key:ChunkKey,bytes:usize) -> bool {
        let all=self.gpu_bytes.saturating_add(self.lod_meshes.bytes()).saturating_add(self.pending_gpu.bytes());
        let own_growth=bytes.saturating_sub(self.gpu_meshes.get(&key.coord).map_or(0,|m|m.bytes));
        let post=self.gpu_bytes.saturating_sub(self.gpu_meshes.get(&key.coord).map_or(0,|m|m.bytes))
            .saturating_add(bytes).saturating_add(self.pending_gpu.projected_growth(self).saturating_sub(own_growth));
        all<=self.residency_limits.1 && post<=self.replacement_limit(key.coord) && self.quality_mesh_fits(key,bytes,true)
            && self.supplemental_gpu_bytes().saturating_sub(
                if self.horizon_pin_coords.contains(&key.coord)||self.edit_proxies.contains(&key.coord) {
                    self.gpu_meshes.get(&key.coord).map_or(0,|m|m.bytes)
                } else {0}).saturating_add(self.pending_gpu.supplemental_bytes(self))<=super::edit_proxies::BYTE_LIMIT
    }

    fn is_near_surface_request(&self, key: ChunkKey) -> bool {
        let settings = super::render_distance_settings();
        let radius = if self.residency_limits.1 <= 512 << 20 {
            settings.view_distance_chunks.min(MAX_COVER_RADIUS)
        } else {
            settings.lod_start_chunks.clamp(2, MAX_COVER_RADIUS)
        };
        let delta = (key.coord - self.center_chunk).as_i64vec3();
        delta.x * delta.x + delta.z * delta.z <= i64::from(radius).pow(2)
            && !self.gpu_meshes.contains_key(&key.coord)
            && self.ordinary_source_interested(key.coord)
    }

    /// A saturated cache may consist entirely of published columns. Retire a
    /// complete, distant column so new near terrain can publish. Removing the
    /// whole bundle also lets the horizon cover its footprint again.
    fn distant_cover(&self) -> impl Iterator<Item = ((i32, i32), i64, usize)> + '_ {
        self.published_columns.iter().filter_map(move |(&column, members)| {
            let dx = i64::from(column.0) - i64::from(self.center_chunk.x);
            let dz = i64::from(column.1) - i64::from(self.center_chunk.z);
            if dx.abs().max(dz.abs()) <= 2 || members.iter().any(|q| {
                self.horizon_pin_coords.contains(q) || self.edit_proxies.contains(q)
                    || self.quality_proxies.coords.contains(q) || self.pending_gpu.contains(q)
            }) {
                return None;
            }
            let bytes = members.iter().filter_map(|q| self.gpu_meshes.get(q)).map(|mesh| mesh.bytes).sum();
            (bytes > 0).then_some((column, dx * dx + dz * dz, bytes))
        })
    }

    fn retire_distant_cover(&mut self, key: ChunkKey, deficit: usize) -> bool {
        if !self.is_near_surface_request(key) { return false; }
        let rank = self.residency_rank(key.coord).1;
        let mut candidates: Vec<_> = self.distant_cover()
            .filter(|&(_, distance, _)| distance > rank)
            .collect();
        if candidates.iter().map(|&(_, _, bytes)| bytes).sum::<usize>() < deficit {
            return false;
        }
        candidates.sort_unstable_by_key(|&(column, distance, _)| std::cmp::Reverse((distance, column)));
        let mut freed = 0;
        for (column, _, bytes) in candidates {
            let members = self.published_columns.remove(&column).expect("selected published column");
            self.arriving_columns.remove(&column);
            for coord in members {
                self.published_coords.remove(&coord);
                if let Some(mesh) = self.gpu_meshes.remove(&coord) {
                    self.gpu_bytes -= mesh.bytes;
                    self.budget_deferred.insert(ChunkKey { coord, lod: mesh.lod }, mesh.bytes);
                    self.gpu_evictions += 1;
                }
            }
            freed += bytes;
            if freed >= deficit { break; }
        }
        self.publication_dirty = true;
        self.mark_mesh_set_changed();
        true
    }

    pub(super) fn admit_mesh(&mut self, key: ChunkKey, bytes: usize) -> bool {
        if !self.quality_mesh_fits(key,bytes,false) {
            self.budget_deferred.insert(key,bytes);return false;
        }
        let replaced = self.gpu_meshes.get(&key.coord).map_or(0, |mesh| mesh.bytes);
        let pending=self.pending_gpu.bytes();
        let pending_growth=self.pending_gpu.projected_growth(self);
        if self.horizon_pin_coords.contains(&key.coord) || self.edit_proxies.contains(&key.coord) {
            let pinned=self.supplemental_gpu_bytes();
            if pinned.saturating_sub(replaced).saturating_add(self.pending_gpu.supplemental_bytes(self)).saturating_add(bytes)>super::edit_proxies::BYTE_LIMIT {
                self.budget_deferred.insert(key,bytes); return false;
            }
        }
        let limit = self.replacement_limit(key.coord);
        // Never remove the displayed mesh before upload succeeds. Reject before
        // evicting anything if even the replacement overlap cannot fit.
        if bytes.saturating_add(replaced).saturating_add(pending) > self.residency_limits.1 {
            self.budget_deferred.insert(key, bytes);
            return false;
        }
        let needed = self
            .gpu_bytes
            .saturating_sub(replaced)
            .saturating_add(bytes).saturating_add(pending_growth);
        if needed > limit && self.frozen {
            self.budget_deferred.insert(key, bytes);
            return false;
        }
        if needed > limit {
            let rank = self.residency_rank(key.coord);
            let mut victims: Vec<_> = self
                .gpu_meshes
                .iter()
                .filter(|(coord, mesh)| {
                    **coord != key.coord && mesh.bytes > 0
                        && !self.horizon_pin_coords.contains(*coord)
                        && !self.published_coords.contains(*coord)
                        && !self.edit_proxies.contains(coord)
                        && !self.pending_gpu.contains(coord)
                })
                .filter_map(|(&coord, mesh)| {
                    let candidate_rank = self.residency_rank(coord);
                    (candidate_rank > rank).then_some((candidate_rank, coord, mesh.bytes))
                })
                .collect();
            // Reject before evicting anything if the candidate still cannot fit.
            let deficit = needed.saturating_sub(limit).saturating_sub(victims.iter().map(|v| v.2).sum());
            if deficit > 0 && !self.retire_distant_cover(key, deficit) {
                self.budget_deferred.insert(key, bytes);
                return false;
            }
            victims.sort_unstable_by(|a, b| b.0.cmp(&a.0));
            for (_, coord, size) in victims {
                if self
                    .gpu_bytes
                    .saturating_sub(replaced)
                    .saturating_add(bytes).saturating_add(pending_growth)
                    <= limit
                {
                    break;
                }
                let old = self.gpu_meshes.remove(&coord).unwrap();
                self.budget_deferred.insert(
                    ChunkKey {
                        coord,
                        lod: old.lod,
                    },
                    size,
                );
                self.gpu_bytes -= size;
                self.gpu_evictions += 1;
                self.mark_mesh_set_changed();
            }
        }
        // Keep the previous mesh through failed/retried GPU uploads.
        self.lod_meshes
            .trim_to(limit.saturating_sub(self.gpu_bytes).saturating_sub(pending).saturating_sub(bytes));
        if self.gpu_bytes.saturating_add(self.lod_meshes.bytes()).saturating_add(pending).saturating_add(bytes) > self.residency_limits.1 {
            self.budget_deferred.insert(key, bytes);
            return false;
        }
        self.budget_deferred.remove(&key);
        true
    }

    pub(super) fn sync_residency_budget(&mut self) {
        let limits = limits();
        if self.residency_limits.1 != limits.1 {
            self.budget_deferred.clear();
        }
        // Keep rejected mesh sizes across movement. Admission ranks them
        // against the current center, so newly near terrain can still retry.
        let old_lod = self.terrain_lod_settings();
        if self.residency_limits.1 != limits.1 {
            self.arena.set_budget(arena_budget(limits.1));
        }
        self.residency_limits = limits;
        let new_lod = self.terrain_lod_settings();
        if old_lod.lod_start_chunks != new_lod.lod_start_chunks || old_lod.lod_levels != new_lod.lod_levels {
            self.publication_dirty = true;
            self.refresh_published_columns();
        }
        self.trim_lod_budget();
        self.evict_meshes_over_budget();
        self.trim_sources_to(self.source_limit());
    }

    pub(super) fn evict_meshes_over_budget(&mut self) {
        if self.frozen {
            return;
        }
        let pending_growth = self.pending_gpu.projected_growth(self);
        if self.gpu_bytes.saturating_add(pending_growth) > self.mesh_limit() {
            // Requested membership may grow while the old column is still
            // published. Resource pressure must not silently unpublish it.
            let mut victims: Vec<_> = self.gpu_meshes.iter()
                .filter(|(q, mesh)| mesh.bytes > 0 && !self.horizon_pin_coords.contains(*q) && !self.published_coords.contains(*q) && !self.edit_proxies.contains(q) && !self.pending_gpu.contains(q))
                .map(|(&coord, _)| (self.residency_rank(coord), coord))
                .collect();
            victims.sort_unstable_by(|a, b| b.0.cmp(&a.0));
            for (_, coord) in victims {
                if self.gpu_bytes.saturating_add(pending_growth) <= self.mesh_limit() {
                    break;
                }
                let mesh = self.gpu_meshes.remove(&coord).unwrap();
                self.gpu_bytes -= mesh.bytes;
                self.budget_deferred.insert(
                    ChunkKey {
                        coord,
                        lod: mesh.lod,
                    },
                    mesh.bytes,
                );
                self.gpu_evictions += 1;
                self.mark_mesh_set_changed();
            }
        }
    }

    pub(super) fn budget_allows_request(&self, key: ChunkKey, scan: &mut RequestBudget) -> bool {
        let Some(&bytes) = self.budget_deferred.get(&key) else { return true; };
        let replaced = self.gpu_meshes.get(&key.coord).map_or(0, |mesh| mesh.bytes);
        let pending = self.pending_gpu.bytes();
        if bytes > super::super::upload_staging::BYTE_LIMIT
            || bytes.saturating_add(replaced).saturating_add(pending) > self.residency_limits.1
        {
            return false;
        }
        let growth = *scan.pending_growth.get_or_insert_with(|| self.pending_gpu.projected_growth(self));
        let projected = self.gpu_bytes.saturating_sub(replaced).saturating_add(bytes).saturating_add(growth);
        let deficit = projected.saturating_sub(self.replacement_limit(key.coord));
        if deficit == 0 {
            // Admission can trim the inactive LOD cache before allocating.
            return self.gpu_bytes.saturating_add(pending).saturating_add(bytes) <= self.residency_limits.1;
        }
        if !self.is_near_surface_request(key) { return false; }

        let cover = scan.cover.get_or_insert_with(|| {
            // Bucket each
            // column by the largest request rank that can reclaim it.
            let mut bytes_by_rank = [0usize; COVER_RANKS];
            for (_, distance, bytes) in self.distant_cover() {
                bytes_by_rank[(distance - 1).min(COVER_RANKS as i64 - 1) as usize] += bytes;
            }
            for rank in (0..COVER_RANKS - 1).rev() {
                bytes_by_rank[rank] += bytes_by_rank[rank + 1];
            }
            bytes_by_rank
        });
        deficit <= cover[self.residency_rank(key.coord).1 as usize]
    }
}

#[cfg(test)]
mod source_trim_tests {
    use super::*;
    use crate::render::streaming::{ChunkCache, ChunkKey};
    use crate::world::{BlockId, CHUNK_SIZE};
    use glam::IVec3;

    fn source(coord: IVec3) -> GeneratedChunk {
        let padded = (CHUNK_SIZE + 2) as usize;
        GeneratedChunk::new(coord, vec![BlockId::Stone; padded * padded * padded])
    }

    #[test]
    fn published_source_is_evicted_for_a_new_source_under_pressure() {
        let mut cache = ChunkCache::new(IVec3::ZERO);
        let bytes = source(IVec3::ZERO).allocation_bytes();
        // Room for one source and a half, so the second admission must evict.
        cache.residency_limits.0 = super::super::terrain::reservation_bytes() + bytes + bytes / 2;
        let far = IVec3::new(6, 0, 0);
        let near = IVec3::new(2, 0, 0);
        cache.visible = vec![ChunkKey { coord: far, lod: 0 }, ChunkKey { coord: near, lod: 0 }];
        cache.rebuild_visible_sets();
        assert!(cache.admit_source(far, source(far)));
        cache.published_coords.insert(far);
        assert!(cache.admit_source(near, source(near)), "a published source must be evictable");
        assert_eq!(cache.cpu_evictions, 1);
        assert!(!cache.cpu_chunks.contains_key(&far));
        assert!(cache.cpu_chunks.contains_key(&near));
    }

    #[test]
    fn pinned_source_is_kept_under_pressure() {
        let mut cache = ChunkCache::new(IVec3::ZERO);
        let bytes = source(IVec3::ZERO).allocation_bytes();
        cache.residency_limits.0 = super::super::terrain::reservation_bytes() + bytes + bytes / 2;
        let pinned = IVec3::new(6, 0, 0);
        cache.visible = vec![ChunkKey { coord: pinned, lod: 0 }];
        cache.rebuild_visible_sets();
        assert!(cache.admit_source(pinned, source(pinned)));
        cache.horizon_pin_coords.insert(pinned);
        assert!(!cache.admit_source(IVec3::new(2, 0, 0), source(IVec3::new(2, 0, 0))));
        assert!(cache.cpu_chunks.contains_key(&pinned));
    }
}
