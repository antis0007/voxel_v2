//! Bounded background representation changes; never queues player edits.
use crate::{MatterError, MatterStore};
use engine_core::{Budget, fast_hash::FastMap};
use glam::IVec3;
use std::{collections::VecDeque, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FineJob {
    Promote,
    Demote,
}

/// A player or hazard focus in 1 m block coordinates. Change radius to 16 or
/// 12 under load; radius reduction never destroys heterogeneous fine matter.
#[derive(Clone, Copy, Debug)]
pub struct FineRegion {
    pub center: IVec3,
    pub radius: u32,
}

impl FineRegion {
    pub fn around(center: IVec3) -> Self {
        Self { center, radius: 24 }
    }

    pub(crate) fn contains(self, block: IVec3) -> bool {
        // Widen before subtracting or squaring, including at i32 world limits.
        let x = i128::from(block.x) - i128::from(self.center.x);
        let y = i128::from(block.y) - i128::from(self.center.y);
        let z = i128::from(block.z) - i128::from(self.center.z);
        x * x + y * y + z * z <= i128::from(self.radius).pow(2)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FineJobStats {
    pub attempted: usize,
    pub promoted: usize,
    pub demoted: usize,
    pub outside_radius: usize,
    pub pool_blocked: usize,
    pub pending: usize,
    pub already_ran: bool,
}

/// FIFO with one entry per block, bounded admission and latest requested intent.
/// Pass the same store for the queue's lifetime. Callers explicitly submit
/// disturbed blocks; no resident-world scan is performed. Successful promotions
/// become demotion maintenance jobs until uniform and at least 60 ticks quiet.
/// Direct `MatterStore` fine edits should also submit a `Demote` maintenance job.
/// Queue rejection is explicit: retain/retry the request; never drop a player edit.
pub struct FineJobs {
    capacity: usize,
    queue: VecDeque<IVec3>,
    intents: FastMap<IVec3, FineJob>,
    last_tick: Option<u32>,
}

impl FineJobs {
    pub const MAX_ATTEMPTS: usize = 2000;
    pub const HYSTERESIS: u32 = 60;
    pub const WALL_SLICE: Duration = Duration::from_micros(500);

    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            queue: VecDeque::new(),
            intents: FastMap::default(),
            last_tick: None,
        }
    }

    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Returns false only when a new block cannot be admitted. Updating an
    /// existing intent succeeds even at capacity and keeps its FIFO position.
    pub fn request(&mut self, block: IVec3, job: FineJob) -> bool {
        if let Some(intent) = self.intents.get_mut(&block) {
            *intent = job;
            return true;
        }
        if self.queue.len() >= self.capacity {
            return false;
        }
        self.intents.insert(block, job);
        self.queue.push_back(block);
        true
    }

    /// At most one pass per tick, 2000 attempts and one visit per initial queue
    /// entry. Expensive failed/deferred attempts count too. The wall slice is
    /// clipped to the parent's deadline; UNBOUNDED keeps deterministic headless
    /// operation, still capped by attempt count. One job is non-preemptible.
    /// Promotions outside the focus wait. Uniform demotions may run anywhere;
    /// neither radius nor pool pressure permits averaged material conversion.
    pub fn run(
        &mut self,
        store: &mut MatterStore,
        tick: u32,
        region: FineRegion,
        parent: Budget,
    ) -> FineJobStats {
        let mut stats = FineJobStats::default();
        if self.last_tick == Some(tick) {
            stats.already_ran = true;
            stats.pending = self.pending();
            return stats;
        }
        self.last_tick = Some(tick);
        let budget = parent.min(parent.child(Self::WALL_SLICE));
        let attempts = self.pending().min(Self::MAX_ATTEMPTS);
        for _ in 0..attempts {
            if !budget.has_time() {
                break;
            }
            let block = self
                .queue
                .pop_front()
                .expect("bounded by initial queue length");
            let intent = self.intents[&block];
            stats.attempted += 1;
            let keep = match intent {
                FineJob::Promote => {
                    if !region.contains(block) {
                        stats.outside_radius += 1;
                    } else {
                        let was_fine = store.is_fine(block);
                        match store.promote(block, tick) {
                            Ok(()) => {
                                stats.promoted += usize::from(!was_fine);
                                self.intents.insert(block, FineJob::Demote);
                            }
                            Err(MatterError::PoolFull) => stats.pool_blocked += 1,
                            Err(MatterError::FineCellOutsideBlock) => {
                                unreachable!("promotion has no local cell")
                            }
                        }
                    }
                    true
                }
                FineJob::Demote => match store.demote_uniform(block, tick, Self::HYSTERESIS) {
                    Ok(true) => {
                        stats.demoted += 1;
                        false
                    }
                    Ok(false) => store.is_fine(block),
                    Err(MatterError::PoolFull) => {
                        stats.pool_blocked += 1;
                        true
                    }
                    Err(MatterError::FineCellOutsideBlock) => {
                        unreachable!("demotion has no local cell")
                    }
                },
            };
            if keep {
                self.queue.push_back(block);
            } else {
                self.intents.remove(&block);
            }
        }
        stats.pending = self.pending();
        stats
    }
}
