use patina_sdk::AudioBlock;

/// Room reverb with damped combs and all-pass diffusion. Decay is the target T60 in seconds.
#[patina_sdk::node(domain = audio, color = "#59366f")]
pub struct Module {
    engine: synth_dsp::Reverb,
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
        decay: f32,
        damping: f32,
        mix: f32,
    ) -> AudioBlock {
        self.engine.process(sound, decay, damping, mix)
    }
}
