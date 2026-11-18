/// Makes two detuned oscillators and a sine sub oscillator. This is the raw sound.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Oscillator {
    engine: synth_dsp::Oscillator,
}

impl Oscillator {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    pub fn process(
        &mut self,
        notes: &synth_dsp::Notes,
        tone: &synth_dsp::Tone,
    ) -> patina_sdk::AudioBlock {
        self.engine.process(notes, tone)
    }
}
