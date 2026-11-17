use patina_sdk::{Ui, node};

const ON: [u8; 3] = [223, 166, 75];
const OFF: [u8; 3] = [102, 102, 97];
const TEXT: [u8; 3] = [238, 238, 235];

/// Gives one tick each `every` frames, as the clock of a computer. 0 counts as 1. `halt` stops
/// the clock. Then the **Step** button gives one tick. The **Pause** and **Run** button sets
/// `halt`. It has no wired inputs, so it names its domain.
#[node(domain = frame, color = "#655219")]
pub struct Clock {
    frame: u32,
    tick: bool,
    step: bool,
    lit: bool,
    halt: bool,
}

impl Clock {
    pub fn init() -> Self {
        Clock { frame: 0, tick: false, step: false, lit: false, halt: false }
    }

    pub fn process(&mut self, every: u32, halt: bool) -> bool {
        let step = std::mem::take(&mut self.step);
        self.halt = halt;
        if halt {
            (self.tick, self.lit) = (step, step);
        } else {
            let every = every.max(1);
            self.tick = self.frame == 0;
            // The LED is lit for the first half of the period, so a slow clock blinks visibly.
            self.lit = self.frame < every.div_ceil(2);
            self.frame = (self.frame + 1) % every;
        }
        self.tick
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        {
            let mut canvas = ui.canvas(if self.tick { "tick" } else { "wait" }, [96.0, 36.0]);
            canvas.circle([18.0, 18.0], 9.0, if self.lit { ON } else { OFF }, self.lit);
            canvas.text([36.0, 23.0], TEXT, "CLK");
        }
        if ui.button(if self.halt { "Run" } else { "Pause" }).clicked() {
            ui.set_param("halt", !self.halt);
        }
        if ui.button("Step").clicked() {
            self.step = true;
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
