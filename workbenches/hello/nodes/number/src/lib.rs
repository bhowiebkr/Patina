use calc::{case, lcd};
use patina_sdk::{Ui, node};

/// The size of the case, and the number of digits of the LCD.
const SIZE: [f32; 2] = [150.0, 98.0];
const DIGITS: usize = 7;

/// Gives a number. Type the number in the field of the node, or set the parameter in the
/// Inspector. The node shows the number on the LCD of a calculator.
#[node(free_layout)]
pub struct Number {
    value: f64,
}

impl Number {
    pub fn init() -> Self {
        Number { value: 0.0 }
    }

    pub fn process(&mut self, value: f64) -> f64 {
        self.value = value;
        value
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let mut c = ui.canvas(&format!("Number {}", self.value), SIZE);
            case(&mut c, SIZE, "NUMBER");
            lcd(&mut c, [14.0, 28.0], DIGITS, self.value);
        }
        ui.place([12.0, 68.0]);
        let mut value = self.value;
        if ui.number("Value", &mut value).changed() {
            self.value = value;
            ui.set_param("value", value);
        }
        ui.pin("out", [SIZE[0], 46.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_changes_the_number() {
        let mut number = Number::init();
        assert_eq!(number.process(2.5), 2.5);
        let mut ui = Ui::new(0, "2 4");
        number.ui(&mut ui);
        assert_eq!(number.value, 4.0);
        assert!(ui.tree().starts_with("canvas\tNumber 2.5\t150\t98\t"), "{}", ui.tree());
    }
}
