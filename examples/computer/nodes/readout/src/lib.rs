use patina_sdk::{Ui, node};

const ON: [u8; 3] = [223, 166, 75];
const OFF: [u8; 3] = [102, 102, 97];
const TEXT: [u8; 3] = [238, 238, 235];

/// Shows 8 bits as lamps and as a number, and writes them to the standard output.
#[node]
pub struct Readout {
    bits: [bool; 8],
}

impl Readout {
    pub fn init() -> Self {
        Readout { bits: [false; 8] }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process(&mut self, b7: &bool, b6: &bool, b5: &bool, b4: &bool, b3: &bool, b2: &bool, b1: &bool, b0: &bool) {
        self.bits = [*b7, *b6, *b5, *b4, *b3, *b2, *b1, *b0];
        println!("{} {}", self.text(), self.number());
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let (text, number) = (self.text(), self.number());
        let mut canvas = ui.canvas(&format!("{text} = {number}"), [214.0, 36.0]);
        for (i, on) in self.bits.iter().enumerate() {
            // A gap splits the two hex digits.
            let x = 12.0 + 20.0 * i as f32 + if i > 3 { 6.0 } else { 0.0 };
            canvas.circle([x, 18.0], 7.0, if *on { ON } else { OFF }, *on);
        }
        canvas.text([178.0, 23.0], TEXT, &number.to_string());
    }

    fn text(&self) -> String {
        self.bits.iter().map(|&b| if b { '1' } else { '0' }).collect()
    }

    fn number(&self) -> u8 {
        self.bits.iter().fold(0, |n, &b| n * 2 + u8::from(b))
    }
}
