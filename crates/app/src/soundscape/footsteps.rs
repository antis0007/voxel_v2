//! Footstep timing from the walk phase and the surface under the feet.
use crate::audio::Surface;
use engine_content::{BlockId, MatterState};
use std::{collections::HashMap, f32::consts::PI, sync::LazyLock};

const TABLE: &str = include_str!("../../../../assets/footsteps.ron");
/// Gain while sneaking, against 1.0 for a normal step.
const SNEAK_GAIN: f32 = 0.3;
/// Footfalls sit at the bottom of the camera bob, `sin(2 * phase)` = -1.
const FOOTFALL_PHASE: f32 = 0.75 * PI;
/// Walk phase advances 2.3 rad per metre and two footfalls share a turn.
pub const STEP_SPACING_M: f32 = PI / 2.3;
/// Electric conductivity above which an unlisted solid sounds as metal, S/m.
const METAL_CONDUCTIVITY: f32 = 1.0e6;

#[derive(Default, serde::Deserialize)]
struct StepTable {
    blocks: HashMap<BlockId, Surface>,
}

static STEPS: LazyLock<StepTable> = LazyLock::new(|| ron::from_str(TABLE).unwrap_or_default());

/// The footstep sound for walking on `block`.
pub fn surface(block: BlockId) -> Surface {
    if let Some(surface) = STEPS.blocks.get(&block) {
        return *surface;
    }
    let material = block.material();
    match material.physics.matter_state {
        MatterState::Liquid => Surface::Water,
        MatterState::Powder => Surface::Soil,
        _ if material.sim.electric_conductivity >= METAL_CONDUCTIVITY => Surface::Metal,
        _ if material.sim.thermal.ignite.is_some() => Surface::Wood,
        _ => Surface::Stone,
    }
}

/// The parts of the player's movement that time a footstep.
#[derive(Clone, Copy, Debug)]
pub struct Gait {
    pub phase: f32,
    pub on_ground: bool,
    pub flying: bool,
    pub swimming: bool,
    pub crouch: f32,
}

#[derive(Default)]
pub struct Tracker {
    last_phase: Option<f32>,
    strides: u32,
}

/// Footfalls passed between two walk phases (radians, wrapped to one turn).
fn footfalls(previous: f32, next: f32) -> u32 {
    let mut next = next;
    if next < previous {
        next += 2.0 * PI;
    }
    let index = |phase: f32| ((phase - FOOTFALL_PHASE) / PI).floor();
    (index(next) - index(previous)).max(0.0) as u32
}

impl Tracker {
    /// Returns the gain and stride number of a footstep due this frame.
    pub fn advance(&mut self, gait: Gait) -> Option<(f32, u32)> {
        let previous = self.last_phase.replace(gait.phase)?;
        if gait.flying || gait.swimming || !gait.on_ground || !gait.phase.is_finite() {
            return None;
        }
        // A long frame can pass several footfalls; one sound stands for all.
        if footfalls(previous, gait.phase) == 0 {
            return None;
        }
        self.strides = self.strides.wrapping_add(1);
        let gain = if gait.crouch > 0.5 { SNEAK_GAIN } else { 1.0 };
        Some((gain, self.strides))
    }
}

/// Distance-based stride counter for a body without a walk phase.
///
/// For remote players: feed it the horizontal distance a grounded pose moved
/// and play `Audio::footstep` with the pose position when it returns true.
/// Not wired in; the multiplayer pose stream lives in files owned elsewhere.
#[derive(Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct Stride {
    travelled_m: f32,
}

#[cfg_attr(not(test), allow(dead_code))]
impl Stride {
    pub fn advance(&mut self, moved_m: f32, grounded: bool) -> bool {
        if !grounded || !moved_m.is_finite() || moved_m <= 0.0 {
            return false;
        }
        // Teleports and respawns are not walking.
        if moved_m > STEP_SPACING_M * 4.0 {
            self.travelled_m = 0.0;
            return false;
        }
        self.travelled_m += moved_m;
        if self.travelled_m < STEP_SPACING_M {
            return false;
        }
        self.travelled_m -= STEP_SPACING_M;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walking(phase: f32) -> Gait {
        Gait { phase, on_ground: true, flying: false, swimming: false, crouch: 0.0 }
    }

    /// Steps over `seconds` at a horizontal speed, phase advanced like `player::physics`.
    fn steps_at(speed_m: f32, seconds: f32, mut gait: impl FnMut(f32) -> Gait) -> u32 {
        let mut tracker = Tracker::default();
        let mut phase = 0.0f32;
        let dt = 1.0 / 60.0;
        let mut count = 0;
        for _ in 0..(seconds / dt) as u32 {
            phase = (phase + dt * speed_m * 2.3).rem_euclid(2.0 * PI);
            count += u32::from(tracker.advance(gait(phase)).is_some());
        }
        count
    }

    #[test]
    fn cadence_follows_the_walk_phase() {
        let walk = steps_at(1.4, 10.0, walking);
        let sprint = steps_at(5.6, 10.0, walking);
        // 14 m at one step per STEP_SPACING_M.
        let expected = (14.0 / STEP_SPACING_M).round() as i64;
        assert!((walk as i64 - expected).abs() <= 1, "walk {walk} steps, expected {expected}");
        assert!((sprint as i64 - 4 * walk as i64).abs() <= 2, "sprint {sprint} vs walk {walk}");
        assert_eq!(steps_at(0.0, 10.0, walking), 0);
    }

    #[test]
    fn no_steps_in_flight_in_the_air_or_swimming() {
        let fly = |phase| Gait { flying: true, ..walking(phase) };
        let air = |phase| Gait { on_ground: false, ..walking(phase) };
        let swim = |phase| Gait { swimming: true, ..walking(phase) };
        assert_eq!(steps_at(4.0, 10.0, fly), 0);
        assert_eq!(steps_at(4.0, 10.0, air), 0);
        assert_eq!(steps_at(4.0, 10.0, swim), 0);
    }

    #[test]
    fn sneaking_steps_are_quieter() {
        let mut tracker = Tracker::default();
        tracker.advance(walking(0.0));
        let normal = tracker.advance(walking(1.0 * PI)).map(|(gain, _)| gain);
        let sneak = tracker.advance(Gait { crouch: 1.0, ..walking(1.9 * PI) }).map(|(gain, _)| gain);
        assert_eq!(normal, Some(1.0));
        assert_eq!(sneak, Some(SNEAK_GAIN));
    }

    #[test]
    fn footstep_table_parses_and_every_material_maps() {
        let table: StepTable = ron::from_str(TABLE).expect("assets/footsteps.ron");
        assert!(table.blocks.len() > 20);
        for &block in engine_content::ALL_BLOCK_IDS {
            assert!(Surface::ALL.contains(&surface(block)), "{block:?}");
        }
        assert_eq!(surface(BlockId::Grass), Surface::Grass);
        assert_eq!(surface(BlockId::Sand), Surface::Sand);
        assert_eq!(surface(BlockId::Planks), Surface::Wood);
        assert_eq!(surface(BlockId::SnowGrass), Surface::Snow);
        assert_eq!(surface(BlockId::Water), Surface::Water);
        assert_eq!(surface(BlockId::Stone), Surface::Stone);
        // Physics fallback for unlisted blocks.
        assert_eq!(surface(BlockId::CopperBlock), Surface::Metal);
        assert_eq!(surface(BlockId::Cobblestone), Surface::Stone);
        let used: std::collections::HashSet<_> = engine_content::ALL_BLOCK_IDS.iter().map(|&b| surface(b)).collect();
        assert_eq!(used.len(), Surface::COUNT, "every surface is reachable");
    }

    #[test]
    fn remote_strides_follow_distance_and_ignore_teleports() {
        let mut stride = Stride::default();
        let steps = (0..100).filter(|_| stride.advance(0.1, true)).count();
        assert_eq!(steps, (10.0 / STEP_SPACING_M) as usize);
        assert!(!stride.advance(50.0, true));
        assert!(!stride.advance(1.0, false));
    }
}
