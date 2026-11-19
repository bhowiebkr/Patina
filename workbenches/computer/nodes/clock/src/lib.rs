use parts::{LIT, SILK, TEXT, UNLIT, board, glass, led};
use patina_sdk::{Ui, node};

/// The longest period of the **-** button, in frames.
const SLOWEST: u32 = 1024;

/// Gives one tick each `every` frames, as the clock of a computer. 0 counts as 1. `halt` stops
/// the clock. Then the **Step** button gives one tick. The **Pause** and **Run** button sets
/// `halt`, and the **-** and **+** buttons halve and double the speed. It has no wired inputs, so
/// it names its domain.
#[node(domain = frame, color = "#655219", free_layout)]
pub struct Clock {
    frame: u32,
    tick: bool,
    step: bool,
    lit: bool,
    halt: bool,
    every: u32,
}

impl Clock {
    pub fn init() -> Self {
        Clock { frame: 0, tick: false, step: false, lit: false, halt: false, every: 1 }
    }

    pub fn process(&mut self, every: u32, halt: bool) -> bool {
        let step = std::mem::take(&mut self.step);
        (self.halt, self.every) = (halt, every.max(1));
        if halt {
            (self.tick, self.lit) = (step, step);
        } else {
            let every = self.every;
            self.tick = self.frame == 0;
            // The LED is lit for the first half of the period, so a slow clock blinks visibly.
            self.lit = self.frame < every.div_ceil(2);
            self.frame = (self.frame + 1) % every;
        }
        self.tick
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let size = [196.0, 72.0];
        {
            let mut c = ui.canvas(if self.tick { "tick" } else { "wait" }, size);
            board(&mut c, size);
            led(&mut c, [17.0, 18.0], 5.0, self.lit);
            c.text([29.0, 23.0], SILK, "CLK");
            // A square wave on the glass of a scope, lit while the clock runs.
            glass(&mut c, [62.0, 7.0], [66.0, 22.0]);
            c.path("M 66 24 H 74 V 12 H 86 V 24 H 98 V 12 H 110 V 24 H 122 V 12 H 124", if self.halt { UNLIT } else { LIT }, false);
            c.text([136.0, 23.0], TEXT, &format!("/{}", self.every));
        }
        ui.pin("every", [0.0, 18.0]);
        ui.pin("halt", [0.0, 52.0]);
        ui.pin("out", [size[0], 18.0]);
        ui.place([8.0, 38.0]);
        if ui.button(if self.halt { "Run" } else { "Pause" }).clicked() {
            ui.set_param("halt", !self.halt);
        }
        ui.place([72.0, 38.0]);
        if ui.button("Step").clicked() {
            self.step = true;
        }
        ui.place([128.0, 38.0]);
        if ui.button("-").clicked() {
            ui.set_param("every", (self.every * 2).min(SLOWEST));
        }
        ui.place([160.0, 38.0]);
        if ui.button("+").clicked() {
            ui.set_param("every", (self.every / 2).max(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_each_every_frames_and_steps_when_halted() {
        let mut clock = Clock::init();
        let ticks: Vec<bool> = (0..6).map(|_| clock.process(3, false)).collect();
        assert_eq!(ticks, [true, false, false, true, false, false]);
        let lit: Vec<bool> = (0..4).map(|_| {
            clock.process(4, false);
            clock.lit
        }).collect();
        assert_eq!(lit, [true, true, false, false]);
        assert!(!clock.process(3, true));
        assert!(clock.halt, "the button shows Run");
        clock.step = true;
        assert!(clock.process(3, true));
        assert!(!clock.process(3, true));
        assert!(clock.process(0, false));
        assert!(clock.process(0, false));
    }
}
