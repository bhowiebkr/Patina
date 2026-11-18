use patina_sdk::AudioBlock;

/// Independent oscillator. Pitch CV uses octaves. Shapes: 0 sine, 1 saw, 2 pulse, 3 triangle.
#[patina_sdk::node(domain = audio, color = "#655219")]
pub struct Module {
    engine: synth_dsp::Vco,
}
impl Module {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, pitch: &AudioBlock, hz: f32, depth: f32, shape: u8) -> AudioBlock {
        self.engine.process(pitch, hz, depth, shape)
    }
}
