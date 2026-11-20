use panel::{Live, Look, Strip};
use patina_sdk::audio::{BLOCK, StereoBlock};

/// Adds two stereo channels without changing their pan or gain.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Mix {
    live: Live,
}
impl Mix {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
        }
    }
    pub fn process(&mut self, a: &StereoBlock, b: &StereoBlock) -> StereoBlock {
        let mut out = StereoBlock::default();
        for i in 0..BLOCK {
            out.left[i] = a.left[i] + b.left[i];
            out.right[i] = a.right[i] + b.right[i];
        }
        self.live.keep(&out.left, &out.right, [0.0; 5]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "STEREO MIX",
            subtitle: "SUM OF TWO",
            color: panel::TEAL,
        };
        gui.show(ui, shown, look, &[]);
    }
}
