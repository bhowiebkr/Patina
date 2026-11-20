use panel::{Live, Look, Strip};
use patina_sdk::{AudioBlock, audio::StereoBlock};

/// Joins two mono channels. Connect the same mono output twice for a centered signal.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Pair {
    live: Live,
}
impl Pair {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
        }
    }
    pub fn process(&mut self, left: &AudioBlock, right: &AudioBlock) -> StereoBlock {
        self.live.keep(left, right, [0.0; 5]);
        StereoBlock {
            left: left.clone(),
            right: right.clone(),
        }
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "STEREO PAIR",
            subtitle: "TWO MONO TO STEREO",
            color: panel::TEAL,
        };
        gui.show(ui, shown, look, &[]);
    }
}
