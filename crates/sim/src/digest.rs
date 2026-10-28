//! A region digest that peers compare to detect a desync, kept current from changed bricks.
//!
//! Three levels: a hash per stored brick, a sum per chunk and a sum over the region. Sums wrap, so
//! the order bricks are folded in (map order, refresh order) cannot change a value, and an absent
//! or all-empty brick contributes zero exactly as `BrickStore::state_hash` treats it. A mismatch in
//! the region sum narrows to chunks, then to bricks, without hashing anything twice.
//!
//! The digest learns what changed from the store's dirty masks, which the kernel leaves set on every
//! brick a tick wrote. Masks are consumed by `step`, so `observe` must run after a batch of edits
//! and again after the step; `step_tracked` does both. Whole-chunk operations that free bricks
//! (`fill_chunk`, `trim`, `remove_chunk`) leave no mask behind and need `mark_chunk`.
use crate::cell::{BRICK_CELLS, BRICKS_PER_CHUNK, Cell};
use crate::kernel::{StepStats, step};
use crate::store::{BrickStore, NO_SLOT};
use engine_core::fast_hash::FastMap;
use engine_core::reaction::SimRules;
use glam::IVec3;

const MUL: u64 = 0x9e37_79b9_7f4a_7c15;

/// The cell as one integer, field by field, so the value does not depend on byte order.
fn word(c: Cell) -> u64 {
    u64::from(c.material)
        | u64::from(c.volume) << 8
        | u64::from(c.temperature) << 16
        | u64::from(c.pressure as u16) << 32
        | u64::from(c.charge as u16) << 48
}

/// The splitmix64 finaliser: every input bit reaches every output bit.
fn avalanche(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Hash of one brick's `BRICK_CELLS` cells at its place in the world; zero when every cell is `EMPTY`.
pub fn brick_hash(chunk: IVec3, brick: u8, cells: &[Cell]) -> u64 {
    debug_assert_eq!(cells.len(), BRICK_CELLS);
    // Four lanes keep four multiplies in flight instead of one long dependency chain.
    let mut lanes = [0x243f_6a88_85a3_08d3_u64, 0x1319_8a2e_0370_7344, 0xa409_3822_299f_31d0, 0x082e_fa98_ec4e_6c89];
    let empty = word(Cell::EMPTY);
    let mut any = false;
    for quad in cells.chunks_exact(4) {
        for (h, &c) in lanes.iter_mut().zip(quad) {
            let w = word(c);
            any |= w != empty;
            *h = (h.rotate_left(23) ^ w).wrapping_mul(MUL);
        }
    }
    if !any {
        return 0;
    }
    let place = [chunk.x, chunk.y, chunk.z].iter().fold(u64::from(brick), |acc, &v| avalanche(acc ^ v as u32 as u64));
    let mixed = lanes.iter().fold(place, |acc, &h| avalanche(acc ^ h));
    // Zero is reserved for empty bricks.
    mixed.max(1)
}

struct ChunkHashes {
    bricks: [u64; BRICKS_PER_CHUNK],
    sum: u64,
}

/// Per-brick, per-chunk and region hashes of one `BrickStore`, refreshed from marked bricks.
#[derive(Default)]
pub struct RegionDigest {
    chunks: FastMap<IVec3, ChunkHashes>,
    total: u64,
    /// Bricks to rehash at the next `refresh`, one mask per chunk.
    marked: FastMap<IVec3, u64>,
}

impl RegionDigest {
    /// A digest of every brick in `store`.
    pub fn of(store: &BrickStore) -> Self {
        let mut d = Self::default();
        for chunk in store.chunks() {
            d.mark_chunk(chunk);
        }
        d.refresh(store);
        d
    }

    /// The region sum. Equal stores give equal sums whatever their pool layout.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// The chunk's sum, zero when it holds nothing.
    pub fn chunk(&self, chunk: IVec3) -> u64 {
        self.chunks.get(&chunk).map_or(0, |c| c.sum)
    }

    /// Every chunk with a nonzero sum, in (z, y, x) order: what a peer sends to narrow a mismatch.
    pub fn chunk_sums(&self) -> Vec<(IVec3, u64)> {
        let mut out: Vec<(IVec3, u64)> = self.chunks.iter().map(|(&c, h)| (c, h.sum)).collect();
        out.sort_unstable_by_key(|(c, _)| (c.z, c.y, c.x));
        out
    }

    /// The 64 brick hashes of a chunk, zeros when it holds nothing.
    pub fn brick_hashes(&self, chunk: IVec3) -> [u64; BRICKS_PER_CHUNK] {
        self.chunks.get(&chunk).map_or([0; BRICKS_PER_CHUNK], |c| c.bricks)
    }

    /// Chunks whose sum differs from `theirs`, a peer's `chunk_sums`, in (z, y, x) order.
    pub fn differing_chunks(&self, theirs: &[(IVec3, u64)]) -> Vec<IVec3> {
        let mut seen: FastMap<IVec3, u64> = theirs.iter().copied().collect();
        let mut out: Vec<IVec3> = Vec::new();
        for (&chunk, h) in &self.chunks {
            if seen.remove(&chunk) != Some(h.sum) {
                out.push(chunk);
            }
        }
        out.extend(seen.into_iter().filter(|&(_, sum)| sum != 0).map(|(c, _)| c));
        out.sort_unstable_by_key(|c| (c.z, c.y, c.x));
        out
    }

    /// Queue one brick for rehashing.
    pub fn mark(&mut self, chunk: IVec3, brick: u8) {
        *self.marked.entry(chunk).or_insert(0) |= 1 << brick;
    }

    /// Queue every brick of a chunk: after `fill_chunk`, `trim` or `remove_chunk`.
    pub fn mark_chunk(&mut self, chunk: IVec3) {
        self.marked.insert(chunk, u64::MAX);
    }

    /// Queue every brick the store has marked dirty. Costs one visit per awake chunk.
    pub fn observe(&mut self, store: &BrickStore) {
        for b in store.dirty_bricks() {
            self.mark(b.chunk, b.brick);
        }
    }

    /// Rehash every queued brick and return how many were hashed. The cost is bounded by the bricks
    /// that changed since the last refresh, never by the size of the region.
    pub fn refresh(&mut self, store: &BrickStore) -> usize {
        let mut hashed = 0;
        for (chunk, mut mask) in std::mem::take(&mut self.marked) {
            let slots = store.chunk_slots(chunk).copied().unwrap_or([NO_SLOT; BRICKS_PER_CHUNK]);
            let entry = self.chunks.entry(chunk).or_insert(ChunkHashes { bricks: [0; BRICKS_PER_CHUNK], sum: 0 });
            while mask != 0 {
                let brick = mask.trailing_zeros() as usize;
                mask &= mask - 1;
                let fresh = match slots[brick] {
                    NO_SLOT => 0,
                    slot => {
                        hashed += 1;
                        brick_hash(chunk, brick as u8, store.slot_cells(slot))
                    }
                };
                let old = std::mem::replace(&mut entry.bricks[brick], fresh);
                entry.sum = entry.sum.wrapping_sub(old).wrapping_add(fresh);
                self.total = self.total.wrapping_sub(old).wrapping_add(fresh);
            }
            if entry.bricks.iter().all(|&h| h == 0) {
                self.chunks.remove(&chunk);
            }
        }
        hashed
    }
}

/// `step`, queueing in `digest` every brick the edits since the last call and the tick itself wrote.
pub fn step_tracked(store: &mut BrickStore, digest: &mut RegionDigest, rules: &SimRules, seed: u32, tick: u32) -> StepStats {
    digest.observe(store);
    let stats = step(store, rules, seed, tick);
    digest.observe(store);
    stats
}
