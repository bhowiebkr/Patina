/// Repeats the sound through a damped delay. Feedback controls the number of repeats.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Echo {
    engine: synth_dsp::Echo,
}

impl Echo {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    pub fn process(
        &mut self,
        sound: &patina_sdk::AudioBlock,
        space: &synth_dsp::Space,
    ) -> patina_sdk::AudioBlock {
        self.engine.process(sound, space)
    }
}
