//! Bounded fixed-step simulation. Wall-clock rendering never controls physics step size.
mod cellular;
pub(crate) mod climate;
mod environment;
mod heat;
pub(crate) mod fixed;
pub(crate) mod cpu_region;
pub(crate) mod residency;
mod rigid;
pub(crate) mod sandbox;
pub(crate) mod storms;
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
pub const STEP_SECONDS: f32 = engine_core::time::PLAYER_STEP_SECONDS;
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

    /// Time banked towards the next step. Presentation-only motion (the orbital
    /// sky) adds it to the day clock so it advances every frame, not in 1/120 s hops.
    pub fn lead_seconds(&self) -> f32 {
        self.accumulated as f32
    }
}

pub use engine_core::Budget;

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
}
