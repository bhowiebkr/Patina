/// Removes high frequencies. Resonance emphasizes the cutoff. Motion sweeps it with an LFO.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Filter {
    capture: synth_dsp::capture::Capture,
    engine: synth_dsp::Filter,
}

impl Filter {
    pub fn init() -> Self {
        Self {
            capture: synth_dsp::capture::Capture::new("Filter"),
            engine: Default::default(),
        }
    }
    pub fn process(
        &mut self,
        sound: &patina_sdk::AudioBlock,
        color: &synth_dsp::Color,
    ) -> patina_sdk::AudioBlock {
        let sound = self.engine.process(sound, color);
        self.capture.push(&sound);
        sound
    }
}
