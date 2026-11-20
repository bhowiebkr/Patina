use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Two modulated delay taps. Rate uses Hz. Depth uses seconds. Mix adds motion to a dry voice.
#[patina_sdk::node(domain = audio, free_layout, color = "#59366f")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Chorus,
}
impl Module {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, sound: &AudioBlock, rate: f32, depth: f32, mix: f32) -> AudioBlock {
        let out = self.engine.process(sound, rate, depth, mix);
        self.live.keep(&out, &out, [rate, depth, mix, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "CHORUS",
            subtitle: "MODULATED DELAY",
            color: panel::VIOLET,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("rate", "RATE", [0.01, 8.0], "Hz"),
                Ctl::dial("depth", "DEPTH", [0.0, 0.012], "s"),
                Ctl::dial("mix", "MIX", [0.0, 1.0], ""),
            ],
        );
    }
}
