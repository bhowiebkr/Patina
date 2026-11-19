use super::Arrangement;
use patina_sdk::{Text, Viewer, audio::BLOCK};
use std::{
    fmt::{self, Write},
    sync::atomic::{
        AtomicBool, AtomicU32, AtomicU64,
        Ordering::{AcqRel, Acquire, Relaxed, Release},
    },
};

#[derive(Debug, Default)]
pub struct Meter {
    left: AtomicU32,
    right: AtomicU32,
    pub stolen: AtomicU64,
}
impl Meter {
    pub fn read(&self) -> [f32; 2] {
        [
            f32::from_bits(self.left.load(Relaxed)),
            f32::from_bits(self.right.load(Relaxed)),
        ]
    }
    pub fn write(&self, peaks: [f32; 2], stolen: u64) {
        for (cell, peak) in [&self.left, &self.right].into_iter().zip(peaks) {
            cell.store(
                peak.max(f32::from_bits(cell.load(Relaxed)) * 0.96)
                    .to_bits(),
                Relaxed,
            );
        }
        self.stolen.store(stolen, Relaxed);
    }
}

#[derive(Debug)]
pub struct Transport {
    pub playing: AtomicBool,
    pub position: AtomicU64,
    seek: AtomicU64,
    pub meters: [Meter; super::MAX_TRACKS],
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            playing: AtomicBool::new(true),
            position: AtomicU64::new(0),
            seek: AtomicU64::new(u64::MAX),
            meters: std::array::from_fn(|_| Meter::default()),
        }
    }
}

impl Transport {
    pub fn seek(&self, beat: f64) {
        if beat.is_finite() && beat >= 0.0 {
            self.seek.store(beat.to_bits(), Release);
        }
    }
    pub fn playing(&self) -> bool {
        self.playing.load(Acquire)
    }
    pub fn play(&self, playing: bool) {
        self.playing.store(playing, Release);
    }
    pub fn beat(&self, samples_per_beat: f64) -> f64 {
        self.position.load(Relaxed) as f64 / samples_per_beat
    }
}

#[derive(Clone, Debug)]
pub struct ClockBlock {
    pub position: [u64; BLOCK],
    pub running: [bool; BLOCK],
    pub reset: [bool; BLOCK],
}

impl Viewer for ClockBlock {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(
            out,
            "sample {} / {}",
            self.position[0],
            if self.running[0] { "play" } else { "hold" }
        )
    }
}

#[derive(Default)]
pub struct BeatClock {
    position: u64,
    revision: u64,
    spb: f64,
    was_playing: bool,
}

impl BeatClock {
    pub fn process(&mut self, song: &Arrangement) -> ClockBlock {
        let spb = song.samples_per_beat();
        let changed = self.revision != song.revision;
        if changed && self.spb > 0.0 {
            self.position = (self.position as f64 / self.spb * spb).round() as u64;
        }
        self.spb = spb;
        self.revision = song.revision;
        let seek = song.transport.seek.swap(u64::MAX, AcqRel);
        if seek != u64::MAX {
            self.position = song.frame(f64::from_bits(seek).min(song.score.length));
        }
        let mut playing = song.transport.playing();
        let mut reset = changed || seek != u64::MAX || playing != self.was_playing;
        let end = song.frame(song.score.length);
        if playing && !self.was_playing && self.position >= end && seek == u64::MAX {
            self.position = 0;
            reset = true;
        }
        let [a, b] = song.score.loop_range.map(|beat| song.frame(beat));
        let mut out = ClockBlock {
            position: [0; BLOCK],
            running: [false; BLOCK],
            reset: [false; BLOCK],
        };
        for i in 0..BLOCK {
            if playing && song.score.looping && self.position >= b {
                self.position = a;
                reset = true;
            } else if self.position >= end {
                self.position = end;
                if playing {
                    reset = true;
                }
                playing = false;
                song.transport.play(false);
            }
            out.position[i] = self.position;
            out.running[i] = playing;
            out.reset[i] = reset;
            reset = false;
            if playing {
                self.position += 1;
            }
        }
        self.was_playing = playing;
        song.transport.position.store(self.position, Relaxed);
        out
    }
}
