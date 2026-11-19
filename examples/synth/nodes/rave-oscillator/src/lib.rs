use patina_sdk::AudioBlock;
use synth_dsp::{Notes, RaveOscillator, RaveTone};

/// A root-and-fifth source. Start with fifth 0.58, body 1500 Hz, and 8 bits.
/// Follow it with Envelope, Saturator, and Filter to shape the distorted lead.
#[patina_sdk::node(domain = audio, color = "#175c59")]
pub struct Rave {
    source: RaveOscillator,
    capture: synth_dsp::capture::Capture,
}

impl Rave {
    pub fn init() -> Self {
        Self {
            source: RaveOscillator::default(),
            capture: synth_dsp::capture::Capture::new("Rave source"),
        }
    }

    pub fn process(&mut self, notes: &Notes, fifth: f32, body: f32, bits: u8) -> AudioBlock {
        let tone = RaveTone { fifth, body, bits };
        let sound = AudioBlock::from_fn(|i| {
            if notes.trigger[i] {
                self.source.reset(notes.hz[i], 0);
            }
            self.source.next(notes.hz[i], tone)
        });
        self.capture.push(&sound);
        sound
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
        let first = node.process(&notes, 0.58, 1500.0, 8);
        let second = node.process(&notes, 0.58, 1500.0, 8);
        assert!(first.iter().zip(second.iter()).all(|(a, b)| a == b));
        assert!(first.peak() > 0.1);
    }
}
