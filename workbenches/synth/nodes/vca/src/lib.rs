use patina_sdk::AudioBlock;

/// Voltage-controlled amplifier. Bias 0 uses an envelope. Bias 1 passes sound without a CV input.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Module {
    engine: synth_dsp::Vca,
}
impl Module {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(
        &mut self,
        sound: &AudioBlock,
        cv: &AudioBlock,
        level: f32,
        bias: f32,
    ) -> AudioBlock {
        self.engine.process(sound, cv, level, bias)
    }
}
