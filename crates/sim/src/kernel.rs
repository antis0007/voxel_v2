//! One cellular tick on the CPU: the reference the GPU kernels must match byte for byte.
//!
//! A tick runs both Margolus phases over the blocks that touch a dirty brick. A
//! brick goes idle only after a whole tick in which nothing in it changed and
//! no move was randomly suppressed, because a state that is static in one phase
//! may not be in the other. A block that changed or is pending wakes every brick
//! it spans, which is how a change reaches the neighbour across a face.
use crate::anchors::anchors;
use crate::cell::{BRICKS_PER_CHUNK, BrickCoord, Cell, brick_and_cell, split_voxel};
use crate::pinned::PHASE_OFFSETS;
use crate::rules::block_rule;
use crate::store::{BrickStore, NO_SLOT};
use engine_core::material::{SIM_MATERIAL_ROWS, SimMaterial};
use glam::IVec3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepStats {
    /// Dirty bricks at the start of the tick: the "active bricks" the overlay shows.
    pub active_bricks: u32,
    pub blocks: u32,
    pub changed_blocks: u32,
    /// Blocks skipped because their move needed a brick the full pool could not give.
    pub stalled_blocks: u32,
}

/// Voxel position of a block's cell `i` (`idx` order).
fn cell_pos(anchor: IVec3, i: usize) -> IVec3 {
    anchor + IVec3::new((i & 1) as i32, (i >> 2) as i32, ((i >> 1) & 1) as i32)
}

/// The common case, with one map lookup instead of sixteen: a block whose eight cells lie in one brick
/// reads and writes the brick directly. Returns false for a block that straddles a face, which takes
/// the general path. A block in an absent brick is all empty, and empty space never moves.
fn run_inside_brick(
    store: &mut BrickStore,
    table: &[SimMaterial],
    seed: u32,
    phase_counter: u32,
    anchor: IVec3,
    stats: &mut StepStats,
) -> bool {
    let (chunk, local) = split_voxel(anchor);
    if (local.x & 7) == 7 || (local.y & 7) == 7 || (local.z & 7) == 7 {
        return false;
    }
    let (brick, base) = brick_and_cell(local);
    stats.blocks += 1;
    let Some(cells) = store.brick_cells_mut(chunk, brick) else { return true };
    let at = |i: usize| base + (i & 1) + 8 * ((i >> 1) & 1) + 64 * (i >> 2);
    let before: [Cell; 8] = std::array::from_fn(|i| cells[at(i)]);
    let mut after = before;
    let outcome = block_rule(table, &mut after, seed, phase_counter, anchor);
    if outcome.changed {
        for i in 0..8 {
            cells[at(i)] = after[i];
        }
        stats.changed_blocks += 1;
    }
    if outcome.changed || outcome.pending {
        store.wake(chunk, brick);
    }
    true
}

/// Pointer tables of the few chunks a phase is working in, and the bricks it has woken in them. Reading
/// through this costs no map lookup per cell; the woken bits reach the store when an entry leaves the
/// cache and at the end of the phase, which is before anything reads them.
struct ChunkCache {
    entries: Vec<(IVec3, [u32; BRICKS_PER_CHUNK], u64)>,
    next: usize,
}

impl ChunkCache {
    const WAYS: usize = 8;

    fn new() -> Self {
        Self { entries: Vec::with_capacity(Self::WAYS), next: 0 }
    }

    /// Index of the entry for `chunk`, loading it (and flushing whichever it replaces) if needed.
    fn entry(&mut self, store: &mut BrickStore, chunk: IVec3) -> usize {
        if let Some(i) = self.entries.iter().position(|e| e.0 == chunk) {
            return i;
        }
        let fresh = (chunk, store.slots_of(chunk), 0);
        if self.entries.len() < Self::WAYS {
            self.entries.push(fresh);
            return self.entries.len() - 1;
        }
        let i = self.next;
        self.next = (self.next + 1) % Self::WAYS;
        let (old, _, woken) = std::mem::replace(&mut self.entries[i], fresh);
        store.or_dirty(old, woken);
        i
    }

    fn flush(&mut self, store: &mut BrickStore) {
        for (chunk, _, woken) in self.entries.drain(..) {
            store.or_dirty(chunk, woken);
        }
        self.next = 0;
    }
}

/// A block that may straddle brick and chunk faces, through the cache.
fn run_straddling(
    store: &mut BrickStore,
    cache: &mut ChunkCache,
    table: &[SimMaterial],
    seed: u32,
    phase_counter: u32,
    anchor: IVec3,
    stats: &mut StepStats,
) {
    let spots: [(IVec3, u8, usize); 8] = std::array::from_fn(|i| {
        let (chunk, local) = split_voxel(cell_pos(anchor, i));
        let (brick, cell) = brick_and_cell(local);
        (chunk, brick, cell)
    });
    let slot_of = |store: &mut BrickStore, cache: &mut ChunkCache, (chunk, brick, _): (IVec3, u8, usize)| {
        let e = cache.entry(store, chunk);
        cache.entries[e].1[brick as usize]
    };
    let before: [Cell; 8] = std::array::from_fn(|i| match slot_of(store, cache, spots[i]) {
        NO_SLOT => Cell::EMPTY,
        slot => store.cell_in_slot(slot, spots[i].2),
    });
    let mut after = before;
    let outcome = block_rule(table, &mut after, seed, phase_counter, anchor);
    stats.blocks += 1;
    let mut written = outcome.changed;
    if written {
        // Give every brick a changed cell lands in a slot first, so a full pool never leaves half a move.
        // The absent ones are counted against the free slots before any is taken, so a stall allocates nothing.
        let mut needed: Vec<(IVec3, u8)> = Vec::new();
        for i in (0..8).filter(|&i| after[i] != before[i]) {
            let (chunk, brick, _) = spots[i];
            if slot_of(store, cache, spots[i]) == NO_SLOT && !needed.contains(&(chunk, brick)) {
                needed.push((chunk, brick));
            }
        }
        if needed.len() > store.free_bricks() {
            written = false;
            stats.stalled_blocks += 1;
        } else {
            for (chunk, brick) in needed {
                // Counted above, so this cannot fail.
                let _ = store.reserve(chunk, brick);
                let e = cache.entry(store, chunk);
                cache.entries[e].1 = store.slots_of(chunk);
            }
        }
    }
    if written {
        stats.changed_blocks += 1;
        for i in (0..8).filter(|&i| after[i] != before[i]) {
            let slot = slot_of(store, cache, spots[i]);
            store.set_cell_in_slot(slot, spots[i].2, after[i]);
        }
    }
    // A changed or pending block wakes every brick it spans (a stalled one is retried once the pool has room).
    if outcome.changed || outcome.pending {
        for spot in spots {
            let slot = slot_of(store, cache, spot);
            if slot != NO_SLOT {
                let e = cache.entry(store, spot.0);
                cache.entries[e].2 |= 1 << spot.1;
            }
        }
    }
}

fn run_phase(
    store: &mut BrickStore,
    table: &[SimMaterial],
    seed: u32,
    phase_counter: u32,
    bricks: &[BrickCoord],
    stats: &mut StepStats,
) {
    let mut cache = ChunkCache::new();
    for anchor in anchors(bricks, PHASE_OFFSETS[(phase_counter & 1) as usize]) {
        if !run_inside_brick(store, table, seed, phase_counter, anchor, stats) {
            run_straddling(store, &mut cache, table, seed, phase_counter, anchor, stats);
        }
    }
    cache.flush(store);
}

/// Advance the stored world one tick. `table` is indexed by material id, one row per possible id.
pub fn step(store: &mut BrickStore, table: &[SimMaterial; SIM_MATERIAL_ROWS], seed: u32, tick: u32) -> StepStats {
    let mut stats = StepStats::default();
    let first = store.take_all_dirty();
    stats.active_bricks = first.len() as u32;
    run_phase(store, table, seed, tick.wrapping_mul(2), &first, &mut stats);
    // The second phase also steps whatever the first one woke.
    let woken = store.take_all_dirty();
    let mut both = first;
    both.extend(woken.iter().copied());
    both.sort_unstable_by_key(|b| (b.chunk.z, b.chunk.y, b.chunk.x, b.brick));
    both.dedup();
    run_phase(store, table, seed, tick.wrapping_mul(2).wrapping_add(1), &both, &mut stats);
    // The next tick steps what either phase touched.
    for b in woken {
        store.wake(b.chunk, b.brick);
    }
    stats
}
