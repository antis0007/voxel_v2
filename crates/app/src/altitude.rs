//! Altitude tiers: surface play, transition, and orbit view.

use crate::render::globe::chart;
use glam::DVec3;

pub const TRANSITION_M: f64 = 1_500.0;
pub const ORBIT_M: f64 = 30_000.0;
pub const LOAD_STOP_M: f64 = 6_000.0;
pub const HYSTERESIS_M: f64 = 200.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tier {
    #[default]
    Surface,
    Transition,
    Orbit,
}

pub struct TierState {
    tier: Tier,
}

impl TierState {
    pub fn new() -> TierState {
        TierState { tier: Tier::Surface }
    }

    /// Moves the tier only once altitude passes an edge by the hysteresis band.
    pub fn update(&mut self, altitude_m: f64) -> Tier {
        let h = HYSTERESIS_M;
        self.tier = match self.tier {
            Tier::Surface if altitude_m > ORBIT_M + h => Tier::Orbit,
            Tier::Surface if altitude_m > TRANSITION_M + h => Tier::Transition,
            Tier::Transition if altitude_m > ORBIT_M + h => Tier::Orbit,
            Tier::Transition if altitude_m < TRANSITION_M - h => Tier::Surface,
            Tier::Orbit if altitude_m < ORBIT_M - h => {
                if altitude_m < TRANSITION_M - h { Tier::Surface } else { Tier::Transition }
            }
            t => t,
        };
        self.tier
    }

    pub fn tier(&self) -> Tier {
        self.tier
    }
}

impl Default for TierState {
    fn default() -> Self {
        Self::new()
    }
}

/// Chunk radius kept resident: full below the transition, zero at the load stop.
pub fn resident_radius_chunks(altitude_m: f64, view_distance_chunks: i32) -> i32 {
    if altitude_m >= LOAD_STOP_M || view_distance_chunks <= 0 {
        return 0;
    }
    let f = 1.0 - (altitude_m - TRANSITION_M) / (LOAD_STOP_M - TRANSITION_M);
    (f * view_distance_chunks as f64).round().clamp(1.0, view_distance_chunks as f64) as i32
}

/// Height above the surface; on a planet the chart face-local y.
pub fn altitude_m(eye_m: DVec3, planet: bool) -> f64 {
    if planet { chart::split(eye_m).1.y } else { eye_m.y }
}

/// Prepare terrain for a Creative descent while keeping the hard space cutoff.
/// Express-lift terminal prefetch remains authoritative for cabin passengers.
pub fn residency_altitude_m(world: &crate::world::World, player: &crate::player::Player, actual_m: f64) -> f64 {
    let terminal = crate::world::orbital::express::residency_altitude_m(&world.orbital, actual_m);
    if terminal != actual_m { return terminal; }
    if planet_world(world) && world.rules.mode.creative_tools() && player.flying
        && player.anchor.is_none() && actual_m < LOAD_STOP_M {
        let descent_mps = (-player.vel.y * world.voxel_scale.meters()).max(0.0) as f64;
        (actual_m - (descent_mps * 3.0).min(LOAD_STOP_M - TRANSITION_M)).max(0.0)
    } else { actual_m }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_edges_carry_200_m_of_hysteresis() {
        let mut s = TierState::new();
        assert_eq!(s.update(1_400.0), Tier::Surface);
        assert_eq!(s.update(1_600.0), Tier::Surface);
        assert_eq!(s.update(1_701.0), Tier::Transition);
        assert_eq!(s.update(1_400.0), Tier::Transition);
        assert_eq!(s.update(1_299.0), Tier::Surface);
        assert_eq!(s.update(30_201.0), Tier::Orbit);
        assert_eq!(s.update(29_801.0), Tier::Orbit);
        assert_eq!(s.update(29_799.0), Tier::Transition);
    }

    #[test]
    fn a_256_m_descent_step_resumes_loading_next_tick() {
        let mut s = TierState::new();
        s.update(31_000.0);
        assert_eq!(s.update(5_900.0), Tier::Transition);
        assert!(resident_radius_chunks(5_900.0, 28) >= 1);
    }

    #[test]
    fn resident_radius_shrinks_linearly_to_zero_at_six_km() {
        assert_eq!(resident_radius_chunks(0.0, 28), 28);
        assert_eq!(resident_radius_chunks(1_500.0, 28), 28);
        assert_eq!(resident_radius_chunks(3_750.0, 28), 14);
        assert_eq!(resident_radius_chunks(6_000.0, 28), 0);
        assert_eq!(resident_radius_chunks(50_000.0, 28), 0);
    }

    #[test]
    fn planet_altitude_is_face_local_height() {
        let p = DVec3::new(2.0 * chart::PITCH_M + 5.0, 777.0, 3.0);
        assert_eq!(altitude_m(p, true), 777.0);
        assert_eq!(altitude_m(p, false), 777.0);
    }
}

/// Whether the world is a planet for altitude purposes.
pub fn planet_world(world: &crate::world::World) -> bool {
    world.sample_config().is_planet()
}
