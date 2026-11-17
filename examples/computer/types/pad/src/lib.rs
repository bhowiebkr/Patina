//! The buttons of the joystick. A view, for example `monitor`, holds buttons with the keys of its
//! viewport, and the `joystick` node reads them. A key is not an edit, so it adds no undo step.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use patina_sdk::Key;

pub const UP: u8 = 1;
pub const DOWN: u8 = 2;
pub const LEFT: u8 = 4;
pub const RIGHT: u8 = 8;
pub const FIRE: u8 = 16;

/// The buttons that the views hold. `patina_sdk::shared` keeps one for each project.
struct Held(AtomicU8);

fn shared() -> &'static AtomicU8 {
    static HELD: OnceLock<&'static Held> = OnceLock::new();
    &HELD.get_or_init(|| patina_sdk::shared(|| Held(AtomicU8::new(0)))).0
}

/// The bit of a key name, or 0 for a key that the joystick does not use.
pub fn button(name: &str) -> u8 {
    match name {
        "ArrowUp" | "KeyW" => UP,
        "ArrowDown" | "KeyS" => DOWN,
        "ArrowLeft" | "KeyA" => LEFT,
        "ArrowRight" | "KeyD" => RIGHT,
        "Space" | "Enter" => FIRE,
        _ => 0,
    }
}

/// The bits of the buttons that the views hold.
pub fn held() -> u8 {
    shared().load(Ordering::Relaxed)
}

/// Holds a button for each key that goes down and lets it go when the key comes up.
pub fn press(keys: &[Key]) {
    for key in keys {
        match key {
            Key::Down(name) => shared().fetch_or(button(name), Ordering::Relaxed),
            Key::Up(name) => shared().fetch_and(!button(name), Ordering::Relaxed),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_hold_and_let_go_of_the_buttons() {
        let down = |name: &str| Key::Down(name.to_owned());
        press(&[down("ArrowUp"), down("Space"), down("KeyQ"), Key::Up("ArrowUp".to_owned())]);
        assert_eq!(held(), FIRE);
        press(&[Key::Up("Enter".to_owned())]);
        assert_eq!(held(), 0);
    }
}
