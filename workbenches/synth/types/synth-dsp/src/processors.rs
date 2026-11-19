use crate::{RATE, finite};
use patina_sdk::AudioBlock;

#[derive(Clone, Default)]
pub(crate) struct Integrators {
    band: f32,
    low: f32,
}
impl Integrators {
    pub(crate) fn next(&mut self, input: f32, g: f32, k: f32) -> [f32; 3] {
        let x = finite(input, 0.0, -32.0, 32.0);
        let band = (self.band + g * (x - self.low)) / (1.0 + g * (g + k));
        let low = self.low + g * band;
        self.band = 2.0 * band - self.band;
        self.low = 2.0 * low - self.low;
        [low, band, x - k * band - low]
    }
}

#[derive(Default)]
pub struct Svf {
    core: Integrators,
    cutoff: f32,
    resonance: f32,
}
impl Svf {
    #[allow(clippy::too_many_arguments)]
    pub fn process(
        &mut self,
        sound: &AudioBlock,
        cv: &AudioBlock,
        cutoff: f32,
        resonance: f32,
        depth: f32,
        mode: u8,
    ) -> AudioBlock {
        let cutoff = finite(cutoff, 1000.0, 20.0, 18_000.0);
        let resonance = finite(resonance, 0.3, 0.0, 0.95);
        let depth = finite(depth, 1.0, -8.0, 8.0);
        AudioBlock::from_fn(|i| {
            let target = (cutoff * 2.0_f32.powf(finite(cv[i], 0.0, -8.0, 8.0) * depth))
                .clamp(20.0, 18_000.0);
            self.cutoff += (target - self.cutoff) * 0.002;
            self.resonance += (resonance - self.resonance) * 0.002;
            let g = (std::f32::consts::PI * self.cutoff / RATE).tan();
            let k = 2.0 - 1.9 * self.resonance;
            let [low, band, high] = self.core.next(sound[i], g, k);
            match mode {
                1 => high,
                2 => k * band,
                3 => high + low,
                _ => low,
            }
        })
    }
}

#[derive(Default)]
pub struct Saturator {
    previous: f64,
    drive: f32,
    mix: f32,
}
impl Saturator {
    /// First-order antiderivative antialiasing, with the midpoint limit near equal inputs.
    pub fn process(&mut self, sound: &AudioBlock, drive: f32, mix: f32) -> AudioBlock {
        let drive = finite(drive, 1.0, 1.0, 20.0);
        let mix = finite(mix, 0.0, 0.0, 1.0);
        AudioBlock::from_fn(|i| {
            self.drive += (drive - self.drive) * 0.002;
            self.mix += (mix - self.mix) * 0.002;
            let dry = finite(sound[i], 0.0, -32.0, 32.0);
            let x = f64::from(dry * self.drive);
            let integral = |x: f64| x.abs() + (-2.0 * x.abs()).exp().ln_1p();
            let delta = x - self.previous;
            let wet = if delta.abs() < 0.00001 {
                ((x + self.previous) * 0.5).tanh()
            } else {
                (integral(x) - integral(self.previous)) / delta
            };
            self.previous = x;
            dry + (wet as f32 - dry) * self.mix
        })
    }
}

#[derive(Default)]
pub struct Crusher {
    phase: f32,
    held: f32,
    mix: f32,
}
impl Crusher {
    /// Reduced bit depth and sample rate deliberately add quantization and aliasing.
    pub fn process(&mut self, sound: &AudioBlock, bits: u8, rate: f32, mix: f32) -> AudioBlock {
        let levels = (1_u32 << bits.clamp(2, 16)) as f32 * 0.5;
        let step = finite(rate, 12_000.0, 500.0, RATE) / RATE;
        let mix = finite(mix, 0.0, 0.0, 1.0);
        AudioBlock::from_fn(|i| {
            let dry = finite(sound[i], 0.0, -32.0, 32.0);
            self.phase += step;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
                self.held = (dry.clamp(-1.0, 1.0) * levels).round() / levels;
            }
            self.mix += (mix - self.mix) * 0.002;
            dry + (self.held - dry) * self.mix
        })
    }
}

#[derive(Clone, Copy)]
pub struct Dynamics {
    pub threshold: f32,
    pub ratio: f32,
    pub attack: f32,
    pub release: f32,
    pub makeup: f32,
}
#[derive(Default)]
pub struct Compressor {
    envelope: f32,
    gain: f32,
}
impl Compressor {
    pub fn process(&mut self, sound: &AudioBlock, settings: Dynamics) -> AudioBlock {
        let threshold = finite(settings.threshold, -18.0, -60.0, 0.0);
        let ratio = finite(settings.ratio, 4.0, 1.0, 20.0);
        let attack = (-1.0 / (finite(settings.attack, 0.01, 0.0001, 1.0) * RATE)).exp();
        let release = (-1.0 / (finite(settings.release, 0.1, 0.005, 3.0) * RATE)).exp();
        let makeup = finite(settings.makeup, 0.0, -24.0, 24.0);
        AudioBlock::from_fn(|i| {
            let x = finite(sound[i], 0.0, -32.0, 32.0);
            let coefficient = if x.abs() > self.envelope {
                attack
            } else {
                release
            };
            self.envelope = x.abs() + coefficient * (self.envelope - x.abs());
            let over = 20.0 * self.envelope.max(0.000001).log10() - threshold;
            let reduction = if over < -3.0 {
                0.0
            } else if over < 3.0 {
                (over + 3.0).powi(2) / 12.0
            } else {
                over
            };
            let target = 10.0_f32.powf((makeup - reduction * (1.0 - 1.0 / ratio)) / 20.0);
            self.gain += (target - self.gain) * 0.002;
            x * self.gain
        })
    }
}
