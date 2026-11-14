//! Sparse activity discovery, independent of the number of sleeping resident chunks.
use super::{BrickStore, INACTIVE};
use crate::cell::BrickCoord;
use glam::IVec3;

impl BrickStore {
    /// Replace a valid dirty mask and maintain the dense awake-chunk index.
    /// Callers exclude absent bricks before reaching this function.
    pub(super) fn set_dirty(&mut self, chunk: IVec3, dirty: u64) {
        let Some(entry) = self.chunks.get_mut(&chunk) else { return };
        entry.dirty = dirty;
        if dirty != 0 {
            if entry.awake_index == INACTIVE {
                entry.awake_index = self.awake_chunks.len();
                self.awake_chunks.push(chunk);
            }
        } else if entry.awake_index != INACTIVE {
            let index = std::mem::replace(&mut entry.awake_index, INACTIVE);
            self.awake_chunks.swap_remove(index);
            if let Some(moved) = self.awake_chunks.get(index)
                && let Some(entry) = self.chunks.get_mut(moved)
            {
                entry.awake_index = index;
            }
        }
    }

    /// Bricks marked dirty: only awake chunks are visited.
    pub fn active_bricks(&self) -> usize {
        self.awake_chunks.iter().map(|c| self.chunks[c].dirty.count_ones() as usize).sum()
    }

    /// The chunk's dirty mask, cleared, removing it from the work index.
    pub fn take_dirty(&mut self, chunk: IVec3) -> u64 {
        let dirty = self.chunks.get(&chunk).map_or(0, |e| e.dirty);
        self.set_dirty(chunk, 0);
        dirty
    }

    /// Every dirty brick in canonical (chunk z, y, x, brick) order. Enumerate
    /// set bits only; swap-removing a chunk never changes dispatch order.
    pub fn dirty_bricks(&self) -> Vec<BrickCoord> {
        let mut chunks = self.awake_chunks.clone();
        chunks.sort_unstable_by_key(|chunk| (chunk.z, chunk.y, chunk.x));
        let mut out = Vec::new();
        for chunk in chunks {
            let entry = &self.chunks[&chunk];
            let mut dirty = entry.dirty;
            while dirty != 0 {
                let brick = dirty.trailing_zeros() as u8;
                out.push(BrickCoord { chunk, brick, scale: entry.scale });
                dirty &= dirty - 1;
            }
        }
        out
    }

    /// Consume one pass's work without scanning or clearing sleeping chunks.
    pub fn take_all_dirty(&mut self) -> Vec<BrickCoord> {
        let bricks = self.dirty_bricks();
        for chunk in self.awake_chunks.drain(..) {
            if let Some(entry) = self.chunks.get_mut(&chunk) {
                entry.dirty = 0;
                entry.awake_index = INACTIVE;
            }
        }
        bricks
    }
}
