/// Makes two detuned oscillators and a sine sub oscillator. This is the raw sound.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Oscillator {
    capture: synth_dsp::capture::Capture,
    engine: synth_dsp::Oscillator,
}

impl Oscillator {
    pub fn init() -> Self {
        Self {
            capture: synth_dsp::capture::Capture::new("Oscillator"),
            engine: Default::default(),
        }
    }
    pub fn process(
        &mut self,
        notes: &synth_dsp::Notes,
        tone: &synth_dsp::Tone,
    ) -> patina_sdk::AudioBlock {
        let sound = self.engine.process(notes, tone);
        self.capture.push(&sound);
        sound
    }
}
