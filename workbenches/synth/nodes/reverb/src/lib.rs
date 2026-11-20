use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Room reverb with damped combs and all-pass diffusion. Decay is the target T60 in seconds.
#[patina_sdk::node(domain = audio, free_layout, color = "#59366f")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Reverb,
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
        decay: f32,
        damping: f32,
        mix: f32,
    ) -> AudioBlock {
        let out = self.engine.process(sound, decay, damping, mix);
        self.live.keep(&out, &out, [decay, damping, mix, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "REVERB",
            subtitle: "ROOM",
            color: panel::VIOLET,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("decay", "DECAY", [0.1, 12.0], "s"),
                Ctl::dial("damping", "DAMPING", [0.0, 1.0], ""),
                Ctl::dial("mix", "MIX", [0.0, 1.0], ""),
            ],
        );
    }
}
