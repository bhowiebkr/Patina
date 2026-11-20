use panel::{Ctl, Live, Look, Strip};
/// A held note at any frequency. A rising gate retriggers its envelope.
#[patina_sdk::node(domain = audio, free_layout, color = "#655219")]
pub struct Note {
    live: Live,
    held: bool,
}
impl Note {
    pub fn init() -> Self {
        Self {
            held: false,
            live: Live::default(),
        }
    }
    pub fn process(&mut self, hz: f32, gate: bool) -> synth_dsp::Notes {
        let mut notes = synth_dsp::Notes {
            hz: [synth_dsp::finite(hz, 110.0, 10.0, 16_000.0); 256],
            gate: [f32::from(gate); 256],
            ..Default::default()
        };
        notes.trigger[0] = gate && !self.held;
        self.held = gate;
        self.live.keep(
            &notes.gate,
            &notes.gate,
            [hz, f32::from(u8::from(gate)), 0.0, 0.0, 0.0],
        );
        notes
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "NOTE",
            subtitle: "PITCH AND GATE",
            color: panel::AMBER,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("hz", "PITCH", [10.0, 16_000.0], "Hz"),
                Ctl::switch("gate", "GATE"),
            ],
        );
    }
}
