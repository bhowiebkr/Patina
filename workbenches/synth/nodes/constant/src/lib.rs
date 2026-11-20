use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;

/// A constant audio or control block. Zero supplies silence to unused audio inputs. One supplies a full gate.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Constant {
    live: Live,
}
impl Constant {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
        }
    }
    pub fn process(&mut self, value: f32) -> AudioBlock {
        let value = synth_dsp::finite(value, 0.0, -32.0, 32.0);
        let out = AudioBlock::from_fn(|_| value);
        self.live.keep(&out, &out, [value, 0.0, 0.0, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "CONSTANT",
            subtitle: "FIXED LEVEL",
            color: panel::TEAL,
        };
        gui.show(
            ui,
            shown,
            look,
            &[Ctl::dial("value", "VALUE", [-32.0, 32.0], "")],
        );
    }
}
