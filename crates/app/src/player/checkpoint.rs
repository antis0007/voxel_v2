use super::{Input, Player};

impl Player {
    pub(crate) fn checkpoint_valid(&self) -> bool {
        self.pos.is_finite() && self.precise_pos.is_finite() && self.vel.is_finite() && self.gravity_frame.is_finite() && (self.gravity_frame.length_squared() - 1.0).abs() < 0.001
            && self.pos.abs().max_element() <= 1_000_000.0
            && self.precise_pos.abs().max_element() <= 1_000_001.0
            && (0.1..=10.0).contains(&self.scale) && (0.0..=1.0).contains(&self.crouch_t)
            && [self.yaw, self.pitch, self.jump_tap_timer, self.step_camera_offset,
                self.step_camera_velocity_meters, self.walk_phase, self.walk_bob_meters,
                self.water_impact, self.jump_buffer, self.coyote_time].into_iter().all(f32::is_finite)
            && (4.0..=64.0).contains(&self.flight.speed) && (1.0..=4.0).contains(&self.flight.boost)
            && (0.0..=1.5).contains(&self.levitation_seconds)
            && self.movement.checkpoint_valid() && self.viewmodel.checkpoint_valid()
            && self.traversal.checkpoint_valid()
    }
}

impl Input {
    pub(crate) fn checkpoint_valid(&self) -> bool {
        self.wheel.is_finite() && self.shift_keys <= 3 && self.control_keys <= 3 && self.alt_keys <= 3
    }
}
