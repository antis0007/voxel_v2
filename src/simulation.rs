//! Bounded fixed-step simulation. Wall-clock rendering never controls physics step size.
mod cellular;
pub(crate) mod residency;
mod rigid;
pub(crate) mod sandbox;
mod thermal;
mod world;
pub use world::WorldSimulation;
pub(crate) use world::discover;

/// True when a later tick can change the cell at `p`: a fluid, grain, rigid
/// core, thermal cell or sandbox cell. Every other block is inert until an edit
/// touches it, so the scheduler does not wake it. Waking inert neighbours of
/// each edit filled the queue with stone and air that never step.
pub(crate) fn acts(world: &crate::world::World, p: glam::IVec3) -> bool {
    use crate::world::BlockId as B;
    let block = world.get(p);
    matches!(block, B::Water | B::Sand | B::Gravel | B::Ash | B::RigidCore)
        || thermal::active(block)
        || world.sandbox.cells.contains_key(&sandbox::key(p))
}
pub const STEP_SECONDS: f32 = 1.0 / 120.0;
const MAX_STEPS: u32 = 16;

#[derive(Default)]
pub struct SimulationClock {
    accumulated: f64,
}

impl SimulationClock {
    pub fn advance(&mut self, elapsed: f32, running: bool) -> u32 {
        if !running || !elapsed.is_finite() || elapsed < 0.0 {
            self.accumulated = 0.0;
            return 0;
        }
        let step = STEP_SECONDS as f64;
        self.accumulated += (elapsed as f64).min(step * MAX_STEPS as f64);
        let steps = ((self.accumulated / step).floor() as u32).min(MAX_STEPS);
        self.accumulated -= steps as f64 * step;
        steps
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
        Budget { deadline: Some(std::time::Instant::now() + d) }
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
