//! Desync detection: peers report a region digest every `interval` ticks and the host compares
//! each report with its own digest of the same tick. Reports may arrive before the host reaches
//! that tick (a client can step up to `input_delay` ticks ahead) or long after; both are bounded.
use super::PeerId;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Match,
    Mismatch,
    /// The host no longer remembers that tick.
    Expired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub peer: PeerId,
    pub tick: u64,
    pub outcome: Outcome,
}

/// Host side. Memory is `history` host digests plus `history` waiting reports per peer at most.
pub struct DesyncMonitor {
    interval: u64,
    history: usize,
    host: VecDeque<(u64, u64)>,
    waiting: VecDeque<(PeerId, u64, u64)>,
    waiting_cap: usize,
}

impl DesyncMonitor {
    /// Checkpoints every `interval` ticks (at least 1); the last `history` are kept.
    pub fn new(interval: u64, history: usize, max_peers: usize) -> Self {
        let history = history.max(1);
        Self {
            interval: interval.max(1),
            history,
            host: VecDeque::new(),
            waiting: VecDeque::new(),
            waiting_cap: history * max_peers.max(1),
        }
    }

    /// Whether the digest after stepping `tick` is compared.
    pub fn is_checkpoint(&self, tick: u64) -> bool {
        tick.is_multiple_of(self.interval)
    }

    /// The host's own digest after stepping `tick`; returns verdicts for reports that waited for it.
    pub fn record_host(&mut self, tick: u64, digest: u64) -> Vec<Verdict> {
        self.host.push_back((tick, digest));
        if self.host.len() > self.history {
            self.host.pop_front();
        }
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.waiting.len() {
            let (peer, at, theirs) = self.waiting[i];
            if at == tick {
                out.push(Verdict { peer, tick, outcome: if theirs == digest { Outcome::Match } else { Outcome::Mismatch } });
                self.waiting.remove(i);
            } else {
                i += 1;
            }
        }
        out
    }

    /// A peer's digest after stepping `tick`. `None` while the host has not reached that tick.
    pub fn report(&mut self, peer: PeerId, tick: u64, digest: u64) -> Option<Verdict> {
        if let Some(&(_, ours)) = self.host.iter().find(|(t, _)| *t == tick) {
            let outcome = if ours == digest { Outcome::Match } else { Outcome::Mismatch };
            return Some(Verdict { peer, tick, outcome });
        }
        let newest = self.host.back().map(|(t, _)| *t);
        if newest.is_some_and(|newest| tick < newest) {
            return Some(Verdict { peer, tick, outcome: Outcome::Expired });
        }
        if self.waiting.len() >= self.waiting_cap {
            self.waiting.pop_front();
        }
        self.waiting.push_back((peer, tick, digest));
        None
    }
}
