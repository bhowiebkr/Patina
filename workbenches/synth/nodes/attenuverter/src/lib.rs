use patina_sdk::AudioBlock;

/// Scales, inverts and offsets audio or modulation. A scale of -1 inverts the signal.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Module {
    engine: synth_dsp::Attenuverter,
}
impl Module {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, signal: &AudioBlock, scale: f32, offset: f32) -> AudioBlock {
        self.engine.process(signal, scale, offset)
    }
}
