use patina_sdk::{Canvas, Pointer, Ui, node};
use scope::{LABEL, RIM, TRACE, panel, readout};

/// The size of the panel, the middle of the knob and its radius.
const SIZE: [f32; 2] = [132.0, 124.0];
const CENTER: [f32; 2] = [66.0, 70.0];
const RADIUS: f32 = 24.0;
/// The drag in pixels that turns the knob from 0 to 1.
const TRAVEL: f32 = 150.0;
/// The skirt and the cap of the knob.
const SKIRT: [u8; 3] = [70, 82, 96];
const CAP: [u8; 3] = [22, 26, 32];

/// A knob on the front panel of a lab instrument. It gives a value from 0 to 1. Drag up or to the
/// right to turn it up. The **value** parameter keeps the value.
#[node(free_layout)]
pub struct Knob {
    value: f64,
    /// The pointer place and the value at the start of a drag.
    drag: Option<([f32; 2], f64)>,
}

impl Knob {
    pub fn init() -> Self {
        Knob { value: 0.0, drag: None }
    }

    pub fn process(&mut self, value: f64) -> f64 {
        self.value = value.clamp(0.0, 1.0);
        self.value
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let before = self.value;
        {
            let mut c = ui.canvas(&format!("Knob {:.2}", self.value), SIZE);
            c.grab();
            for p in c.pointer() {
                match (*p, self.drag) {
                    (Pointer::Down(at), _) => self.drag = Some((at, self.value)),
                    (Pointer::Move([x, y]), Some(([x0, y0], start))) => self.value = turn(start, x - x0 + y0 - y),
                    (Pointer::Up(_), _) => self.drag = None,
                    _ => {}
                }
            }
            draw(&mut c, self.value);
        }
        if self.value != before {
            ui.set_param("value", self.value);
        }
        ui.pin("out", [SIZE[0], CENTER[1]]);
    }
}

/// The value after a drag of `pixels` up or to the right from `start`, in steps of 0.01.
fn turn(start: f64, pixels: f32) -> f64 {
    ((start + f64::from(pixels / TRAVEL)) * 100.0).round().clamp(0.0, 100.0) / 100.0
}

/// The point at the distance `r` from the middle for `value`. The scale goes from 7:30 to 4:30 on
/// a clock face, as on most knobs.
fn at(value: f64, r: f32) -> [f32; 2] {
    let a = (225.0 - 270.0 * value as f32).to_radians();
    [CENTER[0] + r * a.cos(), CENTER[1] - r * a.sin()]
}

fn draw(c: &mut Canvas, value: f64) {
    panel(c, SIZE, "KNOB");
    readout(c, SIZE[0], &format!("{value:.2}"));
    // The scale has 11 ticks. The ticks up to the value are lit, as on an LED ring.
    for i in 0..=10 {
        let v = f64::from(i) / 10.0;
        let end = RADIUS + if i % 5 == 0 { 11.0 } else { 8.0 };
        c.line(at(v, RADIUS + 5.0), at(v, end), if v <= value + 1e-9 { TRACE } else { RIM });
    }
    for (v, mark) in [(0.0, "0"), (1.0, "1")] {
        let [x, y] = at(v, RADIUS + 17.0);
        c.text([x - 3.0, y + 4.0], LABEL, mark);
    }
    c.circle(CENTER, RADIUS, SKIRT, true);
    c.circle(CENTER, RADIUS - 5.0, CAP, true);
    c.line(at(value, 6.0), at(value, RADIUS - 2.0), TRACE);
    let dot = at(value, RADIUS - 9.0);
    c.circle(dot, 2.5, TRACE, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_turns_the_knob() {
        assert_eq!(turn(0.5, 30.0), 0.7);
        assert_eq!([turn(0.9, 60.0), turn(0.1, -60.0)], [1.0, 0.0]);
        let [x, y] = at(0.5, 10.0);
        assert!((x - CENTER[0]).abs() < 1e-4 && (y - (CENTER[1] - 10.0)).abs() < 1e-4, "0.5 is at the top");
        let mut knob = Knob::init();
        knob.process(0.25);
        // A drag of 15 pixels up and 15 pixels to the right adds 0.2.
        let mut ui = Ui::new(0, "0 down 66 70\n0 move 81 55\n0 up 81 55");
        knob.ui(&mut ui);
        assert_eq!((knob.value, knob.drag), (0.45, None));
        let mut ui = Ui::new(0, "");
        knob.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tKnob 0.45\t132\t124\t"), "{}", ui.tree());
    }
}
