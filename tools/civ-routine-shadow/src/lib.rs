//! Offline routine comparison without authority over world state.

#[path = "../../../crates/civ/src/routine.rs"]
pub mod routine;
#[path = "../../../crates/civ/src/time.rs"]
pub mod time;

pub mod candidate;

use routine::{Phase, Routine};
use time::Tick;

/// Policy inputs captured at one service boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Enter(Phase),
    AdvanceNeeds,
    BeginCycle { id: u32 },
    RequestMeal { hungry: bool, threshold: Tick },
    MealCommitted,
    Observe { present: bool, cap: Tick },
    Interrupt,
    Rested,
    Socialized { id: u32, partner: u32 },
}

impl Action {
    pub fn apply(self, state: &mut Routine, now: Tick) {
        match self {
            Self::Enter(phase) => state.enter(phase, now),
            Self::AdvanceNeeds => state.advance_needs(now),
            Self::BeginCycle { id } => state.begin_cycle(id, now),
            Self::RequestMeal { hungry, threshold } => state.request_meal(now, hungry, threshold),
            Self::MealCommitted => state.meal_committed(now),
            Self::Observe { present, cap } => state.observe(now, present, cap),
            Self::Interrupt => state.interrupt(now),
            Self::Rested => state.rested(now),
            Self::Socialized { id, partner } => state.socialized(id, partner, now),
        }
    }
}

/// First mismatch in the shared routine fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    pub sample: u64,
    pub now: Tick,
    pub action: Action,
    pub expected: Routine,
    pub observed: Routine,
}

/// Compares snapshots; never writes a result back to the executor.
#[derive(Clone, Debug, Default)]
pub struct Probe {
    pub checked: u64,
    pub mismatches: u64,
    pub first: Option<Divergence>,
}

impl Probe {
    pub fn observe(
        &mut self,
        before: &Routine,
        now: Tick,
        action: Action,
        after: &Routine,
    ) -> bool {
        let mut expected = before.clone();
        action.apply(&mut expected, now);
        self.checked = self.checked.saturating_add(1);
        if expected == *after {
            return true;
        }
        self.mismatches = self.mismatches.saturating_add(1);
        if self.first.is_none() {
            self.first = Some(Divergence {
                sample: self.checked,
                now,
                action,
                expected,
                observed: after.clone(),
            });
        }
        false
    }
}
