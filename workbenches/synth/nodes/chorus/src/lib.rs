use patina_sdk::AudioBlock;

/// Two modulated delay taps. Rate uses Hz. Depth uses seconds. Mix adds motion to a dry voice.
#[patina_sdk::node(domain = audio, color = "#59366f")]
pub struct Module {
    engine: synth_dsp::Chorus,
}
impl Module {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, sound: &AudioBlock, rate: f32, depth: f32, mix: f32) -> AudioBlock {
        self.engine.process(sound, rate, depth, mix)
    }
}
