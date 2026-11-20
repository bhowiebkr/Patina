use panel::{Live, Look, Strip};
/// Repeats the sound through a damped delay. Feedback controls the number of repeats.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Echo {
    live: Live,
    capture: synth_dsp::capture::Capture,
    engine: synth_dsp::Echo,
}

impl Echo {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            capture: synth_dsp::capture::Capture::new("Echo"),
            engine: Default::default(),
        }
    }
    pub fn process(
        &mut self,
        sound: &patina_sdk::AudioBlock,
        space: &synth_dsp::Space,
    ) -> patina_sdk::AudioBlock {
        let sound = self.engine.process(sound, space);
        self.capture.push(&sound);
        self.live.keep(&sound, &sound, [0.0, 0.0, 0.0, 0.0, 0.0]);
        sound
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "ECHO",
            subtitle: "DELAY",
            color: panel::VIOLET,
        };
        gui.show(ui, shown, look, &[]);
    }
}
