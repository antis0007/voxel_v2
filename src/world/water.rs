//! Chunk-partitioned partial liquid volumes. Paging/snapshots visit selected
//! chunks rather than scanning every active liquid cell in the world.
use super::chunk_coord_from_world_block;
use glam::IVec3;
use std::collections::HashMap;
#[derive(Default)]
pub struct WaterLevels {
    chunks: HashMap<IVec3, HashMap<IVec3, u8>>,
    capacity: usize,
}
impl WaterLevels {
    pub fn get(&self, p: &IVec3) -> Option<&u8> {
        self.chunks.get(&chunk_coord_from_world_block(*p))?.get(p)
    }
    pub fn insert(&mut self, p: IVec3, v: u8) -> Option<u8> {
        let m = self
            .chunks
            .entry(chunk_coord_from_world_block(p))
            .or_default();
        self.capacity -= m.capacity();
        let old = m.insert(p, v);
        self.capacity += m.capacity();
        old
    }
    pub fn remove(&mut self, p: &IVec3) -> Option<u8> {
        let c = chunk_coord_from_world_block(*p);
        let m = self.chunks.get_mut(&c)?;
        let old = m.remove(p);
        if m.is_empty() {
            self.remove_chunk(c);
        }
        old
    }
    pub fn remove_chunk(&mut self, c: IVec3) {
        if let Some(m) = self.chunks.remove(&c) {
            self.capacity -= m.capacity();
        }
    }
    pub fn chunk(&self, c: IVec3) -> Option<&HashMap<IVec3, u8>> {
        self.chunks.get(&c)
    }
    pub fn capacity(&self) -> usize {
        self.capacity + self.chunks.capacity()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&IVec3, &u8)> {
        self.chunks.values().flat_map(|m| m.iter())
    }
    pub fn extend(&mut self, items: impl IntoIterator<Item = (IVec3, u8)>) {
        for (p, v) in items {
            self.insert(p, v);
        }
    }
    pub fn snapshot(&self, coords: impl Iterator<Item = IVec3>) -> HashMap<IVec3, u8> {
        coords
            .filter_map(|c| self.chunks.get(&c))
            .flat_map(|m| m.iter().map(|(&p, &v)| (p, v)))
            .collect()
    }
}
