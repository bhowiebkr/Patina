use crate::{RATE, finite};
use patina_sdk::AudioBlock;
use std::f32::consts::TAU;

pub struct Chorus {
    buffer: Box<[f32]>,
    cursor: usize,
    phase: f32,
    mix: f32,
    depth: f32,
}
impl Default for Chorus {
    fn default() -> Self {
        Self {
            buffer: vec![0.0; 4096].into_boxed_slice(),
            cursor: 0,
            phase: 0.0,
            mix: 0.0,
            depth: 0.0,
        }
    }
}
impl Chorus {
    pub fn process(&mut self, sound: &AudioBlock, rate: f32, depth: f32, mix: f32) -> AudioBlock {
        let rate = finite(rate, 0.3, 0.01, 8.0) / RATE;
        let depth = finite(depth, 0.004, 0.0, 0.012) * RATE;
        let mix = finite(mix, 0.3, 0.0, 1.0);
        AudioBlock::from_fn(|i| {
            self.phase = (self.phase + rate).fract();
            self.depth += (depth - self.depth) * 0.002;
            self.mix += (mix - self.mix) * 0.002;
            let mut wet = 0.0;
            for offset in [0.0, 0.25] {
                let delay = 0.022 * RATE + (TAU * (self.phase + offset)).sin() * self.depth;
                let read = (self.cursor as f32 - delay).rem_euclid(self.buffer.len() as f32);
                let index = read as usize;
                let t = read.fract();
                wet += self.buffer[index] * (1.0 - t)
                    + self.buffer[(index + 1) % self.buffer.len()] * t;
            }
            let dry = finite(sound[i], 0.0, -32.0, 32.0);
            self.buffer[self.cursor] = dry;
            self.cursor = (self.cursor + 1) % self.buffer.len();
            dry * (1.0 - self.mix * 0.5) + wet * 0.25 * self.mix
        })
    }
}

struct Delay {
    samples: Box<[f32]>,
    cursor: usize,
    low: f32,
}
impl Delay {
    fn new(length: usize) -> Self {
        Self {
            samples: vec![0.0; length].into_boxed_slice(),
            cursor: 0,
            low: 0.0,
        }
    }
    fn read(&self) -> f32 {
        self.samples[self.cursor]
    }
    fn write(&mut self, value: f32) {
        self.samples[self.cursor] = if value.abs() < 1e-20 { 0.0 } else { value };
        self.cursor = (self.cursor + 1) % self.samples.len();
    }
}

pub struct Reverb {
    comb: [Delay; 4],
    diffuse: [Delay; 2],
    feedback: [f32; 4],
    mix: f32,
    damping: f32,
}
impl Default for Reverb {
    fn default() -> Self {
        Self::with_spread(0)
    }
}
impl Reverb {
    pub(crate) fn with_spread(spread: usize) -> Self {
        Self {
            comb: [1427, 1783, 1973, 2099].map(|length| Delay::new(length + spread)),
            diffuse: [241, 79].map(|length| Delay::new(length + spread)),
            feedback: [0.0; 4],
            mix: 0.0,
            damping: 0.0,
        }
    }
}
impl Reverb {
    /// Four damped feedback combs feed two all-pass diffusers. Decay is the target T60 in seconds.
    pub fn process(
        &mut self,
        sound: &AudioBlock,
        decay: f32,
        damping: f32,
        mix: f32,
    ) -> AudioBlock {
        let decay = finite(decay, 2.0, 0.1, 12.0);
        let damping = finite(damping, 0.4, 0.0, 0.95);
        let mix = finite(mix, 0.3, 0.0, 1.0);
        let targets = self
            .comb
            .each_ref()
            .map(|line| 10.0_f32.powf(-3.0 * line.samples.len() as f32 / (RATE * decay)));
        AudioBlock::from_fn(|i| {
            let dry = finite(sound[i], 0.0, -32.0, 32.0);
            self.mix += (mix - self.mix) * 0.002;
            self.damping += (damping - self.damping) * 0.002;
            let mut wet = 0.0;
            for ((line, feedback), target) in
                self.comb.iter_mut().zip(&mut self.feedback).zip(targets)
            {
                *feedback += (target - *feedback) * 0.002;
                let value = line.read();
                line.low = value * (1.0 - self.damping) + line.low * self.damping;
                line.write(dry * 0.25 + line.low * *feedback);
                wet += value;
            }
            for line in &mut self.diffuse {
                let delayed = line.read();
                let value = wet + delayed * 0.5;
                line.write(value);
                wet = delayed - value * 0.5;
            }
            dry * (1.0 - self.mix) + wet * self.mix
        })
    }
}
