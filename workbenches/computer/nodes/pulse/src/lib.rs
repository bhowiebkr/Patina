use patina_sdk::node;

/// Gives a pulse at each rising edge of `input`, as the Pulse chip of Digital Logic Sim 2. The
/// output is on for `ticks` frames. 0 counts as 1. An edge during a pulse starts no new pulse.
#[node(color = "#655219")]
pub struct Pulse {
    left: u32,
    last: bool,
}

impl Pulse {
    pub fn init() -> Self {
        Pulse { left: 0, last: false }
    }

    pub fn process(&mut self, input: &bool, ticks: u32) -> bool {
        if self.left == 0 && *input && !self.last {
            self.left = ticks.max(1);
        }
        self.last = *input;
        let on = self.left > 0;
        self.left = self.left.saturating_sub(1);
        on
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
}
