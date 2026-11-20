use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Voltage-controlled amplifier. Bias 0 uses an envelope. Bias 1 passes sound without a CV input.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Vca,
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
        sound: &AudioBlock,
        cv: &AudioBlock,
        level: f32,
        bias: f32,
    ) -> AudioBlock {
        let out = self.engine.process(sound, cv, level, bias);
        self.live.keep(&out, &out, [level, bias, 0.0, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "VCA",
            subtitle: "AMPLIFIER",
            color: panel::TEAL,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("level", "LEVEL", [0.0, 4.0], ""),
                Ctl::dial("bias", "BIAS", [0.0, 1.0], ""),
            ],
        );
    }
}
