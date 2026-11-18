use parts::{board, digit, glass};
use patina_sdk::{Ui, node};
use segments::Segments;

/// Shows the segments a to g of a bus as a 7-segment digit. A change writes them to the standard output.
#[node(color = "#59366f", free_layout)]
pub struct Display {
    on: [bool; 7],
}

impl Display {
    pub fn init() -> Self {
        Display { on: [false; 7] }
    }

    pub fn process(&mut self, seg: &Segments) {
        let on = std::array::from_fn(|i| seg.on(i));
        if on != self.on {
            self.on = on;
            println!("{}", self.text());
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let size = [64.0, 96.0];
        let mut c = ui.canvas(&format!("Segments {}", self.text()), size);
        board(&mut c, size);
        glass(&mut c, [6.0, 6.0], [52.0, 84.0]);
        let bits = self.on.iter().rev().fold(0, |bits, &on| bits << 1 | u8::from(on));
        digit(&mut c, [12.0, 18.0], 60.0, bits);
        drop(c);
        ui.pin("seg", [0.0, 48.0]);
    }

    fn text(&self) -> String {
        self.on.iter().map(|&s| if s { '1' } else { '0' }).collect()
    }
}
