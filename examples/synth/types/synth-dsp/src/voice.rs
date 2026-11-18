use super::{Contour, Notes, RATE, Tone, finite};
use patina_sdk::AudioBlock;
use std::f32::consts::TAU;

#[derive(Default)]
pub struct Oscillator {
    phase: [f32; 3],
    hz: f32,
    detune: f32,
    sub: f32,
    shapes: [WaveShape; 2],
}
impl Oscillator {
    pub fn process(&mut self, notes: &Notes, settings: &Tone) -> AudioBlock {
        let octave = 2.0_f32.powi(i32::from(settings.octave.clamp(-2, 2)));
        let detune = 2.0_f32.powf(finite(settings.detune, 9.0, 0.0, 30.0) / 1200.0);
        let sub = finite(settings.sub, 0.28, 0.0, 1.0);
        AudioBlock::from_fn(|i| {
            let hz = finite(notes.hz[i] * octave, 130.81, 20.0, 8000.0);
            if self.hz == 0.0 {
                self.hz = hz;
                self.detune = detune;
            }
            self.hz += (hz - self.hz) * 0.015;
            self.detune += (detune - self.detune) * 0.002;
            self.sub += (sub - self.sub) * 0.002;
            let delta = [
                self.hz / RATE,
                self.hz * self.detune / RATE,
                self.hz * 0.5 / RATE,
            ];
            let a = self.shapes[0].next(self.phase[0], delta[0], settings.shape);
            let b = self.shapes[1].next(self.phase[1], delta[1], settings.shape);
            let bass = (self.phase[2] * TAU).sin();
            for (phase, d) in self.phase.iter_mut().zip(delta) {
                *phase = (*phase + d).fract();
            }
            ((a + b) * 0.5 + bass * self.sub) / (1.0 + self.sub)
        })
    }
}

/// A polynomial correction removes the sharp jump of saw and pulse waves.
fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

pub fn wave(t: f32, dt: f32, shape: u8) -> f32 {
    match shape {
        0 => (t * TAU).sin(),
        1 => t * 2.0 - 1.0 - blep(t, dt),
        2 => (if t < 0.5 { 1.0 } else { -1.0 }) + blep(t, dt) - blep((t + 0.5).fract(), dt),
        _ => {
            let mut sum = 0.0;
            for harmonic in (1..=31).step_by(2) {
                let h = harmonic as f32;
                if h * dt >= 0.5 {
                    break;
                }
                sum += (TAU * t * h).cos() / (h * h);
            }
            -8.0 / std::f32::consts::PI.powi(2) * sum
        }
    }
}

#[derive(Default)]
pub struct Envelope {
    level: f32,
    stage: u8,
    released: f32,
}
impl Envelope {
    pub fn process(&mut self, input: &AudioBlock, notes: &Notes, settings: &Contour) -> AudioBlock {
        let attack = finite(settings.attack, 0.008, 0.001, 2.0) * RATE;
        let decay = finite(settings.decay, 0.2, 0.005, 2.0) * RATE;
        let sustain = finite(settings.sustain, 0.35, 0.0, 1.0);
        let release = finite(settings.release, 0.24, 0.005, 3.0) * RATE;
        AudioBlock::from_fn(|i| {
            if notes.trigger[i] {
                self.stage = 1;
            }
            if notes.gate[i] == 0.0 && self.stage != 4 {
                self.stage = 4;
                self.released = self.level;
            }
            match self.stage {
                1 => {
                    self.level += 1.0 / attack;
                    if self.level >= 1.0 {
                        self.level = 1.0;
                        self.stage = 2;
                    }
                }
                2 => {
                    self.level -= (1.0 - sustain) / decay;
                    if self.level <= sustain {
                        self.level = sustain;
                        self.stage = 3;
                    }
                }
                3 => self.level += (sustain - self.level) * 0.002,
                4 => self.level = (self.level - self.released / release).max(0.0),
                _ => {}
            }
            input[i] * self.level * 0.65
        })
    }
}

/// A five-millisecond crossfade prevents an abrupt change when a waveform button changes.
#[derive(Default)]
pub(crate) struct WaveShape {
    current: u8,
    previous: u8,
    blend: f32,
    ready: bool,
}
impl WaveShape {
    pub(crate) fn next(&mut self, phase: f32, step: f32, shape: u8) -> f32 {
        let shape = shape.min(3);
        if !self.ready {
            self.current = shape;
            self.previous = shape;
            self.blend = 1.0;
            self.ready = true;
        }
        if shape != self.current {
            self.previous = self.current;
            self.current = shape;
            self.blend = 0.0;
        }
        self.blend = (self.blend + 1.0 / 240.0).min(1.0);
        let current = wave(phase, step, self.current);
        if self.blend == 1.0 {
            current
        } else {
            wave(phase, step, self.previous) * (1.0 - self.blend) + current * self.blend
        }
    }
}
