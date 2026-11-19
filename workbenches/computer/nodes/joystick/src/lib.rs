use pad::{DOWN, FIRE, LEFT, RIGHT, UP, button};
use parts::{BODY, GLASS, LIT, SILK, TEXT, edges, led, package, round_rect};
use patina_sdk::{Key, Mode, Pointer, Ui, node};

/// The cap of the fire button when it is up.
const CAP: [u8; 3] = [101, 82, 25];
/// The width of the drawing of the chip.
const WIDTH: f32 = 224.0;

/// The buttons of the pad: the bit, the top left corner and the size. The arrows make a cross.
const PAD: [(u8, [f32; 2], [f32; 2]); 5] = [
    (UP, [97.0, 70.0], [30.0, 34.0]),
    (DOWN, [97.0, 136.0], [30.0, 34.0]),
    (LEFT, [62.0, 105.0], [34.0, 30.0]),
    (RIGHT, [128.0, 105.0], [34.0, 30.0]),
    (FIRE, [82.0, 220.0], [60.0, 60.0]),
];
/// The arrow on each arm of the cross, in the order of `PAD`.
const ARROWS: [&str; 4] = ["M 112 78 L 120 92 L 104 92 Z", "M 112 162 L 120 148 L 104 148 Z", "M 70 120 L 84 112 L 84 128 Z", "M 154 120 L 140 112 L 140 128 Z"];

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
        let e = edges(&[8, 8, 8], &[8]);
        let mut pad = ui.canvas("Pad", [WIDTH, e.height]);
        // A press on a button acts at once. A drag on the board moves the node (4.3).
        for (_, at, size) in PAD {
            pad.grab_rect(at, size);
        }
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
        package(&mut pad, &e, WIDTH);
        pad.text([46.0, 24.0], TEXT, "PAD");
        pad.text([84.0, 24.0], SILK, "JOYSTICK");
        // The cross of the arrows, with a lit arm for each held arrow.
        round_rect(&mut pad, [94.0, 67.0], [36.0, 106.0], 5.0, GLASS);
        round_rect(&mut pad, [59.0, 102.0], [106.0, 36.0], 5.0, GLASS);
        pad.rect([96.0, 104.0], [32.0, 32.0], BODY, true);
        for ((bit, at, size), arrow) in PAD.iter().zip(ARROWS) {
            round_rect(&mut pad, *at, *size, 4.0, color(*bit, BODY));
            pad.path(arrow, if held & bit != 0 { GLASS } else { SILK }, true);
        }
        pad.circle([112.0, 250.0], 32.0, GLASS, true);
        pad.circle([112.0, 250.0], 26.0, color(FIRE, CAP), true);
        pad.text([97.0, 302.0], SILK, "FIRE");
        // An LED for each button bit of the value that the port gives.
        for (i, (bit, name)) in [(UP, "U"), (DOWN, "D"), (LEFT, "L"), (RIGHT, "R"), (FIRE, "F")].into_iter().enumerate() {
            let x = 68.0 + i as f32 * 22.0;
            led(&mut pad, [x, 318.0], 4.0, held & bit != 0);
            pad.text([x - 4.0, 338.0], SILK, name);
        }
        drop(pad);
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
        let mut ui = Ui::new(0, "0 key down ArrowUp\n0 key down KeyD\n0 key up ArrowUp\n0 down 112 250");
        stick.ui(&mut ui);
        assert_eq!((stick.keys, stick.mouse), (RIGHT, FIRE));
        assert!(ui.tree().starts_with("canvas\tPad\t224\t354\tgrab 97 70 30 34\tgrab 97 136 30 34\tgrab 62 105 34 30\tgrab 128 105 34 30\tgrab 82 220 60 60\tkeys\t"));
        stick.ui(&mut Ui::new(0, "0 move 112 85\n0 up 112 85"));
        assert_eq!(stick.mouse, 0);
        stick.ui(&mut Ui::new(0, "0 down 112 85"));
        assert_eq!(stick.mouse, UP);
        assert_eq!(hit([200.0, 0.0]), 0);
    }
}
