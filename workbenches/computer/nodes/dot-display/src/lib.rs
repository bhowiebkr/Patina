use parts::{SILK, TEXT, edges, package, window};
use patina_sdk::{Ui, node};

/// The width of the drawing of the chip and the size of a dot cell.
const WIDTH: f32 = 288.0;
const CELL: f32 = 12.0;
const ON: [u8; 3] = [238, 238, 235];
const OFF: [u8; 3] = [32, 32, 31];

/// A display of 16 x 16 dots, as the dot display of Digital Logic Sim 2. At a rising edge of
/// **clock**, **reset** clears the back buffer, or **write** sets the dot at **address** to
/// **pixel**. Then **refresh** copies the back buffer to the screen. The address is the row x 16
/// plus the column, and row 0 is at the bottom. The output is the dot at **address** on the
/// screen. When the screen changes, it writes each row as 4 hex digits, from the top row.
#[node(color = "#59366f", free_layout)]
pub struct DotDisplay {
    screen: [bool; 256],
    back: [bool; 256],
    clock: bool,
}

impl DotDisplay {
    pub fn init() -> Self {
        DotDisplay { screen: [false; 256], back: [false; 256], clock: false }
    }

    pub fn process(&mut self, address: &u8, pixel: &bool, reset: &bool, write: &bool, refresh: &bool, clock: &bool) -> bool {
        if *clock && !self.clock {
            if *reset {
                self.back = [false; 256];
            } else if *write {
                self.back[usize::from(*address)] = *pixel;
            }
            if *refresh && self.screen != self.back {
                self.screen = self.back;
                println!("dots {}", self.rows());
            }
        }
        self.clock = *clock;
        self.screen[usize::from(*address)]
    }

    /// The rows from the top, with column 0 in the high bit.
    fn rows(&self) -> String {
        let row = |y: usize| self.screen[y * 16..][..16].iter().fold(0u16, |n, &on| n << 1 | u16::from(on));
        (0..16).rev().map(|y| format!("{:04x}", row(y))).collect::<Vec<_>>().join(" ")
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let e = edges(&[8, 0, 0, 0, 0, 0], &[0]);
        let lit = self.screen.iter().filter(|&&on| on).count();
        let mut c = ui.canvas(&format!("Dot display, dots on: {lit}"), [WIDTH, e.height]);
        let [x0, _] = package(&mut c, &e, WIDTH);
        // The screen fills the height of the rows, and the name goes under it.
        let [left, top] = [x0 + 20.0, 18.0];
        window(&mut c, [left - 4.0, top - 4.0], [16.0 * CELL + 8.0; 2]);
        c.text([left, e.height - 10.0], TEXT, "DOTS");
        c.text([left + 42.0, e.height - 10.0], SILK, "16 x 16");
        for (i, &on) in self.screen.iter().enumerate() {
            let x = left + (i % 16) as f32 * CELL + CELL / 2.0;
            let y = top + (15 - i / 16) as f32 * CELL + CELL / 2.0;
            c.circle([x, y], CELL * 0.4, if on { ON } else { OFF }, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dot_shows_after_a_write_and_a_refresh_at_a_rising_edge() {
        let mut dots = DotDisplay::init();
        // Address 17 is row 1, column 1. The first edge writes the back buffer, and a refresh
        // while the clock stays on does nothing.
        assert!(!dots.process(&17, &true, &false, &true, &false, &true));
        assert!(!dots.process(&17, &true, &false, &true, &true, &true));
        dots.process(&17, &true, &false, &true, &false, &false);
        assert!(!dots.process(&17, &true, &false, &true, &false, &true), "the write goes to the back buffer");
        dots.process(&0, &false, &false, &false, &true, &false);
        assert!(dots.process(&17, &false, &false, &false, &true, &true));
        assert!(dots.rows().ends_with("0000 4000 0000"), "{}", dots.rows());
        let mut ui = Ui::new(0, "");
        dots.ui(&mut ui);
        assert!(ui.tree().contains("dots on: 1"), "{}", ui.tree());
        // A reset clears the back buffer, and the next refresh clears the screen.
        dots.process(&17, &false, &true, &false, &true, &false);
        assert!(!dots.process(&17, &false, &true, &false, &true, &true));
    }
}
