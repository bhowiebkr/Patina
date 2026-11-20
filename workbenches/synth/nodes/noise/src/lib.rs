use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Deterministic noise. Colors: 0 white, 1 pink, 2 brown. A seed repeats the same noise.
#[patina_sdk::node(domain = audio, free_layout, color = "#655219")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Noise,
}
impl Module {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, color: u8, level: f32, seed: u32) -> AudioBlock {
        let out = self.engine.process(color, level, seed);
        self.live
            .keep(&out, &out, [f32::from(color), level, 0.0, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "NOISE",
            subtitle: "RANDOM SOURCE",
            color: panel::AMBER,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::pick("color", "COLOR", &["WHITE", "PINK", "BROWN"]),
                Ctl::dial("level", "LEVEL", [0.0, 1.0], ""),
            ],
        );
    }
}
