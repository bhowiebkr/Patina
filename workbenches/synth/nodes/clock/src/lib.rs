use panel::{Live, Look, Strip};
/// Turns tempo and steps into sample-timed notes and drum triggers.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Clock {
    live: Live,
    engine: synth_dsp::Clock,
}

impl Clock {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            engine: Default::default(),
        }
    }
    pub fn process(&mut self, settings: &synth_dsp::Sequence) -> synth_dsp::Notes {
        let out = self.engine.process(settings);
        self.live
            .keep(&out.gate, &out.gate, [0.0, 0.0, 0.0, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "CLOCK",
            subtitle: "SEQUENCER",
            color: panel::AMBER,
        };
        gui.show(ui, shown, look, &[]);
    }
}
