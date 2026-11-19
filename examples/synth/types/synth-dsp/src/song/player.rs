use super::{Arrangement, ClockBlock, Target, voice::Voice};
use crate::{RATE, Saturator};
use patina_sdk::audio::{BLOCK, StereoBlock};

const VOICES: usize = 32;
const DELAY: usize = 48000 * 8 + 1;

pub struct TrackPlayer {
    voices: [Voice; VOICES],
    cursor: usize,
    revision: u64,
    track: u32,
    filter: [[f32; 2]; 2],
    drive: [Saturator; 2],
    delay: Box<[[f32; 2]]>,
    write: usize,
    gain: f32,
    pan: f32,
    pub stolen: u64,
    last: [f32; 2],
    transition: [f32; 2],
    transition_left: u32,
}

impl Default for TrackPlayer {
    fn default() -> Self {
        Self {
            voices: std::array::from_fn(|_| Voice::default()),
            cursor: 0,
            revision: 0,
            track: u32::MAX,
            filter: [[0.0; 2]; 2],
            drive: Default::default(),
            delay: vec![[0.0; 2]; DELAY].into_boxed_slice(),
            write: 0,
            gain: 0.0,
            pan: 0.0,
            stolen: 0,
            last: [0.0; 2],
            transition: [0.0; 2],
            transition_left: 0,
        }
    }
}

impl TrackPlayer {
    fn start(&mut self, note: super::NoteEvent, elapsed: u64) {
        let index = self
            .voices
            .iter()
            .position(|v| !v.active)
            .unwrap_or_else(|| {
                self.stolen += 1;
                self.voices
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| a.level.total_cmp(&b.level))
                    .map_or(0, |(i, _)| i)
            });
        self.voices[index].start(note, elapsed);
    }

    pub fn process(&mut self, song: &Arrangement, clock: &ClockBlock, id: u32) -> StereoBlock {
        let mut out = StereoBlock::default();
        let Some(index) = song.score.tracks.iter().position(|t| t.id == id) else {
            return out;
        };
        let (track, compiled) = (&song.score.tracks[index], &song.tracks[index]);
        let changed = self.revision != song.revision || self.track != id;
        self.revision = song.revision;
        self.track = id;
        for i in 0..BLOCK {
            let position = clock.position[i];
            if clock.reset[i] || (i == 0 && changed) {
                self.transition = self.last;
                self.transition_left = 240;
                for voice in &mut self.voices {
                    voice.release();
                }
                self.cursor = compiled.notes.partition_point(|n| n.start < position);
                if clock.running[i] {
                    for note in compiled.notes[..self.cursor]
                        .iter()
                        .filter(|n| n.end > position)
                    {
                        self.start(*note, position - note.start);
                    }
                }
            }
            if clock.running[i] {
                while let Some(note) = compiled
                    .notes
                    .get(self.cursor)
                    .filter(|n| n.start <= position)
                {
                    if note.end > position {
                        self.start(*note, position - note.start);
                    }
                    self.cursor += 1;
                }
            }
            let mut pair = [0.0; 2];
            for voice in self.voices.iter_mut().filter(|v| v.active) {
                if !clock.running[i] || position >= voice.end {
                    voice.release();
                }
                let value = voice.next(track, compiled.sample.as_deref());
                pair[0] += value[0];
                pair[1] += value[1];
            }
            if clock.running[i] {
                for clip in compiled
                    .audio
                    .iter()
                    .take_while(|a| a.start <= position)
                    .filter(|a| position < a.end)
                {
                    let elapsed = position - clip.start;
                    let fade_in = if clip.fade_in > 0.0 {
                        (elapsed as f32 / clip.fade_in).min(1.0)
                    } else {
                        1.0
                    };
                    let fade_out = if clip.fade_out > 0.0 {
                        ((clip.end - position) as f32 / clip.fade_out).min(1.0)
                    } else {
                        1.0
                    };
                    for (sum, value) in pair.iter_mut().zip(
                        clip.sample
                            .at(clip.offset + elapsed as f64 * clip.step, clip.step),
                    ) {
                        *sum += value * fade_in * fade_out;
                    }
                }
            }
            if self.transition_left > 0 {
                let old = self.transition_left as f32 / 240.0;
                for (value, previous) in pair.iter_mut().zip(self.transition) {
                    *value = previous * old + *value * (1.0 - old);
                }
                self.transition_left -= 1;
            }
            self.last = pair;
            out.left[i] = pair[0];
            out.right[i] = pair[1];
        }
        if track.drive > 1.0 {
            out.left = self.drive[0].process(&out.left, track.drive, 1.0);
            out.right = self.drive[1].process(&out.right, track.drive, 1.0);
        }
        let solo = song.score.tracks.iter().any(|t| t.solo);
        let audible = !track.mute && (!solo || track.solo);
        let spb = song.samples_per_beat();
        let delay = (f64::from(track.delay_beats) * spb)
            .round()
            .clamp(1.0, (DELAY - 1) as f64) as usize;
        for i in 0..BLOCK {
            let beat = clock.position[i] as f64 / spb;
            let gain = if audible {
                track.value(Target::Gain, beat, track.gain)
            } else {
                0.0
            };
            self.gain += (gain - self.gain) * 0.004;
            self.pan += (track.value(Target::Pan, beat, track.pan) - self.pan) * 0.004;
            let cutoff = track.value(Target::Cutoff, beat, track.cutoff);
            let alpha = 1.0 - (-std::f32::consts::TAU * cutoff / RATE).exp();
            let mix = track.value(Target::Delay, beat, track.delay_mix);
            let echo = self.delay[(self.write + DELAY - delay) % DELAY];
            let pair = [out.left[i], out.right[i]];
            let mut final_pair = [0.0; 2];
            for channel in 0..2 {
                self.filter[channel][0] += alpha * (pair[channel] - self.filter[channel][0]);
                self.filter[channel][1] +=
                    alpha * (self.filter[channel][0] - self.filter[channel][1]);
                let dry = if cutoff >= 18000.0 {
                    pair[channel]
                } else {
                    self.filter[channel][1]
                };
                self.delay[self.write][channel] = dry * mix + echo[1 - channel] * 0.38;
                let pan = if channel == 0 {
                    (1.0 - self.pan).min(1.0)
                } else {
                    (1.0 + self.pan).min(1.0)
                };
                final_pair[channel] = (dry + echo[channel]) * self.gain * pan;
            }
            self.write = (self.write + 1) % DELAY;
            out.left[i] = final_pair[0];
            out.right[i] = final_pair[1];
        }
        song.transport.meters[index].write([out.left.peak(), out.right.peak()], self.stolen);
        out
    }
}
