use patina_sdk::{Ui, node};

const ON: [u8; 3] = [223, 166, 75];
const OFF: [u8; 3] = [60, 60, 57];

/// The place and size of segments a to g of a digit, in the canvas.
const SEGMENTS: [([f32; 2], [f32; 2]); 7] = [
    ([16.0, 8.0], [28.0, 6.0]),
    ([44.0, 14.0], [6.0, 28.0]),
    ([44.0, 48.0], [6.0, 28.0]),
    ([16.0, 76.0], [28.0, 6.0]),
    ([10.0, 48.0], [6.0, 28.0]),
    ([10.0, 14.0], [6.0, 28.0]),
    ([16.0, 42.0], [28.0, 6.0]),
];

/// Shows segments a to g as a 7-segment digit, and writes them to the standard output.
#[node(color = "#59366f")]
pub struct Display {
    on: [bool; 7],
}

impl Display {
    pub fn init() -> Self {
        Display { on: [false; 7] }
    }

    pub fn process(&mut self, a: &bool, b: &bool, c: &bool, d: &bool, e: &bool, f: &bool, g: &bool) {
        self.on = [*a, *b, *c, *d, *e, *f, *g];
        println!("{}", self.text());
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let mut canvas = ui.canvas(&format!("Segments {}", self.text()), [60.0, 90.0]);
        for (&(at, size), &on) in SEGMENTS.iter().zip(&self.on) {
            canvas.rect(at, size, if on { ON } else { OFF }, true);
        }
    }

    fn text(&self) -> String {
        self.on.iter().map(|&s| if s { '1' } else { '0' }).collect()
    }
}
