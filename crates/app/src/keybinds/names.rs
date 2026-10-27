//! Key and mouse binding names for the bindings file and the controls screen.

use winit::{event::MouseButton, keyboard::KeyCode};

/// One physical input an action can be bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Binding {
    Key(KeyCode),
    Mouse(MouseButton),
}

macro_rules! keys {
    ($($key:ident),* $(,)?) => {
        /// Keys a binding file may name; anything else is dropped on load.
        const KEYS: &[KeyCode] = &[$(KeyCode::$key),*];
    };
}

keys!(
    KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM,
    KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8,
    Numpad9, NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide, NumpadDecimal,
    NumpadEnter, ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    ShiftLeft, ShiftRight, ControlLeft, ControlRight, AltLeft, AltRight,
    Space, Tab, CapsLock, Enter, Backspace, Insert, Delete, Home, End, PageUp,
    PageDown, Backquote, Minus, Equal, BracketLeft, BracketRight, Backslash,
    Semicolon, Quote, Comma, Period, Slash,
);

impl Binding {
    /// Stable name written to the bindings file.
    pub fn name(self) -> String {
        match self {
            Self::Key(code) => format!("{code:?}"),
            Self::Mouse(MouseButton::Left) => "MouseLeft".to_owned(),
            Self::Mouse(MouseButton::Right) => "MouseRight".to_owned(),
            Self::Mouse(MouseButton::Middle) => "MouseMiddle".to_owned(),
            Self::Mouse(MouseButton::Back) => "MouseBack".to_owned(),
            Self::Mouse(MouseButton::Forward) => "MouseForward".to_owned(),
            Self::Mouse(MouseButton::Other(n)) => format!("Mouse{n}"),
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        let mouse = match name {
            "MouseLeft" => Some(MouseButton::Left),
            "MouseRight" => Some(MouseButton::Right),
            "MouseMiddle" => Some(MouseButton::Middle),
            "MouseBack" => Some(MouseButton::Back),
            "MouseForward" => Some(MouseButton::Forward),
            _ => name.strip_prefix("Mouse").and_then(|n| n.parse().ok()).map(MouseButton::Other),
        };
        mouse.map(Self::Mouse).or_else(|| {
            KEYS.iter().copied().find(|code| format!("{code:?}") == name).map(Self::Key)
        })
    }

    /// Short label for buttons and HUD hints.
    pub fn label(self) -> String {
        let code = match self {
            Self::Key(code) => code,
            Self::Mouse(MouseButton::Left) => return "LMB".to_owned(),
            Self::Mouse(MouseButton::Right) => return "RMB".to_owned(),
            Self::Mouse(MouseButton::Middle) => return "MMB".to_owned(),
            Self::Mouse(MouseButton::Back) => return "Mouse 4".to_owned(),
            Self::Mouse(MouseButton::Forward) => return "Mouse 5".to_owned(),
            Self::Mouse(MouseButton::Other(n)) => return format!("Mouse {n}"),
        };
        let fixed = match code {
            KeyCode::ShiftLeft => "Left Shift",
            KeyCode::ShiftRight => "Right Shift",
            KeyCode::ControlLeft => "Left Ctrl",
            KeyCode::ControlRight => "Right Ctrl",
            KeyCode::AltLeft => "Left Alt",
            KeyCode::AltRight => "Right Alt",
            KeyCode::CapsLock => "Caps Lock",
            KeyCode::BracketLeft => "[",
            KeyCode::BracketRight => "]",
            KeyCode::Backquote => "`",
            KeyCode::Minus => "-",
            KeyCode::Equal => "=",
            KeyCode::Backslash => "\\",
            KeyCode::Semicolon => ";",
            KeyCode::Quote => "'",
            KeyCode::Comma => ",",
            KeyCode::Period => ".",
            KeyCode::Slash => "/",
            KeyCode::PageUp => "Page Up",
            KeyCode::PageDown => "Page Down",
            _ => "",
        };
        if !fixed.is_empty() {
            return fixed.to_owned();
        }
        let name = format!("{code:?}");
        for (prefix, shown) in [("Key", ""), ("Digit", ""), ("Arrow", ""), ("Numpad", "Num ")] {
            if let Some(rest) = name.strip_prefix(prefix) {
                return format!("{shown}{rest}");
            }
        }
        name
    }
}
