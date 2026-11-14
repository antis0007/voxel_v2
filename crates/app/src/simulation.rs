//! Bounded fixed-step simulation. Wall-clock rendering never controls physics step size.
mod cellular;
pub(crate) mod climate;
mod environment;
mod springs;
pub(crate) mod heat;
pub(crate) mod fixed;
pub(crate) mod cpu_region;
pub(crate) mod tools;
pub(crate) mod residency;
mod rigid;
pub(crate) mod sandbox;
pub(crate) mod storms;
mod thermal;
mod world;
mod tool_source;
pub(crate) use tool_source::review_backlog;
pub use world::WorldSimulation;
pub(crate) use world::discover;

/// True when a later tick can change the cell at `p`: a fluid, grain, rigid
/// core, edited construction, thermal cell or sandbox cell. Every other block is inert until an edit
/// touches it, so the scheduler does not wake it. Waking inert neighbours of
/// each edit filled the queue with stone and air that never step.
pub(crate) fn acts(world: &crate::world::World, p: glam::IVec3) -> bool {
    use crate::world::BlockId as B;
    let block = world.get(p);
    matches!(block, B::Water | B::Sand | B::Gravel | B::Ash | B::RigidCore)
        || rigid::fragment_material(block)
        || thermal::active(block)
        || world.sandbox.cells.contains_key(&sandbox::key(p))
}
pub const STEP_SECONDS: f32 = engine_core::time::PLAYER_STEP_SECONDS;
const MAX_STEPS: u32 = 16;

#[derive(Default)]
pub struct SimulationClock {
    accumulated: f64,
    deferred: u32,
}

impl SimulationClock {
    pub fn advance(&mut self, elapsed: f32, running: bool) -> u32 {
        if !running || !elapsed.is_finite() || elapsed < 0.0 {
            self.accumulated = 0.0;
            self.deferred = 0;
            return 0;
        }
        // Retry the unfinished batch before accruing more wall time during a stall.
        if self.deferred > 0 { return std::mem::take(&mut self.deferred); }
        let step = STEP_SECONDS as f64;
        self.accumulated += (elapsed as f64).min(step * MAX_STEPS as f64);
        let steps = ((self.accumulated / step).floor() as u32).min(MAX_STEPS);
        self.accumulated -= steps as f64 * step;
        steps
    }

    /// Retain the unfinished batch when authority cannot advance.
    pub fn defer_steps(&mut self, steps: u32) {
        self.deferred = self.deferred.saturating_add(steps).min(MAX_STEPS);
    }

    /// Time banked towards the next step. Presentation-only motion (the orbital
    /// sky) adds it to the day clock so it advances every frame, not in 1/120 s hops.
    pub fn lead_seconds(&self) -> f32 {
        self.accumulated as f32
    }

    /// Presentation seconds for one frame: the steps taken plus the change in
    /// banked lead. Tracks frame time during ordinary running and freezes when
    /// authority waits; deferred steps contribute only when they complete.
    pub fn presented_seconds(&self, lead_before: f32, steps: u32) -> f32 {
        (steps as f32 * STEP_SECONDS + self.lead_seconds() - lead_before).max(0.0)
    }
}

pub use engine_core::Budget;

#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod clock_tests {
    use super::*;

    #[test]
    fn steps_plus_lead_track_elapsed_time_every_frame() {
        // 144 Hz frames against a 120 Hz step: raw steps alternate 0 and 1, but
        // steps * STEP + lead (what the orbital sky draws) grows evenly.
        let mut clock = SimulationClock::default();
        let (frame, mut steps_total, mut previous) = (1.0_f32 / 144.0, 0_u32, 0.0_f32);
        for i in 1..=288 {
            steps_total += clock.advance(frame, true);
            let drawn = steps_total as f32 * STEP_SECONDS + clock.lead_seconds();
            assert!((drawn - i as f32 * frame).abs() < 1e-4, "frame {i}: {drawn}");
            assert!(drawn - previous > frame * 0.99, "frame {i} stalled");
            previous = drawn;
        }
    }

    #[test]
    fn presented_time_is_the_frame_time_and_zero_when_stopped() {
        let mut clock = SimulationClock::default();
        let frame = 1.0_f32 / 144.0;
        for i in 1..=288 {
            let lead = clock.lead_seconds();
            let steps = clock.advance(frame, true);
            let presented = clock.presented_seconds(lead, steps);
            assert!((presented - frame).abs() < 1e-5, "frame {i}: {presented}");
        }
        let lead = clock.lead_seconds();
        let steps = clock.advance(frame, false);
        assert_eq!(clock.presented_seconds(lead, steps), 0.0);
    }
}
