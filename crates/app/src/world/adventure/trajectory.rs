//! Distance-based paths shared by swept projectiles and terrain-cutting beams.
use glam::{DVec3, Quat, Vec3};

pub const CURVE_RADIUS: f32 = 5.0;
pub const ORBIT_RADIUS: f32 = 1.75;
pub const WEAVE_LENGTH: f32 = 6.0;
pub const WEAVE_ANGLE: f32 = 1.05;

#[derive(Clone, Copy)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Flight {
    normal: Vec3,
    distance: f32,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    precise_position: Option<DVec3>,
    #[cfg_attr(feature = "multiplayer", serde(default))]
    pub snapline: Option<super::snapline::Tether>,
}

impl Flight {
    pub fn new(direction: Vec3) -> Self {
        let forward = direction.normalize_or(Vec3::Z);
        let side = forward.cross(Vec3::Y).normalize_or(Vec3::X);
        Self { normal: side.cross(forward).normalize_or(Vec3::Y), distance: 0.0, precise_position: None, snapline: None }
    }

    /// Accumulates movement without rounding away small planet-chart displacements.
    pub fn translate(&mut self, position: Vec3, delta: Vec3) -> Vec3 {
        // A collision, phase exit or network correction replaces the rounded position.
        let start = self.position(position);
        let end = start + delta.as_dvec3();
        self.precise_position = Some(end);
        end.as_vec3()
    }

    pub fn position(&self, rounded: Vec3) -> DVec3 {
        self.precise_position.filter(|p| p.as_vec3() == rounded)
            .unwrap_or(rounded.as_dvec3())
    }

    pub fn rebase(&mut self, position: Vec3) {
        self.precise_position = Some(position.as_dvec3());
    }

    /// Advance one collision substep. Paths rotate velocity without adding speed;
    /// gravity, homing and ricochets can still change the incident velocity.
    pub fn advance(&mut self, velocity: &mut Vec3, dt: f32, path: u8) -> Vec3 {
        let distance = velocity.length() * dt;
        let angle = match path {
            1 => distance / CURVE_RADIUS,
            2 => {
                let phase = std::f32::consts::TAU / WEAVE_LENGTH;
                WEAVE_ANGLE * (((self.distance + distance) * phase).sin()
                    - (self.distance * phase).sin())
            }
            3 => distance / ORBIT_RADIUS,
            _ => return *velocity * dt,
        };
        self.distance = (self.distance + distance) % WEAVE_LENGTH;
        // Integrate the circular arc, rather than taking a tangent step that
        // grows the orbit on every tick. Parallel motion is unaffected.
        let half = angle * 0.5;
        let sinc = if half.abs() < 0.0001 { 1.0 } else { half.sin() / half };
        let parallel = self.normal * velocity.dot(self.normal);
        let perpendicular = *velocity - parallel;
        let travel = (parallel + Quat::from_axis_angle(self.normal, half) * perpendicular * sinc) * dt;
        *velocity = Quat::from_axis_angle(self.normal, angle) * *velocity;
        travel
    }
}
