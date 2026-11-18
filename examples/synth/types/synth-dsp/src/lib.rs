//! The sound and control values of Ember. Processing uses fixed buffers at 48 kHz.

use patina_sdk::{Text, Viewer, audio::BLOCK};
use std::fmt::{self, Write};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering::Relaxed},
};

mod effects;
mod rhythm;
mod voice;
pub use effects::{Echo, Filter};
pub use rhythm::{Clock, Drums};
pub use voice::{Envelope, Oscillator};

pub const RATE: f32 = 48_000.0;
pub const SCALE: [u8; 8] = [48, 51, 53, 55, 58, 60, 63, 65];
pub const NAMES: [&str; 8] = ["C3", "Eb3", "F3", "G3", "Bb3", "C4", "Eb4", "F4"];
pub const PATTERNS: [&str; 3] = [
    "0 -1 3 4 0 5 3 -1 1 -1 4 3 0 7 5 3",
    "0 3 5 7 1 4 6 7 0 3 5 7 1 4 6 5",
    "0 0 -1 3 0 -1 4 0 1 0 -1 3 0 5 3 -1",
];

/// The display is owned by its control node. A second instrument gets a separate display.
#[derive(Debug, Default)]
pub struct Position(pub AtomicU32);
impl Position {
    pub fn step(&self) -> usize {
        (self.0.load(Relaxed) % 16) as usize
    }
    pub fn set(&self, step: usize) {
        self.0.store(step as u32, Relaxed);
    }
}

#[derive(Clone, Debug)]
pub struct Sequence {
    pub notes: [i8; 16],
    pub bpm: f32,
    pub swing: f32,
    pub playing: bool,
    pub gate: f32,
    pub drums: [u16; 3],
    pub keyboard: i8,
    pub strike: u64,
    pub position: Arc<Position>,
}

impl Default for Sequence {
    fn default() -> Self {
        Self {
            notes: parse_pattern(PATTERNS[0]),
            bpm: 112.0,
            swing: 0.12,
            playing: true,
            gate: 0.58,
            drums: [0x1111, 0x1010, 0x5555],
            keyboard: -1,
            strike: 0,
            position: Arc::new(Position::default()),
        }
    }
}

impl Viewer for Sequence {
    fn text(&self, out: &mut Text) -> fmt::Result {
        write!(out, "{:.0} BPM / 16 steps", self.bpm)
    }
}

/// A pitch, gate, retrigger and drum mask for each sample.
#[derive(Clone, Debug)]
pub struct Notes {
    pub hz: [f32; BLOCK],
    pub gate: [f32; BLOCK],
    pub trigger: [bool; BLOCK],
    pub drums: [u8; BLOCK],
}
impl Default for Notes {
    fn default() -> Self {
        Self {
            hz: [130.8128; BLOCK],
            gate: [0.0; BLOCK],
            trigger: [false; BLOCK],
            drums: [0; BLOCK],
        }
    }
}
impl Viewer for Notes {
    fn text(&self, out: &mut Text) -> fmt::Result {
        write!(
            out,
            "{:.1} Hz / gate {:.0}",
            self.hz[BLOCK - 1],
            self.gate[BLOCK - 1]
        )
    }
    fn wave(&self) -> &[f32] {
        &self.gate
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Tone {
    pub shape: u8,
    pub detune: f32,
    pub sub: f32,
    pub octave: i8,
}
impl Default for Tone {
    fn default() -> Self {
        Self {
            shape: 1,
            detune: 9.0,
            sub: 0.28,
            octave: 0,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Contour {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}
impl Default for Contour {
    fn default() -> Self {
        Self {
            attack: 0.008,
            decay: 0.20,
            sustain: 0.35,
            release: 0.24,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub cutoff: f32,
    pub resonance: f32,
    pub motion: f32,
    pub rate: f32,
}
impl Default for Color {
    fn default() -> Self {
        Self {
            cutoff: 1600.0,
            resonance: 0.4,
            motion: 1.2,
            rate: 0.18,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Space {
    pub seconds: f32,
    pub feedback: f32,
    pub mix: f32,
}
impl Default for Space {
    fn default() -> Self {
        Self {
            seconds: 0.402,
            feedback: 0.42,
            mix: 0.3,
        }
    }
}

pub fn finite(value: f32, fallback: f32, lo: f32, hi: f32) -> f32 {
    if value.is_finite() {
        value.clamp(lo, hi)
    } else {
        fallback
    }
}

pub fn parse_pattern(text: &str) -> [i8; 16] {
    let mut notes = [-1; 16];
    for (to, from) in notes.iter_mut().zip(text.split_whitespace()) {
        *to = from.parse::<i8>().unwrap_or(-1).clamp(-1, 7);
    }
    notes
}

pub fn pattern_text(notes: &[i8; 16]) -> String {
    notes
        .iter()
        .map(i8::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn frequency(midi: u8) -> f32 {
    440.0 * 2.0_f32.powf((f32::from(midi) - 69.0) / 12.0)
}

#[cfg(test)]
mod tests;
