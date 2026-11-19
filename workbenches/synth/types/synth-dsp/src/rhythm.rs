use super::{Notes, RATE, SCALE, Sequence, finite, frequency};
use patina_sdk::{AudioBlock, audio::BLOCK};

pub struct Clock {
    step: usize,
    phase: f64,
    first: bool,
    strike: u64,
    hz: f32,
    pitches: [f32; 8],
    keys: super::keyboard::KeyLease,
}
impl Default for Clock {
    fn default() -> Self {
        Self {
            step: 0,
            phase: 0.0,
            first: true,
            strike: 0,
            hz: frequency(48),
            pitches: SCALE.map(frequency),
            keys: Default::default(),
        }
    }
}
impl Clock {
    pub fn process(&mut self, settings: &Sequence) -> Notes {
        let mut out = Notes::default();
        let live = self.keys.read(&settings.live_keys);
        let live_hz = live.map(|(note, _, _)| frequency(note));
        let bpm = f64::from(finite(settings.bpm, 112.0, 40.0, 220.0));
        let swing = f64::from(finite(settings.swing, 0.0, 0.0, 0.45));
        let gate = f64::from(finite(settings.gate, 0.58, 0.05, 0.95));
        let manual = usize::try_from(settings.keyboard)
            .ok()
            .filter(|&i| i < SCALE.len());
        let step_samples = 60.0 / bpm / 4.0 * f64::from(RATE);
        let lengths = [step_samples * (1.0 + swing), step_samples * (1.0 - swing)];
        for i in 0..BLOCK {
            let length = lengths[self.step % 2];
            let tick = settings.playing && (self.first || self.phase >= length);
            if tick {
                if self.first {
                    self.first = false;
                } else {
                    self.phase -= length;
                    self.step = (self.step + 1) % 16;
                }
                for (lane, pattern) in settings.drums.iter().enumerate() {
                    if pattern & (1 << self.step) != 0 {
                        out.drums[i] |= 1 << lane;
                    }
                }
            }
            let note = manual.or_else(|| {
                usize::try_from(settings.notes[self.step])
                    .ok()
                    .filter(|&n| n < SCALE.len())
            });
            if let Some(note) = note {
                self.hz = self.pitches[note];
            }
            if let Some(hz) = live_hz {
                self.hz = hz;
            }
            out.hz[i] = self.hz;
            let current_length = lengths[self.step % 2];
            out.gate[i] = f32::from(
                manual.is_some()
                    || (settings.playing && note.is_some() && self.phase < current_length * gate),
            );
            if let Some((_, velocity, _)) = live {
                out.gate[i] = velocity;
            }
            out.trigger[i] = if let Some((_, _, strike)) = live {
                i == 0 && strike
            } else if manual.is_some() {
                i == 0 && settings.strike != self.strike
            } else {
                tick && note.is_some()
            };
            if settings.playing {
                self.phase += 1.0;
            }
        }
        self.strike = settings.strike;
        settings.position.set(self.step);
        out
    }
}

pub struct Drums {
    kick: f32,
    snare: f32,
    hat: f32,
    phase: f32,
    noise: u32,
    last_noise: f32,
}
impl Default for Drums {
    fn default() -> Self {
        Self {
            kick: 0.0,
            snare: 0.0,
            hat: 0.0,
            phase: 0.0,
            noise: 0x9e3779b9,
            last_noise: 0.0,
        }
    }
}
impl Drums {
    pub fn process(&mut self, notes: &Notes) -> AudioBlock {
        AudioBlock::from_fn(|i| {
            let hits = notes.drums[i];
            if hits & 1 != 0 {
                self.kick = 1.0;
                self.phase = 0.0;
            }
            if hits & 2 != 0 {
                self.snare = 1.0;
            }
            if hits & 4 != 0 {
                self.hat = 1.0;
            }
            self.noise ^= self.noise << 13;
            self.noise ^= self.noise >> 17;
            self.noise ^= self.noise << 5;
            let noise = self.noise as f32 / u32::MAX as f32 * 2.0 - 1.0;
            let high = (noise - self.last_noise) * 0.5;
            self.last_noise = noise;
            self.phase = (self.phase + (47.0 + 105.0 * self.kick.powi(4)) / RATE).fract();
            let kick = (self.phase * std::f32::consts::TAU).sin() * self.kick;
            let snare =
                (noise * 0.8 + (self.phase * std::f32::consts::TAU * 3.8).sin() * 0.2) * self.snare;
            let out = kick * 0.7 + snare * 0.28 + high * self.hat * 0.19;
            self.kick *= 0.99965;
            self.snare *= 0.99935;
            self.hat *= 0.9981;
            out
        })
    }
}
