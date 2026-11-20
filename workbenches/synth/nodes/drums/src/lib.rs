use panel::{Live, Look, Strip};
/// Synthesizes kick, snare and hi-hat from sine waves and noise. No samples are used.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Drums {
    live: Live,
    capture: synth_dsp::capture::Capture,
    engine: synth_dsp::Drums,
}

impl Drums {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            capture: synth_dsp::capture::Capture::new("Drums"),
            engine: Default::default(),
        }
    }
    pub fn process(&mut self, notes: &synth_dsp::Notes) -> patina_sdk::AudioBlock {
        let sound = self.engine.process(notes);
        self.capture.push(&sound);
        self.live.keep(&sound, &sound, [0.0, 0.0, 0.0, 0.0, 0.0]);
        sound
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "DRUMS",
            subtitle: "KICK SNARE HAT",
            color: panel::AMBER,
        };
        gui.show(ui, shown, look, &[]);
    }
}
