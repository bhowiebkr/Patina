use panel::{Ctl, Live, Look, Strip};
use patina_sdk::audio::{self, BLOCK, StereoBlock};
/// A stereo master with linked peak protection and the shared analysis capture.
#[patina_sdk::node(domain = audio, free_layout, color = "#613979")]
pub struct StereoOutput {
    live: Live,
    gain: f32,
    limiter: f32,
    capture: synth_dsp::capture::Capture,
    left_capture: synth_dsp::capture::Capture,
    right_capture: synth_dsp::capture::Capture,
}
impl StereoOutput {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            gain: 0.0,
            limiter: 1.0,
            capture: synth_dsp::capture::Capture::new("Song master"),
            left_capture: synth_dsp::capture::Capture::new("Song master L"),
            right_capture: synth_dsp::capture::Capture::new("Song master R"),
        }
    }
    pub fn process(&mut self, sound: &StereoBlock, level: f32, audible: bool) -> StereoBlock {
        let mut out = StereoBlock::default();
        let level = synth_dsp::finite(level, 0.7, 0.0, 1.0);
        let mut mono = [0.0; BLOCK];
        for i in 0..BLOCK {
            self.gain += (level - self.gain) * 0.004;
            let l = sound.left[i] * self.gain;
            let r = sound.right[i] * self.gain;
            let peak = l.abs().max(r.abs());
            let required = if peak > 0.95 { 0.95 / peak } else { 1.0 };
            self.limiter = required.min(self.limiter + (1.0 - self.limiter) * 0.0002);
            out.left[i] = l * self.limiter;
            out.right[i] = r * self.limiter;
            mono[i] = (out.left[i] + out.right[i]) * 0.5;
        }
        self.capture.push(&mono);
        self.left_capture.push(&out.left);
        self.right_capture.push(&out.right);
        if audible {
            audio::play_stereo(&out);
        }
        self.live.keep(
            &out.left,
            &out.right,
            [level, f32::from(u8::from(audible)), 0.0, 0.0, 0.0],
        );
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "STEREO OUTPUT",
            subtitle: "MASTER",
            color: panel::VIOLET,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("level", "LEVEL", [0.0, 1.0], ""),
                Ctl::switch("audible", "AUDIBLE"),
            ],
        );
    }
}
