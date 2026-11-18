/// Removes high frequencies. Resonance emphasizes the cutoff. Motion sweeps it with an LFO.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Filter {
    engine: synth_dsp::Filter,
}

impl Filter {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    pub fn process(
        &mut self,
        sound: &patina_sdk::AudioBlock,
        color: &synth_dsp::Color,
    ) -> patina_sdk::AudioBlock {
        self.engine.process(sound, color)
    }
}
