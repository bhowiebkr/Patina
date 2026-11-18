use calc::{BEZEL, case, fit};
use patina_sdk::{Ui, node};

/// The size of the node, the number of lines on the paper, and the digits of a line.
const SIZE: [f32; 2] = [150.0, 168.0];
const LINES: usize = 6;
const DIGITS: usize = 10;
/// The paper and the ink of the printer.
const PAPER: [u8; 3] = [236, 232, 218];
const INK: [u8; 3] = [52, 56, 70];
/// The red ink of the newest line, as on the two-color ribbon of a printing calculator.
const RED: [u8; 3] = [178, 44, 40];

/// Prints each new value on the paper tape of a printing calculator. The tape keeps the last 6
/// values, and the newest value is red.
#[node(free_layout)]
pub struct Tape {
    lines: Vec<f64>,
}

impl Tape {
    pub fn init() -> Self {
        Tape { lines: Vec::with_capacity(LINES + 1) }
    }

    pub fn process(&mut self, value: &f64) {
        // A run starts for each edit, so the tape prints only a value that changed.
        if self.lines.last().is_none_or(|last| last.to_bits() != value.to_bits()) {
            self.lines.push(*value);
            if self.lines.len() > LINES {
                self.lines.remove(0);
            }
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let last = self.lines.last().map_or_else(String::new, |v| fit(*v, DIGITS));
            let mut c = ui.canvas(&format!("Tape {last}"), SIZE);
            case(&mut c, [SIZE[0], 40.0], "TAPE");
            // The slot of the printer, and the paper that comes out of it.
            c.rect([10.0, 28.0], [SIZE[0] - 20.0, 5.0], BEZEL, true);
            c.rect([16.0, 33.0], [SIZE[0] - 32.0, SIZE[1] - 41.0], PAPER, true);
            tear(&mut c, SIZE[1] - 8.0);
            let top = 33.0 + 18.0 * (LINES - self.lines.len()) as f32;
            for (i, v) in self.lines.iter().enumerate() {
                let text = fit(*v, DIGITS);
                let y = top + 18.0 * i as f32 + 14.0;
                let newest = i + 1 == self.lines.len();
                // A digit is about 6.6 pixels wide, so the numbers end at the same right edge.
                c.text([SIZE[0] - 34.0 - text.len() as f32 * 6.6, y], if newest { RED } else { INK }, &text);
                c.text([SIZE[0] - 30.0, y], if newest { RED } else { INK }, if newest { "*" } else { "+" });
            }
        }
        ui.pin("value", [0.0, 20.0]);
    }
}

/// The torn lower edge of the paper, as a row of small teeth.
fn tear(c: &mut patina_sdk::Canvas, y: f32) {
    let mut d = format!("M 16 {y}");
    let mut x = 16.0;
    while x < SIZE[0] - 16.0 {
        d += &format!(" L {} {} L {} {y}", x + 3.0, y + 4.0, (x + 6.0).min(SIZE[0] - 16.0));
        x += 6.0;
    }
    d += &format!(" L {} {} Z", SIZE[0] - 16.0, y - 1.0);
    c.path(&d, PAPER, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tape_prints_each_new_value() {
        let mut tape = Tape::init();
        for v in [1.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.5] {
            tape.process(&v);
        }
        assert_eq!(tape.lines, [2.0, 3.0, 4.0, 5.0, 6.0, 7.5], "a same value prints once");
        let mut ui = Ui::new(0, "");
        tape.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tTape 7.5\t150\t168\t"), "{}", ui.tree());
    }
}
