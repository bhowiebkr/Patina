use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Slow modulation. Shapes: 0 sine, 1 saw, 2 pulse, 3 triangle. Rate is in Hz.
#[patina_sdk::node(domain = audio, free_layout, color = "#655219")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Lfo,
}
impl Module {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, rate: f32, shape: u8, depth: f32, offset: f32) -> AudioBlock {
        let out = self.engine.process(rate, shape, depth, offset);
        self.live
            .keep(&out, &out, [rate, f32::from(shape), depth, offset, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "LFO",
            subtitle: "LOW FREQUENCY",
            color: panel::AMBER,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("rate", "RATE", [0.001, 40.0], "Hz"),
                Ctl::pick("shape", "SHAPE", &["SINE", "SAW", "PULSE", "TRI"]),
                Ctl::dial("depth", "DEPTH", [-16.0, 16.0], ""),
                Ctl::dial("offset", "OFFSET", [-16.0, 16.0], ""),
            ],
        );
    }
}
