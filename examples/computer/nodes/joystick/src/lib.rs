use pad::{DOWN, FIRE, LEFT, RIGHT, UP, button};
use patina_sdk::{Key, Pointer, Ui, node};

const ON: [u8; 3] = [223, 166, 75];
const OFF: [u8; 3] = [64, 64, 61];
const EDGE: [u8; 3] = [102, 102, 97];
const TEXT: [u8; 3] = [178, 178, 173];

/// The buttons of the pad: the bit, the top left corner and the size.
const PAD: [(u8, [f32; 2], [f32; 2]); 5] = [
    (UP, [26.0, 4.0], [20.0, 18.0]),
    (DOWN, [26.0, 42.0], [20.0, 18.0]),
    (LEFT, [4.0, 23.0], [22.0, 18.0]),
    (RIGHT, [46.0, 23.0], [22.0, 18.0]),
    (FIRE, [80.0, 18.0], [28.0, 28.0]),
];

/// A joystick on IO ports of the CPU. Click the pad, then hold the arrow keys or W, A, S and D,
/// and Space for fire. A mouse press on a button of the pad holds it too, and so do the keys of the
/// `monitor` view (the `pad` crate). LD at port `address`
/// gives the x axis: 1 for right, 255 (-1) for left, else 0. LD at `address + 1` gives the y axis:
/// 1 for down, 255 for up. LD at `address + 2` gives the bits of the buttons: 1 up, 2 down,
/// 4 left, 8 right and 16 fire. At the other ports, the byte on `bus` goes through.
#[node(color = "#59366f")]
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
        let mut pad = ui.canvas("Pad", [112.0, 64.0]);
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
        for (bit, at, size) in PAD {
            pad.rect(at, size, if held & bit != 0 { ON } else { OFF }, true);
            pad.rect(at, size, EDGE, false);
        }
        pad.text([82.0, 60.0], TEXT, "FIRE");
        ui.label("Click the pad, then hold");
        ui.label("the arrow keys and Space.");
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
        let mut ui = Ui::new(0, "0 key down ArrowUp\n0 key down KeyD\n0 key up ArrowUp\n0 down 90 30");
        stick.ui(&mut ui);
        assert_eq!((stick.keys, stick.mouse), (RIGHT, FIRE));
        assert!(ui.tree().starts_with("canvas\tPad\t112\t64\tkeys\t"));
        stick.ui(&mut Ui::new(0, "0 move 30 10\n0 up 30 10"));
        assert_eq!(stick.mouse, 0);
        stick.ui(&mut Ui::new(0, "0 down 30 10"));
        assert_eq!(stick.mouse, UP);
        assert_eq!(hit([200.0, 0.0]), 0);
    }
}
