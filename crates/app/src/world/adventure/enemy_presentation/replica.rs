//! Presentation targets never advance authority, collisions, or damage.
use glam::Vec3;
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy)]
pub(crate) struct ReplicaPose {
    pub pos: Vec3,
    pub yaw: f32,
    pub phase: f32,
    pub gait: f32,
}
impl ReplicaPose {
    pub fn can_smooth(self, shown: Self, phase_can_reverse: bool) -> bool {
        self.pos.distance_squared(shown.pos) < 4.0 && (phase_can_reverse || self.phase >= shown.phase)
    }

    pub fn blend_from(self, shown: Self, dt: f32) -> Self {
        let blend = 1.0 - (-20.0 * dt.max(0.0)).exp();
        let turn = (self.yaw - shown.yaw + PI).rem_euclid(TAU) - PI;
        Self {
            pos: shown.pos.lerp(self.pos, blend),
            yaw: shown.yaw + turn * blend,
            phase: shown.phase + (self.phase - shown.phase) * blend,
            gait: shown.gait + (self.gait - shown.gait) * blend,
        }
    }
}

pub(crate) fn advance_attack(tell: f32, recovery: f32, dt: f32, duration: f32) -> (f32, f32) {
    let next_recovery = if tell > 0.0 && dt >= tell {
        (duration - (dt - tell)).max(0.0)
    } else {
        (recovery - dt).max(0.0)
    };
    ((tell - dt).max(0.0), next_recovery)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pose() -> ReplicaPose { ReplicaPose { pos: Vec3::ZERO, yaw: 0.0, phase: 0.0, gait: 0.0 } }

    #[test]
    fn replica_pose_converges_at_matching_rates_instead_of_stair_stepping() {
        let target = ReplicaPose { pos: Vec3::X, yaw: 1.0, phase: 1.0, gait: 1.0 };
        for hz in [30, 60, 120, 144, 240] {
            let mut shown = pose();
            for _ in 0..hz / 2 { shown = target.blend_from(shown, 1.0 / hz as f32); }
            let expected = 1.0 - (-10.0_f32).exp();
            for value in [shown.pos.x, shown.yaw, shown.phase, shown.gait] {
                assert!((value - expected).abs() < 0.00001, "{hz} Hz: {value}");
            }
        }
    }

    #[test]
    fn replica_turn_takes_the_short_path_across_pi() {
        let old = ReplicaPose { yaw: PI - 0.1, ..pose() };
        let target = ReplicaPose { yaw: -PI + 0.1, ..pose() };
        let shown = target.blend_from(old, 1.0 / 60.0);
        assert!(shown.yaw > old.yaw && shown.yaw < PI + 0.1);
    }

    #[test]
    fn teleports_and_reset_spawns_are_not_blended_from_old_poses() {
        assert!(!ReplicaPose { pos: Vec3::X * 2.0, ..pose() }.can_smooth(pose(), false));
        assert!(!pose().can_smooth(ReplicaPose { phase: 12.0, ..pose() }, false));
        assert!(ReplicaPose { pos: Vec3::X, phase: 0.5, ..pose() }.can_smooth(pose(), false));
    }

    #[test]
    fn retreating_freight_keeps_position_and_pose_smoothing() {
        let shown = ReplicaPose { phase: 0.8, ..pose() };
        let target = ReplicaPose { phase: 0.7, ..pose() };
        assert!(target.can_smooth(shown, true));
        let next = target.blend_from(shown, 1.0 / 60.0);
        assert!(next.phase > target.phase && next.phase < shown.phase);
    }

    #[test]
    fn attack_release_and_recovery_continue_between_snapshots() {
        for hz in [30, 60, 120, 144, 240] {
            let (mut tell, mut recovery) = (0.2, 0.0);
            for _ in 0..hz / 2 {
                (tell, recovery) = advance_attack(tell, recovery, 1.0 / hz as f32, 0.42);
            }
            assert_eq!(tell, 0.0);
            assert!((recovery - 0.12).abs() < 0.00001, "{hz} Hz: {recovery}");
        }
        assert_eq!(advance_attack(0.0, 0.0, 1.0, 0.42), (0.0, 0.0));
    }
}
