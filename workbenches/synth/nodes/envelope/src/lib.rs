use panel::{Live, Look, Strip};
/// Shapes loudness with attack, decay, sustain and release.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Envelope {
    live: Live,
    capture: synth_dsp::capture::Capture,
    engine: synth_dsp::Envelope,
}

impl Envelope {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
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
        self.live.keep(&sound, &sound, [0.0, 0.0, 0.0, 0.0, 0.0]);
        sound
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "ENVELOPE",
            subtitle: "AMPLITUDE",
            color: panel::PINK,
        };
        gui.show(ui, shown, look, &[]);
    }
}
