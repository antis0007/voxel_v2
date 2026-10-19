//! Bounded fixed-step simulation. Wall-clock rendering never controls physics step size.
mod cellular;
pub(crate) mod sandbox;
mod thermal;
mod rigid;
pub(crate) mod residency;
mod world;
pub use world::WorldSimulation;
pub(crate) use world::discover;
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
