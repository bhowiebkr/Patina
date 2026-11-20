use patina_sdk::audio::{self, SAMPLE_RATE};
use patina_sdk::{AudioBlock, node};

/// The loudness at a volume of 15.
const LOUDEST: f32 = 0.25;

/// A square wave, as the buzzer of Digital Logic Sim 2. `pitch` is a MIDI note number: 69 is the
/// A at 440 Hz, and a value above 127 plays 127. The low 4 bits of `volume` set the loudness, and
/// 0 is silent.
#[node(domain = audio, color = "#59366f")]
pub struct Buzzer {
    /// The part of the period at the next sample, from 0 to below 1.
    phase: f32,
}

impl Buzzer {
    pub fn init() -> Self {
        Buzzer { phase: 0.0 }
    }

    pub fn process(&mut self, pitch: u8, volume: u8) {
        audio::play(&self.tone(pitch, volume));
    }

    fn tone(&mut self, pitch: u8, volume: u8) -> AudioBlock {
        let level = f32::from(volume & 15) / 15.0 * LOUDEST;
        let step = 440.0 * 2f32.powf((f32::from(pitch.min(127)) - 69.0) / 12.0) / SAMPLE_RATE as f32;
        AudioBlock::from_fn(|_| {
            let sample = if self.phase < 0.5 { level } else { -level };
            self.phase = (self.phase + step).fract();
            sample
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_buzzer_plays_a_square_wave_at_the_pitch() {
        let mut buzzer = Buzzer::init();
        let samples: Vec<f32> = (0..8).flat_map(|_| buzzer.tone(69, 15).to_vec()).collect();
        assert!(samples.iter().all(|s| s.abs() == LOUDEST));
        // A period of 440 Hz is 109.1 samples, so each rising edge is 109 or 110 samples after the one before.
        let rises: Vec<usize> = (1..samples.len()).filter(|&i| samples[i] > samples[i - 1]).collect();
        assert!(rises.len() > 10 && rises.windows(2).all(|w| matches!(w[1] - w[0], 109 | 110)), "{rises:?}");
        assert_eq!(buzzer.tone(69, 16).peak(), 0.0);
        assert_eq!(buzzer.tone(255, 0x13).peak(), 3.0 / 15.0 * LOUDEST);
    }
}
