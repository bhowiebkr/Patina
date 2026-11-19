use parts::{board, led};
use patina_sdk::{Ui, node};

/// A lamp that is on when its input is true, as the LED of Digital Logic Sim 2. When the input
/// changes, it writes the state to the standard output.
#[node(color = "#59366f", free_layout)]
pub struct Led {
    on: bool,
}

impl Led {
    pub fn init() -> Self {
        Led { on: false }
    }

    pub fn process(&mut self, on: &bool) {
        if *on != self.on {
            self.on = *on;
            println!("led {}", if self.on { "on" } else { "off" });
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let mut c = ui.canvas(if self.on { "LED on" } else { "LED off" }, [32.0, 32.0]);
            board(&mut c, [32.0, 32.0]);
            led(&mut c, [18.0, 16.0], 7.0, self.on);
        }
        ui.pin("on", [0.0, 16.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lamp_follows_its_input() {
        let mut lamp = Led::init();
        lamp.process(&true);
        let mut ui = Ui::new(0, "");
        lamp.ui(&mut ui);
        assert!(ui.tree().contains("LED on"), "{}", ui.tree());
    }
}
