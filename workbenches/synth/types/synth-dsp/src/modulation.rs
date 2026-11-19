use crate::{
    Contour, Envelope, Notes, RATE, finite,
    voice::{WaveShape, wave},
};
use patina_sdk::AudioBlock;

#[derive(Default)]
pub struct Lfo {
    phase: f32,
    rate: f32,
}
impl Lfo {
    pub fn process(&mut self, rate: f32, shape: u8, depth: f32, offset: f32) -> AudioBlock {
        let rate = finite(rate, 1.0, 0.001, 40.0);
        let depth = finite(depth, 1.0, -16.0, 16.0);
        let offset = finite(offset, 0.0, -16.0, 16.0);
        AudioBlock::from_fn(|_| {
            self.rate += (rate - self.rate) * 0.002;
            self.phase = (self.phase + self.rate / RATE).fract();
            offset + depth * wave(self.phase, self.rate / RATE, shape)
        })
    }
}

#[derive(Default)]
pub struct Vco {
    phase: f32,
    hz: f32,
    shape: WaveShape,
}
impl Vco {
    /// Pitch modulation uses octaves. One unit doubles the oscillator frequency.
    pub fn process(&mut self, pitch: &AudioBlock, hz: f32, depth: f32, shape: u8) -> AudioBlock {
        let hz = finite(hz, 110.0, 20.0, 16_000.0);
        let depth = finite(depth, 1.0, -8.0, 8.0);
        AudioBlock::from_fn(|i| {
            if self.hz == 0.0 {
                self.hz = hz;
            }
            self.hz += (hz - self.hz) * 0.002;
            let step = (self.hz * 2.0_f32.powf(finite(pitch[i], 0.0, -8.0, 8.0) * depth))
                .clamp(10.0, 16_000.0)
                / RATE;
            let value = self.shape.next(self.phase, step, shape);
            self.phase = (self.phase + step).fract();
            value
        })
    }
}

pub struct Adsr {
    engine: Envelope,
    ones: AudioBlock,
}
impl Default for Adsr {
    fn default() -> Self {
        Self {
            engine: Envelope::default(),
            ones: AudioBlock::from_fn(|_| 1.0),
        }
    }
}
impl Adsr {
    pub fn process(&mut self, notes: &Notes, contour: &Contour) -> AudioBlock {
        let mut block = self.engine.process(&self.ones, notes, contour);
        for sample in block.iter_mut() {
            *sample /= 0.65;
        }
        block
    }
}

#[derive(Default)]
pub struct Vca {
    gain: f32,
}
impl Vca {
    pub fn process(
        &mut self,
        sound: &AudioBlock,
        cv: &AudioBlock,
        level: f32,
        bias: f32,
    ) -> AudioBlock {
        let level = finite(level, 0.0, 0.0, 4.0);
        let bias = finite(bias, 0.0, 0.0, 1.0);
        AudioBlock::from_fn(|i| {
            self.gain += (level - self.gain) * 0.002;
            finite(sound[i], 0.0, -32.0, 32.0)
                * (bias + finite(cv[i], 0.0, -1.0, 1.0)).clamp(0.0, 1.0)
                * self.gain
        })
    }
}

#[derive(Default)]
pub struct Mixer {
    gain: [f32; 4],
}
impl Mixer {
    pub fn process(&mut self, inputs: [&AudioBlock; 4], levels: [f32; 4]) -> AudioBlock {
        let levels = levels.map(|v| finite(v, 0.0, -2.0, 2.0));
        AudioBlock::from_fn(|i| {
            let mut sum = 0.0;
            for ((gain, target), input) in self.gain.iter_mut().zip(levels).zip(inputs) {
                *gain += (target - *gain) * 0.002;
                sum += finite(input[i], 0.0, -32.0, 32.0) * *gain;
            }
            sum
        })
    }
}

#[derive(Default)]
pub struct Attenuverter {
    scale: f32,
    offset: f32,
}
impl Attenuverter {
    pub fn process(&mut self, signal: &AudioBlock, scale: f32, offset: f32) -> AudioBlock {
        let scale = finite(scale, 1.0, -8.0, 8.0);
        let offset = finite(offset, 0.0, -16.0, 16.0);
        AudioBlock::from_fn(|i| {
            self.scale += (scale - self.scale) * 0.002;
            self.offset += (offset - self.offset) * 0.002;
            finite(signal[i], 0.0, -32.0, 32.0) * self.scale + self.offset
        })
    }
}

#[derive(Default)]
pub struct SampleHold {
    previous: bool,
    held: f32,
}
impl SampleHold {
    pub fn process(&mut self, signal: &AudioBlock, trigger: &AudioBlock) -> AudioBlock {
        AudioBlock::from_fn(|i| {
            let high = trigger[i] > 0.0;
            if high && !self.previous {
                self.held = finite(signal[i], 0.0, -32.0, 32.0);
            }
            self.previous = high;
            self.held
        })
    }
}

#[derive(Default)]
pub struct Slew {
    value: f32,
}
impl Slew {
    pub fn process(&mut self, signal: &AudioBlock, rise: f32, fall: f32) -> AudioBlock {
        let coefficient =
            |seconds| 1.0 - (-1.0 / (finite(seconds, 0.1, 0.0001, 10.0) * RATE)).exp();
        let (rise, fall) = (coefficient(rise), coefficient(fall));
        AudioBlock::from_fn(|i| {
            let target = finite(signal[i], 0.0, -32.0, 32.0);
            self.value += (target - self.value) * if target > self.value { rise } else { fall };
            self.value
        })
    }
}

pub struct Noise {
    random: u32,
    seed: u32,
    count: u32,
    pink: [f32; 8],
    brown: f32,
    level: f32,
}
impl Default for Noise {
    fn default() -> Self {
        Self {
            random: 1,
            seed: 0,
            count: 0,
            pink: [0.0; 8],
            brown: 0.0,
            level: 0.0,
        }
    }
}
impl Noise {
    fn next(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        self.random as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
    pub fn process(&mut self, color: u8, level: f32, seed: u32) -> AudioBlock {
        if self.seed != seed {
            self.seed = seed;
            self.random = seed.max(1);
        }
        let level = finite(level, 0.2, 0.0, 1.0);
        AudioBlock::from_fn(|_| {
            let white = self.next();
            self.count = self.count.wrapping_add(1);
            let index = self.count.trailing_zeros() as usize;
            if index < self.pink.len() {
                self.pink[index] = self.next();
            }
            self.brown = (self.brown * 0.995 + white * 0.03).clamp(-1.0, 1.0);
            self.level += (level - self.level) * 0.002;
            let value = match color {
                1 => (self.pink.iter().sum::<f32>() + white) / 4.0,
                2 => self.brown * 3.0,
                _ => white,
            };
            value * self.level
        })
    }
}

#[derive(Default)]
pub struct DcBlock {
    input: f32,
    output: f32,
}
impl DcBlock {
    pub fn process(&mut self, sound: &AudioBlock) -> AudioBlock {
        AudioBlock::from_fn(|i| {
            let x = finite(sound[i], 0.0, -32.0, 32.0);
            self.output = x - self.input + 0.997_385 * self.output;
            self.input = x;
            self.output
        })
    }
}
