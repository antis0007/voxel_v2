//! Shield block, parry and spike reflection for the player's raised guard.

use glam::Vec3;

/// Seconds after raising in which a hit is parried instead of blocked.
pub const PARRY_WINDOW_S: f32 = 0.25;
/// Seconds the shield stays down after its stamina runs out.
pub const BREAK_LOCKOUT_S: f32 = 1.5;
/// Cosine of the half-angle of the protected front arc (70 degrees).
const FRONT_COS: f32 = 0.342;
/// Stamina regained per second while the shield is lowered.
const REGEN_PER_S: f32 = 18.0;
/// Share of capacity returned by a parry.
const PARRY_REFUND: f32 = 0.25;

/// Stats of the carried shield, read from its assembled tool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gear {
    /// Share of a blocked hit that never lands, 0..1.
    pub block: f32,
    /// Damage dealt back to a melee attacker per blocked hit.
    pub thorns: f32,
    /// Stamina the shield holds; a blocked hit costs its damage.
    pub capacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outcome {
    /// Damage that still reaches the player.
    pub taken: f32,
    /// Damage dealt back to the attacker.
    pub reflected: f32,
    pub parried: bool,
    pub blocked: bool,
}

impl Outcome {
    fn through(damage: f32) -> Self {
        Self { taken: damage, reflected: 0.0, parried: false, blocked: false }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Guard {
    raised_for: f32,
    stamina: f32,
    lockout: f32,
    primed: bool,
}

impl Guard {
    /// Advances timers; a lowered shield restarts its parry window and recovers.
    pub fn tick(&mut self, gear: Option<&Gear>, raised: bool, dt: f32) {
        let dt = dt.max(0.0);
        let Some(gear) = gear else {
            *self = Self::default();
            return;
        };
        if !self.primed {
            self.stamina = gear.capacity;
            self.primed = true;
        }
        self.lockout = (self.lockout - dt).max(0.0);
        if raised && self.lockout <= 0.0 {
            self.raised_for += dt;
        } else {
            self.raised_for = 0.0;
            self.stamina = (self.stamina + REGEN_PER_S * dt).min(gear.capacity);
        }
        self.stamina = self.stamina.min(gear.capacity);
    }

    pub fn is_up(&self) -> bool {
        self.raised_for > 0.0
    }

    /// Stamina as a share of capacity, for the HUD.
    pub fn fraction(&self, gear: &Gear) -> f32 {
        if gear.capacity <= 0.0 { 0.0 } else { (self.stamina / gear.capacity).clamp(0.0, 1.0) }
    }

    /// Resolves one melee hit from `toward` (player to attacker) while facing `facing`.
    pub fn absorb(&mut self, gear: &Gear, damage: f32, toward: Vec3, facing: Vec3) -> Outcome {
        if !self.is_up() || damage <= 0.0 {
            return Outcome::through(damage);
        }
        let front = facing.normalize_or_zero().dot(toward.normalize_or_zero());
        if front < FRONT_COS {
            return Outcome::through(damage);
        }
        if self.raised_for <= PARRY_WINDOW_S {
            self.stamina = (self.stamina + gear.capacity * PARRY_REFUND).min(gear.capacity);
            return Outcome { taken: 0.0, reflected: gear.thorns * 2.0, parried: true, blocked: true };
        }
        let stopped = damage * gear.block.clamp(0.0, 1.0);
        if stopped > self.stamina {
            // Overwhelmed: the shield drops and only the stamina left is absorbed.
            let soaked = self.stamina;
            self.stamina = 0.0;
            self.lockout = BREAK_LOCKOUT_S;
            self.raised_for = 0.0;
            return Outcome { taken: damage - soaked, reflected: 0.0, parried: false, blocked: true };
        }
        self.stamina -= stopped;
        Outcome { taken: damage - stopped, reflected: gear.thorns, parried: false, blocked: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEAR: Gear = Gear { block: 0.8, thorns: 0.0, capacity: 40.0 };
    const SPIKED: Gear = Gear { block: 0.8, thorns: 6.0, capacity: 40.0 };

    fn raised(gear: &Gear, seconds: f32) -> Guard {
        let mut g = Guard::default();
        g.tick(Some(gear), true, seconds);
        g
    }

    #[test]
    fn lowered_shield_stops_nothing() {
        let mut g = Guard::default();
        g.tick(Some(&GEAR), false, 0.5);
        assert_eq!(g.absorb(&GEAR, 10.0, Vec3::Z, Vec3::Z).taken, 10.0);
    }

    #[test]
    fn frontal_hit_is_cut_and_costs_stamina() {
        let mut g = raised(&GEAR, 0.5);
        let out = g.absorb(&GEAR, 10.0, Vec3::Z, Vec3::Z);
        assert_eq!((out.taken, out.blocked, out.parried), (2.0, true, false));
        assert_eq!(g.fraction(&GEAR), 0.8);
    }

    #[test]
    fn hit_from_behind_ignores_the_shield() {
        let mut g = raised(&GEAR, 0.5);
        let out = g.absorb(&GEAR, 10.0, -Vec3::Z, Vec3::Z);
        assert_eq!((out.taken, out.blocked), (10.0, false));
    }

    #[test]
    fn early_hit_parries_and_refunds() {
        let mut g = raised(&SPIKED, 0.1);
        g.stamina = 10.0;
        let out = g.absorb(&SPIKED, 30.0, Vec3::Z, Vec3::Z);
        assert!(out.parried);
        assert_eq!(out.taken, 0.0);
        assert_eq!(out.reflected, 12.0);
        assert_eq!(g.fraction(&SPIKED), 0.5);
    }

    #[test]
    fn spikes_reflect_only_when_blocking() {
        let mut g = raised(&SPIKED, 0.5);
        assert_eq!(g.absorb(&SPIKED, 5.0, Vec3::Z, Vec3::Z).reflected, 6.0);
        let mut plain = raised(&GEAR, 0.5);
        assert_eq!(plain.absorb(&GEAR, 5.0, Vec3::Z, Vec3::Z).reflected, 0.0);
    }

    #[test]
    fn overwhelmed_shield_breaks_and_locks_out() {
        let mut g = raised(&GEAR, 0.5);
        g.stamina = 4.0;
        let out = g.absorb(&GEAR, 20.0, Vec3::Z, Vec3::Z);
        assert_eq!(out.taken, 16.0);
        assert!(!g.is_up());
        g.tick(Some(&GEAR), true, 0.5);
        assert!(!g.is_up(), "locked out while broken");
        g.tick(Some(&GEAR), true, BREAK_LOCKOUT_S);
        g.tick(Some(&GEAR), true, 0.1);
        assert!(g.is_up());
    }

    #[test]
    fn lowering_recovers_stamina() {
        let mut g = raised(&GEAR, 0.5);
        g.absorb(&GEAR, 20.0, Vec3::Z, Vec3::Z);
        g.tick(Some(&GEAR), false, 1.0);
        assert!(g.fraction(&GEAR) > 0.6);
    }
}
