use parts::{SILK, board, digits, glass, led};
use patina_sdk::{Ui, node};

/// The labels of the bits, from the high bit.
const BITS: [&str; 8] = ["7", "6", "5", "4", "3", "2", "1", "0"];

/// Shows a byte as 8 LEDs, as two hex digits and as a decimal number. A change writes it to the
/// standard output.
#[node(color = "#59366f", free_layout)]
pub struct Readout {
    value: u8,
}

impl Readout {
    pub fn init() -> Self {
        Readout { value: 0 }
    }

    pub fn process(&mut self, value: &u8) {
        if *value != self.value {
            self.value = *value;
            println!("{} {}", self.text(), self.value);
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let (text, number) = (self.text(), self.value);
        let size = [240.0, 62.0];
        let mut c = ui.canvas(&format!("{text} = {number}"), size);
        board(&mut c, size);
        for (i, bit) in BITS.iter().enumerate() {
            // A gap splits the two hex digits.
            let x = 18.0 + 18.0 * i as f32 + if i > 3 { 8.0 } else { 0.0 };
            c.text([x - 3.5, 18.0], SILK, bit);
            led(&mut c, [x, 30.0], 5.0, number >> (7 - i) & 1 == 1);
        }
        c.text([12.0, 54.0], SILK, &format!("dec {number}"));
        glass(&mut c, [166.0, 9.0], [66.0, 44.0]);
        digits(&mut c, [177.0, 16.0], 30.0, 2, &format!("{number:02X}"));
        drop(c);
        ui.pin("value", [0.0, 30.0]);
    }

    fn text(&self) -> String {
        format!("{:08b}", self.value)
    }
}
