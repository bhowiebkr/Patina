use panel::{Ctl, Live, Look, Strip};
use synth_dsp::Notes;

/// Transposes pitch while preserving the gates, retriggers and drum events.
#[patina_sdk::node(domain = audio, free_layout, color = "#655219")]
pub struct Transpose {
    live: Live,
}
impl Transpose {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
        }
    }
    pub fn process(&mut self, notes: &Notes, semitones: f32) -> Notes {
        let semitones = synth_dsp::finite(semitones, 0.0, -48.0, 48.0);
        let ratio = 2.0_f32.powf(semitones / 12.0);
        let mut out = notes.clone();
        for hz in &mut out.hz {
            *hz = synth_dsp::finite(*hz * ratio, 110.0, 10.0, 16_000.0);
        }
        self.live
            .keep(&out.gate, &out.gate, [semitones, 0.0, 0.0, 0.0, 0.0]);
        out
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "TRANSPOSE",
            subtitle: "SHIFT PITCH",
            color: panel::AMBER,
        };
        gui.show(
            ui,
            shown,
            look,
            &[Ctl::whole("semitones", "SEMI", [-48.0, 48.0])],
        );
    }
}
