use patina_sdk::AudioBlock;

/// Removes constant offset with a 20 Hz high-pass filter. Use it after asymmetric processing.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Module {
    engine: synth_dsp::DcBlock,
}
impl Module {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, sound: &AudioBlock) -> AudioBlock {
        self.engine.process(sound)
    }
}
