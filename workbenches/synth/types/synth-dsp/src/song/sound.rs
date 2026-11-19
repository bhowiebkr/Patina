//! Instrument controls. Old scores receive defaults without changing their timing.
use super::{Instrument, Track};
use crate::RATE;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Sound {
    pub decay: f32,
    pub sustain: f32,
    pub detune: f32,
    pub width: f32,
    pub brightness: f32,
    pub resonance: f32,
    pub sweep: f32,
    pub sweep_decay: f32,
    pub edge: f32,
    pub drum_length: f32,
    pub drum_tone: f32,
    pub highpass: f32,
    pub room: f32,
    pub duck: f32,
}

impl Default for Sound {
    fn default() -> Self {
        Self {
            decay: 0.16,
            sustain: 0.65,
            detune: 14.0,
            width: 0.7,
            brightness: 12000.0,
            resonance: 0.15,
            sweep: 0.0,
            sweep_decay: 0.12,
            edge: 1.0,
            drum_length: 1.0,
            drum_tone: 0.55,
            highpass: 20.0,
            room: 0.0,
            duck: 0.0,
        }
    }
}

impl Sound {
    pub fn valid(self) -> bool {
        [
            (self.decay, 0.005, 4.0),
            (self.sustain, 0.0, 1.0),
            (self.detune, 0.0, 40.0),
            (self.width, 0.0, 1.0),
            (self.brightness, 40.0, 18000.0),
            (self.resonance, 0.0, 0.9),
            (self.sweep, 0.0, 6.0),
            (self.sweep_decay, 0.005, 4.0),
            (self.edge, 1.0, 16.0),
            (self.drum_length, 0.2, 3.0),
            (self.drum_tone, 0.0, 1.0),
            (self.highpass, 20.0, 2000.0),
            (self.room, 0.0, 0.6),
            (self.duck, 0.0, 0.9),
        ]
        .iter()
        .all(|(value, lo, hi)| value.is_finite() && (*lo..=*hi).contains(value))
    }
}

pub(super) struct Prepared {
    pub ratios: [f32; 7],
    pub release: f32,
    pub drum: bool,
}

impl Prepared {
    pub fn new(track: &Track) -> Self {
        Self {
            ratios: [-1.0, -0.61, -0.27, 0.0, 0.23, 0.59, 1.0]
                .map(|offset| 2.0_f32.powf(offset * track.sound.detune / 1200.0)),
            release: (-6.91 / (track.release * RATE)).exp(),
            drum: matches!(
                track.instrument,
                Instrument::Kick | Instrument::Snare | Instrument::Hat
            ),
        }
    }
}
