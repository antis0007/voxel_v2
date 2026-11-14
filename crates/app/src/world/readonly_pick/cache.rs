//! Bounded exact source cache. Coordinate lookup is independent of retained count.
use super::*;
struct Entry { value: (Key, Arc<SourceBlocks>), stamp: u64 }
#[derive(Default)]
pub(super) struct Cache {
    entries: std::collections::HashMap<(bool, IVec3), Entry>, bytes: usize, clock: u64,
}
impl Cache {
    pub fn clear(&mut self) { self.entries.clear(); self.bytes = 0; }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn iter(&self) -> impl Iterator<Item = &(Key, Arc<SourceBlocks>)> { self.entries.values().map(|e| &e.value) }
    pub fn get(&mut self, key: &Key, p: IVec3) -> Option<BlockId> {
        let entry = self.entries.get_mut(&key.address())?;
        if entry.value.0 != *key { return None; }
        self.clock = self.clock.saturating_add(1); entry.stamp = self.clock;
        Some(entry.value.1[key.local_index(p)])
    }
    pub fn insert(&mut self, key: Key, data: Arc<SourceBlocks>) {
        let address = key.address();
        if let Some(old) = self.entries.remove(&address) { self.bytes -= old.value.1.len() * std::mem::size_of::<BlockId>(); }
        let incoming = data.len() * std::mem::size_of::<BlockId>();
        while self.entries.len() >= CACHE_CHUNKS || self.bytes + incoming > CACHE_BYTES {
            let Some(oldest) = self.entries.iter().min_by_key(|(_,e)| e.stamp).map(|(a,_)| *a) else { break; };
            if let Some(old) = self.entries.remove(&oldest) { self.bytes -= old.value.1.len() * std::mem::size_of::<BlockId>(); }
        }
        self.clock = self.clock.saturating_add(1); self.bytes += incoming;
        self.entries.insert(address, Entry { value: (key, data), stamp: self.clock });
    }
    #[cfg(test)]
    pub fn push_back(&mut self, value: (Key, Arc<SourceBlocks>)) { self.insert(value.0, value.1); }
    #[cfg(test)]
    pub fn front(&self) -> Option<&(Key, Arc<SourceBlocks>)> { self.entries.values().min_by_key(|e|e.stamp).map(|e|&e.value) }
}
#[cfg(test)]
impl std::ops::Index<usize> for Cache {
    type Output = (Key, Arc<SourceBlocks>);
    fn index(&self, i: usize) -> &Self::Output { self.iter().nth(i).expect("test cache index") }
}
