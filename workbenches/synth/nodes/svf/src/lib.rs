use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Modulated filter. Modes: 0 low-pass, 1 high-pass, 2 band-pass, 3 notch. CV depth uses octaves.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Module {
    live: Live,
    engine: synth_dsp::Svf,
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
        cutoff: f32,
        resonance: f32,
        depth: f32,
        mode: u8,
    ) -> AudioBlock {
        let out = self
            .engine
            .process(sound, cv, cutoff, resonance, depth, mode);
        self.live
            .keep(&out, &out, [cutoff, resonance, depth, f32::from(mode), 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "SVF",
            subtitle: "STATE VARIABLE",
            color: panel::TEAL,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("cutoff", "CUTOFF", [20.0, 18_000.0], "Hz"),
                Ctl::dial("resonance", "RES", [0.0, 0.95], ""),
                Ctl::dial("depth", "DEPTH", [-8.0, 8.0], ""),
                Ctl::pick("mode", "MODE", &["LOW", "HIGH", "BAND", "NOTCH"]),
            ],
        );
    }
}
