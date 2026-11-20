use panel::{Live, Look, Strip};
use synth_dsp::song::{Arrangement, BeatClock, ClockBlock};
/// The only song clock. Every track uses this sample position and loop boundary.
#[patina_sdk::node(domain = audio, free_layout, color = "#655219")]
pub struct SongClock {
    live: Live,
    clock: BeatClock,
}
impl SongClock {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            clock: BeatClock::default(),
        }
    }
    pub fn process(&mut self, score: &Arrangement) -> ClockBlock {
        let out = self.clock.process(score);
        self.live.keep(&[], &[], [0.0, 0.0, 0.0, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "SONG CLOCK",
            subtitle: "SAMPLE POSITION",
            color: panel::AMBER,
        };
        gui.show(ui, shown, look, &[]);
    }
}
