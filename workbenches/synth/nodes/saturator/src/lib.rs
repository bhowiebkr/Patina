use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Soft distortion with antiderivative antialiasing. Drive sets input gain. Mix blends the dry and wet signals.
#[patina_sdk::node(domain = audio, free_layout, color = "#59366f")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Saturator,
}
impl Module {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, sound: &AudioBlock, drive: f32, mix: f32) -> AudioBlock {
        let out = self.engine.process(sound, drive, mix);
        self.live.keep(&out, &out, [drive, mix, 0.0, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "SATURATOR",
            subtitle: "SOFT CLIP",
            color: panel::VIOLET,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("drive", "DRIVE", [1.0, 20.0], ""),
                Ctl::dial("mix", "MIX", [0.0, 1.0], ""),
            ],
        );
    }
}
