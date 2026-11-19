/// Shapes loudness with attack, decay, sustain and release.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Envelope {
    capture: synth_dsp::capture::Capture,
    engine: synth_dsp::Envelope,
}

impl Envelope {
    pub fn init() -> Self {
        Self {
            capture: synth_dsp::capture::Capture::new("Envelope"),
            engine: Default::default(),
        }
    }
    pub fn process(
        &mut self,
        sound: &patina_sdk::AudioBlock,
        notes: &synth_dsp::Notes,
        contour: &synth_dsp::Contour,
    ) -> patina_sdk::AudioBlock {
        let sound = self.engine.process(sound, notes, contour);
        self.capture.push(&sound);
        sound
    }
}
