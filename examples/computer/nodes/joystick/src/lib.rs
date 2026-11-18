use pad::{DOWN, FIRE, LEFT, RIGHT, UP, button};
use parts::{BODY, GLASS, LIT, PIN_STEP, SILK, board, pin_names, round_rect};
use patina_sdk::{Key, Mode, Pointer, Ui, node};

/// The cap of the fire button when it is up.
const CAP: [u8; 3] = [101, 82, 25];
/// The size of the board.
const SIZE: [f32; 2] = [176.0, 88.0];
/// The inputs on the left edge, in order.
const PINS: [&str; 3] = ["bus", "port", "address"];

/// The buttons of the pad: the bit, the top left corner and the size. The arrows make a cross.
const PAD: [(u8, [f32; 2], [f32; 2]); 5] = [
    (UP, [74.0, 12.0], [20.0, 22.0]),
    (DOWN, [74.0, 54.0], [20.0, 22.0]),
    (LEFT, [52.0, 34.0], [22.0, 20.0]),
    (RIGHT, [94.0, 34.0], [22.0, 20.0]),
    (FIRE, [128.0, 24.0], [32.0, 32.0]),
];
/// The arrow on each arm of the cross, in the order of `PAD`.
const ARROWS: [&str; 4] = ["M 84 17 L 89 24 L 79 24 Z", "M 84 71 L 89 64 L 79 64 Z", "M 57 44 L 64 39 L 64 49 Z", "M 111 44 L 104 39 L 104 49 Z"];

/// A joystick on IO ports of the CPU. Click the pad, then hold the arrow keys or W, A, S and D,
/// and Space for fire. A mouse press on a button of the pad holds it too, and so do the keys of the
/// `monitor` view (the `pad` crate). LD at port `address`
/// gives the x axis: 1 for right, 255 (-1) for left, else 0. LD at `address + 1` gives the y axis:
/// 1 for down, 255 for up. LD at `address + 2` gives the bits of the buttons: 1 up, 2 down,
/// 4 left, 8 right and 16 fire. At the other ports, the byte on `bus` goes through.
#[node(color = "#59366f", free_layout)]
pub struct Joystick {
    keys: u8,
    mouse: u8,
}

impl Joystick {
    pub fn init() -> Self {
        Joystick { keys: 0, mouse: 0 }
    }

    pub fn process(&mut self, bus: &u8, port: &u8, address: u8) -> u8 {
        let held = self.keys | self.mouse | pad::held();
        let axis = |minus: u8, plus: u8| match (held & minus != 0, held & plus != 0) {
            (false, true) => 1,
            (true, false) => 255,
            _ => 0,
        };
        match port.wrapping_sub(address) {
            0 => axis(LEFT, RIGHT),
            1 => axis(UP, DOWN),
            2 => held,
            _ => *bus,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let mut pad = ui.canvas("Pad", SIZE);
        pad.grab();
        for key in pad.keys() {
            match key {
                Key::Down(name) => self.keys |= button(name),
                Key::Up(name) => self.keys &= !button(name),
            }
        }
        if let Some(&last) = pad.pointer().last() {
            self.mouse = match last {
                Pointer::Down(p) | Pointer::Move(p) => hit(p),
                Pointer::Up(_) => 0,
            };
        }
        let held = self.keys | self.mouse | pad::held();
        let color = |bit: u8, off: [u8; 3]| if held & bit != 0 { LIT } else { off };
        board(&mut pad, SIZE);
        pin_names(&mut pad, 20.0, &["bus", "port", "adr"]);
        // The cross of the arrows, with a lit arm for each held arrow.
        round_rect(&mut pad, [72.0, 10.0], [24.0, 68.0], 4.0, GLASS);
        round_rect(&mut pad, [50.0, 32.0], [68.0, 24.0], 4.0, GLASS);
        pad.rect([74.0, 34.0], [20.0, 20.0], BODY, true);
        for ((bit, at, size), arrow) in PAD.iter().zip(ARROWS) {
            round_rect(&mut pad, *at, *size, 3.0, color(*bit, BODY));
            pad.path(arrow, if held & bit != 0 { GLASS } else { SILK }, true);
        }
        pad.circle([144.0, 40.0], 17.0, GLASS, true);
        pad.circle([144.0, 40.0], 14.0, color(FIRE, CAP), true);
        pad.text([129.0, 76.0], SILK, "FIRE");
        drop(pad);
        for (i, key) in PINS.iter().enumerate() {
            ui.pin(key, [0.0, 20.0 + i as f32 * PIN_STEP]);
        }
        ui.pin("out", [SIZE[0], 44.0]);
        if ui.mode() != Mode::Compact {
            ui.label("Click the pad, then hold");
            ui.label("the arrow keys and Space.");
        }
    }
}

/// The bit of the button of the pad at a point, or 0.
fn hit([x, y]: [f32; 2]) -> u8 {
    PAD.iter().find(|(_, at, size)| x >= at[0] && x < at[0] + size[0] && y >= at[1] && y < at[1] + size[1]).map_or(0, |b| b.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ports_give_the_axes_and_the_buttons() {
        let mut stick = Joystick::init();
        assert_eq!([14, 15, 16].map(|p| stick.process(&7, &p, 14)), [0, 0, 0]);
        stick.keys = button("ArrowLeft") | button("KeyS") | button("Space");
        assert_eq!([14, 15, 16].map(|p| stick.process(&7, &p, 14)), [255, 1, LEFT | DOWN | FIRE]);
        assert_eq!(stick.process(&7, &13, 14), 7, "port 13 passes the bus");
        stick.mouse = RIGHT;
        assert_eq!(stick.process(&7, &14, 14), 0, "left and right together give 0");
        stick.keys = 0;
        pad::press(&[Key::Down("ArrowUp".to_owned())]);
        assert_eq!(stick.process(&7, &16, 14), UP | RIGHT, "a view holds a button too");
    }

    #[test]
    fn the_pad_reads_the_keys_and_the_mouse() {
        let mut stick = Joystick::init();
        let mut ui = Ui::new(0, "0 key down ArrowUp\n0 key down KeyD\n0 key up ArrowUp\n0 down 144 40");
        stick.ui(&mut ui);
        assert_eq!((stick.keys, stick.mouse), (RIGHT, FIRE));
        assert!(ui.tree().starts_with("canvas\tPad\t176\t88\tgrab\tkeys\t"));
        stick.ui(&mut Ui::new(0, "0 move 84 20\n0 up 84 20"));
        assert_eq!(stick.mouse, 0);
        stick.ui(&mut Ui::new(0, "0 down 84 20"));
        assert_eq!(stick.mouse, UP);
        assert_eq!(hit([200.0, 0.0]), 0);
    }
}
