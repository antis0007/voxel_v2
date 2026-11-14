//! Charged swing of heavy tools: hold to build force, release to crack a crater.
use crate::world::BlockId;
use glam::IVec3;

/// Seconds of holding for a full charge.
pub const FULL_S: f32 = 0.85;
/// Smallest charge that still lands as a swing; a tap does nothing.
pub const MIN_FRACTION: f32 = 0.25;
/// Crater radius in metres at the smallest and the full charge.
const RADIUS_M: (f32, f32) = (0.9, 2.6);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Charge {
    held_for: f32,
    target: Option<(IVec3, BlockId)>,
}

impl Charge {
    /// Aim changes cancel stored force instead of releasing it on another block.
    pub fn track(&mut self, target: (IVec3, BlockId), held: bool, dt: f32) -> Option<f32> {
        if self.target != Some(target) {
            *self = Self::default();
            if !held { return None; }
            self.target = Some(target);
        }
        self.tick(held, dt)
    }

    /// Builds charge while `held`; returns the fraction when the button is released.
    pub fn tick(&mut self, held: bool, dt: f32) -> Option<f32> {
        if held {
            self.held_for = (self.held_for + dt.max(0.0)).min(FULL_S);
            return None;
        }
        let fraction = self.fraction();
        self.held_for = 0.0;
        (fraction >= MIN_FRACTION).then_some(fraction)
    }

    pub fn fraction(&self) -> f32 {
        self.held_for / FULL_S
    }

    /// Charge for the HUD ring while building, hidden at zero.
    pub fn display(&self) -> Option<f32> {
        (self.held_for > 0.0).then(|| self.fraction())
    }
}

/// Crater radius in metres for a released charge.
pub fn radius_m(fraction: f32) -> f32 {
    let t = ((fraction - MIN_FRACTION) / (1.0 - MIN_FRACTION)).clamp(0.0, 1.0);
    RADIUS_M.0 + (RADIUS_M.1 - RADIUS_M.0) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_releases_nothing() {
        let mut c = Charge::default();
        assert_eq!(c.tick(true, 0.1), None);
        assert_eq!(c.tick(false, 0.0), None);
        assert_eq!(c.display(), None);
    }

    #[test]
    fn release_reports_fraction_and_resets() {
        let mut c = Charge::default();
        c.tick(true, FULL_S * 0.5);
        assert_eq!(c.tick(false, 0.0), Some(0.5));
        assert_eq!(c.tick(false, 0.0), None);
    }

    #[test]
    fn charge_caps_at_full() {
        let mut c = Charge::default();
        c.tick(true, 10.0);
        assert_eq!(c.tick(false, 0.0), Some(1.0));
    }

    #[test]
    fn radius_grows_with_charge() {
        assert!(radius_m(MIN_FRACTION) < radius_m(0.6));
        assert!(radius_m(0.6) < radius_m(1.0));
        assert_eq!(radius_m(1.0), 2.6);
    }
}
