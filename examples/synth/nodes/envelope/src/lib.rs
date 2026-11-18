/// Shapes loudness with attack, decay, sustain and release.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Envelope {
    engine: synth_dsp::Envelope,
}

impl Envelope {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    pub fn process(
        &mut self,
        sound: &patina_sdk::AudioBlock,
        notes: &synth_dsp::Notes,
        contour: &synth_dsp::Contour,
    ) -> patina_sdk::AudioBlock {
        self.engine.process(sound, notes, contour)
    }
}
