use panel::{Live, Look, Strip};
use patina_sdk::AudioBlock;
use synth_dsp::capture::{Capture, Stream};

/// Captures every audio block for a Spectrogram. Connect this in parallel with the sound path.
#[patina_sdk::node(domain = audio, free_layout, color = "#59366f")]
pub struct AudioCapture {
    live: Live,
    capture: Capture,
}

impl AudioCapture {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            capture: Capture::new("Audio capture"),
        }
    }
    pub fn process(&mut self, sound: &AudioBlock) -> Stream {
        self.capture.push(sound);
        self.live.keep(sound, sound, [0.0, 0.0, 0.0, 0.0, 0.0]);
        self.capture.stream()
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "CAPTURE",
            subtitle: "FOR SPECTROGRAM",
            color: panel::VIOLET,
        };
        gui.show(ui, shown, look, &[]);
    }
}
