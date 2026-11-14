//! Player-side status effect hooks the adventure needs while resolving hits.
//! Progression owns the effect list; it mirrors the parts combat reads here
//! each fixed step and drains `pending_effects` back into the list.
use super::Adventure;
use crate::world::progression::effects::Kind;

/// Poison a toxic enemy's melee hit applies: seconds and level.
pub const TOXIC_HIT: (f32, u8) = (4.0, 1);

impl Adventure {
    /// Resistance scales a hit, then absorption health soaks what it can.
    /// Returns what reaches health.
    pub(super) fn soak(&mut self, amount: f32) -> f32 {
        let amount = amount * (1.0 - 0.2 * f32::from(self.resist_level)).max(0.2);
        let soaked = amount.min(self.absorption).max(0.0);
        self.absorption -= soaked;
        amount - soaked
    }

    /// Queues an effect for progression to apply on its next step.
    pub(super) fn inflict(&mut self, kind: Kind, seconds: f32, level: u8) {
        if !self.creative && self.pending_effects.len() < 8 {
            self.pending_effects.push((kind, seconds, level));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resistance_then_absorption() {
        let mut a = Adventure::default();
        a.resist_level = 1;
        a.absorption = 10.0;
        let taken = a.soak(25.0);
        assert_eq!(a.absorption, 0.0);
        assert!((taken - 10.0).abs() < 1e-4, "{taken}");
        a.resist_level = 9;
        assert!((a.soak(10.0) - 2.0).abs() < 1e-4, "resistance floors at 20%");
    }
}
