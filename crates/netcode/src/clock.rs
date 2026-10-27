//! Client estimate of the server tick, and the server's own tick load.
//!
//! The client keeps a tick ahead of the server by a round trip plus a jitter
//! margin, so an input it stamps arrives just before the server needs it. The
//! server time it reads from arrivals is already one downlink behind, so the lead
//! covers both directions. Drift
//! is corrected by running the local clock up to 3% fast or slow instead of
//! jumping, so motion never hitches; only an error past `SNAP_TICKS` jumps.
//! Times are passed in as microseconds.

/// Ticks of error past which the clock jumps instead of slewing.
pub const SNAP_TICKS: f64 = 30.0;
const MAX_SLEW: f64 = 0.03;

pub struct TickClock {
    tick_us: f64,
    /// Estimated server tick at `last_us`, including the lead.
    tick: Option<f64>,
    last_us: u64,
    /// Smallest one-way offset seen recently: local time minus server time.
    offset_us: Option<f64>,
}

impl TickClock {
    pub fn new(ticks_per_second: u32) -> Self {
        Self { tick_us: 1_000_000.0 / f64::from(ticks_per_second.max(1)), tick: None, last_us: 0, offset_us: None }
    }

    /// A message stamped with `server_tick` arrived at `now_us`.
    pub fn observe(&mut self, server_tick: u64, now_us: u64) {
        let offset = now_us as f64 - server_tick as f64 * self.tick_us;
        // The least-delayed arrival is the best estimate; drift upward slowly so a
        // path that gets longer is followed.
        self.offset_us = Some(match self.offset_us {
            Some(o) if offset < o => offset,
            Some(o) => o + (offset - o) * 0.002,
            None => offset,
        });
    }

    /// The tick to stamp inputs with at `now_us`, given round trip and jitter.
    /// `None` until a server tick has been seen.
    pub fn advance(&mut self, now_us: u64, rtt_ms: f32, jitter_ms: f32) -> Option<u64> {
        let offset = self.offset_us?;
        let lead_us = f64::from(rtt_ms) * 1_000.0 + f64::from(jitter_ms) * 2_000.0 + self.tick_us;
        let target = (now_us as f64 - offset + lead_us) / self.tick_us;
        let elapsed = now_us.saturating_sub(self.last_us) as f64 / self.tick_us;
        self.last_us = now_us;
        let next = match self.tick {
            None => target,
            Some(t) => {
                let free = t + elapsed;
                let error = target - free;
                if error.abs() > SNAP_TICKS {
                    target
                } else {
                    // Slew at most 3% of elapsed time toward the target.
                    free + error.clamp(-elapsed * MAX_SLEW, elapsed * MAX_SLEW)
                }
            }
        };
        self.tick = Some(next);
        Some(next.max(0.0) as u64)
    }
}

/// Server tick load for the player list footer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Load {
    pub ticks_per_second: f32,
    pub mean_ms: f32,
    pub worst_ms: f32,
    /// Share of the tick budget used on average, 0..=1 and above when behind.
    pub busy: f32,
}

/// Server side: tick durations over the last `window` ticks.
pub struct TickLoad {
    budget_us: u64,
    durations: std::collections::VecDeque<(u64, u64)>,
    window: usize,
}

impl TickLoad {
    pub fn new(ticks_per_second: u32, window: usize) -> Self {
        Self { budget_us: 1_000_000 / u64::from(ticks_per_second.max(1)), durations: Default::default(), window: window.max(2) }
    }

    /// One tick started at `start_us` and took `took_us`.
    pub fn record(&mut self, start_us: u64, took_us: u64) {
        self.durations.push_back((start_us, took_us));
        while self.durations.len() > self.window {
            self.durations.pop_front();
        }
    }

    pub fn load(&self) -> Load {
        let (Some(first), Some(last)) = (self.durations.front(), self.durations.back()) else { return Load::default() };
        let n = self.durations.len() as f32;
        let span = last.0.saturating_sub(first.0) as f32 / 1_000_000.0;
        let total: u64 = self.durations.iter().map(|d| d.1).sum();
        let mean_ms = total as f32 / n / 1_000.0;
        Load {
            ticks_per_second: if span > 0.0 { (n - 1.0) / span } else { 0.0 },
            mean_ms,
            worst_ms: self.durations.iter().map(|d| d.1).max().unwrap_or(0) as f32 / 1_000.0,
            busy: mean_ms * 1_000.0 / self.budget_us as f32,
        }
    }
}

#[cfg(test)]
mod tests;
