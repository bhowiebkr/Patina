use patina_sdk::AudioBlock;

/// Soft distortion with antiderivative antialiasing. Drive sets input gain. Mix blends the dry and wet signals.
#[patina_sdk::node(domain = audio, color = "#59366f")]
pub struct Module {
    engine: synth_dsp::Saturator,
}
impl Module {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, sound: &AudioBlock, drive: f32, mix: f32) -> AudioBlock {
        self.engine.process(sound, drive, mix)
    }
}
