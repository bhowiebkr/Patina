use parts::{BOARD, GLASS, METAL, board, led, round_rect};
use patina_sdk::{Pointer, Ui, node};

/// A constant bit that the user sets. Click the switch to flip it. The LED shows the output.
#[node(color = "#655219", free_layout)]
pub struct Switch {
    on: bool,
}

impl Switch {
    pub fn init() -> Self {
        Switch { on: false }
    }

    pub fn process(&mut self, on: bool) -> bool {
        self.on = on;
        on
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let flip = {
            let mut c = ui.canvas(if self.on { "Switch on" } else { "Switch off" }, [76.0, 32.0]);
            board(&mut c, [76.0, 32.0]);
            // A slide switch: the knob is at the right when the switch is on.
            round_rect(&mut c, [10.0, 8.0], [34.0, 16.0], 8.0, GLASS);
            let x = if self.on { 36.0 } else { 18.0 };
            c.circle([x, 16.0], 6.5, METAL, true);
            c.line([x, 12.5], [x, 19.5], BOARD);
            led(&mut c, [60.0, 16.0], 5.0, self.on);
            c.pointer().iter().any(|p| matches!(p, Pointer::Down(_)))
        };
        if flip {
            ui.set_param("on", !self.on);
        }
        ui.pin("on", [0.0, 16.0]);
        ui.pin("out", [76.0, 16.0]);
    }
}
