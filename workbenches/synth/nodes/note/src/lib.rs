/// A held note at any frequency. A rising gate retriggers its envelope.
#[patina_sdk::node(domain = audio, color = "#655219")]
pub struct Note {
    held: bool,
}
impl Note {
    pub fn init() -> Self {
        Self { held: false }
    }
    pub fn process(&mut self, hz: f32, gate: bool) -> synth_dsp::Notes {
        let mut notes = synth_dsp::Notes {
            hz: [synth_dsp::finite(hz, 110.0, 10.0, 16_000.0); 256],
            gate: [f32::from(gate); 256],
            ..Default::default()
        };
        notes.trigger[0] = gate && !self.held;
        self.held = gate;
        notes
    }
}
