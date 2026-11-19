use patina_sdk::AudioBlock;
use synth_dsp::capture::{Capture, Stream};

/// Captures every audio block for a Spectrogram. Connect this in parallel with the sound path.
#[patina_sdk::node(domain = audio, color = "#59366f")]
pub struct AudioCapture {
    capture: Capture,
}

impl AudioCapture {
    pub fn init() -> Self {
        Self {
            capture: Capture::new("Audio capture"),
        }
    }
    pub fn process(&mut self, sound: &AudioBlock) -> Stream {
        self.capture.push(sound);
        self.capture.stream()
    }
}
