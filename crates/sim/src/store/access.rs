//! What the kernel and the scenario snapshots need from the store: direct brick access, waking,
//! reserving and the canonical state hash. A child module so it sees the store's private fields.
use super::{BrickStore, NO_SLOT, PoolFull};
use crate::cell::{BRICK_CELLS, BRICKS_PER_CHUNK, Cell};
use glam::IVec3;

impl BrickStore {
    /// Every allocated pool slot's cells, indexed by slot: disjoint borrows for kernels that work on many
    /// bricks at once.
    pub(crate) fn slot_cells_mut(&mut self) -> std::slice::IterMut<'_, [Cell; BRICK_CELLS]> {
        self.pool.as_chunks_mut::<BRICK_CELLS>().0.iter_mut()
    }

    /// A copy of the chunk's pointer table (all `NO_SLOT` when the chunk is absent).
    pub(crate) fn slots_of(&self, chunk: IVec3) -> [u32; BRICKS_PER_CHUNK] {
        self.chunks.get(&chunk).map_or([NO_SLOT; BRICKS_PER_CHUNK], |e| e.slots)
    }

    pub(crate) fn cell_in_slot(&self, slot: u32, cell: usize) -> Cell {
        self.pool[slot as usize * BRICK_CELLS + cell]
    }

    /// The cells of one pool slot, for the region digest.
    pub(crate) fn slot_cells(&self, slot: u32) -> &[Cell] {
        self.brick(slot)
    }

    pub(crate) fn brick_cells_mut(&mut self, slot: u32) -> &mut [Cell] {
        let start = slot as usize * BRICK_CELLS;
        &mut self.pool[start..start + BRICK_CELLS]
    }

    pub(crate) fn set_cell_in_slot(&mut self, slot: u32, cell: usize, value: Cell) {
        self.pool[slot as usize * BRICK_CELLS + cell] = value;
    }

    /// Mark the bricks in `mask` dirty; bits of absent bricks are ignored.
    pub(crate) fn or_dirty(&mut self, chunk: IVec3, mask: u64) {
        if mask == 0 { return; }
        let Some(entry) = self.chunks.get(&chunk) else { return };
        let mut candidates = mask & !entry.dirty;
        let mut dirty = entry.dirty;
        while candidates != 0 {
            let brick = candidates.trailing_zeros() as usize;
            candidates &= candidates - 1;
            if entry.slots[brick] != NO_SLOT {
                dirty |= 1 << brick;
            }
        }
        if dirty != entry.dirty {
            self.set_dirty(chunk, dirty);
        }
    }

    /// Mark a stored brick dirty because a neighbour across a face changed. An absent brick is all
    /// empty and needs no stepping, so waking it does nothing.
    pub fn wake(&mut self, chunk: IVec3, brick: u8) {
        if let Some(e) = self.chunks.get(&chunk)
            && e.slots[brick as usize] != NO_SLOT
            && e.dirty & (1 << brick) == 0
        {
            self.set_dirty(chunk, e.dirty | (1 << brick));
        }
    }

    /// Make sure the brick exists, so a later write into it cannot run out of pool.
    pub fn reserve(&mut self, chunk: IVec3, brick: u8) -> Result<(), PoolFull> {
        self.ensure_brick(chunk, brick).map(|_| ())
    }

    /// FNV-1a over every stored brick in (chunk z, y, x, brick) order: the same cells hash the same
    /// whatever pool slots they landed in. This is what scenario snapshots pin.
    pub fn state_hash(&self) -> u64 {
        let mut chunks: Vec<IVec3> = self.chunks.keys().copied().collect();
        chunks.sort_by_key(|c| (c.z, c.y, c.x));
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut feed = |bytes: &[u8]| {
            for &b in bytes {
                hash = (hash ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        for chunk in chunks {
            let entry = &self.chunks[&chunk];
            for brick in 0..BRICKS_PER_CHUNK {
                // A stored brick that has emptied out is the same world as an absent one.
                if entry.slots[brick] != NO_SLOT && self.brick(entry.slots[brick]).iter().any(|c| *c != Cell::EMPTY) {
                    for axis in [chunk.x, chunk.y, chunk.z] {
                        feed(&axis.to_le_bytes());
                    }
                    feed(&[brick as u8]);
                    feed(bytemuck::cast_slice(self.brick(entry.slots[brick])));
                }
            }
        }
        hash
    }
}
