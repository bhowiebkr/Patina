use super::{Instrument, Sound};
use crate::{RATE, processors::Integrators, voice::wave};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Default)]
pub(super) struct Percussion {
    noise: u32,
    metal: [f32; 6],
    band: Integrators,
    air: Integrators,
}

impl Percussion {
    pub fn reset(&mut self, seed: u32) {
        *self = Self {
            noise: seed.max(1),
            ..Self::default()
        };
        self.metal = [0.13, 0.71, 0.37, 0.93, 0.19, 0.53];
    }

    pub fn next(&mut self, kind: Instrument, key: u8, age: f32, sound: Sound) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        let noise = self.noise as f32 / u32::MAX as f32 * 2.0 - 1.0;
        let length = sound.drum_length;
        let tone = sound.drum_tone;
        match kind {
            Instrument::Kick => {
                let base = 45.0 + tone * 12.0;
                let phase = base * age + 125.0 * (1.0 - (-age * 42.0).exp()) / 42.0;
                let body = (TAU * phase).sin() * (-age * 9.0 / length).exp();
                let click = self.air.next(noise, 0.34, 1.3)[2]
                    * (-age * 650.0).exp()
                    * (0.08 + 0.18 * tone);
                // The low-frequency body saturates before the short click is added.
                (body * 1.5).tanh() * 0.86 + click
            }
            Instrument::Snare => {
                let body = ((TAU * 185.0 * age).sin() * 0.42 + (TAU * 337.0 * age).sin() * 0.2)
                    * (-age * 32.0 / length).exp();
                let wire = self
                    .band
                    .next(noise, (PI * (1900.0 + tone * 3600.0) / RATE).tan(), 1.2);
                let snap = wire[1] * (-age * 22.0 / length).exp() * 1.25;
                let air = self.air.next(noise, 0.38, 1.4)[2] * (-age * 55.0 / length).exp() * 0.18;
                (body + snap + air) * (1.0 - (-age * 1500.0).exp())
            }
            Instrument::Hat => {
                let mut metal = 0.0;
                for (phase, hz) in self
                    .metal
                    .iter_mut()
                    .zip([205.3, 304.4, 369.6, 522.7, 800.0, 1101.0])
                {
                    let dt = hz * (0.9 + tone * 0.4) / RATE;
                    metal += wave(*phase, dt, 2);
                    *phase = (*phase + dt).fract();
                }
                let source = metal * 0.13 + noise * 0.5;
                let high = self.band.next(source, 0.32, 0.85)[2];
                let band = self.air.next(high, 0.72, 1.0)[1];
                let decay = if key == 46 { 11.0 } else { 65.0 };
                band * (-age * decay / length).exp() * (1.0 - (-age * 3000.0).exp()) * 0.65
            }
            _ => 0.0,
        }
    }
}
