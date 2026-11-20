use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Smooths rising and falling values separately. Rise and fall are time constants in seconds.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Slew,
}
impl Module {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, signal: &AudioBlock, rise: f32, fall: f32) -> AudioBlock {
        let out = self.engine.process(signal, rise, fall);
        self.live.keep(&out, &out, [rise, fall, 0.0, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "SLEW",
            subtitle: "LAG",
            color: panel::TEAL,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("rise", "RISE", [0.0001, 10.0], "s"),
                Ctl::dial("fall", "FALL", [0.0001, 10.0], "s"),
            ],
        );
    }
}
