//! Gamepad polling: device events become pad binding edges once per frame.

mod state;
#[cfg(test)]
mod tests;

use crate::keybinds::PadButton;
use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};
use glam::Vec2;
use state::{PadEvent, PadState};
use std::time::Instant;

/// Full-deflection turn rate at the default mouse sensitivity, radians per second.
const LOOK_RATE: f32 = 3.0;
const DEFAULT_MOUSE_SENSITIVITY: f32 = 0.0025;
/// A hitch turns the camera as if the frame were this long, so the view cannot spin.
const MAX_LOOK_STEP: f32 = 0.05;

pub struct Gamepads {
    /// `None` when the platform backend failed; the game then behaves as with no pad.
    gilrs: Option<Gilrs>,
    /// Sticks are read from the pad that sent the latest event.
    active: Option<GamepadId>,
    state: PadState,
    last_poll: Option<Instant>,
    step: f32,
}

impl Gamepads {
    pub fn open() -> Self {
        let gilrs = Gilrs::new().map_err(|e| eprintln!("[gamepad] unavailable: {e}")).ok();
        Self { gilrs, active: None, state: PadState::default(), last_poll: None, step: 0.0 }
    }

    /// Drains device events and returns this frame's binding edges.
    /// Unfocused, events are dropped and everything held releases.
    pub fn poll(&mut self, focused: bool) -> Vec<(PadButton, bool)> {
        let now = Instant::now();
        self.step = self.last_poll.map_or(0.0, |t| (now - t).as_secs_f32().min(MAX_LOOK_STEP));
        self.last_poll = Some(now);
        let mut out = Vec::new();
        let Some(gilrs) = self.gilrs.as_mut() else { return out };
        while let Some(event) = gilrs.next_event() {
            let pad = match event.event {
                EventType::ButtonPressed(button, _) => pad_button(button).map(|b| PadEvent::Button(b, true)),
                EventType::ButtonReleased(button, _) => pad_button(button).map(|b| PadEvent::Button(b, false)),
                EventType::Disconnected => {
                    if self.active == Some(event.id) {
                        self.active = None;
                    }
                    Some(PadEvent::Lost)
                }
                _ => None,
            };
            if matches!(event.event, EventType::ButtonPressed(..) | EventType::AxisChanged(..)) {
                self.active = Some(event.id);
            }
            if let Some(pad) = pad.filter(|_| focused) {
                self.state.feed(pad, &mut out);
            }
        }
        let sticks = self.active.filter(|_| focused).and_then(|id| gilrs.connected_gamepad(id)).map(|pad| {
            let stick = |x, y| Vec2::new(pad.value(x), pad.value(y));
            PadEvent::Sticks {
                left: stick(Axis::LeftStickX, Axis::LeftStickY),
                right: stick(Axis::RightStickX, Axis::RightStickY),
            }
        });
        self.state.feed(sticks.unwrap_or(PadEvent::Lost), &mut out);
        out
    }

    /// This frame's camera turn in radians (x yaw right, y pitch up), scaled like the mouse.
    pub fn look(&self, mouse_sensitivity: f32) -> Vec2 {
        self.state.look() * LOOK_RATE * (mouse_sensitivity / DEFAULT_MOUSE_SENSITIVITY) * self.step
    }
}

fn pad_button(button: Button) -> Option<PadButton> {
    Some(match button {
        Button::South => PadButton::South,
        Button::East => PadButton::East,
        Button::West => PadButton::West,
        Button::North => PadButton::North,
        Button::LeftTrigger => PadButton::LeftBumper,
        Button::RightTrigger => PadButton::RightBumper,
        Button::LeftTrigger2 => PadButton::LeftTrigger,
        Button::RightTrigger2 => PadButton::RightTrigger,
        Button::Select => PadButton::Select,
        Button::Start => PadButton::Start,
        Button::LeftThumb => PadButton::LeftStick,
        Button::RightThumb => PadButton::RightStick,
        Button::DPadUp => PadButton::DPadUp,
        Button::DPadDown => PadButton::DPadDown,
        Button::DPadLeft => PadButton::DPadLeft,
        Button::DPadRight => PadButton::DPadRight,
        _ => return None,
    })
}
