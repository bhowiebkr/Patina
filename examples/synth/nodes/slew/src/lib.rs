use patina_sdk::AudioBlock;

/// Smooths rising and falling values separately. Rise and fall are time constants in seconds.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Module {
    engine: synth_dsp::Slew,
}
impl Module {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, signal: &AudioBlock, rise: f32, fall: f32) -> AudioBlock {
        self.engine.process(signal, rise, fall)
    }
}
