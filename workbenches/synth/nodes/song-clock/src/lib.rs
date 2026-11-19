use synth_dsp::song::{Arrangement, BeatClock, ClockBlock};
/// The only song clock. Every track uses this sample position and loop boundary.
#[patina_sdk::node(domain = audio, color = "#655219")]
pub struct SongClock {
    clock: BeatClock,
}
impl SongClock {
    pub fn init() -> Self {
        Self {
            clock: BeatClock::default(),
        }
    }
    pub fn process(&mut self, score: &Arrangement) -> ClockBlock {
        self.clock.process(score)
    }
}
