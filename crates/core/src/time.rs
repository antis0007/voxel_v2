/// Session-local completed player steps. Zero is the initial state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickId(pub u64);

pub const PLAYER_STEP_SECONDS: f32 = 1.0 / 120.0;
/// The experimental cellular backend completes one tick after two player steps.
pub const PLAYER_STEPS_PER_CA: u64 = 2;

impl TickId {
    pub fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
    pub fn cellular_tick(self) -> Option<u64> {
        (self.0 != 0 && self.0.is_multiple_of(PLAYER_STEPS_PER_CA))
            .then_some(self.0 / PLAYER_STEPS_PER_CA)
    }
}

/// How long a simulation pass may keep working.
///
/// The windowed game bounds every pass by wall-clock so frames keep pacing;
/// headless runs and tests use [`Budget::UNBOUNDED`] so the same inputs always
/// do the same work. A budget derived from an unbounded parent stays unbounded.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    deadline: Option<std::time::Instant>,
}

impl Budget {
    pub const UNBOUNDED: Budget = Budget { deadline: None };

    /// A wall-clock budget of `d` from now.
    pub fn wall(d: std::time::Duration) -> Budget {
        Budget {
            deadline: Some(std::time::Instant::now() + d),
        }
    }

    /// A sub-budget of `d` from now, or unbounded when `self` is.
    pub fn child(self, d: std::time::Duration) -> Budget {
        match self.deadline {
            Some(_) => Budget::wall(d),
            None => Budget::UNBOUNDED,
        }
    }

    /// The earlier of two deadlines.
    pub fn min(self, other: Budget) -> Budget {
        Budget {
            deadline: match (self.deadline, other.deadline) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            },
        }
    }

    pub fn has_time(self) -> bool {
        self.deadline.is_none_or(|d| std::time::Instant::now() < d)
    }
}
