use patina_sdk::{Ui, node};
use scope::{LABEL, TRACE, bar, panel, screen};

const SIZE: [f32; 2] = [200.0, 92.0];

/// Writes the value to the standard output. The node shows the value and a bar from 0 to 1 on the
/// screen of a lab instrument.
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
            let text = if self.value.is_finite() { format!("{:.3}", self.value) } else { "--".to_owned() };
            let mut c = ui.canvas(&format!("Show {text}"), SIZE);
            panel(&mut c, SIZE, "SHOW");
            screen(&mut c, [14.0, 24.0], [172.0, 58.0]);
            c.text([22.0, 42.0], TRACE, &text);
            c.text([150.0, 42.0], LABEL, "0..1");
            bar(&mut c, [22.0, 48.0], 158.0, self.value);
        }
        ui.pin("value", [0.0, 53.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_shows_the_value() {
        let mut show = Show::init();
        show.process(&0.3361);
        let mut ui = Ui::new(0, "");
        show.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tShow 0.336\t200\t92\t"), "{}", ui.tree());
        show.process(&f64::NAN);
        let mut ui = Ui::new(0, "");
        show.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tShow --\t"));
    }
}
