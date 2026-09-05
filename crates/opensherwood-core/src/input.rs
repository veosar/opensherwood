//! Canonical input events: the only way a player (or a test) acts on the world (ADR-0004).

use serde::{Deserialize, Serialize};

/// Mouse buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    /// Left.
    Left,
    /// Right.
    Right,
    /// Middle.
    Middle,
}

/// Physical keys (a small, stable subset; extended as gameplay needs them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Key {
    /// Escape.
    Escape,
    /// Space.
    Space,
    /// Shift.
    Shift,
    /// Control.
    Control,
    /// Alt.
    Alt,
    /// Tab.
    Tab,
    /// Enter.
    Enter,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Letter keys by physical position (US layout names).
    Letter(char),
    /// Digit row.
    Digit(u8),
    /// Function keys 1..12.
    Function(u8),
    /// Backspace (text fields).
    Backspace,
}

/// One input event in logical viewport coordinates (24.8 fixed point, `x256 = x * 256`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputEvent {
    /// Pointer moved to an absolute logical position.
    PointerMove {
        /// x in 24.8.
        x256: i32,
        /// y in 24.8.
        y256: i32,
    },
    /// Button pressed at the current pointer position.
    PointerDown {
        /// Button.
        button: Button,
    },
    /// Button released.
    PointerUp {
        /// Button.
        button: Button,
    },
    /// Wheel rotated (positive = away from the user).
    Wheel {
        /// Steps.
        delta: i32,
    },
    /// Key pressed.
    KeyDown {
        /// Key.
        key: Key,
    },
    /// Key released.
    KeyUp {
        /// Key.
        key: Key,
    },
}

impl InputEvent {
    /// Canonical byte encoding for hashing and replays (never derived from JSON).
    pub fn encode(&self, out: &mut Vec<u8>) {
        match *self {
            InputEvent::PointerMove { x256, y256 } => {
                out.push(1);
                out.extend_from_slice(&x256.to_le_bytes());
                out.extend_from_slice(&y256.to_le_bytes());
            }
            InputEvent::PointerDown { button } => {
                out.push(2);
                out.push(button_tag(button));
            }
            InputEvent::PointerUp { button } => {
                out.push(3);
                out.push(button_tag(button));
            }
            InputEvent::Wheel { delta } => {
                out.push(4);
                out.extend_from_slice(&delta.to_le_bytes());
            }
            InputEvent::KeyDown { key } => {
                out.push(5);
                encode_key(key, out);
            }
            InputEvent::KeyUp { key } => {
                out.push(6);
                encode_key(key, out);
            }
        }
    }
}

/// Stable tag of a button for canonical encodings.
#[must_use]
pub fn button_tag(button: Button) -> u8 {
    match button {
        Button::Left => 1,
        Button::Right => 2,
        Button::Middle => 3,
    }
}

/// Canonical encoding of a key (explicit tags, never enum discriminants).
pub fn encode_key(key: Key, out: &mut Vec<u8>) {
    match key {
        Key::Escape => out.push(1),
        Key::Space => out.push(2),
        Key::Shift => out.push(3),
        Key::Control => out.push(4),
        Key::Alt => out.push(5),
        Key::Tab => out.push(6),
        Key::Enter => out.push(7),
        Key::Up => out.push(8),
        Key::Down => out.push(9),
        Key::Left => out.push(10),
        Key::Right => out.push(11),
        Key::Backspace => out.push(12),
        Key::Letter(c) => {
            out.push(32);
            out.extend_from_slice(&(c as u32).to_le_bytes());
        }
        Key::Digit(d) => {
            out.push(33);
            out.push(d);
        }
        Key::Function(f) => {
            out.push(34);
            out.push(f);
        }
    }
}
