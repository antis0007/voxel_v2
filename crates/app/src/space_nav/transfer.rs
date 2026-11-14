//! Transfer figures from where the player is to a chosen body.
use super::TICKS_PER_SECOND;
use engine_space::{BodyId, System, synodic_period};

/// Parking and capture orbit height as a fraction of each body's radius.
const LOW_ORBIT: f64 = 0.25;
/// Window scan steps per shorter orbit.
const SCAN_STEPS: f64 = 720.0;

/// Patched-conic figures for one departure body and one destination.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transfer {
    /// Body the burn leaves: the reference body or the ancestor sharing a parent with the target.
    pub from: BodyId,
    pub depart_mps: f64,
    pub capture_mps: f64,
    pub coast_s: f64,
    /// Seconds until the phase window; `None` when any time works.
    pub window_s: Option<f64>,
    /// True when `to` orbits `from`.
    pub child: bool,
}

/// Body to leave for `to`: `near` or its first ancestor that `to` orbits or shares a parent with.
fn departure(system: &System, near: BodyId, to: BodyId) -> Option<BodyId> {
    let mut from = near;
    loop {
        if from == to {
            return None;
        }
        if system.parent(to) == Some(from) || (system.parent(from).is_some() && system.parent(from) == system.parent(to)) {
            return Some(from);
        }
        from = system.parent(from)?;
    }
}

/// Transfer from the body holding the player to `to`; `None` when no single Hohmann leg joins them.
pub fn transfer(system: &System, near: BodyId, to: BodyId, tick: u64) -> Option<Transfer> {
    let from = departure(system, near, to)?;
    // A star has no parking orbit worth quoting.
    system.parent(from)?;
    let plan = system.transfer_plan(from, to, system.radius(from) * LOW_ORBIT, system.radius(to) * LOW_ORBIT)?;
    let child = system.parent(to) == Some(from);
    let window_s = if child {
        None
    } else {
        let (t1, t2) = (system.period_ticks(from) as f64, system.period_ticks(to) as f64);
        let horizon = (synodic_period(t1, t2) * 1.05).min(1.0e12) as u64;
        let step = (t1.min(t2) / SCAN_STEPS).max(1.0) as u64;
        // A refined crossing is the first tick just past alignment. Recognise
        // that instant (and a one-second tolerance) rather than skipping a full
        // synodic period when the map previews the returned departure tick.
        let tolerance = TICKS_PER_SECOND as u64;
        system.next_window(from, to, tick.saturating_sub(tolerance), step, horizon)
            .map(|w| if w <= tick.saturating_add(tolerance) { 0.0 } else { (w - tick) as f64 / TICKS_PER_SECOND })
    };
    Some(Transfer { from, depart_mps: plan.depart, capture_mps: plan.capture, coast_s: plan.cruise.time_s, window_s, child })
}
