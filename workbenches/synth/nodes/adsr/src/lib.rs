use patina_sdk::AudioBlock;

/// Independent envelope, 0 to 1. Attack, decay and release use seconds. Connect it to VCA or filter CV.
#[patina_sdk::node(domain = audio, color = "#71344f")]
pub struct Module {
    engine: synth_dsp::Adsr,
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
        notes: &synth_dsp::Notes,
        attack: f32,
        decay: f32,
        sustain: f32,
        release: f32,
    ) -> AudioBlock {
        self.engine.process(
            notes,
            &synth_dsp::Contour {
                attack,
                decay,
                sustain,
                release,
            },
        )
    }
}
