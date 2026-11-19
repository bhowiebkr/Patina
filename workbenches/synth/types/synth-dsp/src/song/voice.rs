use super::{Instrument, NoteEvent, Track, wav::Sample};
use crate::{RATE, frequency, voice::wave};
use std::f32::consts::TAU;

#[derive(Clone, Default)]
pub struct Voice {
    pub active: bool,
    pub end: u64,
    pub level: f32,
    key: u8,
    velocity: f32,
    phase: [f32; 3],
    age: u64,
    release: bool,
    noise: u32,
    highpass: f32,
    rave: crate::RaveOscillator,
}

impl Voice {
    pub fn start(&mut self, note: NoteEvent, elapsed: u64) {
        *self = Self {
            active: true,
            end: note.end,
            key: note.key,
            velocity: note.velocity,
            age: elapsed + note.offset,
            noise: (note.start as u32 ^ u32::from(note.key)).wrapping_add(0x9e3779b9),
            ..Self::default()
        };
        let cycles =
            (elapsed + note.offset) as f64 * f64::from(frequency(note.key)) / f64::from(RATE);
        self.phase = [
            cycles.fract() as f32,
            (cycles * 1.004).fract() as f32,
            (cycles * 0.996).fract() as f32,
        ];
        self.rave.reset(frequency(note.key), elapsed + note.offset);
    }

    pub fn release(&mut self) {
        self.release = true;
    }

    pub fn next(&mut self, track: &Track, sample: Option<&Sample>) -> [f32; 2] {
        if !self.active {
            return [0.0; 2];
        }
        let age = self.age as f32 / RATE;
        if self.release {
            self.level *= (-6.91 / (track.release * RATE)).exp();
            if self.level < 0.00001 {
                self.active = false;
                return [0.0; 2];
            }
        } else {
            let sustain = if track.instrument == Instrument::Pad {
                0.85
            } else {
                0.5
            };
            self.level = if age < track.attack {
                age / track.attack
            } else {
                sustain + (1.0 - sustain) * (-(age - track.attack) * 14.0).exp()
            };
        }
        let hz = frequency(self.key);
        let dt = (hz / RATE).min(0.45);
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        let noise = self.noise as f32 / u32::MAX as f32 * 2.0 - 1.0;
        let high = noise - self.highpass;
        self.highpass += (noise - self.highpass) * 0.3;
        let mut pair = match track.instrument {
            Instrument::Rave => [self.rave.next(hz, track.rave); 2],
            Instrument::Lead | Instrument::Pad => {
                let shape = if track.instrument == Instrument::Lead {
                    2
                } else {
                    1
                };
                let a = wave(self.phase[0], dt, shape);
                let b = wave(self.phase[1], dt * 1.004, 1);
                let c = wave(self.phase[2], dt * 0.996, 1);
                [(a * 0.6 + b * 0.4) * 0.34, (a * 0.6 + c * 0.4) * 0.34]
            }
            Instrument::Bass => {
                let value =
                    (wave(self.phase[0], dt, 1) * 0.5 + (TAU * self.phase[2]).sin() * 0.5) * 0.48;
                [value; 2]
            }
            Instrument::Kick => {
                let phase = 48.0 * age + 120.0 * (1.0 - (-age * 35.0).exp()) / 35.0;
                let value =
                    (TAU * phase).sin() * (-age * 8.0).exp() + noise * (-age * 700.0).exp() * 0.2;
                [value * 0.85; 2]
            }
            Instrument::Snare => {
                let value = (TAU * 180.0 * age).sin() * (-age * 25.0).exp() * 0.25
                    + high * (-age * 18.0).exp() * 0.5;
                [value; 2]
            }
            Instrument::Hat => {
                let value = high * (-age * 55.0).exp() * 0.3;
                [value; 2]
            }
            Instrument::Sampler => {
                let Some(sample) = sample else {
                    self.active = false;
                    return [0.0; 2];
                };
                let step = 2.0_f64.powf((f64::from(self.key) - f64::from(track.root_note)) / 12.0)
                    * f64::from(sample.rate)
                    / f64::from(RATE);
                if self.age as f64 * step >= sample.frames.len() as f64 {
                    self.active = false;
                    return [0.0; 2];
                }
                sample.at(self.age as f64 * step, step)
            }
        };
        for (phase, factor) in self.phase.iter_mut().zip([1.0, 1.004, 0.996]) {
            *phase = (*phase + dt * factor).fract();
        }
        for value in &mut pair {
            *value *= self.level * self.velocity;
        }
        self.age += 1;
        pair
    }
}
