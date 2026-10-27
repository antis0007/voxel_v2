//! Pad state without the device layer: stick shaping and binding edges.

use crate::keybinds::PadButton;
use glam::Vec2;

/// Rested sticks drift up to about 0.1 off centre.
const LOOK_DEAD_ZONE: f32 = 0.15;
/// Power on look magnitude: fine aim near centre, full speed at the rim.
const LOOK_CURVE: f32 = 2.0;
const MOVE_DEAD_ZONE: f32 = 0.25;
/// Hysteresis on the move stick, so a stick resting near the threshold does not chatter.
const MOVE_PRESS: f32 = 0.35;
const MOVE_RELEASE: f32 = 0.2;

/// Zeroes deflection inside `dead` and rescales the rest to reach 1 at the rim.
pub fn radial_dead_zone(v: Vec2, dead: f32) -> Vec2 {
    let len = v.length();
    if !len.is_finite() || len <= dead {
        return Vec2::ZERO;
    }
    v / len * ((len - dead) / (1.0 - dead)).min(1.0)
}

/// Look stick after the dead zone and response curve, magnitude 0 to 1.
pub fn shape_look(v: Vec2) -> Vec2 {
    let v = radial_dead_zone(v, LOOK_DEAD_ZONE);
    let len = v.length();
    if len == 0.0 { Vec2::ZERO } else { v / len * len.powf(LOOK_CURVE) }
}

/// One device observation, already mapped off the backend.
pub enum PadEvent {
    Button(PadButton, bool),
    /// Stick positions with +y up, sampled once per frame.
    Sticks { left: Vec2, right: Vec2 },
    /// Pad gone or window unfocused: everything held releases.
    Lost,
}

#[derive(Default)]
pub struct PadState {
    held: u32,
    look: Vec2,
}

impl PadState {
    /// Applies `event`, pushing each binding that changed state.
    pub fn feed(&mut self, event: PadEvent, out: &mut Vec<(PadButton, bool)>) {
        match event {
            PadEvent::Button(button, pressed) => self.set(button, pressed, out),
            PadEvent::Sticks { left, right } => {
                self.look = shape_look(right);
                let m = radial_dead_zone(left, MOVE_DEAD_ZONE);
                for (button, along) in [
                    (PadButton::LeftStickUp, m.y),
                    (PadButton::LeftStickDown, -m.y),
                    (PadButton::LeftStickLeft, -m.x),
                    (PadButton::LeftStickRight, m.x),
                ] {
                    let threshold = if self.held & button.bit() != 0 { MOVE_RELEASE } else { MOVE_PRESS };
                    self.set(button, along > threshold, out);
                }
            }
            PadEvent::Lost => {
                self.look = Vec2::ZERO;
                for &button in PadButton::ALL {
                    self.set(button, false, out);
                }
            }
        }
    }

    fn set(&mut self, button: PadButton, pressed: bool, out: &mut Vec<(PadButton, bool)>) {
        if (self.held & button.bit() != 0) != pressed {
            self.held ^= button.bit();
            out.push((button, pressed));
        }
    }

    /// Shaped look stick, magnitude 0 to 1.
    pub fn look(&self) -> Vec2 {
        self.look
    }
}
