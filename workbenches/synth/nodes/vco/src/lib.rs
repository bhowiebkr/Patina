use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Independent oscillator. Pitch CV uses octaves. Shapes: 0 sine, 1 saw, 2 pulse, 3 triangle.
#[patina_sdk::node(domain = audio, free_layout, color = "#655219")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Vco,
}
impl Module {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, pitch: &AudioBlock, hz: f32, depth: f32, shape: u8) -> AudioBlock {
        let out = self.engine.process(pitch, hz, depth, shape);
        self.live
            .keep(&out, &out, [hz, depth, f32::from(shape), 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "VCO",
            subtitle: "OSCILLATOR",
            color: panel::AMBER,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("hz", "PITCH", [20.0, 16_000.0], "Hz"),
                Ctl::dial("depth", "DEPTH", [-8.0, 8.0], ""),
                Ctl::pick("shape", "SHAPE", &["SINE", "SAW", "PULSE", "TRI"]),
            ],
        );
    }
}
