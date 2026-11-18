use patina_sdk::AudioBlock;

/// Slow modulation. Shapes: 0 sine, 1 saw, 2 pulse, 3 triangle. Rate is in Hz.
#[patina_sdk::node(domain = audio, color = "#655219")]
pub struct Module {
    engine: synth_dsp::Lfo,
}
impl Module {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, rate: f32, shape: u8, depth: f32, offset: f32) -> AudioBlock {
        self.engine.process(rate, shape, depth, offset)
    }
}
