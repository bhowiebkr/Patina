use panel::{Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Removes constant offset with a 20 Hz high-pass filter. Use it after asymmetric processing.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Module {
    live: Live,
    engine: synth_dsp::DcBlock,
}
impl Module {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, sound: &AudioBlock) -> AudioBlock {
        let out = self.engine.process(sound);
        self.live.keep(&out, &out, [0.0, 0.0, 0.0, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "DC BLOCK",
            subtitle: "OFFSET FILTER",
            color: panel::TEAL,
        };
        gui.show(ui, shown, look, &[]);
    }
}
