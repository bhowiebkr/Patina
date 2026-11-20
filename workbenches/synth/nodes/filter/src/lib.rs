use panel::{Live, Look, Strip};
/// Removes high frequencies. Resonance emphasizes the cutoff. Motion sweeps it with an LFO.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Filter {
    live: Live,
    capture: synth_dsp::capture::Capture,
    engine: synth_dsp::Filter,
}

impl Filter {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
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
        self.live.keep(&sound, &sound, [0.0, 0.0, 0.0, 0.0, 0.0]);
        sound
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "FILTER",
            subtitle: "LOW PASS",
            color: panel::TEAL,
        };
        gui.show(ui, shown, look, &[]);
    }
}
