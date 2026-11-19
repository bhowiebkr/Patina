use parts::{digit, edges, package, window};
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
        let (e, w) = (edges(&[7], &[]), 128.0);
        let mut c = ui.canvas(&format!("Segments {}", self.text()), [w, e.height]);
        let [x0, _] = package(&mut c, &e, w);
        window(&mut c, [x0 + 20.0, 12.0], [62.0, 82.0]);
        let bits = self.on.iter().rev().fold(0, |bits, &on| bits << 1 | u8::from(on));
        digit(&mut c, [x0 + 30.0, 22.0], 62.0, bits);
    }

    fn text(&self) -> String {
        self.on.iter().map(|&s| if s { '1' } else { '0' }).collect()
    }
}
