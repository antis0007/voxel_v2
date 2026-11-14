//! The coordinated tick: the block kernel on uniform matter, then the fine pass inside `R_fine`.
//!
//! A 1 m block whose eight cells include a fine block is skipped by the block kernel and stepped at
//! 0.25 m instead, where its uniform cells read as 64 identical fine cells. Fine blocks sleep by the
//! brick rule and, once uniform and quiet for the hysteresis, demote back to one cell.
use super::MatterStore;
use super::fine_kernel::fine_anchors;
use crate::kernel::cell_pos;
use crate::pinned::{PHASE_OFFSETS, is_heat_tick};
use crate::{BRICK_SIZE, FineJobs, FineRegion, StepStats, step_skipping};
use engine_core::Budget;
use engine_core::fast_hash::{FastMap, FastSet};
use engine_core::reaction::SimRules;
use glam::IVec3;

/// Fine blocks one tick steps at most; the rest wait, awake, for the next tick.
pub const MAX_FINE_AWAKE: usize = 2000;
/// Sleeping fine blocks tested for demotion per tick.
pub const DORMANT_CHECKS_PER_TICK: usize = 256;
/// Anchors between two looks at the clock.
const BUDGET_STRIDE: usize = 32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FineStepStats {
    /// Fine blocks awake when the tick began.
    pub awake: u32,
    /// Awake but outside the region: left exactly as they are.
    pub frozen: u32,
    /// Awake and inside, beyond `MAX_FINE_AWAKE`: they wait for the next tick.
    pub deferred: u32,
    pub blocks: u32,
    pub changed_blocks: u32,
    pub stalled_blocks: u32,
    pub promoted: u32,
    pub demoted: u32,
    /// The budget ran out first; every block the pass was stepping stays awake.
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatterStepStats {
    pub blocks: StepStats,
    pub fine: FineStepStats,
}

fn canonical(blocks: &mut Vec<IVec3>) {
    blocks.sort_unstable_by_key(|b| (b.z, b.y, b.x));
    blocks.dedup();
}

fn fine_bit(masks: &FastMap<IVec3, [u64; 8]>, block: IVec3) -> bool {
    let (brick, word, bit) = MatterStore::address(block);
    masks.get(&brick).is_some_and(|m| m[word] & bit != 0)
}

impl MatterStore {
    pub fn active_bricks(&self) -> usize {
        self.blocks.active_bricks()
    }

    /// Distinct fine blocks the next tick steps.
    pub fn fine_awake(&self) -> usize {
        let mut awake = self.awake.clone();
        canonical(&mut awake);
        awake.iter().filter(|b| self.is_fine(**b)).count()
    }

    /// Step a fine block next tick; a block that is not fine is ignored.
    pub fn wake_fine(&mut self, block: IVec3) {
        if self.is_fine(block) {
            self.awake.push(block);
        }
    }

    /// The brick hash folded with every fine block's cells in `(z, y, x)` order.
    pub fn state_hash(&self) -> u64 {
        let mut hash = self.blocks.state_hash();
        let mut feed = |bytes: &[u8]| {
            for &b in bytes {
                hash = (hash ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        let mut blocks: Vec<IVec3> = self.fine.blocks().collect();
        canonical(&mut blocks);
        for block in blocks {
            for axis in [block.x, block.y, block.z] {
                feed(&axis.to_le_bytes());
            }
            if let Some(cells) = self.fine.cells(block) {
                feed(bytemuck::cast_slice(&cells[..]));
            }
        }
        hash
    }

    /// One tick: the block kernel, then the fine pass over awake fine blocks inside `region` under
    /// `fine_budget`. `Budget::UNBOUNDED` never cuts the pass, so headless runs stay deterministic.
    pub fn step(
        &mut self,
        rules: &SimRules,
        seed: u32,
        tick: u32,
        region: FineRegion,
        fine_budget: Budget,
    ) -> MatterStepStats {
        let blocks = self.step_blocks(rules, seed, tick);
        let fine = self.step_fine(rules, seed, tick, region, fine_budget);
        MatterStepStats { blocks, fine }
    }

    fn step_blocks(&mut self, rules: &SimRules, seed: u32, tick: u32) -> StepStats {
        let masks = &self.masks;
        if masks.is_empty() {
            return step_skipping(&mut self.blocks, rules, seed, tick, |_| false);
        }
        // Each stencil reaches only toward increasing coordinates. Its anchor
        // can be in the fine block's brick or one brick lower on each axis.
        let mut halo: FastSet<IVec3> = FastSet::default();
        for &brick in masks.keys() {
            for dy in -1..=0 {
                for dz in -1..=0 {
                    for dx in -1..=0 {
                        halo.insert(brick + IVec3::new(dx, dy, dz));
                    }
                }
            }
        }
        let skip = |anchor: IVec3| {
            halo.contains(&anchor.div_euclid(IVec3::splat(BRICK_SIZE)))
                && (0..8).any(|i| fine_bit(masks, cell_pos(anchor, i)))
        };
        step_skipping(&mut self.blocks, rules, seed, tick, skip)
    }

    fn step_fine(
        &mut self,
        rules: &SimRules,
        seed: u32,
        tick: u32,
        region: FineRegion,
        budget: Budget,
    ) -> FineStepStats {
        let mut stats = FineStepStats::default();
        let mut awake = std::mem::take(&mut self.awake);
        canonical(&mut awake);
        awake.retain(|b| self.is_fine(*b));
        stats.awake = awake.len() as u32;
        let (mut first, frozen): (Vec<IVec3>, Vec<IVec3>) = awake.into_iter().partition(|b| region.contains(*b));
        let deferred = first.split_off(first.len().min(MAX_FINE_AWAKE));
        stats.frozen = frozen.len() as u32;
        stats.deferred = deferred.len() as u32;
        let mut woken = Vec::new();
        let even = tick.wrapping_mul(2);
        self.run_fine_phase(rules, seed, even, tick, &first, budget, &mut woken, &mut stats);
        // The second phase also steps whatever the first one woke.
        let mut both = first.clone();
        both.extend(woken.iter().copied());
        canonical(&mut both);
        self.run_fine_phase(rules, seed, even.wrapping_add(1), tick, &both, budget, &mut woken, &mut stats);
        let mut next = woken;
        next.extend(frozen);
        next.extend(deferred);
        // Blocks promoted during the phases pushed themselves here.
        next.append(&mut self.awake);
        if stats.truncated {
            next.extend(both.iter().copied());
        }
        // Heat conducts only on heat ticks, so only a heat tick may put a block to sleep.
        let can_sleep = is_heat_tick(tick);
        if !can_sleep {
            next.extend(first.iter().copied());
        }
        canonical(&mut next);
        if can_sleep {
            for block in first {
                if next.binary_search_by_key(&(block.z, block.y, block.x), |n| (n.z, n.y, n.x)).is_err() {
                    self.dormant.push_back(block);
                }
            }
        }
        self.awake = next;
        stats.demoted = self.demote_dormant(tick);
        stats
    }

    #[allow(clippy::too_many_arguments)]
    fn run_fine_phase(
        &mut self,
        rules: &SimRules,
        seed: u32,
        phase_counter: u32,
        tick: u32,
        blocks: &[IVec3],
        budget: Budget,
        woken: &mut Vec<IVec3>,
        stats: &mut FineStepStats,
    ) {
        let anchors = fine_anchors(blocks, PHASE_OFFSETS[(phase_counter & 1) as usize]);
        for (i, anchor) in anchors.into_iter().enumerate() {
            if i % BUDGET_STRIDE == 0 && !budget.has_time() {
                stats.truncated = true;
                return;
            }
            let out = self.run_fine_block(rules, seed, phase_counter, tick, anchor, woken);
            stats.blocks += 1;
            stats.changed_blocks += u32::from(out.changed);
            stats.stalled_blocks += u32::from(out.stalled);
            stats.promoted += out.promoted;
        }
    }

    /// Sleeping fine blocks that are uniform return to one cell once the hysteresis has passed.
    /// A heterogeneous one leaves the queue; it comes back the next time it sleeps.
    fn demote_dormant(&mut self, tick: u32) -> u32 {
        let mut demoted = 0;
        for _ in 0..self.dormant.len().min(DORMANT_CHECKS_PER_TICK) {
            let Some(block) = self.dormant.pop_front() else { break };
            if !self.is_fine(block) {
                continue;
            }
            let uniform = self.fine.cells(block).is_some_and(|c| c.iter().all(|x| *x == c[0]));
            match self.demote_uniform(block, tick, FineJobs::HYSTERESIS) {
                Ok(true) => demoted += 1,
                Ok(false) if uniform => self.dormant.push_back(block),
                Ok(false) => {}
                // The block pool is full: the fine cells stay authoritative until it has room.
                Err(_) => self.dormant.push_back(block),
            }
        }
        demoted
    }
}
