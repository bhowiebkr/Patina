use patina_sdk::AudioBlock;

/// Four input mixer with smooth gains. Negative levels invert polarity. Leave room below full scale.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Module {
    engine: synth_dsp::Mixer,
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
        a: &AudioBlock,
        b: &AudioBlock,
        c: &AudioBlock,
        d: &AudioBlock,
        a_level: f32,
        b_level: f32,
        c_level: f32,
        d_level: f32,
    ) -> AudioBlock {
        self.engine
            .process([a, b, c, d], [a_level, b_level, c_level, d_level])
    }
}
