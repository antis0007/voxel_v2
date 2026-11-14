//! Client-side prediction, reconciliation and interpolation.
//!
//! The host stays authoritative. The client runs the same fixed step locally so
//! the player sees their own motion with no round trip, keeps the inputs it has
//! not had confirmed yet, and snaps back to the host's state when a frame arrives:
//! the confirmed state is adopted and the unconfirmed inputs are replayed on top.
//!
//! Everything is keyed by tick. Nothing here reads a clock, so the behaviour is
//! reproducible in a test with a simulated link.

use crate::protocol::Input;
use crate::session::MAX_LEAD;
use std::collections::VecDeque;

/// Inputs kept while waiting for confirmation, about one second at `TICK_HZ`.
/// Past that the oldest is forgotten locally; the host still applies it, so the
/// prediction is off by that input only until a later frame confirms it.
pub const MAX_PENDING: usize = 32;

const MIN_LEAD: u64 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reconcile {
    /// Inputs replayed on top of the confirmed state.
    pub replayed: usize,
    /// The snapshot was older than what we already had and was ignored.
    pub stale: bool,
}

/// Local player prediction against an authoritative host.
#[derive(Clone, Debug)]
pub struct Predictor<S> {
    confirmed: S,
    confirmed_tick: u64,
    predicted: S,
    pending: VecDeque<(u64, Input)>,
}

impl<S: Clone> Predictor<S> {
    pub fn new(initial: S) -> Self {
        Self {
            confirmed: initial.clone(),
            confirmed_tick: 0,
            predicted: initial,
            pending: VecDeque::new(),
        }
    }

    pub fn confirmed_tick(&self) -> u64 {
        self.confirmed_tick
    }

    /// What the player sees right now: confirmed state plus unconfirmed inputs.
    pub fn state(&self) -> &S {
        &self.predicted
    }

    pub fn pending_inputs(&self) -> usize {
        self.pending.len()
    }

    /// Applies one local input at once, before any host answer, and returns the
    /// predicted state. Feeding the same input to the host later must converge.
    pub fn push(&mut self, tick: u64, input: Input, step: &impl Fn(&S, &Input) -> S) -> &S {
        if self.pending.len() >= MAX_PENDING {
            self.pending.pop_front();
        }
        self.predicted = step(&self.predicted, &input);
        self.pending.push_back((tick, input));
        &self.predicted
    }

    /// Adopt the host's state for `tick` and replay everything still in flight.
    ///
    /// A snapshot older than the confirmed tick is ignored rather than rolled back:
    /// the relay is ordered, so a stale frame means a duplicate, not a rewind.
    pub fn reconcile(
        &mut self,
        tick: u64,
        state: S,
        step: &impl Fn(&S, &Input) -> S,
    ) -> Reconcile {
        if tick < self.confirmed_tick {
            return Reconcile { replayed: 0, stale: true };
        }
        while self.pending.front().is_some_and(|(queued, _)| *queued <= tick) {
            self.pending.pop_front();
        }
        self.confirmed = state;
        self.confirmed_tick = tick;
        let mut predicted = self.confirmed.clone();
        for (_, input) in &self.pending {
            predicted = step(&predicted, input);
        }
        self.predicted = predicted;
        Reconcile { replayed: self.pending.len(), stale: false }
    }

    /// The state we last had confirmed, for a divergence diagnostic.
    pub fn confirmed_state(&self) -> &S {
        &self.confirmed
    }
}

/// Renders other players slightly in the past so their motion can be smoothed
/// between samples instead of teleporting when a frame arrives.
#[derive(Clone, Debug)]
pub struct Interpolator<S> {
    samples: VecDeque<(u64, S)>,
    delay: u64,
    capacity: usize,
}

impl<S: Clone> Interpolator<S> {
    /// `delay` is in ticks; two or three at 30 Hz covers ordinary jitter.
    pub fn new(delay: u64) -> Self {
        Self { samples: VecDeque::new(), delay, capacity: MAX_PENDING }
    }

    pub fn delay(&self) -> u64 {
        self.delay
    }

    /// Late or repeated ticks are dropped, never reordered.
    pub fn push(&mut self, tick: u64, state: S) {
        if self.samples.back().is_some_and(|(last, _)| tick <= *last) {
            return;
        }
        self.samples.push_back((tick, state));
        while self.samples.len() > self.capacity {
            self.samples.pop_front();
        }
    }

    /// State to draw at `now`, in fractional ticks, `delay` ticks in the past.
    ///
    /// Render frames fall between net ticks, so `now` must keep its fraction or
    /// remote players move in steps. Positions are clamped at both ends: a player
    /// is never extrapolated into a wall while a packet is late.
    pub fn sample(&self, now: f64, lerp: impl Fn(&S, &S, f32) -> S) -> Option<S> {
        let target = (now - self.delay as f64).max(0.0);
        let first = self.samples.front()?;
        let last = self.samples.back()?;
        if target <= first.0 as f64 {
            return Some(first.1.clone());
        }
        if target >= last.0 as f64 {
            return Some(last.1.clone());
        }
        let mut previous: Option<(u64, &S)> = None;
        for (tick, state) in &self.samples {
            if let Some((a_tick, a)) = previous
                && target <= *tick as f64
            {
                let span = (*tick - a_tick).max(1) as f64;
                let t = ((target - a_tick as f64) / span) as f32;
                return Some(lerp(a, state, t.clamp(0.0, 1.0)));
            }
            previous = Some((*tick, state));
        }
        Some(last.1.clone())
    }
}

/// Round-trip estimate, jitter, and the input lead those justify.
///
/// A fixed lead either drops inputs on a slow link or adds latency on a fast one.
/// Two exponentially weighted averages and a clamped ramp avoid both.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rtt {
    ewma_ms: f32,
    jitter_ms: f32,
    samples: u32,
}

impl Rtt {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sample(&mut self, ms: f32) {
        let ms = ms.max(0.0);
        if self.samples == 0 {
            self.ewma_ms = ms;
        } else {
            let delta = (ms - self.ewma_ms).abs();
            self.jitter_ms += (delta - self.jitter_ms) * 0.25;
            self.ewma_ms += (ms - self.ewma_ms) * 0.125;
        }
        self.samples = self.samples.saturating_add(1);
    }

    pub fn estimate_ms(&self) -> f32 {
        self.ewma_ms
    }

    pub fn jitter_ms(&self) -> f32 {
        self.jitter_ms
    }

    /// Mean plus two jitters, in ticks, kept inside the session's accepted range.
    pub fn lead_ticks(&self, tick_hz: u16) -> u64 {
        let frame_ms = 1000.0 / f32::from(tick_hz.max(1));
        let want = (self.ewma_ms + 2.0 * self.jitter_ms) / frame_ms;
        (want.ceil() as u64).clamp(MIN_LEAD, MAX_LEAD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(dx: i16, buttons: u16) -> Input {
        Input { sequence: 1, tick: 1, movement: [dx, 0], look: [0, 0], buttons }
    }

    /// One axis, one wall at 100, so the host and client can disagree on purpose.
    fn step(pos: &i32, input: &Input) -> i32 {
        let moved = pos + i32::from(input.movement[0]);
        if moved > 100 { 100 } else { moved }
    }

    #[test]
    fn prediction_moves_before_any_host_answer() {
        let mut predictor = Predictor::new(0);
        let after = *predictor.push(1, input(5, 0), &step);
        assert_eq!(after, 5, "the player must not wait for a round trip");
        assert_eq!(predictor.confirmed_tick(), 0);
        assert_eq!(predictor.pending_inputs(), 1);
    }

    #[test]
    fn agreeing_host_changes_nothing() {
        let mut predictor = Predictor::new(0);
        for tick in 1..=3 {
            predictor.push(tick, input(5, 0), &step);
        }
        let before = *predictor.state();
        // The host computed the same thing for tick 2.
        let report = predictor.reconcile(2, 10, &step);
        assert!(!report.stale);
        assert_eq!(report.replayed, 1, "tick 3 is still in flight");
        assert_eq!(*predictor.state(), before);
    }

    #[test]
    fn disagreeing_host_is_adopted_and_replayed() {
        // The client has no collision knowledge, so it walks through the wall.
        // The host clamps. That difference is the whole reason to reconcile.
        let no_wall = |pos: &i32, input: &Input| pos + i32::from(input.movement[0]);
        let mut predictor = Predictor::new(0);
        predictor.push(1, input(60, 0), &no_wall);
        predictor.push(2, input(60, 0), &no_wall);
        predictor.push(3, input(10, 0), &no_wall);
        assert_eq!(*predictor.state(), 130);

        // The host reports tick 2 at the wall; tick 3 is still in flight.
        let report = predictor.reconcile(2, 100, &no_wall);
        assert_eq!(report.replayed, 1);
        assert_eq!(*predictor.state(), 110, "confirmed 100 plus the input in flight");
        assert_eq!(*predictor.confirmed_state(), 100);

        // Reconciling with the host's own step keeps the wall authoritative.
        let mut strict = Predictor::new(0);
        strict.push(1, input(60, 0), &no_wall);
        strict.reconcile(1, 100, &step);
        assert_eq!(*strict.state(), 100);
    }

    #[test]
    fn a_stale_snapshot_is_ignored() {
        let mut predictor = Predictor::new(0);
        predictor.push(1, input(5, 0), &step);
        predictor.reconcile(5, 25, &step);
        let report = predictor.reconcile(4, 1, &step);
        assert!(report.stale);
        assert_eq!(*predictor.state(), 25);
    }

    #[test]
    fn pending_inputs_stay_bounded() {
        let mut predictor = Predictor::new(0);
        for tick in 1..=(MAX_PENDING as u64 + 5) {
            predictor.push(tick, input(0, 0), &step);
        }
        assert_eq!(predictor.pending_inputs(), MAX_PENDING);
    }

    #[test]
    fn interpolation_renders_between_samples_and_clamps() {
        let mut interp = Interpolator::new(2);
        interp.push(10, 0.0f32);
        interp.push(11, 10.0f32);
        let lerp = |a: &f32, b: &f32, t: f32| a + (b - a) * t;
        // now 13 renders tick 11, the newest sample.
        assert_eq!(interp.sample(13.0, lerp), Some(10.0));
        // now 12 renders tick 10.
        assert_eq!(interp.sample(12.0, lerp), Some(0.0));
        // now 11 renders tick 9, older than the first sample.
        assert_eq!(interp.sample(11.0, lerp), Some(0.0));
        interp.push(12, 20.0);
        assert_eq!(interp.sample(13.0, lerp), Some(10.0), "renders tick 11 exactly");
    }

    #[test]
    fn interpolation_moves_between_ticks() {
        let mut interp = Interpolator::new(2);
        interp.push(10, 0.0f32);
        interp.push(11, 10.0f32);
        let lerp = |a: &f32, b: &f32, t: f32| a + (b - a) * t;
        // Four render frames inside one 30 Hz tick give four distinct positions.
        let frames: Vec<f32> = [12.0, 12.25, 12.5, 12.75]
            .iter()
            .filter_map(|now| interp.sample(*now, lerp))
            .collect();
        assert_eq!(frames, vec![0.0, 2.5, 5.0, 7.5]);
    }

    #[test]
    fn interpolation_ignores_repeated_ticks() {
        let mut interp = Interpolator::new(1);
        interp.push(4, 1.0f32);
        interp.push(4, 99.0f32);
        assert_eq!(interp.sample(5.0, |a: &f32, _b: &f32, _t: f32| *a), Some(1.0));
    }

    #[test]
    fn rtt_raises_the_lead_and_clamps() {
        let mut rtt = Rtt::new();
        assert_eq!(rtt.lead_ticks(30), MIN_LEAD, "no samples means the floor");
        rtt.sample(40.0);
        assert_eq!(rtt.estimate_ms(), 40.0);
        let fast = rtt.lead_ticks(30);
        rtt.sample(400.0);
        assert!(rtt.lead_ticks(30) >= fast);
        for _ in 0..40 {
            rtt.sample(900.0);
        }
        assert_eq!(rtt.lead_ticks(30), MAX_LEAD, "the lead is capped");
        assert!(rtt.jitter_ms() >= 0.0);
    }
}
