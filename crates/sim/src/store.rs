//! The brick pool, the per-chunk pointer tables and dirty masks.
//!
//! This is the CPU mirror. The GPU pool is the same bytes: `pool_bytes` is the
//! upload, `chunk_slots` the pointer table, `take_dirty` the dispatch list.
use crate::cell::{BRICK_CELLS, BRICKS_PER_CHUNK, BrickCoord, Cell, Scale, brick_and_cell, local_of};
use engine_core::fast_hash::FastMap;
use glam::IVec3;

/// No brick in this slot of a chunk's pointer table.
pub const NO_SLOT: u32 = u32::MAX;

/// The pool has no free brick. The store is unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolFull;

struct ChunkEntry {
    scale: Scale,
    /// Bit `i` set: brick `i` changed and must be stepped. Clear: not dispatched at all.
    dirty: u64,
    /// Pool slot of each brick, or `NO_SLOT`. The only record of which bricks exist.
    slots: [u32; BRICKS_PER_CHUNK],
}

impl ChunkEntry {
    fn new() -> Self {
        Self { scale: Scale::FINEST, dirty: 0, slots: [NO_SLOT; BRICKS_PER_CHUNK] }
    }

    fn stored(&self) -> usize {
        self.slots.iter().filter(|s| **s != NO_SLOT).count()
    }
}

pub struct BrickStore {
    capacity: u32,
    /// `bricks * BRICK_CELLS` cells; grows up to `capacity` bricks.
    pool: Vec<Cell>,
    free: Vec<u32>,
    chunks: FastMap<IVec3, ChunkEntry>,
}

impl BrickStore {
    /// A store of at most `capacity` bricks (4 KiB each), the size of the GPU pool.
    pub fn new(capacity: u32) -> Self {
        Self { capacity, pool: Vec::new(), free: Vec::new(), chunks: FastMap::default() }
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn bricks_in_use(&self) -> usize {
        self.pool.len() / BRICK_CELLS - self.free.len()
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Bricks marked dirty across the store: the "active bricks" number the overlay shows.
    pub fn active_bricks(&self) -> usize {
        self.chunks.values().map(|c| c.dirty.count_ones() as usize).sum()
    }

    pub fn get(&self, chunk: IVec3, local: IVec3) -> Cell {
        let (brick, cell) = brick_and_cell(local);
        match self.chunks.get(&chunk).map(|e| e.slots[brick as usize]) {
            Some(slot) if slot != NO_SLOT => self.pool[slot as usize * BRICK_CELLS + cell],
            _ => Cell::EMPTY,
        }
    }

    /// Write one cell. A value equal to the current one changes nothing and
    /// wakes nothing; writing `EMPTY` into an absent brick allocates nothing.
    pub fn set(&mut self, chunk: IVec3, local: IVec3, value: Cell) -> Result<(), PoolFull> {
        if self.get(chunk, local) == value {
            return Ok(());
        }
        let (brick, cell) = brick_and_cell(local);
        let slot = self.ensure_brick(chunk, brick)?;
        self.pool[slot as usize * BRICK_CELLS + cell] = value;
        self.entry_mut(chunk).dirty |= 1 << brick;
        Ok(())
    }

    /// Replace a whole chunk from `f(local)`. Bricks that come out entirely
    /// `EMPTY` are not stored; the rest are marked dirty. All or nothing: on
    /// `PoolFull` the chunk keeps its previous contents.
    pub fn fill_chunk(&mut self, chunk: IVec3, f: impl Fn(IVec3) -> Cell) -> Result<(), PoolFull> {
        let mut bricks = Vec::with_capacity(BRICKS_PER_CHUNK);
        for brick in 0..BRICKS_PER_CHUNK as u8 {
            let cells: Vec<Cell> = (0..BRICK_CELLS).map(|c| f(local_of(brick, c))).collect();
            if cells.iter().any(|c| *c != Cell::EMPTY) {
                bricks.push((brick, cells));
            }
        }
        let held = self.chunks.get(&chunk).map_or(0, ChunkEntry::stored);
        if bricks.len() > self.capacity as usize - self.bricks_in_use() + held {
            return Err(PoolFull);
        }
        self.remove_chunk(chunk);
        for (brick, cells) in bricks {
            let slot = self.ensure_brick(chunk, brick)?;
            self.pool[slot as usize * BRICK_CELLS..][..BRICK_CELLS].copy_from_slice(&cells);
            self.entry_mut(chunk).dirty |= 1 << brick;
        }
        Ok(())
    }

    /// Call `f(local, cell)` for every cell of every stored brick of `chunk`.
    /// Cells of absent bricks are `EMPTY` and are not visited.
    pub fn for_each_cell(&self, chunk: IVec3, mut f: impl FnMut(IVec3, Cell)) {
        let Some(entry) = self.chunks.get(&chunk) else { return };
        for (brick, &slot) in entry.slots.iter().enumerate() {
            if slot != NO_SLOT {
                for (c, cell) in self.brick(slot).iter().enumerate() {
                    f(local_of(brick as u8, c), *cell);
                }
            }
        }
    }

    /// Free every brick that holds only `EMPTY`; returns how many. Drops the chunk when none remain.
    pub fn trim(&mut self, chunk: IVec3) -> usize {
        let Some(entry) = self.chunks.get(&chunk) else { return 0 };
        let empty: Vec<usize> = (0..BRICKS_PER_CHUNK)
            .filter(|&b| entry.slots[b] != NO_SLOT && self.brick(entry.slots[b]).iter().all(|c| *c == Cell::EMPTY))
            .collect();
        let mut freed = Vec::with_capacity(empty.len());
        if let Some(entry) = self.chunks.get_mut(&chunk) {
            for &b in &empty {
                freed.push(std::mem::replace(&mut entry.slots[b], NO_SLOT));
                entry.dirty &= !(1 << b);
            }
            if entry.stored() == 0 {
                self.chunks.remove(&chunk);
            }
        }
        self.free.extend(freed);
        empty.len()
    }

    /// Free every brick of `chunk` (it left residency).
    pub fn remove_chunk(&mut self, chunk: IVec3) {
        if let Some(entry) = self.chunks.remove(&chunk) {
            self.free.extend(entry.slots.iter().filter(|s| **s != NO_SLOT));
        }
    }

    /// The chunk's dirty mask, cleared.
    pub fn take_dirty(&mut self, chunk: IVec3) -> u64 {
        self.chunks.get_mut(&chunk).map_or(0, |e| std::mem::take(&mut e.dirty))
    }

    /// Every dirty brick in a fixed order (chunk z, y, x, then brick), for the dispatch list.
    pub fn dirty_bricks(&self) -> Vec<BrickCoord> {
        let mut out: Vec<BrickCoord> = self
            .chunks
            .iter()
            .flat_map(|(&chunk, e)| {
                (0..BRICKS_PER_CHUNK as u8)
                    .filter(move |b| e.dirty >> b & 1 != 0)
                    .map(move |brick| BrickCoord { chunk, brick, scale: e.scale })
            })
            .collect();
        out.sort_by_key(|b| (b.chunk.z, b.chunk.y, b.chunk.x, b.brick));
        out
    }

    /// The chunk's pointer table, `NO_SLOT` where no brick is stored.
    pub fn chunk_slots(&self, chunk: IVec3) -> Option<&[u32; BRICKS_PER_CHUNK]> {
        self.chunks.get(&chunk).map(|e| &e.slots)
    }

    /// The whole pool as bytes: the GPU upload.
    pub fn pool_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.pool)
    }

    fn brick(&self, slot: u32) -> &[Cell] {
        &self.pool[slot as usize * BRICK_CELLS..][..BRICK_CELLS]
    }

    fn entry_mut(&mut self, chunk: IVec3) -> &mut ChunkEntry {
        self.chunks.entry(chunk).or_insert_with(ChunkEntry::new)
    }

    /// The brick's slot, allocating an `EMPTY` brick if it has none.
    fn ensure_brick(&mut self, chunk: IVec3, brick: u8) -> Result<u32, PoolFull> {
        let held = self.chunks.get(&chunk).map(|e| e.slots[brick as usize]).filter(|s| *s != NO_SLOT);
        if let Some(slot) = held {
            return Ok(slot);
        }
        let slot = match self.free.pop() {
            Some(slot) => slot,
            None if self.pool.len() / BRICK_CELLS < self.capacity as usize => {
                self.pool.resize(self.pool.len() + BRICK_CELLS, Cell::EMPTY);
                (self.pool.len() / BRICK_CELLS - 1) as u32
            }
            None => return Err(PoolFull),
        };
        self.pool[slot as usize * BRICK_CELLS..][..BRICK_CELLS].fill(Cell::EMPTY);
        self.entry_mut(chunk).slots[brick as usize] = slot;
        Ok(slot)
    }

    /// Panics if the pool bookkeeping is inconsistent. For tests.
    #[cfg(test)]
    pub(crate) fn check_invariants(&self) {
        let used = self.pool.len() / BRICK_CELLS;
        let mut seen = vec![false; used];
        for entry in self.chunks.values() {
            for &slot in entry.slots.iter().filter(|s| **s != NO_SLOT) {
                assert!((slot as usize) < used, "slot {slot} beyond the pool");
                assert!(!seen[slot as usize], "slot {slot} used twice");
                seen[slot as usize] = true;
            }
            let present: u64 = (0..BRICKS_PER_CHUNK).filter(|&b| entry.slots[b] != NO_SLOT).fold(0, |m, b| m | 1 << b);
            assert_eq!(entry.dirty & !present, 0, "dirty bit on an absent brick");
        }
        for &slot in &self.free {
            assert!(!seen[slot as usize], "free slot {slot} is also in use");
            seen[slot as usize] = true;
        }
        assert!(seen.iter().all(|s| *s), "a slot is neither used nor free");
        assert!(used <= self.capacity as usize);
    }
}
