//! Explicit budgets for regenerable world data. Worker-held voxel allocations
//! remain charged after cache eviction. Edited world data is never evicted here.
use crate::mesh::GeneratedChunk;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const MIB: usize = 1024 * 1024;
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

#[derive(Default)]
pub(super) struct VoxelAllocations(pub AtomicUsize);

/// Dropped only when the cache AND every worker release the source allocation.
pub struct ResidentChunk {
    data: GeneratedChunk,
    allocations: Arc<VoxelAllocations>,
    bytes: usize,
}

impl ResidentChunk {
    pub(super) fn new(data: GeneratedChunk, allocations: &Arc<VoxelAllocations>) -> Self {
        let bytes = data.allocation_bytes();
        allocations.0.fetch_add(bytes, Ordering::Relaxed);
        Self {
            data,
            allocations: Arc::clone(allocations),
            bytes,
        }
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

impl Deref for ResidentChunk {
    type Target = GeneratedChunk;
    fn deref(&self) -> &GeneratedChunk {
        &self.data
    }
}
impl DerefMut for ResidentChunk {
    fn deref_mut(&mut self) -> &mut GeneratedChunk {
        &mut self.data
    }
}
impl Drop for ResidentChunk {
    fn drop(&mut self) {
        // Free the actual array before making its budget available again.
        drop(std::mem::take(&mut self.data.blocks));
        drop(std::mem::take(&mut self.data.water_levels));
        self.allocations.0.fetch_sub(self.bytes, Ordering::Relaxed);
    }
}

use super::{ChunkCache, ChunkKey};
use glam::IVec3;

impl ChunkCache {
    pub(super) fn mesh_limit(&self) -> usize {
        self.residency_limits
            .1
            .saturating_sub(GPU_STREAMING_RESERVE)
    }

    pub(super) fn trim_lod_budget(&mut self) {
        self.lod_meshes
            .trim_to(self.mesh_limit().saturating_sub(self.gpu_bytes));
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

    fn source_limit(&self) -> usize {
        self.residency_limits
            .0
            .saturating_sub(super::terrain::reservation_bytes())
    }

    pub(super) fn cpu_bytes(&self) -> usize {
        self.voxel_allocations.0.load(Ordering::Relaxed)
    }

    fn trim_sources_to(&mut self, limit: usize) {
        let mut victims: Vec<_> = self
            .cpu_chunks
            .iter()
            .filter(|(_, source)| Arc::strong_count(source) == 1)
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
        self.cpu_chunks.remove(&coord);
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

    pub(super) fn admit_mesh(&mut self, key: ChunkKey, bytes: usize) -> bool {
        let replaced = self.gpu_meshes.get(&key.coord).map_or(0, |mesh| mesh.bytes);
        let limit = self.mesh_limit();
        // Never remove the displayed mesh before upload succeeds. Reject before
        // evicting anything if even the replacement overlap cannot fit.
        if bytes.saturating_add(replaced) > self.residency_limits.1 {
            self.budget_deferred.insert(key, bytes);
            return false;
        }
        let needed = self
            .gpu_bytes
            .saturating_sub(replaced)
            .saturating_add(bytes);
        if needed > limit {
            let rank = self.residency_rank(key.coord);
            let mut victims: Vec<_> = self
                .gpu_meshes
                .iter()
                .filter(|(coord, mesh)| {
                    **coord != key.coord && mesh.bytes > 0 && self.residency_rank(**coord) > rank
                })
                .map(|(&coord, mesh)| (self.residency_rank(coord), coord, mesh.bytes))
                .collect();
            victims.sort_unstable_by(|a, b| b.0.cmp(&a.0));
            // Reject before evicting anything if the candidate still cannot fit.
            if needed.saturating_sub(victims.iter().map(|v| v.2).sum::<usize>()) > limit {
                self.budget_deferred.insert(key, bytes);
                return false;
            }
            for (_, coord, size) in victims {
                if self
                    .gpu_bytes
                    .saturating_sub(replaced)
                    .saturating_add(bytes)
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
                self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
            }
        }
        // Keep the previous mesh through failed/retried GPU uploads.
        self.lod_meshes
            .trim_to(limit.saturating_sub(self.gpu_bytes).saturating_sub(bytes));
        if self.gpu_bytes.saturating_add(bytes) > self.residency_limits.1 {
            self.budget_deferred.insert(key, bytes);
            return false;
        }
        self.budget_deferred.remove(&key);
        true
    }

    pub(super) fn sync_residency_budget(&mut self) {
        let limits = limits();
        if self.budget_center != self.center_chunk || self.residency_limits != limits {
            self.budget_deferred.clear();
            self.budget_center = self.center_chunk;
        }
        self.residency_limits = limits;
        self.trim_lod_budget();
        if self.gpu_bytes > self.mesh_limit() {
            let mut victims: Vec<_> = self.gpu_meshes.keys().copied().collect();
            victims.sort_unstable_by_key(|coord| std::cmp::Reverse(self.residency_rank(*coord)));
            for coord in victims {
                if self.gpu_bytes <= self.mesh_limit() {
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
                self.gpu_mesh_revision = self.gpu_mesh_revision.wrapping_add(1);
            }
        }
        if self.cpu_bytes() > self.source_limit() {
            self.trim_sources_to(self.source_limit());
        }
    }

    pub(super) fn budget_allows_request(&self, key: ChunkKey) -> bool {
        self.budget_deferred.get(&key).is_none_or(|bytes| {
            self.gpu_bytes.saturating_add(*bytes) <= self.residency_limits.1
                && self
                    .gpu_bytes
                    .saturating_sub(self.gpu_meshes.get(&key.coord).map_or(0, |mesh| mesh.bytes))
                    .saturating_add(*bytes)
                    <= self.mesh_limit()
        })
    }
}
