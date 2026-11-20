use parts::{LIT, SILK, TEXT, UNLIT, board, glass, led};
use patina_sdk::{Ui, node};

/// The longest pulse of the **+** button, in frames.
const LONGEST: u32 = 1024;

/// Gives a pulse at each rising edge of `input`, as the Pulse chip of Digital Logic Sim 2. The
/// output is on for `ticks` frames. 0 counts as 1. An edge during a pulse starts no new pulse.
/// The **-** and **+** buttons halve and double `ticks`.
#[node(color = "#655219", free_layout)]
pub struct Pulse {
    left: u32,
    last: bool,
    on: bool,
    ticks: u32,
}

impl Pulse {
    pub fn init() -> Self {
        Pulse { left: 0, last: false, on: false, ticks: 1 }
    }

    pub fn process(&mut self, input: &bool, ticks: u32) -> bool {
        self.ticks = ticks.max(1);
        if self.left == 0 && *input && !self.last {
            self.left = self.ticks;
        }
        self.last = *input;
        self.on = self.left > 0;
        self.left = self.left.saturating_sub(1);
        self.on
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let size = [132.0, 72.0];
        {
            let mut c = ui.canvas(if self.on { "pulse on" } else { "pulse off" }, size);
            board(&mut c, size);
            led(&mut c, [17.0, 18.0], 5.0, self.on);
            c.text([29.0, 23.0], SILK, "PULSE");
            // One pulse on the glass of a scope, lit during a pulse.
            glass(&mut c, [76.0, 7.0], [48.0, 22.0]);
            c.path("M 80 24 H 90 V 12 H 108 V 24 H 120", if self.on { LIT } else { UNLIT }, false);
            c.text([10.0, 56.0], SILK, "LEN");
            c.text([36.0, 56.0], TEXT, self.ticks);
        }
        ui.pin("input", [0.0, 18.0]);
        ui.pin("ticks", [0.0, 52.0]);
        ui.pin("out", [size[0], 18.0]);
        ui.place([68.0, 38.0]);
        if ui.button("-").clicked() {
            ui.set_param("ticks", (self.ticks / 2).max(1));
        }
        ui.place([100.0, 38.0]);
        if ui.button("+").clicked() {
            ui.set_param("ticks", (self.ticks * 2).min(LONGEST));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_on_for_ticks_frames_after_a_rising_edge() {
        let mut pulse = Pulse::init();
        let out: Vec<bool> = [false, true, true, true, false, true, true].iter().map(|i| pulse.process(i, 2)).collect();
        assert_eq!(out, [false, true, true, false, false, true, true]);
        assert_eq!([false, true, true].iter().map(|i| Pulse::init().process(i, 0)).collect::<Vec<_>>(), [false, true, true]);
    }

    #[test]
    fn the_node_shows_the_pulse() {
        let mut pulse = Pulse::init();
        pulse.process(&true, 4);
        let mut ui = Ui::new(0, "");
        pulse.ui(&mut ui);
        assert!(ui.tree().starts_with("canvas\tpulse on\t"), "{}", ui.tree());
        assert!(ui.tree().contains("text 36 56 #eeeeeb 4"), "{}", ui.tree());
    }
}
