//! Resident edit allocations are accounted at mutation boundaries, not per frame.
use super::ChunkData;
use glam::IVec3;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};

fn bytes(c: &ChunkData) -> usize {
    c.blocks.len() * std::mem::size_of::<super::BlockId>()
        + c.light.len()
        + c.torches.capacity() * (std::mem::size_of::<IVec3>() + 1) * 2
}
#[derive(Default)]
pub struct ResidentEdits {
    chunks: HashMap<IVec3, Arc<ChunkData>>,
    bytes: usize,
}
impl Deref for ResidentEdits {
    type Target = HashMap<IVec3, Arc<ChunkData>>;
    fn deref(&self) -> &Self::Target {
        &self.chunks
    }
}
impl ResidentEdits {
    pub fn allocation_bytes(&self) -> usize {
        self.bytes
    }
    pub fn insert(&mut self, c: IVec3, chunk: Arc<ChunkData>) {
        self.bytes += bytes(&chunk);
        if let Some(old) = self.chunks.insert(c, chunk) {
            self.bytes -= bytes(&old);
        }
    }
    pub fn remove(&mut self, c: &IVec3) {
        if let Some(old) = self.chunks.remove(c) {
            self.bytes -= bytes(&old);
        }
    }
    pub fn get_mut(&mut self, c: &IVec3) -> Option<Edit<'_>> {
        let chunk = self.chunks.get_mut(c)?;
        let previous = bytes(chunk);
        Some(Edit {
            chunk,
            total: &mut self.bytes,
            previous,
        })
    }
}
pub struct Edit<'a> {
    chunk: &'a mut Arc<ChunkData>,
    total: &'a mut usize,
    previous: usize,
}
impl Deref for Edit<'_> {
    type Target = Arc<ChunkData>;
    fn deref(&self) -> &Self::Target {
        self.chunk
    }
}
impl DerefMut for Edit<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.chunk
    }
}
impl Drop for Edit<'_> {
    fn drop(&mut self) {
        *self.total = *self.total - self.previous + bytes(self.chunk);
    }
}
