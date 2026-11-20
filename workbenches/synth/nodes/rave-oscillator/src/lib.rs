use panel::{Ctl, Live, Look, Strip};
use patina_sdk::AudioBlock;
use synth_dsp::{Notes, RaveOscillator, RaveTone};

/// A pulse at the played pitch and a low saw 31 semitones below it.
/// Start with fifth 1, body 4600 Hz, 8 bits, and edge 1.05. Follow with Filter and Envelope.
#[patina_sdk::node(domain = audio, free_layout, color = "#175c59")]
pub struct Rave {
    live: Live,
    source: RaveOscillator,
    capture: synth_dsp::capture::Capture,
}

impl Rave {
    pub fn init() -> Self {
        Self {
            live: Live::default(),
            source: RaveOscillator::default(),
            capture: synth_dsp::capture::Capture::new("Rave source"),
        }
    }

    pub fn process(
        &mut self,
        notes: &Notes,
        fifth: f32,
        body: f32,
        bits: u8,
        edge: f32,
    ) -> AudioBlock {
        let tone = RaveTone { fifth, body, bits };
        let sound = AudioBlock::from_fn(|i| {
            if notes.trigger[i] {
                self.source.reset(notes.hz[i], 0);
            }
            self.source.next_driven(notes.hz[i], tone, edge)
        });
        self.capture.push(&sound);
        self.live
            .keep(&sound, &sound, [fifth, body, f32::from(bits), edge, 0.0]);
        sound
    }
    pub fn display(&self) -> Live {
        self.live
    }
    pub fn ui(gui: &mut Strip, shown: &Live, ui: &mut patina_sdk::Ui) {
        let look = Look {
            title: "RAVE",
            subtitle: "PULSE AND SAW",
            color: panel::AMBER,
        };
        gui.show(
            ui,
            shown,
            look,
            &[
                Ctl::dial("fifth", "FIFTH", [0.0, 1.0], ""),
                Ctl::dial("body", "BODY", [200.0, 8000.0], "Hz"),
                Ctl::whole("bits", "BITS", [4.0, 16.0]),
                Ctl::dial("edge", "EDGE", [1.0, 16.0], ""),
            ],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_trigger_restarts_the_source() {
        let mut node = Rave::init();
        let mut notes = Notes::default();
        notes.hz.fill(246.94165);
        notes.trigger[0] = true;
        let first = node.process(&notes, 0.42, 3200.0, 8, 10.0);
        let second = node.process(&notes, 0.42, 3200.0, 8, 10.0);
        assert!(first.iter().zip(second.iter()).all(|(a, b)| a == b));
        assert!(first.peak() > 0.1);
    }
}
