use super::{Color, RATE, Space, finite, tap};
use crate::processors::Integrators;
use patina_sdk::AudioBlock;

pub struct Filter {
    core: Integrators,
    phase: f32,
    cutoff: f32,
    resonance: f32,
}
impl Default for Filter {
    fn default() -> Self {
        Self {
            core: Integrators::default(),
            phase: 0.0,
            cutoff: 1600.0,
            resonance: 0.4,
        }
    }
}
impl Filter {
    pub fn process(&mut self, input: &AudioBlock, settings: &Color) -> AudioBlock {
        let base = finite(settings.cutoff, 1600.0, 60.0, 16000.0);
        let resonance = finite(settings.resonance, 0.4, 0.0, 0.95);
        let depth = finite(settings.motion, 1.2, 0.0, 4.0);
        let rate = finite(settings.rate, 0.18, 0.02, 8.0) / RATE;
        AudioBlock::from_fn(|i| {
            self.phase = (self.phase + rate).fract();
            let target = (base * 2.0_f32.powf((self.phase * std::f32::consts::TAU).sin() * depth))
                .clamp(40.0, 18000.0);
            self.cutoff += (target - self.cutoff) * 0.002;
            self.resonance += (resonance - self.resonance) * 0.002;
            let g = (std::f32::consts::PI * self.cutoff / RATE).tan();
            let k = 2.0 - 1.9 * self.resonance;
            self.core.next(input[i], g, k)[0]
        })
    }
}

pub struct Echo {
    buffer: Box<[f32]>,
    cursor: usize,
    delay: f32,
    feedback: f32,
    mix: f32,
    low: f32,
}
impl Default for Echo {
    fn default() -> Self {
        Self {
            buffer: vec![0.0; 96_002].into_boxed_slice(),
            cursor: 0,
            delay: RATE * 0.402,
            feedback: 0.42,
            mix: 0.3,
            low: 0.0,
        }
    }
}
impl Echo {
    pub fn process(&mut self, input: &AudioBlock, settings: &Space) -> AudioBlock {
        let delay = finite(settings.seconds, 0.402, 0.02, 1.5) * RATE;
        let feedback = finite(settings.feedback, 0.42, 0.0, 0.85);
        let mix = finite(settings.mix, 0.3, 0.0, 0.8);
        let len = self.buffer.len();
        AudioBlock::from_fn(|i| {
            self.delay += (delay - self.delay) * 0.0002;
            self.feedback += (feedback - self.feedback) * 0.002;
            self.mix += (mix - self.mix) * 0.002;
            let wet = tap(&self.buffer, self.cursor as f32 - self.delay);
            self.low += 0.25 * (wet - self.low);
            self.buffer[self.cursor] = input[i] + self.low * self.feedback;
            self.cursor = (self.cursor + 1) % len;
            input[i] * (1.0 - self.mix * 0.5) + wet * self.mix
        })
    }
}
