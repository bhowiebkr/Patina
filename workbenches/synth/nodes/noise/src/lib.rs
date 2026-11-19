use patina_sdk::AudioBlock;

/// Deterministic noise. Colors: 0 white, 1 pink, 2 brown. A seed repeats the same noise.
#[patina_sdk::node(domain = audio, color = "#655219")]
pub struct Module {
    engine: synth_dsp::Noise,
}
impl Module {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, color: u8, level: f32, seed: u32) -> AudioBlock {
        self.engine.process(color, level, seed)
    }
}
