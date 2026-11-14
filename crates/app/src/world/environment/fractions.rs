//! Sparse fractional fluid encoding, partitioned for immutable render views.
//! Snapshots share unchanged chunk maps; no world-sized copy on a water edit.
use super::{Key, position};
use crate::fast_hash::FastMap;
use crate::world::chunk_coord_from_world_block as chunk;
use glam::IVec3;
use std::sync::Arc;

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Fractions {
    chunks: FastMap<IVec3, Arc<FastMap<Key, u64>>>,
    count: usize,
}
impl Fractions {
    pub(crate) fn chunk_identities(&self) -> impl Iterator<Item=(IVec3,usize)> + '_ {
        self.chunks.iter().map(|(&coord,cells)|(coord,Arc::as_ptr(cells) as usize))
    }

    pub fn get(&self, p: &Key) -> Option<&u64> {
        self.chunks.get(&chunk(position(*p)))?.get(p)
    }
    pub fn contains_key(&self, p: &Key) -> bool {
        self.get(p).is_some()
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn insert(&mut self, p: Key, value: u64) -> Option<u64> {
        if self.get(&p) == Some(&value) {
            return Some(value);
        }
        let cells = self.chunks.entry(chunk(position(p))).or_default();
        let old = Arc::make_mut(cells).insert(p, value);
        self.count += usize::from(old.is_none());
        old
    }
    pub fn remove(&mut self, p: &Key) -> Option<u64> {
        self.get(p)?;
        let c = chunk(position(*p));
        let cells = self.chunks.get_mut(&c)?;
        let old = Arc::make_mut(cells).remove(p);
        self.count -= usize::from(old.is_some());
        if cells.is_empty() {
            self.chunks.remove(&c);
        }
        old
    }
    pub fn iter(&self) -> impl Iterator<Item = (&Key, &u64)> {
        self.chunks.values().flat_map(|m| m.iter())
    }
    pub fn in_box(&self, min: IVec3, max: IVec3) -> impl Iterator<Item = (IVec3, u64)> + '_ {
        let lo = chunk(min);
        let hi = chunk(max - IVec3::ONE);
        (lo.y..=hi.y)
            .flat_map(move |y| {
                (lo.z..=hi.z).flat_map(move |z| (lo.x..=hi.x).map(move |x| IVec3::new(x, y, z)))
            })
            .filter_map(|c| self.chunks.get(&c))
            .flat_map(|m| m.iter())
            .map(|(&k, &v)| (position(k), v))
            .filter(move |(p, _)| p.cmpge(min).all() && p.cmplt(max).all())
    }
    pub fn changed(&self, old: &Self, c: IVec3) -> bool {
        match (self.chunks.get(&c), old.chunks.get(&c)) {
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            (None, None) => false,
            _ => true,
        }
    }
    pub fn changed_chunks<'a>(&'a self, old: &'a Self) -> impl Iterator<Item = IVec3> + 'a {
        self.chunks
            .keys()
            .filter(|&&c| self.changed(old, c))
            .copied()
            .chain(old.chunks.keys().filter(|c| !self.chunks.contains_key(c)).copied())
    }
}
