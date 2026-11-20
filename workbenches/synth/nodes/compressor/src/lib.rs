use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Soft-knee peak compressor. Threshold and makeup use dB. Attack and release use seconds.
#[patina_sdk::node(domain = audio, free_layout, color = "#59366f")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Compressor,
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
        threshold: f32,
        ratio: f32,
        attack: f32,
        release: f32,
        makeup: f32,
    ) -> AudioBlock {
        let out = self.engine.process(
            sound,
            synth_dsp::Dynamics {
                threshold,
                ratio,
                attack,
                release,
                makeup,
            },
        );
        self.live
            .keep(&out, &out, [threshold, ratio, attack, release, makeup]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "COMPRESSOR",
            subtitle: "DYNAMICS",
            color: panel::VIOLET,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("threshold", "THRESH", [-60.0, 0.0], "dB"),
                Ctl::dial("ratio", "RATIO", [1.0, 20.0], ""),
                Ctl::dial("attack", "ATTACK", [0.0001, 1.0], "s"),
                Ctl::dial("release", "RELEASE", [0.005, 3.0], "s"),
                Ctl::dial("makeup", "MAKEUP", [-24.0, 24.0], "dB"),
            ],
        );
    }
}
