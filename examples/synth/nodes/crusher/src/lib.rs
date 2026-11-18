use patina_sdk::AudioBlock;

/// Lo-fi effect. Bits sets resolution. Rate sets the held sample rate in Hz. Aliasing is intentional.
#[patina_sdk::node(domain = audio, color = "#59366f")]
pub struct Module {
    engine: synth_dsp::Crusher,
}
impl Module {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, sound: &AudioBlock, bits: u8, rate: f32, mix: f32) -> AudioBlock {
        self.engine.process(sound, bits, rate, mix)
    }
}
