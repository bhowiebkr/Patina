use panel::{Live, Look, Strip};
use patina_sdk::AudioBlock;

/// Holds a sample when the trigger crosses above zero. Noise plus a pulse LFO gives stepped random modulation.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Module {
    live: Live,
    engine: synth_dsp::SampleHold,
}
impl Module {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            engine: Default::default(),
        }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a graph input.
    pub fn process(&mut self, signal: &AudioBlock, trigger: &AudioBlock) -> AudioBlock {
        let out = self.engine.process(signal, trigger);
        self.live.keep(&out, &out, [0.0, 0.0, 0.0, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "SAMPLE HOLD",
            subtitle: "STEP ON TRIGGER",
            color: panel::TEAL,
        };
        gui.show(ui, shown, look, &[]);
    }
}
