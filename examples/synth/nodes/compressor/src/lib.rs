use patina_sdk::AudioBlock;

/// Soft-knee peak compressor. Threshold and makeup use dB. Attack and release use seconds.
#[patina_sdk::node(domain = audio, color = "#59366f")]
pub struct Module {
    engine: synth_dsp::Compressor,
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
        threshold: f32,
        ratio: f32,
        attack: f32,
        release: f32,
        makeup: f32,
    ) -> AudioBlock {
        self.engine.process(
            sound,
            synth_dsp::Dynamics {
                threshold,
                ratio,
                attack,
                release,
                makeup,
            },
        )
    }
}
