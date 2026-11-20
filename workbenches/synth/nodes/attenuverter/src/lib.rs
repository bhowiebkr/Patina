use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Scales, inverts and offsets audio or modulation. A scale of -1 inverts the signal.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Attenuverter,
}
impl Module {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, signal: &AudioBlock, scale: f32, offset: f32) -> AudioBlock {
        let out = self.engine.process(signal, scale, offset);
        self.live.keep(&out, &out, [scale, offset, 0.0, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "ATTENUVERTER",
            subtitle: "SCALE AND OFFSET",
            color: panel::TEAL,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("scale", "SCALE", [-8.0, 8.0], ""),
                Ctl::dial("offset", "OFFSET", [-16.0, 16.0], ""),
            ],
        );
    }
}
