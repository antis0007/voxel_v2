//! One cellular tick on the CPU: the reference the GPU kernels must match byte for byte.
//!
//! A tick runs both Margolus phases over the blocks that touch a dirty brick. A
//! brick goes idle only after a whole tick in which nothing in it changed and
//! no move was randomly suppressed, because a state that is static in one phase
//! may not be in the other. A block that changed or is pending wakes every brick
//! it spans, which is how a change reaches the neighbour across a face.
use crate::anchors::anchors;
use crate::cell::{BRICK_SIZE, BRICKS_PER_AXIS, BRICKS_PER_CHUNK, BrickCoord, Cell, brick_and_cell, split_voxel};
use crate::pinned::{PHASE_OFFSETS, is_heat_tick};
use crate::rules::{Outcome, block_rule};
use crate::store::{BrickStore, NO_SLOT};
use engine_core::reaction::SimRules;
use engine_core::{CHUNK_SIZE, floor_div};
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
pub(crate) fn cell_pos(anchor: IVec3, i: usize) -> IVec3 {
    anchor + IVec3::new((i & 1) as i32, (i >> 2) as i32, ((i >> 1) & 1) as i32)
}

/// `block_rule`, except that a block of eight `EMPTY` cells returns at once when empty space has no
/// reaction: it cannot fall, slide, level or conduct, so the rule would report no change. Awake
/// bricks above water and sand are mostly such blocks.
fn step_block(rules: &SimRules, cells: &mut [Cell; 8], seed: u32, phase_counter: u32, anchor: IVec3) -> Outcome {
    if !rules.reactive(Cell::EMPTY.material) && cells.iter().all(|c| *c == Cell::EMPTY) {
        return Outcome::default();
    }
    block_rule(rules, cells, seed, phase_counter, anchor)
}

/// Brick and first cell of a block whose eight cells lie in one brick; `None` when it straddles a face.
fn inside_brick(anchor: IVec3) -> Option<(IVec3, u8, usize)> {
    let (chunk, local) = split_voxel(anchor);
    if (local.x & 7) == 7 || (local.y & 7) == 7 || (local.z & 7) == 7 {
        return None;
    }
    let (brick, base) = brick_and_cell(local);
    Some((chunk, brick, base))
}

/// The blocks of one phase that lie inside one brick. Their cells belong to no other block of the
/// phase and they never allocate, so groups may run in any order or at once on separate threads.
struct BrickGroup {
    chunk: IVec3,
    brick: u8,
    /// (anchor, index of the block's first cell in the brick)
    blocks: Vec<(IVec3, usize)>,
}

/// Run every block of a group on its brick's cells: (changed blocks, whether the brick must stay awake).
fn run_group(cells: &mut [Cell], rules: &SimRules, seed: u32, phase_counter: u32, blocks: &[(IVec3, usize)]) -> (u32, bool) {
    let (mut changed, mut awake) = (0, false);
    for &(anchor, base) in blocks {
        let at = |i: usize| base + (i & 1) + 8 * ((i >> 1) & 1) + 64 * (i >> 2);
        let before: [Cell; 8] = std::array::from_fn(|i| cells[at(i)]);
        let mut after = before;
        let outcome = step_block(rules, &mut after, seed, phase_counter, anchor);
        if outcome.changed {
            for i in 0..8 {
                cells[at(i)] = after[i];
            }
            changed += 1;
        }
        awake |= outcome.changed || outcome.pending;
    }
    (changed, awake)
}

/// Below this many blocks a phase runs on the calling thread; spawning workers costs more than it saves.
const PARALLEL_MIN_BLOCKS: usize = 8192;

/// 0 until first read; then the worker limit for brick groups.
static WORKER_LIMIT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Let brick groups use up to `workers` threads (1 = the calling thread only). Results are the same
/// bytes for any count. Workers are spawned per phase and a phase waits for the slowest, so under
/// competing load (peer builds measured 2026-11-06) more workers ran slower than one.
pub fn set_kernel_workers(workers: usize) {
    WORKER_LIMIT.store(workers.clamp(1, 8), std::sync::atomic::Ordering::Relaxed);
}

/// Worker threads for brick groups: `set_kernel_workers`, else `VOXEL_SIM_WORKERS`, else one.
fn worker_count() -> usize {
    #[cfg(test)]
    if tests::FORCE_SERIAL.with(std::cell::Cell::get) {
        return 1;
    }
    let limit = match WORKER_LIMIT.load(std::sync::atomic::Ordering::Relaxed) {
        0 => {
            let env = std::env::var("VOXEL_SIM_WORKERS").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
            set_kernel_workers(env);
            WORKER_LIMIT.load(std::sync::atomic::Ordering::Relaxed)
        }
        n => n,
    };
    std::thread::available_parallelism().map_or(1, |n| n.get()).min(limit)
}

/// Run the groups, in parallel when large, and fold their counts and wakes into the store. A group
/// in an absent brick is all empty, and empty space never moves.
fn run_groups(store: &mut BrickStore, rules: &SimRules, seed: u32, phase_counter: u32, groups: &[BrickGroup], stats: &mut StepStats) {
    let total: usize = groups.iter().map(|g| g.blocks.len()).sum();
    stats.blocks += total as u32;
    let slots: Vec<u32> = groups.iter().map(|g| store.chunk_slots(g.chunk).map_or(NO_SLOT, |s| s[g.brick as usize])).collect();
    let mut results = vec![(0u32, false); groups.len()];
    {
        let mut bricks: Vec<Option<&mut [Cell]>> = store.slot_cells_mut().map(|b| Some(&mut b[..])).collect();
        // Each listed brick appears once (the phase's brick list is deduplicated), so every slot is taken once.
        let mut jobs: Vec<(usize, &mut [Cell])> = Vec::with_capacity(groups.len());
        for (g, &slot) in slots.iter().enumerate() {
            if slot != NO_SLOT
                && let Some(cells) = bricks.get_mut(slot as usize).and_then(Option::take)
            {
                jobs.push((g, cells));
            }
        }
        let workers = worker_count();
        if total < PARALLEL_MIN_BLOCKS || workers < 2 {
            for (g, cells) in jobs {
                results[g] = run_group(cells, rules, seed, phase_counter, &groups[g].blocks);
            }
        } else {
            let per = jobs.len().div_ceil(workers).max(1);
            let parts: Vec<Vec<(usize, (u32, bool))>> = std::thread::scope(|scope| {
                let handles: Vec<_> = jobs
                    .chunks_mut(per)
                    .map(|part| {
                        scope.spawn(move || {
                            part.iter_mut()
                                .map(|(g, cells)| (*g, run_group(cells, rules, seed, phase_counter, &groups[*g].blocks)))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap_or_else(|e| std::panic::resume_unwind(e))).collect()
            });
            for (g, result) in parts.into_iter().flatten() {
                results[g] = result;
            }
        }
    }
    for (group, (changed, awake)) in groups.iter().zip(results) {
        stats.changed_blocks += changed;
        if awake {
            store.wake(group.chunk, group.brick);
        }
    }
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

/// Chunk, brick and cell of each of a block's eight cells in `idx` order. A block's two cells on one
/// axis are `v` and `v + 1`, so each axis is split once instead of once per cell.
pub(crate) fn block_spots(anchor: IVec3) -> [(IVec3, u8, usize); 8] {
    let split = |v: i32| {
        let (c0, c1) = (floor_div(v, CHUNK_SIZE), floor_div(v + 1, CHUNK_SIZE));
        [(c0, v - c0 * CHUNK_SIZE), (c1, v + 1 - c1 * CHUNK_SIZE)]
    };
    let (xs, ys, zs) = (split(anchor.x), split(anchor.y), split(anchor.z));
    std::array::from_fn(|i| {
        let ((cx, lx), (cy, ly), (cz, lz)) = (xs[i & 1], ys[i >> 2], zs[(i >> 1) & 1]);
        let (per, size) = (BRICKS_PER_AXIS, BRICK_SIZE);
        let brick = (lx / size + per * (lz / size + per * (ly / size))) as u8;
        let cell = (lx % size + size * (lz % size + size * (ly % size))) as usize;
        (IVec3::new(cx, cy, cz), brick, cell)
    })
}

/// Pool slot of each spot. Consecutive spots in one chunk share a cache lookup; between two lookups
/// nothing else touches the cache, so the remembered entry cannot have been replaced.
fn resolve_slots(store: &mut BrickStore, cache: &mut ChunkCache, spots: &[(IVec3, u8, usize); 8]) -> [u32; 8] {
    let mut last = (spots[0].0, cache.entry(store, spots[0].0));
    std::array::from_fn(|i| {
        if spots[i].0 != last.0 {
            last = (spots[i].0, cache.entry(store, spots[i].0));
        }
        cache.entries[last.1].1[spots[i].1 as usize]
    })
}

/// A block that may straddle brick and chunk faces, through the cache.
fn run_straddling(
    store: &mut BrickStore,
    cache: &mut ChunkCache,
    rules: &SimRules,
    seed: u32,
    phase_counter: u32,
    anchor: IVec3,
    stats: &mut StepStats,
) {
    let spots = block_spots(anchor);
    // Resolved once and reused for the read, the write and the wake; a reservation below refreshes them.
    let mut slots = resolve_slots(store, cache, &spots);
    let before: [Cell; 8] = std::array::from_fn(|i| match slots[i] {
        NO_SLOT => Cell::EMPTY,
        slot => store.cell_in_slot(slot, spots[i].2),
    });
    let mut after = before;
    let outcome = step_block(rules, &mut after, seed, phase_counter, anchor);
    stats.blocks += 1;
    let mut written = outcome.changed;
    if written {
        // Give every brick a changed cell lands in a slot first, so a full pool never leaves half a move.
        // The absent ones are counted against the free slots before any is taken, so a stall allocates nothing.
        let mut needed: Vec<(IVec3, u8)> = Vec::new();
        for i in (0..8).filter(|&i| after[i] != before[i]) {
            let (chunk, brick, _) = spots[i];
            if slots[i] == NO_SLOT && !needed.contains(&(chunk, brick)) {
                needed.push((chunk, brick));
            }
        }
        if needed.len() > store.free_bricks() {
            written = false;
            stats.stalled_blocks += 1;
        } else if !needed.is_empty() {
            for (chunk, brick) in needed {
                // Counted above, so this cannot fail.
                let _ = store.reserve(chunk, brick);
                let e = cache.entry(store, chunk);
                cache.entries[e].1 = store.slots_of(chunk);
            }
            slots = resolve_slots(store, cache, &spots);
        }
    }
    if written {
        stats.changed_blocks += 1;
        for i in (0..8).filter(|&i| after[i] != before[i]) {
            store.set_cell_in_slot(slots[i], spots[i].2, after[i]);
        }
    }
    // A changed or pending block wakes every brick it spans (a stalled one is retried once the pool has room).
    if outcome.changed || outcome.pending {
        let mut last: Option<(IVec3, usize)> = None;
        for (spot, &slot) in spots.into_iter().zip(&slots) {
            if slot != NO_SLOT {
                let e = match last {
                    Some((chunk, e)) if chunk == spot.0 => e,
                    _ => cache.entry(store, spot.0),
                };
                last = Some((spot.0, e));
                cache.entries[e].2 |= 1 << spot.1;
            }
        }
    }
}

fn run_phase(
    store: &mut BrickStore,
    rules: &SimRules,
    seed: u32,
    phase_counter: u32,
    bricks: &[BrickCoord],
    skip: &impl Fn(IVec3) -> bool,
    stats: &mut StepStats,
) {
    // `anchors` lists a brick's inside blocks while it handles that brick, so they arrive in runs.
    let mut groups: Vec<BrickGroup> = Vec::with_capacity(bricks.len());
    let mut straddling = Vec::new();
    for anchor in anchors(bricks, PHASE_OFFSETS[(phase_counter & 1) as usize]) {
        if skip(anchor) {
            continue;
        }
        match inside_brick(anchor) {
            Some((chunk, brick, base)) => match groups.last_mut() {
                Some(g) if g.chunk == chunk && g.brick == brick => g.blocks.push((anchor, base)),
                _ => groups.push(BrickGroup { chunk, brick, blocks: vec![(anchor, base)] }),
            },
            None => straddling.push(anchor),
        }
    }
    // Inside blocks never allocate and share no cell with a straddling block, so running them first
    // changes no result; straddling blocks keep their order, which decides who stalls on a full pool.
    run_groups(store, rules, seed, phase_counter, &groups, stats);
    let mut cache = ChunkCache::new();
    for anchor in straddling {
        run_straddling(store, &mut cache, rules, seed, phase_counter, anchor, stats);
    }
    cache.flush(store);
}

/// Advance the stored world one tick under `rules`: the material table and the reactions.
pub fn step(store: &mut BrickStore, rules: &SimRules, seed: u32, tick: u32) -> StepStats {
    step_skipping(store, rules, seed, tick, |_| false)
}

/// `step`, leaving every block whose anchor `skip` accepts untouched and uncounted: the blocks that
/// fine matter owns are stepped at 0.25 m instead (ADR 0023).
pub fn step_skipping(
    store: &mut BrickStore,
    rules: &SimRules,
    seed: u32,
    tick: u32,
    skip: impl Fn(IVec3) -> bool,
) -> StepStats {
    step_with(store, tick, |store, phase_counter, bricks, stats| {
        run_phase(store, rules, seed, phase_counter, bricks, &skip, stats)
    })
}

/// The tick driver every backend shares: which bricks each phase steps, and which stay awake after.
/// `run_phase(store, phase_counter, bricks, stats)` steps every block of one Margolus phase that touches
/// `bricks`; the CPU kernel and an accelerator (`offload`) differ only there.
pub fn step_with(
    store: &mut BrickStore,
    tick: u32,
    mut run_phase: impl FnMut(&mut BrickStore, u32, &[BrickCoord], &mut StepStats),
) -> StepStats {
    let mut stats = StepStats::default();
    let first = store.take_all_dirty();
    stats.active_bricks = first.len() as u32;
    let awake_at_start = first.clone();
    run_phase(store, tick.wrapping_mul(2), &first, &mut stats);
    // The second phase also steps whatever the first one woke.
    let woken = store.take_all_dirty();
    let mut both = first;
    both.extend(woken.iter().copied());
    both.sort_unstable_by_key(|b| (b.chunk.z, b.chunk.y, b.chunk.x, b.brick));
    both.dedup();
    run_phase(store, tick.wrapping_mul(2).wrapping_add(1), &both, &mut stats);
    // The next tick steps what either phase touched.
    for b in woken {
        store.wake(b.chunk, b.brick);
    }
    // Heat conducts only on heat ticks, so a tick without it says nothing about whether heat is still
    // flowing: whatever was awake stays awake, and the decision to sleep is made on a heat tick.
    if !is_heat_tick(tick) {
        for b in awake_at_start {
            store.wake(b.chunk, b.brick);
        }
    }
    stats
}

#[cfg(test)]
#[path = "kernel_tests.rs"]
mod tests;
