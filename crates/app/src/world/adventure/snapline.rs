//! A finite tether with one hit and one refund per paid cast.
use glam::Vec3;

pub const SPELL: u32 = 99;
pub const REACH: f32 = 5.5;
pub const LIFE: f32 = 1.0;
const RETURN_SPEED: f32 = 27.0;
const CATCH_RADIUS: f32 = 0.16;
const TETHER_LIMIT: f32 = REACH + 2.0;
pub(super) const REFUND_COLOR: [f32; 3] = [1.0, 0.78, 0.34];

#[derive(Clone, Copy)]
#[cfg_attr(feature = "multiplayer", derive(serde::Serialize, serde::Deserialize))]
pub struct Tether {
    pub anchor: Vec3,
    pub returning: bool,
    pub connected: bool,
    reach_left: f32,
    paid: f32,
}

impl Tether {
    pub fn new(anchor: Vec3, paid: f32) -> Self {
        Self { anchor, returning: false, connected: false, reach_left: REACH, paid }
    }

    pub fn hand(eye: Vec3, player: &crate::player::Player) -> Vec3 {
        eye + player.right_dir() * 0.22 - player.up_dir() * 0.21
    }

    pub fn travel(&mut self, pos: Vec3, velocity: &mut Vec3, dt: f32) -> Vec3 {
        self.returning |= self.reach_left <= 0.0;
        if self.returning {
            let to_hand = self.anchor - pos;
            *velocity = to_hand.normalize_or_zero() * RETURN_SPEED;
            return to_hand.clamp_length_max(RETURN_SPEED * dt);
        }
        let distance = (velocity.length() * dt).min(self.reach_left);
        self.reach_left -= distance;
        velocity.normalize_or_zero() * distance
    }

    pub fn connect(&mut self) -> f32 {
        self.connected = true;
        self.returning = true;
        std::mem::take(&mut self.paid)
    }

    pub(crate) fn color(&self, cast: [f32; 3]) -> [f32; 3] {
        if self.connected { REFUND_COLOR } else { cast }
    }

    pub fn caught(&self, pos: Vec3) -> bool {
        self.returning && pos.distance_squared(self.anchor) <= CATCH_RADIUS * CATCH_RADIUS
    }

    pub fn detached(&self, pos: Vec3) -> bool {
        pos.distance_squared(self.anchor) > TETHER_LIMIT * TETHER_LIMIT
    }
}
