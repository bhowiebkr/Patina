/// Synthesizes kick, snare and hi-hat from sine waves and noise. No samples are used.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Drums {
    capture: synth_dsp::capture::Capture,
    engine: synth_dsp::Drums,
}

impl Drums {
    pub fn init() -> Self {
        Self {
            capture: synth_dsp::capture::Capture::new("Drums"),
            engine: Default::default(),
        }
    }
    pub fn process(&mut self, notes: &synth_dsp::Notes) -> patina_sdk::AudioBlock {
        let sound = self.engine.process(notes);
        self.capture.push(&sound);
        sound
    }
}
