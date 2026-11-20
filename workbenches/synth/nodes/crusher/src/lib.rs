use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Lo-fi effect. Bits sets resolution. Rate sets the held sample rate in Hz. Aliasing is intentional.
#[patina_sdk::node(domain = audio, free_layout, color = "#59366f")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Crusher,
}
impl Module {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, sound: &AudioBlock, bits: u8, rate: f32, mix: f32) -> AudioBlock {
        let out = self.engine.process(sound, bits, rate, mix);
        self.live
            .keep(&out, &out, [f32::from(bits), rate, mix, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "CRUSHER",
            subtitle: "BITS AND RATE",
            color: panel::VIOLET,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::whole("bits", "BITS", [2.0, 16.0]),
                Ctl::dial("rate", "RATE", [500.0, 48_000.0], "Hz"),
                Ctl::dial("mix", "MIX", [0.0, 1.0], ""),
            ],
        );
    }
}
