//! Keyboard input collection for one rendered frame.

use macroquad::prelude::{KeyCode, is_key_down, is_key_pressed};

const ACTION_KEYS: [KeyCode; 20] = [
    KeyCode::L,
    KeyCode::D,
    KeyCode::F,
    KeyCode::A,
    KeyCode::B,
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::O,
    KeyCode::C,
    KeyCode::M,
    KeyCode::T,
    KeyCode::P,
    KeyCode::R,
    KeyCode::H,
    KeyCode::I,
    KeyCode::E,
    KeyCode::G,
    KeyCode::K,
];

#[derive(Default)]
pub struct FrameInput {
    pub keys_pressed: Vec<KeyCode>,
    pub restart_with_recovered_seed: bool,
}

pub fn capture_frame_input() -> FrameInput {
    let mut keys_pressed = Vec::with_capacity(ACTION_KEYS.len() + 2);

    if is_key_pressed(KeyCode::Space) {
        keys_pressed.push(KeyCode::Space);
    }
    if is_key_pressed(KeyCode::Right) {
        keys_pressed.push(KeyCode::Right);
    }
    for key in ACTION_KEYS {
        if is_key_pressed(key) {
            keys_pressed.push(key);
        }
    }

    let restart_with_recovered_seed = (is_key_down(KeyCode::LeftShift)
        || is_key_down(KeyCode::RightShift))
        && is_key_pressed(KeyCode::K);

    FrameInput { keys_pressed, restart_with_recovered_seed }
}
