//! Wake selection from supplied facts, without world access.
use crate::{
    routine::Phase,
    time::{PULSE, Tick},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub present: bool,
    pub activity_end: Option<Tick>,
    pub navigation_wait: bool,
    pub has_path: bool,
    pub motion_at: Tick,
    pub body_needs_tick: bool,
    pub has_task: bool,
    pub carrying: bool,
    pub batch: bool,
    pub mission: bool,
    pub phase: Phase,
    pub has_homes: bool,
    pub retry_at: Tick,
    pub routine_due: Tick,
    pub next_meal: Option<Tick>,
}

pub fn next_wake(now: Tick, facts: Facts) -> Tick {
    let earliest = now.saturating_add(PULSE);
    if !facts.present {
        return earliest;
    }
    if let Some(end) = facts.activity_end {
        return end.max(now.saturating_add(1));
    }
    if facts.navigation_wait {
        return now.saturating_add(600);
    }
    if facts.has_path && facts.motion_at.saturating_add(PULSE) < now {
        return now.saturating_add(1);
    }
    if facts.body_needs_tick
        || facts.has_task
        || facts.has_path
        || facts.carrying
        || facts.batch
        || facts.mission
        || facts.phase != Phase::Work
    {
        return earliest;
    }
    let mut due = if facts.retry_at > now {
        facts.retry_at.max(earliest)
    } else {
        now.saturating_add(if facts.has_homes { 6000 } else { PULSE })
    };
    if facts.routine_due > now {
        due = due.min(facts.routine_due.max(earliest));
    }
    if let Some(meal) = facts.next_meal.filter(|&meal| meal > now) {
        due = due.min(meal.max(earliest));
    }
    due
}
