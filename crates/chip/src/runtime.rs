//! An installed controller: a few words of state driven by its revision's
//! frozen `Model`. It does work only when it is due (a timer wake or a
//! pending output); between wakes the owner skips it entirely, and the idle
//! energy of the gap is charged at the next service.
use crate::characterize::{Model, TICK_HZ};
use crate::firmware::{OPEN, decide};
use serde::{Deserialize, Serialize};

pub const MAX_CHANNELS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// No supply: outputs are held off (the safe state).
    Unpowered,
    /// Power-on reset after supply returned; outputs off until it ends.
    Booting,
    Running,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub phase: Phase,
    pub output: bool,
    /// Permille thresholds the player set on this installed part.
    pub lower: u16,
    pub upper: u16,
    pub readings: [u16; MAX_CHANNELS],
    pub next_wake: u64,
    pub pending: Option<(u64, bool)>,
    /// Energy drawn but not yet paid to the machine's supply, nJ.
    pub debt_nj: u64,
    accounted: u64,
    pub wakes: u64,
}

impl State {
    pub fn new(now: u64) -> Self {
        Self {
            phase: Phase::Unpowered,
            output: false,
            lower: 300,
            upper: 900,
            readings: [OPEN; MAX_CHANNELS],
            next_wake: now,
            pending: None,
            debt_nj: 0,
            accounted: now,
            wakes: 0,
        }
    }

    /// Next tick at which `service` has anything to do.
    pub fn due(&self) -> u64 {
        self.pending.map_or(self.next_wake, |(t, _)| t.min(self.next_wake))
    }

    /// Advance to `now`. `powered` says whether the supply has charge;
    /// `read` returns a level in permille, or `OPEN`, per channel.
    pub fn service(&mut self, model: &Model, now: u64, powered: bool, mut read: impl FnMut(usize) -> u16) {
        let elapsed = now.saturating_sub(self.accounted);
        self.accounted = now;
        if self.phase != Phase::Unpowered {
            self.debt_nj += model.idle_nw * elapsed / u64::from(TICK_HZ);
        }
        if !powered {
            self.phase = Phase::Unpowered;
            self.output = false;
            self.pending = None;
            // Check again on the next wake period; supply return is noticed then.
            self.next_wake = now + u64::from(model.wake_ticks);
            return;
        }
        if self.phase == Phase::Unpowered {
            self.phase = Phase::Booting;
            self.output = false;
            self.next_wake = now + u64::from(model.boot_ticks);
            return;
        }
        if let Some((t, value)) = self.pending
            && now >= t
        {
            self.output = value;
            self.pending = None;
        }
        if now < self.next_wake {
            return;
        }
        self.phase = Phase::Running;
        let n = usize::from(model.channels).min(MAX_CHANNELS);
        for (i, r) in self.readings.iter_mut().enumerate() {
            *r = if i < n { read(i) } else { OPEN };
        }
        let prev = self.pending.map_or(self.output, |(_, v)| v);
        let next = decide(model.task, prev, &self.readings[..n.max(1)], self.lower, self.upper);
        self.pending = Some((now + u64::from(model.response_ticks), next));
        self.debt_nj += model.wake_nj;
        self.wakes += 1;
        // A wake missed while unpowered or booting is not replayed.
        let next = self.next_wake + u64::from(model.wake_ticks);
        self.next_wake = if next <= now { now + u64::from(model.wake_ticks) } else { next };
    }

    /// Nothing to read yet (the owner's networks are being rebuilt): apply a
    /// due output and wait until `now + 1` without deciding.
    pub fn defer(&mut self, now: u64) {
        if let Some((t, value)) = self.pending
            && t <= now
        {
            self.output = value;
            self.pending = None;
        }
        self.next_wake = self.next_wake.max(now + 1);
    }

    /// Pay whole supply units of `unit_nj` from `available`; returns units taken.
    pub fn pay(&mut self, unit_nj: u64, available: u32) -> u32 {
        let owed = self.debt_nj / unit_nj.max(1);
        let paid = owed.min(u64::from(available)) as u32;
        self.debt_nj -= u64::from(paid) * unit_nj;
        paid
    }
}
