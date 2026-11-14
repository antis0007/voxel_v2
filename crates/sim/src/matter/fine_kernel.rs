//! The Margolus pass at 0.25 m: which fine blocks a phase visits and one block's step.
use super::{BlockView, MatterStore};
use crate::kernel::cell_pos;
use crate::rules::block_rule;
use crate::{Cell, FINE_SIZE};
use engine_core::fast_hash::FastSet;
use engine_core::reaction::SimRules;
use glam::IVec3;

/// Folded into the seed of every fine block so its dice differ from the 1 m block at the same anchor.
pub(super) const FINE_SEED_SALT: u32 = 0x5EED_F1CE;

/// What one fine block did. `stalled`: its move needed pool slots the fine pool could not give, so
/// nothing was written and the block must be retried.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct FineOutcome {
    pub changed: bool,
    pub pending: bool,
    pub stalled: bool,
    /// Uniform neighbours that changed and so became fine.
    pub promoted: u32,
}

/// Block coordinate and the fine cell inside it (each axis `0..4`) of a fine coordinate.
pub(super) fn split_fine(fine: IVec3) -> (IVec3, IVec3) {
    let size = IVec3::splat(FINE_SIZE);
    (fine.div_euclid(size), fine.rem_euclid(size))
}

fn fine_index(local: IVec3) -> usize {
    (local.x + FINE_SIZE * (local.z + FINE_SIZE * local.y)) as usize
}

/// Odd-phase anchor offsets per axis, and which neighbour (-1, 0 or +1) the block reaches there.
const ODD_STEPS: [(i32, i32); 3] = [(-1, -1), (1, 0), (3, 1)];

/// Anchors, in fine coordinates, of this phase's 2×2×2 blocks that touch any of `blocks`, each once,
/// in the order of `blocks`. The even phase tiles a fine block exactly; the odd phase's blocks start
/// one cell before it and the last ones reach one cell into the next block, which is how fine matter
/// meets a uniform neighbour. A block shared with a listed neighbour that comes first in `(z, y, x)`
/// order is that neighbour's to list, the rule `anchors` uses for bricks.
pub(super) fn fine_anchors(blocks: &[IVec3], offset: IVec3) -> Vec<IVec3> {
    if offset == IVec3::ZERO {
        let mut out = Vec::with_capacity(blocks.len() * 8);
        for &block in blocks {
            let origin = block * FINE_SIZE;
            for dy in [0, 2] {
                for dz in [0, 2] {
                    for dx in [0, 2] {
                        out.push(origin + IVec3::new(dx, dy, dz));
                    }
                }
            }
        }
        return out;
    }
    let listed: FastSet<IVec3> = blocks.iter().copied().collect();
    let mut out = Vec::with_capacity(blocks.len() * 27);
    for &block in blocks {
        let origin = block * FINE_SIZE;
        for (dy, ry) in ODD_STEPS {
            for (dz, rz) in ODD_STEPS {
                for (dx, rx) in ODD_STEPS {
                    let mut claimed = true;
                    'reach: for sz in rz.min(0)..=rz.max(0) {
                        for sy in ry.min(0)..=ry.max(0) {
                            for sx in rx.min(0)..=rx.max(0) {
                                if (sz, sy, sx) < (0, 0, 0) && listed.contains(&(block + IVec3::new(sx, sy, sz))) {
                                    claimed = false;
                                    break 'reach;
                                }
                            }
                        }
                    }
                    if claimed {
                        out.push(origin + IVec3::new(dx, dy, dz));
                    }
                }
            }
        }
    }
    out
}

impl MatterStore {
    /// The eight cells of a block, resolving each distinct block once: the even phase reads one
    /// block, the odd phase up to eight.
    fn read_cells(&self, spots: &[(IVec3, IVec3); 8], crossed: usize) -> [Cell; 8] {
        let mut views = [BlockView::Uniform(Cell::EMPTY); 8];
        std::array::from_fn(|i| {
            let (block, local) = spots[i];
            let first = i & crossed;
            if i == first {
                views[first] = self.block(block);
            }
            match views[first] {
                BlockView::Uniform(cell) => cell,
                BlockView::Fine(fine) => fine[fine_index(local)],
            }
        })
    }

    /// Step the fine block at `anchor`. Uniform blocks are read as 64 identical cells and promoted
    /// only when one of their cells changes; every fine block the eight cells lie in is pushed onto
    /// `woken` when anything changed, is pending or stalled.
    pub(super) fn run_fine_block(
        &mut self,
        rules: &SimRules,
        seed: u32,
        phase_counter: u32,
        tick: u32,
        anchor: IVec3,
        woken: &mut Vec<IVec3>,
    ) -> FineOutcome {
        let (block, local) = split_fine(anchor);
        // Cell indices encode x, z, y in their low three bits. Only axes that
        // cross a block boundary distinguish the owning blocks, so i & crossed
        // identifies both the owner and its first cell in this stencil.
        let crossed = usize::from(local.x == FINE_SIZE - 1)
            | (usize::from(local.z == FINE_SIZE - 1) << 1)
            | (usize::from(local.y == FINE_SIZE - 1) << 2);
        let spots = std::array::from_fn(|i| {
            let neighbor = cell_pos(IVec3::ZERO, i & crossed);
            (block + neighbor, cell_pos(local, i) - neighbor * FINE_SIZE)
        });
        let before = self.read_cells(&spots, crossed);
        let mut after = before;
        let outcome = block_rule(rules, &mut after, seed ^ FINE_SEED_SALT, phase_counter, anchor);
        let mut result = FineOutcome { pending: outcome.pending, ..FineOutcome::default() };
        if outcome.changed {
            // Every uniform block a changed cell lands in needs a pool slot first, so a full pool
            // never leaves half a move.
            let mut checked = 0u8;
            let mut needed = 0;
            for i in (0..8).filter(|&i| after[i] != before[i]) {
                let bit = 1 << (i & crossed);
                if checked & bit == 0 {
                    checked |= bit;
                    needed += usize::from(!self.is_fine(spots[i].0));
                }
            }
            if needed > self.fine.free_slots() {
                result.stalled = true;
            } else {
                for i in (0..8).filter(|&i| after[i] != before[i]) {
                    // Counted above, so promotion cannot run out of pool; the local cell is inside the block.
                    if self.set_fine(spots[i].0, spots[i].1, after[i], tick).is_err() {
                        result.stalled = true;
                    }
                }
                result.changed = true;
                result.promoted = needed as u32;
            }
        }
        if result.changed || result.pending || result.stalled {
            for (i, (block, _)) in spots.into_iter().enumerate() {
                if i == i & crossed && self.is_fine(block) {
                    woken.push(block);
                }
            }
        }
        result
    }
}
