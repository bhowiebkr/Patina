use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Independent envelope, 0 to 1. Attack, decay and release use seconds. Connect it to VCA or filter CV.
#[patina_sdk::node(domain = audio, free_layout, color = "#71344f")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Adsr,
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
        notes: &synth_dsp::Notes,
        attack: f32,
        decay: f32,
        sustain: f32,
        release: f32,
    ) -> AudioBlock {
        let out = self.engine.process(
            notes,
            &synth_dsp::Contour {
                attack,
                decay,
                sustain,
                release,
            },
        );
        self.live
            .keep(&out, &out, [attack, decay, sustain, release, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "ADSR",
            subtitle: "ENVELOPE",
            color: panel::PINK,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("attack", "ATTACK", [0.001, 2.0], "s"),
                Ctl::dial("decay", "DECAY", [0.005, 2.0], "s"),
                Ctl::dial("sustain", "SUSTAIN", [0.0, 1.0], ""),
                Ctl::dial("release", "RELEASE", [0.005, 3.0], "s"),
            ],
        );
    }
}
