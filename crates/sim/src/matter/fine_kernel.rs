//! The Margolus pass at 0.25 m: which fine blocks a phase visits and one block's step.
use super::MatterStore;
use crate::kernel::cell_pos;
use crate::rules::block_rule;
use crate::{Cell, FINE_SIZE};
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

/// Anchors, in fine coordinates, of this phase's 2×2×2 blocks that touch any of `blocks`, each once,
/// in `(z, y, x)` order. The even phase tiles a fine block exactly; the odd phase's blocks start one
/// cell before it and the last ones reach one cell into the next block, which is how fine matter meets
/// a uniform neighbour.
pub(super) fn fine_anchors(blocks: &[IVec3], offset: IVec3) -> Vec<IVec3> {
    let steps: &[i32] = if offset == IVec3::ZERO { &[0, 2] } else { &[-1, 1, 3] };
    let mut out = Vec::with_capacity(blocks.len() * steps.len().pow(3));
    for &block in blocks {
        let origin = block * FINE_SIZE;
        for &dy in steps {
            for &dz in steps {
                for &dx in steps {
                    out.push(origin + IVec3::new(dx, dy, dz));
                }
            }
        }
    }
    out.sort_unstable_by_key(|a| (a.z, a.y, a.x));
    out.dedup();
    out
}

impl MatterStore {
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
        let spots: [(IVec3, IVec3); 8] = std::array::from_fn(|i| split_fine(cell_pos(anchor, i)));
        let before: [Cell; 8] = std::array::from_fn(|i| self.fine_cell_at(spots[i].0, fine_index(spots[i].1)));
        let mut after = before;
        let outcome = block_rule(rules, &mut after, seed ^ FINE_SEED_SALT, phase_counter, anchor);
        let mut result = FineOutcome { pending: outcome.pending, ..FineOutcome::default() };
        if outcome.changed {
            // Every uniform block a changed cell lands in needs a pool slot first, so a full pool
            // never leaves half a move.
            let mut needed: Vec<IVec3> = Vec::new();
            for i in (0..8).filter(|&i| after[i] != before[i]) {
                let block = spots[i].0;
                if !self.is_fine(block) && !needed.contains(&block) {
                    needed.push(block);
                }
            }
            if needed.len() > self.fine.free_slots() {
                result.stalled = true;
            } else {
                for i in (0..8).filter(|&i| after[i] != before[i]) {
                    // Counted above, so promotion cannot run out of pool; the local cell is inside the block.
                    if self.set_fine(spots[i].0, spots[i].1, after[i], tick).is_err() {
                        result.stalled = true;
                    }
                }
                result.changed = true;
                result.promoted = needed.len() as u32;
            }
        }
        if result.changed || result.pending || result.stalled {
            let mut touched: Vec<IVec3> = Vec::with_capacity(8);
            for (block, _) in spots {
                if self.is_fine(block) && !touched.contains(&block) {
                    touched.push(block);
                }
            }
            woken.extend(touched);
        }
        result
    }
}
