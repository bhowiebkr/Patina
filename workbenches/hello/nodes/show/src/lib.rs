use calc::{case, lcd, solar};
use patina_sdk::{Ui, node};

/// The size of the case, and the number of digits of the LCD.
const SIZE: [f32; 2] = [180.0, 78.0];
const DIGITS: usize = 9;

/// Writes the value to the standard output. The node shows the value on the LCD of a calculator.
#[node(free_layout)]
pub struct Show {
    value: f64,
}

impl Show {
    pub fn init() -> Self {
        Show { value: 0.0 }
    }

    pub fn process(&mut self, value: &f64) {
        self.value = *value;
        println!("{value}");
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let mut c = ui.canvas(&format!("Show {}", self.value), SIZE);
            case(&mut c, SIZE, "SHOW");
            solar(&mut c, [SIZE[0] - 12.0, 7.0]);
            lcd(&mut c, [14.0, 28.0], DIGITS, self.value);
        }
        ui.pin("value", [0.0, 46.0]);
    }
}
