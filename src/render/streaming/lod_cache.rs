//! Completed inactive GPU LODs are reusable until their source/halo changes.
//! Both bytes and entries are capped, including zero-triangle chunk entries.
use super::{ChunkGpuMesh, ChunkKey};
use glam::IVec3;
use std::collections::HashMap;

const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 512;

#[derive(Default)]
pub(super) struct LodMeshCache {
    entries: HashMap<ChunkKey, (u64, ChunkGpuMesh)>,
    bytes: usize,
    clock: u64,
}

impl LodMeshCache {
    pub fn trim_to(&mut self, limit: usize) {
        while self.bytes > limit {
            let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, (used, _))| *used)
                .map(|(&key, _)| key)
            else {
                break;
            };
            drop(self.take(key));
        }
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }

    pub fn take(&mut self, key: ChunkKey) -> Option<ChunkGpuMesh> {
        let (_, mesh) = self.entries.remove(&key)?;
        self.bytes -= mesh.bytes;
        Some(mesh)
    }

    pub fn remember(&mut self, coord: IVec3, mesh: ChunkGpuMesh) {
        let key = ChunkKey {
            coord,
            lod: mesh.lod,
        };
        drop(self.take(key));
        if mesh.bytes > MAX_BYTES {
            return;
        }
        while self.entries.len() >= MAX_ENTRIES || self.bytes + mesh.bytes > MAX_BYTES {
            let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, (used, _))| *used)
                .map(|(&key, _)| key)
            else {
                break;
            };
            drop(self.take(key));
        }
        self.clock = self.clock.wrapping_add(1);
        self.bytes += mesh.bytes;
        self.entries.insert(key, (self.clock, mesh));
    }

    pub fn retain(&mut self, keep: impl Fn(IVec3) -> bool) {
        self.entries.retain(|key, (_, mesh)| {
            if keep(key.coord) {
                true
            } else {
                self.bytes -= mesh.bytes;
                false
            }
        });
    }
}
