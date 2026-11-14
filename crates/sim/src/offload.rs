//! The kernel split for an accelerator: gather one phase's blocks, let something else apply the block
//! rule to all of them at once, then scatter the results with exactly the CPU kernel's bookkeeping
//! (bricks reserved for writes, a block stalled when the pool is full, bricks woken by a changed or
//! pending block). Blocks of one phase are disjoint, so applying the rule to all of them before writing
//! any back is the same as the CPU kernel's one-at-a-time loop; scatter keeps the CPU's block order, so
//! reservations and stalls happen in the same order too.
use crate::anchors::anchors;
use crate::cell::{BrickCoord, Cell};
use crate::kernel::{StepStats, block_spots};
use crate::pinned::PHASE_OFFSETS;
use crate::rules::Outcome;
use crate::store::{BrickStore, NO_SLOT};
use glam::IVec3;

/// Where a block's cell lives: chunk, brick in the chunk, cell in the brick.
type Spot = (IVec3, u8, usize);

/// One phase's blocks: anchors and the eight cells of each (an absent brick reads as empty space).
/// Blocks whose eight cells all lie in absent bricks are left out: empty space never moves.
#[derive(Clone, Debug, Default)]
pub struct PhaseBlocks {
    pub anchors: Vec<IVec3>,
    pub cells: Vec<[Cell; 8]>,
    spots: Vec<[Spot; 8]>,
}

impl PhaseBlocks {
    pub fn len(&self) -> usize {
        self.anchors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.anchors.is_empty()
    }
}

fn slot(store: &BrickStore, (chunk, brick, _): Spot) -> u32 {
    store.chunk_slots(chunk).map_or(NO_SLOT, |s| s[brick as usize])
}

/// The blocks of phase `phase_counter` (`2 * tick + phase`) that touch `bricks`, in the CPU kernel's order.
pub fn gather(store: &BrickStore, bricks: &[BrickCoord], phase_counter: u32) -> PhaseBlocks {
    let mut out = PhaseBlocks::default();
    // Gather only reads the store, so a chunk's slot table can be reused across blocks.
    let mut cached = None;
    for anchor in anchors(bricks, PHASE_OFFSETS[(phase_counter & 1) as usize]) {
        let at = block_spots(anchor);
        let slots = at.map(|(chunk, brick, _)| {
            let table = match cached {
                Some((key, table)) if key == chunk => table,
                _ => {
                    let table = store.chunk_slots(chunk);
                    cached = Some((chunk, table));
                    table
                }
            };
            table.map_or(NO_SLOT, |slots| slots[brick as usize])
        });
        if slots.iter().all(|&s| s == NO_SLOT) {
            continue;
        }
        out.anchors.push(anchor);
        out.cells.push(std::array::from_fn(|i| if slots[i] == NO_SLOT { Cell::EMPTY } else { store.cell_in_slot(slots[i], at[i].2) }));
        out.spots.push(at);
    }
    out
}

/// Write `after` back for every block, as the CPU kernel would have block by block.
pub fn scatter(store: &mut BrickStore, blocks: &PhaseBlocks, after: &[[Cell; 8]], outcomes: &[Outcome], stats: &mut StepStats) {
    assert!(after.len() == blocks.len() && outcomes.len() == blocks.len(), "one result per gathered block");
    for (k, at) in blocks.spots.iter().enumerate() {
        let (before, after, outcome) = (&blocks.cells[k], &after[k], outcomes[k]);
        stats.blocks += 1;
        let mut written = outcome.changed;
        if written {
            let mut needed = [(IVec3::ZERO, 0); 8];
            let mut count = 0;
            for i in (0..8).filter(|&i| after[i] != before[i]) {
                let (chunk, brick, _) = at[i];
                if slot(store, at[i]) == NO_SLOT && !needed[..count].contains(&(chunk, brick)) {
                    needed[count] = (chunk, brick);
                    count += 1;
                }
            }
            if count > store.free_bricks() {
                written = false;
                stats.stalled_blocks += 1;
            } else {
                for &(chunk, brick) in &needed[..count] {
                    // Counted above, so this cannot fail.
                    let _ = store.reserve(chunk, brick);
                }
            }
        }
        if written {
            stats.changed_blocks += 1;
            for i in (0..8).filter(|&i| after[i] != before[i]) {
                store.set_cell_in_slot(slot(store, at[i]), at[i].2, after[i]);
            }
        }
        if outcome.changed || outcome.pending {
            for &(chunk, brick, _) in at {
                store.wake(chunk, brick);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use crate::*;
    use engine_core::hash::hash_u32;

    /// A random world of every synthetic material, hot and cold, across a chunk corner at negative coordinates.
    fn random_world(seed: u32) -> BrickStore {
        let mut s = BrickStore::new(400);
        let palette = [AIR, STONE, SAND, WATER, SMOKE, DIRT, LAVA, FOAM];
        for x in -6..6 {
            for y in -6..6 {
                for z in -6..6 {
                    let h = hash_u32(seed ^ hash_u32((x + 64) as u32 | ((y + 64) as u32) << 8 | ((z + 64) as u32) << 16));
                    let m = palette[(h % 8) as usize];
                    if m != AIR {
                        let volume = if matches!(m, WATER | SMOKE | LAVA) { 1 + (h >> 8) as u8 % VOLUME_FULL } else { VOLUME_FULL };
                        put(&mut s, IVec3::new(x, y, z), Cell { volume, temperature: 2500 + (h >> 16) as u16 % 4000, ..Cell::filled(m) });
                    }
                }
            }
        }
        s
    }

    fn cells(s: &BrickStore) -> Vec<Cell> {
        (-8..8).flat_map(|x| (-8..8).flat_map(move |y| (-8..8).map(move |z| at(s, IVec3::new(x, y, z))))).collect()
    }

    /// Gather, apply the Rust rule to every block, scatter: the same world and the same dirty set as `step`.
    #[test]
    fn gather_rule_scatter_equals_the_cpu_kernel() {
        let t = table();
        for seed in 0..12 {
            let (mut cpu, mut split) = (random_world(seed), random_world(seed));
            for tick in 0..24 {
                let a = step(&mut cpu, &t, seed, tick);
                let b = step_with(&mut split, tick, |store, pc, bricks, stats| {
                    let blocks = gather(store, bricks, pc);
                    let mut after = blocks.cells.clone();
                    let outcomes: Vec<Outcome> =
                        after.iter_mut().zip(&blocks.anchors).map(|(c, &anchor)| block_rule(&t, c, seed, pc, anchor)).collect();
                    scatter(store, &blocks, &after, &outcomes, stats);
                });
                assert_eq!(a.changed_blocks, b.changed_blocks, "seed {seed} tick {tick}");
                assert_eq!(cpu.dirty_bricks(), split.dirty_bricks(), "seed {seed} tick {tick}");
            }
            assert_eq!(cells(&cpu), cells(&split), "seed {seed}");
        }
    }
}
