//! Camera comfort scales and the hold-to-zoom lens.
//!
//! Scales only touch presentation (camera bob, held-item sway, FOV kick), never
//! simulation state, so a player's setting cannot change a replay or a peer's world.

use std::sync::atomic::{AtomicU32, Ordering};

static CAMERA_MOTION_PERCENT: AtomicU32 = AtomicU32::new(100);
static FOV_EFFECTS_PERCENT: AtomicU32 = AtomicU32::new(100);

/// Field-of-view fraction at full zoom, measured on the tangent so aim feels linear.
const ZOOM_RATIO: f32 = 0.25;
const ZOOM_RATE: f32 = 16.0;

pub fn camera_motion_percent() -> u32 {
    CAMERA_MOTION_PERCENT.load(Ordering::Relaxed)
}

pub fn set_camera_motion_percent(percent: u32) {
    CAMERA_MOTION_PERCENT.store(percent.min(100), Ordering::Relaxed);
}

/// Multiplier for walk bob and held-item sway, 0 to 1.
pub fn camera_motion() -> f32 {
    camera_motion_percent() as f32 / 100.0
}

pub fn fov_effects_percent() -> u32 {
    FOV_EFFECTS_PERCENT.load(Ordering::Relaxed)
}

pub fn set_fov_effects_percent(percent: u32) {
    FOV_EFFECTS_PERCENT.store(percent.min(100), Ordering::Relaxed);
}

/// Multiplier for sprint and dash FOV kick, 0 to 1.
pub fn fov_effects() -> f32 {
    fov_effects_percent() as f32 / 100.0
}

/// Hold-to-zoom state; eases toward the zoomed lens unless FOV effects are off.
#[derive(Default, Clone, Copy)]
pub struct Zoom {
    t: f32,
}

impl Zoom {
    pub fn update(&mut self, held: bool, dt: f32) {
        let target = f32::from(u8::from(held));
        if fov_effects_percent() == 0 || !dt.is_finite() {
            self.t = target;
            return;
        }
        self.t += (target - self.t) * (1.0 - (-ZOOM_RATE * dt.max(0.0)).exp());
        if (self.t - target).abs() < 1.0e-3 {
            self.t = target;
        }
    }

    fn tangent_scale(&self) -> f32 {
        1.0 + (ZOOM_RATIO - 1.0) * self.t.clamp(0.0, 1.0)
    }

    /// Lens FOV for an unzoomed FOV of `base` degrees.
    pub fn fov_degrees(&self, base: f32) -> f32 {
        if self.t <= 0.0 {
            return base;
        }
        let half = (base.to_radians() * 0.5).tan() * self.tangent_scale();
        (half.atan() * 2.0).to_degrees()
    }

    /// Render FOV from the chosen FOV and the movement kick, after comfort scaling.
    pub fn lens(&self, base: f32, kick: f32) -> f32 {
        self.fov_degrees(base + kick * fov_effects())
    }

    /// Mouse-look multiplier that keeps screen-space aim speed constant while zoomed.
    pub fn sensitivity_scale(&self) -> f32 {
        self.tangent_scale()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_is_identity_at_rest_and_narrows_when_held() {
        let mut zoom = Zoom::default();
        assert_eq!(zoom.fov_degrees(70.0), 70.0);
        assert_eq!(zoom.sensitivity_scale(), 1.0);
        for _ in 0..240 {
            zoom.update(true, 1.0 / 120.0);
        }
        let narrow = zoom.fov_degrees(70.0);
        let expected = ((35.0_f32.to_radians().tan() * ZOOM_RATIO).atan() * 2.0).to_degrees();
        assert!((narrow - expected).abs() < 1.0e-3, "{narrow} vs {expected}");
        assert!((zoom.sensitivity_scale() - ZOOM_RATIO).abs() < 1.0e-6);
        for _ in 0..240 {
            zoom.update(false, 1.0 / 120.0);
        }
        assert_eq!(zoom.fov_degrees(70.0), 70.0);
    }

    #[test]
    fn zoom_rejects_non_finite_time() {
        let mut zoom = Zoom::default();
        zoom.update(true, f32::NAN);
        assert!(zoom.fov_degrees(70.0).is_finite());
    }
}
