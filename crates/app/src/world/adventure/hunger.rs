//! Player hunger policy. Food heals; sprinting does not drain by default.
//! `SprintDrain` is the opt-in mode that keeps the original sprint-linked drain.

pub const SPRINT_DRAIN_PER_SECOND: f32 = 0.04;
pub const SPRINT_FACTOR: f32 = 2.2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HungerMode {
    /// Hunger stays at its current value; eating still restores it.
    #[default]
    FoodHeals,
    /// Hunger drains while walking and faster while sprinting.
    SprintDrain,
}

/// Hunger lost per second for this movement state. Flying never drains.
pub fn drain_per_second(mode: HungerMode, sprinting: bool, flying: bool) -> f32 {
    if flying || mode == HungerMode::FoodHeals {
        return 0.0;
    }
    if sprinting { SPRINT_DRAIN_PER_SECOND * SPRINT_FACTOR } else { SPRINT_DRAIN_PER_SECOND }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_never_drains() {
        assert_eq!(HungerMode::default(), HungerMode::FoodHeals);
        for sprinting in [false, true] {
            for flying in [false, true] {
                assert_eq!(drain_per_second(HungerMode::FoodHeals, sprinting, flying), 0.0);
            }
        }
    }

    #[test]
    fn sprint_drain_mode_keeps_the_original_rates() {
        let walk = drain_per_second(HungerMode::SprintDrain, false, false);
        let sprint = drain_per_second(HungerMode::SprintDrain, true, false);
        assert!((walk - 0.04).abs() < 1e-6);
        assert!((sprint - 0.088).abs() < 1e-6);
    }

    #[test]
    fn flying_never_drains_in_either_mode() {
        assert_eq!(drain_per_second(HungerMode::SprintDrain, true, true), 0.0);
        assert_eq!(drain_per_second(HungerMode::SprintDrain, false, true), 0.0);
    }
}
