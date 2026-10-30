//! Exact checkpoints of the brick store: the pool, free list, pointer tables, dirty masks and the
//! awake order, so a restored store steps to the same bytes as one that never stopped.
//! A child module so it sees the store's private fields.
use super::{BrickStore, ChunkEntry, INACTIVE, NO_SLOT};
use crate::cell::{BRICK_CELLS, BRICKS_PER_CHUNK, Cell, Scale};
use crate::codec::{CheckpointError, Reader, put_coord};
use engine_core::fast_hash::FastMap;
use glam::IVec3;

const MAGIC: &[u8; 8] = b"VXSIMCK1";

impl BrickStore {
    /// The store as checkpoint bytes. Chunks are written in canonical (z, y, x) order, so equal
    /// stores give equal bytes whatever their hash-map history.
    pub fn encode_checkpoint(&self) -> Vec<u8> {
        let bricks = self.pool.len() / BRICK_CELLS;
        let mut out = Vec::with_capacity(32 + self.pool.len() * size_of::<Cell>() + self.chunks.len() * 280);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.capacity.to_le_bytes());
        out.extend_from_slice(&(bricks as u32).to_le_bytes());
        out.extend_from_slice(bytemuck::cast_slice(&self.pool));
        out.extend_from_slice(&(self.free.len() as u32).to_le_bytes());
        for slot in &self.free {
            out.extend_from_slice(&slot.to_le_bytes());
        }
        let mut chunks: Vec<IVec3> = self.chunks.keys().copied().collect();
        chunks.sort_unstable_by_key(|c| (c.z, c.y, c.x));
        out.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
        for chunk in chunks {
            let entry = &self.chunks[&chunk];
            put_coord(&mut out, chunk);
            out.push(entry.scale.0);
            out.extend_from_slice(&entry.dirty.to_le_bytes());
            for slot in entry.slots {
                out.extend_from_slice(&slot.to_le_bytes());
            }
        }
        // Dispatch sorts its list, but swap-removal order decides future awake order, so keep it.
        out.extend_from_slice(&(self.awake_chunks.len() as u32).to_le_bytes());
        for &chunk in &self.awake_chunks {
            put_coord(&mut out, chunk);
        }
        out
    }

    /// A store from `encode_checkpoint` bytes, checked against every pool invariant.
    pub fn decode_checkpoint(bytes: &[u8]) -> Result<Self, CheckpointError> {
        use CheckpointError::Inconsistent;
        let mut r = Reader::open(bytes, MAGIC)?;
        let capacity = r.u32()?;
        let bricks = r.u32()? as usize;
        if bricks > capacity as usize {
            return Err(Inconsistent("more bricks than capacity"));
        }
        let cell_bytes = bricks.checked_mul(BRICK_CELLS * size_of::<Cell>()).ok_or(CheckpointError::Truncated)?;
        let raw = r.take(cell_bytes)?;
        let mut pool = vec![Cell::EMPTY; bricks * BRICK_CELLS];
        bytemuck::cast_slice_mut::<Cell, u8>(&mut pool).copy_from_slice(raw);

        let mut seen = vec![false; bricks];
        let mut claim = |slot: u32| -> Result<(), CheckpointError> {
            let used = seen.get_mut(slot as usize).ok_or(Inconsistent("slot beyond the pool"))?;
            if std::mem::replace(used, true) {
                return Err(Inconsistent("slot used twice"));
            }
            Ok(())
        };
        let free_len = r.count(4)?;
        let mut free = Vec::with_capacity(free_len);
        for _ in 0..free_len {
            let slot = r.u32()?;
            claim(slot)?;
            free.push(slot);
        }

        let chunk_len = r.count(12 + 1 + 8 + 4 * BRICKS_PER_CHUNK)?;
        let mut chunks = FastMap::default();
        chunks.reserve(chunk_len);
        let mut dirty_chunks = 0;
        for _ in 0..chunk_len {
            let chunk = r.coord()?;
            let scale = Scale(r.u8()?);
            let dirty = r.u64()?;
            let mut slots = [NO_SLOT; BRICKS_PER_CHUNK];
            let mut present = 0u64;
            for (b, slot) in slots.iter_mut().enumerate() {
                *slot = r.u32()?;
                if *slot != NO_SLOT {
                    claim(*slot)?;
                    present |= 1 << b;
                }
            }
            if dirty & !present != 0 {
                return Err(Inconsistent("dirty bit on an absent brick"));
            }
            dirty_chunks += usize::from(dirty != 0);
            let entry = ChunkEntry { scale, dirty, awake_index: INACTIVE, slots };
            if chunks.insert(chunk, entry).is_some() {
                return Err(Inconsistent("chunk listed twice"));
            }
        }
        if !seen.iter().all(|s| *s) {
            return Err(Inconsistent("a slot is neither used nor free"));
        }

        let awake_len = r.count(12)?;
        if awake_len != dirty_chunks {
            return Err(Inconsistent("awake list does not match the dirty chunks"));
        }
        let mut awake_chunks = Vec::with_capacity(awake_len);
        for index in 0..awake_len {
            let chunk = r.coord()?;
            let entry = chunks.get_mut(&chunk).ok_or(Inconsistent("awake chunk is not stored"))?;
            if entry.dirty == 0 || entry.awake_index != INACTIVE {
                return Err(Inconsistent("awake chunk is clean or listed twice"));
            }
            entry.awake_index = index;
            awake_chunks.push(chunk);
        }
        r.finish()?;
        Ok(Self { capacity, pool, free, chunks, awake_chunks })
    }
}
