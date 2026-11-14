//! Physical access shared by inhabited settlements and historical structures.
//! A bearing toward another site is not a connection: an access way has two
//! built endpoints, a continuous corridor and reserved walking clearance.
use glam::{Vec2, Vec3};

/// Physical station endpoints for inspection or a future transport service.
/// Their presence alone makes no claim that a regional route is operational.
#[derive(Clone, Copy, Debug)]
pub struct Station {
    pub entrance: Vec3,
    pub platform: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct AccessWay {
    pub entrance: Vec2,
    pub street: Vec2,
    pub half_width: f32,
}

impl AccessWay {
    pub fn contains(self, point: Vec2) -> bool {
        let delta = self.street - self.entrance;
        let t = ((point - self.entrance).dot(delta) / delta.length_squared().max(1e-6)).clamp(0.0, 1.0);
        point.distance_squared(self.entrance + delta * t) <= self.half_width * self.half_width
    }

    pub fn bounds(self) -> (Vec2, Vec2) {
        let margin = Vec2::splat(self.half_width);
        (self.entrance.min(self.street) - margin, self.entrance.max(self.street) + margin)
    }
}
