//! Chunk send rate set by how fast the client integrates chunks.
//!
//! The client times each batch it unpacks and meshes, keeps a bounded average of
//! nanoseconds per chunk, and asks for as many chunks per send interval as fit its
//! budget. The server spends a quota that grows by that request each interval and
//! keeps few batches unacknowledged, so a slow client is never buried under a
//! burst on join and a fast one is not held to a fixed rate.

/// Average assumed before the first measurement.
const START_NS_PER_CHUNK: f64 = 2_000_000.0;
/// Weight the average gives its history, so one hitch moves it little.
const HISTORY: u32 = 49;
/// Largest batch the server sends in one interval.
pub const MAX_BATCH: u32 = 64;
const MIN_RATE: f32 = 0.01;

/// Client side: measured integration cost.
pub struct ChunkPace {
    ns_per_chunk: f64,
    samples: u32,
}

impl Default for ChunkPace {
    fn default() -> Self {
        Self { ns_per_chunk: START_NS_PER_CHUNK, samples: 1 }
    }
}

impl ChunkPace {
    /// Adds one batch of `chunks` that took `nanos` to integrate.
    pub fn record(&mut self, chunks: u32, nanos: u64) {
        if chunks == 0 {
            return;
        }
        // A single sample may move the estimate at most threefold either way.
        let sample = (nanos as f64 / f64::from(chunks)).clamp(self.ns_per_chunk / 3.0, self.ns_per_chunk * 3.0);
        let weight = f64::from(self.samples.min(HISTORY));
        self.ns_per_chunk = (self.ns_per_chunk * weight + sample) / (weight + 1.0);
        self.samples = self.samples.saturating_add(1);
    }

    /// Chunks per send interval that fit `budget_ns` of integration time.
    pub fn desired(&self, budget_ns: u64) -> f32 {
        ((budget_ns as f64 / self.ns_per_chunk) as f32).clamp(MIN_RATE, MAX_BATCH as f32)
    }
}

/// Server side: per-player batch sizing.
pub struct ChunkSender {
    quota: f32,
    desired: f32,
    unacked: u32,
    max_unacked: u32,
}

impl Default for ChunkSender {
    fn default() -> Self {
        // One batch in flight until the client has proved it answers.
        Self { quota: 0.0, desired: 9.0, unacked: 0, max_unacked: 1 }
    }
}

impl ChunkSender {
    /// Chunks to send this interval from `queued` waiting, as one batch; 0 sends nothing.
    pub fn interval(&mut self, queued: usize) -> u32 {
        if self.unacked >= self.max_unacked {
            return 0;
        }
        self.quota = (self.quota + self.desired).min(MAX_BATCH as f32);
        let count = (self.quota.floor() as u32).min(u32::try_from(queued).unwrap_or(u32::MAX));
        if count == 0 {
            return 0;
        }
        self.quota -= count as f32;
        self.unacked += 1;
        count
    }

    /// The client finished a batch and asks for `desired` chunks per interval.
    pub fn acknowledge(&mut self, desired: f32) {
        self.unacked = self.unacked.saturating_sub(1);
        self.max_unacked = 10;
        self.desired = if desired.is_finite() { desired.clamp(MIN_RATE, MAX_BATCH as f32) } else { MIN_RATE };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sends `total` chunks to a client that needs `ns` per chunk with a 7 ms budget and
    /// answers each batch two intervals later; returns intervals taken and the largest batch.
    fn stream(ns: u64, total: usize) -> (u32, u32) {
        let (mut pace, mut sender) = (ChunkPace::default(), ChunkSender::default());
        let (mut left, mut intervals, mut largest) = (total, 0, 0);
        let mut in_flight = std::collections::VecDeque::new();
        while left > 0 {
            intervals += 1;
            let n = sender.interval(left);
            left -= n as usize;
            largest = largest.max(n);
            in_flight.push_back((intervals + 2, n));
            while in_flight.front().is_some_and(|&(due, _)| due <= intervals) {
                let Some((_, n)) = in_flight.pop_front() else { break };
                pace.record(n, ns * u64::from(n));
                sender.acknowledge(pace.desired(7_000_000));
            }
        }
        (intervals, largest)
    }

    #[test]
    fn a_fast_client_ramps_to_large_batches() {
        // The bounded average ramps over tens of batches rather than jumping.
        let (intervals, largest) = stream(100_000, 4_000);
        assert!(largest >= 5 * 9, "{largest}");
        assert!(intervals < 150, "{intervals}");
    }

    #[test]
    fn a_slow_client_gets_small_batches() {
        let (_, largest) = stream(5_000_000, 200);
        assert!(largest <= 9, "{largest}");
        let mut pace = ChunkPace::default();
        for _ in 0..100 {
            pace.record(4, 20_000_000);
        }
        assert!(pace.desired(7_000_000) < 2.0);
    }

    #[test]
    fn one_hitch_moves_the_estimate_little() {
        let mut pace = ChunkPace::default();
        for _ in 0..60 {
            pace.record(8, 8 * 500_000);
        }
        let before = pace.desired(7_000_000);
        pace.record(8, 8 * 500_000_000);
        assert!(pace.desired(7_000_000) > before * 0.9);
    }

    #[test]
    fn unacknowledged_batches_stop_sending_and_bad_requests_clamp() {
        let mut sender = ChunkSender::default();
        assert_eq!(sender.interval(100), 9);
        assert_eq!(sender.interval(100), 0);
        sender.acknowledge(f32::NAN);
        assert_eq!(sender.interval(100), 0);
        sender.acknowledge(1.0e9);
        assert_eq!(sender.interval(100), MAX_BATCH);
    }
}
