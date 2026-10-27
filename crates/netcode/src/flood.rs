//! Per-connection flood limits for chat, commands, edits and raw packets.
//!
//! Each event adds debt that drains at the allowed rate. Past `burst` events of
//! debt the event is dropped; past `kick` the connection is closed. Dropped events
//! still add debt, so a client that keeps flooding reaches the kick line instead of
//! being throttled forever.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    pub per_second: u32,
    /// Events that may arrive at once before dropping starts.
    pub burst: u32,
    /// Events of debt that close the connection; `None` never kicks.
    pub kick: Option<u32>,
}

impl Rate {
    /// One line a second with ten at once, as Minecraft's chat counter allows.
    pub const CHAT: Self = Self { per_second: 1, burst: 10, kick: Some(20) };
    pub const COMMANDS: Self = Self { per_second: 2, burst: 10, kick: Some(30) };
    /// Dig and place actions, not cells; a held button repeats well under this.
    pub const EDITS: Self = Self { per_second: 40, burst: 40, kick: None };
    /// Every datagram from a client; a 120 Hz pose stream plus actions fits easily.
    pub const PACKETS: Self = Self { per_second: 500, burst: 500, kick: Some(2_000) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flood {
    Allow,
    Drop,
    Kick,
}

pub struct Limiter {
    rate: Rate,
    debt_us: u64,
    last_us: u64,
}

impl Limiter {
    pub fn new(rate: Rate) -> Self {
        Self { rate, debt_us: 0, last_us: 0 }
    }

    /// Counts one event at `now_us`; a clock that steps backwards drains nothing.
    pub fn admit(&mut self, now_us: u64) -> Flood {
        let cost = 1_000_000 / u64::from(self.rate.per_second.max(1));
        self.debt_us = self.debt_us.saturating_sub(now_us.saturating_sub(self.last_us)) + cost;
        self.last_us = self.last_us.max(now_us);
        match self.rate.kick {
            Some(kick) if self.debt_us > cost * u64::from(kick) => Flood::Kick,
            _ if self.debt_us > cost * u64::from(self.rate.burst) => Flood::Drop,
            _ => Flood::Allow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burst_passes_then_drops_then_kicks() {
        let mut chat = Limiter::new(Rate::CHAT);
        let results: Vec<Flood> = (0..21).map(|_| chat.admit(0)).collect();
        assert!(results[..10].iter().all(|r| *r == Flood::Allow));
        assert!(results[10..20].iter().all(|r| *r == Flood::Drop));
        assert_eq!(results[20], Flood::Kick);
    }

    #[test]
    fn a_steady_rate_at_the_limit_is_never_dropped() {
        let mut chat = Limiter::new(Rate::CHAT);
        assert!((0..600u64).all(|s| chat.admit(s * 1_000_000) == Flood::Allow));
    }

    #[test]
    fn debt_drains_with_time_and_ignores_a_backward_clock() {
        let mut chat = Limiter::new(Rate::CHAT);
        for _ in 0..10 {
            chat.admit(5_000_000);
        }
        assert_eq!(chat.admit(5_000_000), Flood::Drop);
        assert_eq!(chat.admit(0), Flood::Drop);
        assert_eq!(chat.admit(17_000_000), Flood::Allow);
    }

    #[test]
    fn a_rate_without_a_kick_only_drops() {
        let mut edits = Limiter::new(Rate::EDITS);
        assert!((0..10_000).all(|_| edits.admit(0) != Flood::Kick));
    }
}
