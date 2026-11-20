use panel::{Live, Look, Strip};
/// Makes two detuned oscillators and a sine sub oscillator. This is the raw sound.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Oscillator {
    live: Live,
    capture: synth_dsp::capture::Capture,
    engine: synth_dsp::Oscillator,
}

impl Oscillator {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            capture: synth_dsp::capture::Capture::new("Oscillator"),
            engine: Default::default(),
        }
    }
    pub fn process(
        &mut self,
        notes: &synth_dsp::Notes,
        tone: &synth_dsp::Tone,
    ) -> patina_sdk::AudioBlock {
        let sound = self.engine.process(notes, tone);
        self.capture.push(&sound);
        self.live.keep(&sound, &sound, [0.0, 0.0, 0.0, 0.0, 0.0]);
        sound
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "OSCILLATOR",
            subtitle: "TWO SAWS AND SUB",
            color: panel::AMBER,
        };
        gui.show(ui, shown, look, &[]);
    }
}
