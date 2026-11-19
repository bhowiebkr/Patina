use parts::{SILK, digits, edges, package, window};
use patina_sdk::{Ui, node};

/// Shows a byte as two hex digits and as a decimal number. The socket of `value` shows the 8 bits.
/// A change writes the byte to the standard output.
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
        let (e, w) = (edges(&[8], &[]), 136.0);
        let mut c = ui.canvas(&format!("{text} = {number}"), [w, e.height]);
        let [x0, _] = package(&mut c, &e, w);
        window(&mut c, [x0 + 10.0, 14.0], [84.0, 70.0]);
        digits(&mut c, [x0 + 19.0, 25.0], 48.0, 2, &format!("{number:02X}"));
        c.text([x0 + 10.0, 104.0], SILK, &format!("HEX  dec {number}"));
    }

    fn text(&self) -> String {
        format!("{:08b}", self.value)
    }
}
