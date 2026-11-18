use patina_sdk::AudioBlock;

/// Modulated filter. Modes: 0 low-pass, 1 high-pass, 2 band-pass, 3 notch. CV depth uses octaves.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Module {
    engine: synth_dsp::Svf,
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
        cutoff: f32,
        resonance: f32,
        depth: f32,
        mode: u8,
    ) -> AudioBlock {
        self.engine
            .process(sound, cv, cutoff, resonance, depth, mode)
    }
}
