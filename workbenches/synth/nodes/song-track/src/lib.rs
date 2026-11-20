use panel::{Live, Look, Strip};
use patina_sdk::audio::StereoBlock;
use synth_dsp::song::{Arrangement, ClockBlock, TrackPlayer};
/// A hybrid track: polyphonic instrument notes and imported stereo recordings.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct SongTrack {
    live: Live,
    player: TrackPlayer,
}
impl SongTrack {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            player: TrackPlayer::default(),
        }
    }
    pub fn process(&mut self, score: &Arrangement, time: &ClockBlock, track: u32) -> StereoBlock {
        let out = self.player.process(score, time, track);
        self.live.keep(&out.left, &out.right, [0.0; 5]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "SONG TRACK",
            subtitle: "NOTES AND AUDIO",
            color: panel::AMBER,
        };
        gui.show(ui, shown, look, &[]);
    }
}
