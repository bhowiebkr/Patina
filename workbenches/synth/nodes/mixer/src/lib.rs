use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Four input mixer with smooth gains. Negative levels invert polarity. Leave room below full scale.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Mixer,
}
impl Module {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(
        &mut self,
        a: &AudioBlock,
        b: &AudioBlock,
        c: &AudioBlock,
        d: &AudioBlock,
        a_level: f32,
        b_level: f32,
        c_level: f32,
        d_level: f32,
    ) -> AudioBlock {
        let out = self
            .engine
            .process([a, b, c, d], [a_level, b_level, c_level, d_level]);
        self.live
            .keep(&out, &out, [a_level, b_level, c_level, d_level, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "MIXER",
            subtitle: "FOUR CHANNELS",
            color: panel::TEAL,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("a_level", "A", [-2.0, 2.0], ""),
                Ctl::dial("b_level", "B", [-2.0, 2.0], ""),
                Ctl::dial("c_level", "C", [-2.0, 2.0], ""),
                Ctl::dial("d_level", "D", [-2.0, 2.0], ""),
            ],
        );
    }
}
