/// Turns tempo and steps into sample-timed notes and drum triggers.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Clock {
    engine: synth_dsp::Clock,
}

impl Clock {
    pub fn init() -> Self {
        Self {
            engine: Default::default(),
        }
    }
    pub fn process(&mut self, settings: &synth_dsp::Sequence) -> synth_dsp::Notes {
        self.engine.process(settings)
    }
}
