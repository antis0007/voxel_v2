//! Daily routine policy: work, meal, rest, social.
//!
//! Integer port of the app's `jobs/routine` policy and its constants. The
//! kernel supplies the facts the app reads from the world (hungry, arrived,
//! partner present); every transition here is pure.

use crate::time::{PULSE, Tick};

/// Dwell at home that completes a rest.
pub const REST_TICKS: Tick = 16 * PULSE;
/// Shared dwell that completes a visit.
pub const SOCIAL_TICKS: Tick = 12 * PULSE;
/// Work interval before the next cycle may start.
pub const WORK_TICKS: Tick = 240 * PULSE;
/// Longest stay in rest or social, and longest meeting age.
pub const TIMEOUT: Tick = 960 * PULSE;
/// Fatigue that lets a due, hungry worker leave work for a meal.
pub const MEAL_FATIGUE: Tick = 60 * PULSE;
/// Idle wake interval for a resident with a home.
pub const IDLE_WAKE: Tick = 600;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Phase {
    #[default]
    Work = 0,
    Meal = 1,
    Rest = 2,
    Social = 3,
}

impl Phase {
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Work),
            1 => Some(Self::Meal),
            2 => Some(Self::Rest),
            3 => Some(Self::Social),
            _ => None,
        }
    }
}

/// One resident's routine state. Zero `due` and `needs_at` mean "never serviced".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Routine {
    pub phase: Phase,
    pub due: Tick,
    pub fatigue: Tick,
    pub social_need: Tick,
    pub needs_at: Tick,
    pub since: Tick,
    pub last: Tick,
    pub dwell: Tick,
    pub rested_at: Tick,
    pub social_at: Tick,
    pub social_partner: Option<u32>,
    pub rests: u32,
    pub socials: u32,
    pub interrupted: u32,
}

impl Routine {
    pub fn enter(&mut self, phase: Phase, now: Tick) {
        self.phase = phase;
        self.since = now;
        self.last = now;
        self.dwell = 0;
    }

    /// Needs grow with the clock and are materialised only when read.
    pub fn advance_needs(&mut self, now: Tick) {
        let elapsed = if self.needs_at == 0 { PULSE } else { now.saturating_sub(self.needs_at) };
        self.fatigue = self.fatigue.saturating_add(elapsed);
        self.social_need = self.social_need.saturating_add(elapsed);
        self.needs_at = now;
    }

    /// First service staggers the first cycle by resident id.
    pub fn begin_cycle(&mut self, id: u32, now: Tick) {
        if self.due == 0 {
            self.due = now.saturating_add(60 * PULSE + Tick::from(id % 24) * 8 * PULSE);
            self.since = now;
        }
    }

    pub fn request_meal(&mut self, now: Tick, hungry: bool, threshold: Tick) {
        if self.phase == Phase::Work && now >= self.due && self.fatigue >= threshold && hungry {
            self.enter(Phase::Meal, now);
        }
    }

    /// A meal eaten during work changes no phase.
    pub fn meal_committed(&mut self, now: Tick) {
        if self.phase == Phase::Meal {
            self.enter(Phase::Rest, now);
        }
    }

    /// Adds at most `cap` of elapsed time while present; absence clears the dwell.
    pub fn observe(&mut self, now: Tick, present: bool, cap: Tick) {
        self.dwell = if present { self.dwell.saturating_add(now.saturating_sub(self.last).min(cap)) } else { 0 };
        self.last = now;
    }

    pub fn interrupt(&mut self, now: Tick) {
        if self.phase != Phase::Work {
            self.interrupted = self.interrupted.saturating_add(1);
        }
        self.enter(Phase::Work, now);
        self.due = now.saturating_add(WORK_TICKS);
    }

    pub fn rested(&mut self, now: Tick) {
        self.rests = self.rests.saturating_add(1);
        self.fatigue = 0;
        self.rested_at = now;
        self.enter(Phase::Social, now);
    }

    pub fn socialized(&mut self, id: u32, partner: u32, now: Tick) {
        self.socials = self.socials.saturating_add(1);
        self.social_need = 0;
        self.social_partner = Some(partner);
        self.social_at = now;
        self.enter(Phase::Work, now);
        self.due = now.saturating_add(WORK_TICKS + Tick::from(id % 8) * PULSE);
    }
}

/// A reserved visit between two residents, by person index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meeting {
    pub ids: [u32; 2],
    pub started: Tick,
    pub last: Tick,
    pub dwell: Tick,
    pub done: [bool; 2],
    /// Both were present at the previous observation.
    pub together: bool,
}

impl Meeting {
    pub const fn new(ids: [u32; 2], now: Tick) -> Self {
        Self { ids, started: now, last: now, dwell: 0, done: [false; 2], together: false }
    }

    /// Frozen once `required` is reached. `exact` counts only time both were
    /// present at both ends of the interval; otherwise the app's capped rule.
    pub fn observe(&mut self, now: Tick, present: bool, cap: Tick, required: Tick, exact: bool) {
        if self.dwell >= required { return; }
        let elapsed = now.saturating_sub(self.last);
        self.dwell = match (present, exact) {
            (false, _) => 0,
            (true, false) => self.dwell.saturating_add(elapsed.min(cap)),
            (true, true) => self.dwell.saturating_add(if self.together { elapsed } else { 0 }),
        };
        self.together = present;
        self.last = now;
    }

    pub fn live(&self, now: Tick) -> bool {
        !(self.done[0] && self.done[1]) && now.saturating_sub(self.started) <= TIMEOUT
    }
}

/// The executor's wake rule: one pulse when busy, else the earliest future need.
pub fn next_wake(now: Tick, busy: bool, has_home: bool, retry_at: Tick, routine_due: Tick, next_meal: Tick) -> Tick {
    let earliest = now.saturating_add(PULSE);
    if busy { return earliest; }
    let mut due = retry_at.max(now.saturating_add(if has_home { IDLE_WAKE } else { PULSE }));
    if routine_due > now { due = due.min(routine_due.max(earliest)); }
    if next_meal > now { due = due.min(next_meal.max(earliest)); }
    due
}

#[cfg(test)]
mod tests {
    use super::*;

    // The first four mirror the app's policy tests one for one.
    #[test]
    fn due_meal_request_does_not_require_an_idle_job() {
        let mut r = Routine { due: 10, fatigue: 100, ..Routine::default() };
        r.request_meal(20, false, 60);
        assert_eq!(r.phase, Phase::Work);
        r.request_meal(20, true, 60);
        assert_eq!((r.phase, r.rests, r.dwell), (Phase::Meal, 0, 0));
    }

    #[test]
    fn only_a_committed_requested_meal_starts_rest() {
        let mut r = Routine::default();
        r.meal_committed(10);
        assert_eq!(r.phase, Phase::Work);
        r.enter(Phase::Meal, 20);
        r.meal_committed(30);
        assert_eq!((r.phase, r.since, r.rests), (Phase::Rest, 30, 0));
    }

    #[test]
    fn empty_destination_never_satisfies_rest() {
        let mut r = Routine::default();
        r.enter(Phase::Rest, 10);
        r.observe(1000, false, 30);
        assert_eq!(r.dwell, 0);
        r.observe(2000, true, 30);
        assert_eq!(r.dwell, 30);
        r.observe(2010, false, 30);
        assert_eq!(r.dwell, 0);
    }

    #[test]
    fn paired_observations_do_not_double_elapsed_time() {
        let mut m = Meeting::new([1, 2], 0);
        m.observe(15, true, 30, 120, false);
        m.observe(30, true, 30, 120, false);
        assert_eq!(m.dwell, 30);
        m.observe(10_000, false, 30, 120, false);
        assert_eq!(m.dwell, 0);
    }

    #[test]
    fn exact_meeting_counts_only_shared_time() {
        let mut m = Meeting::new([1, 2], 0);
        m.observe(500, true, PULSE, SOCIAL_TICKS, true);
        assert_eq!(m.dwell, 0, "the wait before the partner arrived does not count");
        m.observe(500 + SOCIAL_TICKS, true, PULSE, SOCIAL_TICKS, true);
        assert_eq!(m.dwell, SOCIAL_TICKS);
        m.observe(9_999, false, PULSE, SOCIAL_TICKS, true);
        assert_eq!(m.dwell, SOCIAL_TICKS, "a finished meeting is frozen");
    }

    #[test]
    fn needs_use_a_pulse_on_first_service_then_elapsed_clock() {
        let mut r = Routine::default();
        r.advance_needs(5_000);
        assert_eq!((r.fatigue, r.needs_at), (PULSE, 5_000));
        r.advance_needs(5_700);
        assert_eq!(r.fatigue, PULSE + 700);
        r.fatigue = u64::MAX - 1;
        r.advance_needs(9_000);
        assert_eq!(r.fatigue, u64::MAX);
    }

    #[test]
    fn cycle_constants_match_the_executor() {
        let mut r = Routine::default();
        r.begin_cycle(25, 100);
        assert_eq!(r.due, 100 + 1_800 + 240);
        r.interrupt(1_000);
        assert_eq!((r.due, r.interrupted), (1_000 + 7_200, 0));
        r.enter(Phase::Rest, 2_000);
        r.rested(2_480);
        assert_eq!((r.phase, r.rests, r.fatigue), (Phase::Social, 1, 0));
        r.socialized(13, 14, 3_000);
        assert_eq!((r.phase, r.due, r.social_partner), (Phase::Work, 3_000 + 7_200 + 150, Some(14)));
    }

    #[test]
    fn next_wake_matches_the_executor_rule() {
        assert_eq!(next_wake(100, true, true, 0, 0, 0), 130);
        assert_eq!(next_wake(100, false, true, 0, 0, 0), 700);
        assert_eq!(next_wake(100, false, false, 0, 0, 0), 130);
        assert_eq!(next_wake(100, false, true, 900, 0, 0), 900, "retry only delays");
        assert_eq!(next_wake(100, false, true, 0, 400, 0), 400);
        assert_eq!(next_wake(100, false, true, 0, 110, 0), 130, "never before one pulse");
        assert_eq!(next_wake(100, false, true, 0, 400, 250), 250);
        assert_eq!(next_wake(100, false, true, 0, 0, 50), 700, "a past meal time does not pull the wake");
    }
}
