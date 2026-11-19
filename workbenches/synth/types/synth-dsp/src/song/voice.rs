use super::{Instrument, NoteEvent, Track, percussion::Percussion, sound::Prepared, wav::Sample};
use crate::{RATE, frequency, processors::Integrators, voice::wave};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Default)]
pub struct Voice {
    pub active: bool,
    pub end: u64,
    pub level: f32,
    pub key: u8,
    hz: f32,
    velocity: f32,
    phase: [f32; 7],
    age: u64,
    release: bool,
    rave: crate::RaveOscillator,
    drums: Percussion,
    filter: [Integrators; 2],
    filter_g: f32,
    filter_target: f32,
}

impl Voice {
    pub fn start(&mut self, note: NoteEvent, elapsed: u64) {
        *self = Self {
            active: true,
            end: note.end,
            key: note.key,
            hz: frequency(note.key),
            velocity: note.velocity,
            age: elapsed + note.offset,
            ..Self::default()
        };
        let cycles = self.age as f64 * f64::from(self.hz) / f64::from(RATE);
        self.phase = [0.11, 0.47, 0.73, 0.0, 0.29, 0.83, 0.61]
            .map(|offset| (cycles + offset).fract() as f32);
        self.rave.reset(self.hz, self.age);
        self.drums
            .reset((note.start as u32 ^ u32::from(note.key)).wrapping_add(0x9e3779b9));
    }

    pub fn release(&mut self) {
        self.release = true;
    }

    pub(super) fn next(
        &mut self,
        track: &Track,
        prepared: &Prepared,
        sample: Option<&Sample>,
    ) -> [f32; 2] {
        if !self.active {
            return [0.0; 2];
        }
        let age = self.age as f32 / RATE;
        if self.release {
            self.level *= prepared.release;
            if self.level < 0.00001 {
                self.active = false;
                return [0.0; 2];
            }
        } else if prepared.drum {
            self.level = 1.0;
            let tail = match track.instrument {
                Instrument::Kick => 1.3,
                Instrument::Snare => 0.7,
                _ if self.key == 46 => 1.1,
                _ => 0.2,
            };
            if age > tail * track.sound.drum_length {
                self.active = false;
                return [0.0; 2];
            }
        } else {
            self.level = if age < track.attack {
                age / track.attack
            } else {
                track.sound.sustain
                    + (1.0 - track.sound.sustain)
                        * (-3.0 * (age - track.attack) / track.sound.decay).exp()
            };
        }
        let dt = (self.hz / RATE).min(0.45);
        let mut pair = match track.instrument {
            Instrument::Rave => {
                [self.rave.next_driven(self.hz, track.rave, track.sound.edge) * 0.72; 2]
            }
            Instrument::Lead | Instrument::Pad => self.unison(track, prepared, dt),
            Instrument::Bass => {
                let source = wave(self.phase[3], dt, 1) * 0.38;
                // A rising saw has a negative sine fundamental; align the body with it.
                let sub = -(TAU * self.phase[3]).sin() * 0.5;
                let value = (source * track.sound.edge).tanh() * 0.45 + sub;
                [value; 2]
            }
            Instrument::Kick | Instrument::Snare | Instrument::Hat => {
                [self.drums
                    .next(track.instrument, self.key, age, track.sound); 2]
            }
            Instrument::Sampler => {
                let Some(sample) = sample else {
                    self.active = false;
                    return [0.0; 2];
                };
                let step = f64::from(self.hz / frequency(track.root_note)) * f64::from(sample.rate)
                    / f64::from(RATE);
                if self.age as f64 * step >= sample.frames.len() as f64 {
                    self.active = false;
                    return [0.0; 2];
                }
                sample.at(self.age as f64 * step, step)
            }
        };
        if !prepared.drum && track.instrument != Instrument::Sampler {
            self.filter(track, age, &mut pair);
        }
        for (phase, ratio) in self.phase.iter_mut().zip(prepared.ratios) {
            *phase = (*phase + (dt * ratio).min(0.49)).fract();
        }
        for value in &mut pair {
            *value *= self.level * self.velocity;
        }
        self.age += 1;
        pair
    }

    fn unison(&self, track: &Track, prepared: &Prepared, dt: f32) -> [f32; 2] {
        let mut pair = [0.0; 2];
        for (i, (phase, ratio)) in self.phase.iter().zip(prepared.ratios).enumerate() {
            let shape = if track.instrument == Instrument::Lead && i == 3 {
                2
            } else {
                1
            };
            let value = wave(*phase, (dt * ratio).min(0.49), shape);
            let pan = (i as f32 - 3.0) / 3.0 * track.sound.width;
            let level = if i == 3 { 0.25 } else { 0.125 };
            pair[0] += value * level * (1.0 - pan).sqrt();
            pair[1] += value * level * (1.0 + pan).sqrt();
        }
        pair
    }

    fn filter(&mut self, track: &Track, age: f32, pair: &mut [f32; 2]) {
        let sound = track.sound;
        if self.age.is_multiple_of(16) || self.filter_target == 0.0 {
            let envelope = (-3.0 * age / sound.sweep_decay).exp();
            let cutoff = (sound.brightness * 2.0_f32.powf(sound.sweep * envelope * self.velocity))
                .clamp(40.0, 18000.0);
            self.filter_target = (PI * cutoff / RATE).tan();
            if self.filter_g == 0.0 {
                self.filter_g = self.filter_target;
            }
        }
        self.filter_g += (self.filter_target - self.filter_g) * 0.08;
        let k = 2.0 - 1.85 * sound.resonance;
        for (value, filter) in pair.iter_mut().zip(&mut self.filter) {
            *value = filter.next(*value, self.filter_g, k)[0];
        }
    }
}
