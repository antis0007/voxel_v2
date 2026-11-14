//! Resumable work discovery for derived data, independent of game state.
use std::ops::Range;
use std::time::{Duration, Instant};

/// A monotonic cursor. Completed work is never repeated when a slice expires.
#[derive(Default, Debug)]
pub struct WorkCursor {
    next: usize,
    end: usize,
}

impl WorkCursor {
    pub fn new(units: usize) -> Self {
        Self {
            next: 0,
            end: units,
        }
    }

    pub fn take(&mut self, limit: usize) -> Range<usize> {
        let start = self.next;
        self.next = self.next.saturating_add(limit).min(self.end);
        start..self.next
    }

    pub fn complete(&self) -> bool {
        self.next == self.end
    }
}

/// Cooperative background work: callers check between small atomic units.
/// Render-derived work may use wall time; authoritative simulation need not.
pub struct WorkSlice {
    start: Instant,
    budget: Duration,
}

impl WorkSlice {
    pub fn new(budget: Duration) -> Self {
        Self {
            start: Instant::now(),
            budget,
        }
    }
    pub fn exhausted(&self) -> bool {
        self.start.elapsed() >= self.budget
    }
}
