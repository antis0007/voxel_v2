//! Shared GPU storage for chunk meshes (docs/plans/2026-10-22-render-arena-and-horizon-evidence.md).
//! A mesh owns an `ArenaLease`; dropping the lease returns its ranges through a
//! channel that the arena drains once per frame, after the previous submit, so a
//! freed range is never rewritten while an unsubmitted draw still reads it.
use std::collections::BTreeMap;
use std::sync::mpsc::Sender;

const VERTEX_BYTES: u64 = std::mem::size_of::<crate::mesh::Vertex3>() as u64;
const INDEX_BYTES: u64 = 4;

/// First-fit sub-allocator over one buffer. Sizes round up to `align`, so every
/// offset handed out is a multiple of it.
pub(super) struct RangeAllocator {
    capacity: u64,
    align: u64,
    used: u64,
    /// Free ranges, offset -> length; neighbours are always coalesced.
    free: BTreeMap<u64, u64>,
}

impl RangeAllocator {
    pub(super) fn new(capacity: u64, align: u64) -> Self {
        let align = align.max(1);
        let capacity = capacity / align * align;
        let free = if capacity > 0 { BTreeMap::from([(0, capacity)]) } else { BTreeMap::new() };
        Self { capacity, align, used: 0, free }
    }

    fn round(&self, size: u64) -> u64 {
        size.div_ceil(self.align) * self.align
    }

    pub(super) fn alloc(&mut self, size: u64) -> Option<u64> {
        if size == 0 {
            return None;
        }
        let size = self.round(size);
        let (&offset, &len) = self.free.iter().find(|(_, len)| **len >= size)?;
        self.free.remove(&offset);
        if len > size {
            self.free.insert(offset + size, len - size);
        }
        self.used += size;
        Some(offset)
    }

    pub(super) fn free(&mut self, offset: u64, size: u64) {
        let size = self.round(size);
        debug_assert!(offset + size <= self.capacity, "free outside the allocator");
        let (mut start, mut len) = (offset, size);
        if let Some((&prev, &prev_len)) = self.free.range(..offset).next_back()
            && prev + prev_len == offset
        {
            self.free.remove(&prev);
            start = prev;
            len += prev_len;
        }
        if let Some(next_len) = self.free.remove(&(offset + size)) {
            len += next_len;
        }
        self.free.insert(start, len);
        self.used -= size;
    }

    pub(super) fn used(&self) -> u64 {
        self.used
    }

    pub(super) fn largest_free(&self) -> u64 {
        self.free.values().copied().max().unwrap_or(0)
    }

    pub(super) fn capacity(&self) -> u64 {
        self.capacity
    }
}

/// Where one mesh lives: a page and a byte range in each of its two buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ArenaRange {
    pub page: u32,
    pub vertex_offset: u64,
    pub vertex_bytes: u64,
    pub index_offset: u64,
    pub index_bytes: u64,
}

impl ArenaRange {
    pub(super) fn first_index(&self) -> u32 {
        (self.index_offset / INDEX_BYTES) as u32
    }

    pub(super) fn base_vertex(&self) -> i32 {
        debug_assert_eq!(self.vertex_offset % VERTEX_BYTES, 0, "vertex range not vertex aligned");
        (self.vertex_offset / VERTEX_BYTES) as i32
    }
}

/// Ownership of an `ArenaRange`. Dropping it hands the range back to the arena.
pub(super) struct ArenaLease {
    range: ArenaRange,
    returns: Sender<ArenaRange>,
}

impl ArenaLease {
    pub(super) fn new(range: ArenaRange, returns: Sender<ArenaRange>) -> Self {
        Self { range, returns }
    }

    pub(super) fn range(&self) -> &ArenaRange {
        &self.range
    }
}

impl Drop for ArenaLease {
    fn drop(&mut self) {
        // The arena may already be gone (device reset); then nothing owns pages.
        let _ = self.returns.send(self.range);
    }
}

#[cfg(test)]
#[path = "arena_tests.rs"]
mod tests;
