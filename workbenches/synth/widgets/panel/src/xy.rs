use crate::{DARK, EDGE, INK, MUTED, TEAL, inside, rounded};
use patina_sdk::{Key, Pointer, Ui};

/// A two-axis performance pad. Its caller maps normalized values to sound parameters.
#[derive(Default)]
pub struct XyPad {
    dragging: bool,
}

impl XyPad {
    pub fn show(&mut self, ui: &mut Ui, value: &mut [f32; 2]) -> bool {
        let before = *value;
        let mut c = ui.canvas(
            "Filter XY: left/right cutoff, up/down resonance",
            [544.0, 172.0],
        );
        c.grab();
        for event in c.pointer() {
            match *event {
                Pointer::Down(at) => {
                    self.dragging = inside(at, [18.0, 22.0, 508.0, 116.0]);
                    if self.dragging {
                        *value = from_pointer(at);
                    }
                }
                Pointer::Move(at) if self.dragging => *value = from_pointer(at),
                Pointer::Up(_) => self.dragging = false,
                _ => {}
            }
        }
        for event in c.keys() {
            if let Key::Down(key) = event {
                match key.as_str() {
                    "ArrowLeft" => value[0] -= 0.01,
                    "ArrowRight" => value[0] += 0.01,
                    "ArrowDown" => value[1] -= 0.01,
                    "ArrowUp" => value[1] += 0.01,
                    _ => {}
                }
            }
        }
        *value = value.map(|v| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                0.0
            }
        });
        rounded(&mut c, [0.0, 0.0, 544.0, 172.0], 6.0, DARK);
        for i in 0..=8 {
            let x = 18.0 + i as f32 * 63.5;
            c.line([x, 22.0], [x, 138.0], EDGE);
        }
        for i in 0..=4 {
            let y = 22.0 + i as f32 * 29.0;
            c.line([18.0, y], [526.0, y], EDGE);
        }
        let [x, y] = [18.0 + value[0] * 508.0, 138.0 - value[1] * 116.0];
        c.line([18.0, y], [526.0, y], TEAL);
        c.line([x, 22.0], [x, 138.0], TEAL);
        c.circle([x, y], 7.0, DARK, true);
        c.circle([x, y], 6.0, TEAL, false);
        c.circle([x, y], 2.0, INK, true);
        c.text([18.0, 15.0], MUTED, "MORE RESONANCE");
        c.text([18.0, 158.0], MUTED, "DARK / 60 Hz");
        c.text([394.0, 158.0], MUTED, "BRIGHT / 16 kHz");
        *value != before
    }
}

fn from_pointer([x, y]: [f32; 2]) -> [f32; 2] {
    [
        ((x - 18.0) / 508.0).clamp(0.0, 1.0),
        ((138.0 - y) / 116.0).clamp(0.0, 1.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pad_clamps_a_drag_and_accepts_keyboard_input() {
        let mut pad = XyPad::default();
        let mut value = [0.5, 0.5];
        assert!(pad.show(
            &mut Ui::new(2, "0 down 272 80\n0 move 1000 -20\n0 up 1000 -20"),
            &mut value
        ));
        assert_eq!(value, [1.0, 1.0]);
        assert!(!pad.show(&mut Ui::new(2, "0 move 18 138"), &mut value));
        assert!(pad.show(
            &mut Ui::new(2, "0 key down ArrowLeft\n0 key down ArrowDown"),
            &mut value
        ));
        assert_eq!(value, [0.99, 0.99]);
        assert_eq!(from_pointer([18.0, 138.0]), [0.0, 0.0]);
    }
}
