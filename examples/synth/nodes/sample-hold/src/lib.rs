use patina_sdk::AudioBlock;

/// Holds a sample when the trigger crosses above zero. Noise plus a pulse LFO gives stepped random modulation.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Module {
    engine: synth_dsp::SampleHold,
}
impl Module {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, signal: &AudioBlock, trigger: &AudioBlock) -> AudioBlock {
        self.engine.process(signal, trigger)
    }
}
