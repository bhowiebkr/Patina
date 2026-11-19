use patina_sdk::audio::StereoBlock;
use synth_dsp::song::{Arrangement, ClockBlock, TrackPlayer};
/// A hybrid track: polyphonic instrument notes and imported stereo recordings.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct SongTrack {
    player: TrackPlayer,
}
impl SongTrack {
    pub fn init() -> Self {
        Self {
            player: TrackPlayer::default(),
        }
    }
    pub fn process(&mut self, score: &Arrangement, time: &ClockBlock, track: u32) -> StereoBlock {
        self.player.process(score, time, track)
    }
}
