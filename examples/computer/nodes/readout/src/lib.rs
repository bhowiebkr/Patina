use patina_sdk::{Ui, node};

const ON: [u8; 3] = [223, 166, 75];
const OFF: [u8; 3] = [102, 102, 97];
const TEXT: [u8; 3] = [238, 238, 235];

/// Shows a byte as 8 lamps and as a number, and writes it to the standard output.
#[node]
pub struct Readout {
    value: u8,
}

impl Readout {
    pub fn init() -> Self {
        Readout { value: 0 }
    }

    pub fn process(&mut self, value: &u8) {
        self.value = *value;
        println!("{} {}", self.text(), self.value);
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let (text, number) = (self.text(), self.value);
        let mut canvas = ui.canvas(&format!("{text} = {number}"), [214.0, 36.0]);
        for i in 0..8 {
            // A gap splits the two hex digits.
            let x = 12.0 + 20.0 * i as f32 + if i > 3 { 6.0 } else { 0.0 };
            let on = number >> (7 - i) & 1 == 1;
            canvas.circle([x, 18.0], 7.0, if on { ON } else { OFF }, on);
        }
        canvas.text([178.0, 23.0], TEXT, &number.to_string());
    }

    fn text(&self) -> String {
        format!("{:08b}", self.value)
    }
}
