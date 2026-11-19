//! Saved musical data. Times are quarter-note beats unless a field names seconds.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_TRACKS: usize = 32;
pub const MAX_CLIPS: usize = 64;
pub const MAX_NOTES: usize = 2048;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Score {
    pub version: u32,
    pub bpm: f64,
    pub length: f64,
    pub looping: bool,
    pub loop_range: [f64; 2],
    pub tracks: Vec<Track>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Instrument {
    Lead,
    Pad,
    Bass,
    Kick,
    Snare,
    Hat,
    Sampler,
    Rave,
}

impl Instrument {
    pub const ALL: [Self; 8] = [
        Self::Lead,
        Self::Pad,
        Self::Bass,
        Self::Kick,
        Self::Snare,
        Self::Hat,
        Self::Sampler,
        Self::Rave,
    ];
    pub const NAMES: [&'static str; 8] = [
        "Lead",
        "Pad",
        "Bass",
        "Kick",
        "Snare",
        "Hat",
        "Sampler",
        "Rave lead",
    ];
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub id: u32,
    pub name: String,
    pub instrument: Instrument,
    pub sample: String,
    pub root_note: u8,
    pub gain: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub attack: f32,
    pub release: f32,
    pub cutoff: f32,
    pub drive: f32,
    pub delay_mix: f32,
    pub delay_beats: f32,
    #[serde(default)]
    pub rave: crate::RaveTone,
    pub clips: Vec<Clip>,
    pub automation: Vec<Automation>,
}

impl Track {
    pub fn new(id: u32, name: &str, instrument: Instrument) -> Self {
        Self {
            id,
            name: name.into(),
            instrument,
            sample: String::new(),
            root_note: 60,
            gain: 0.5,
            pan: 0.0,
            mute: false,
            solo: false,
            attack: 0.004,
            release: 0.08,
            cutoff: 8000.0,
            drive: 1.0,
            delay_mix: 0.0,
            delay_beats: 0.75,
            rave: crate::RaveTone::default(),
            clips: Vec::new(),
            automation: Vec::new(),
        }
    }

    pub fn value(&self, target: Target, beat: f64, fallback: f32) -> f32 {
        self.automation
            .iter()
            .find(|a| a.target == target && !a.points.is_empty())
            .map_or(fallback, |a| a.value(beat))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub id: u32,
    pub name: String,
    pub start: f64,
    pub length: f64,
    pub fade_in: f32,
    pub fade_out: f32,
    pub content: Content,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Content {
    Notes {
        period: f64,
        offset: f64,
        notes: Vec<Note>,
    },
    Audio {
        path: String,
        offset_seconds: f64,
        rate: f64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub start: f64,
    pub length: f64,
    pub key: u8,
    pub velocity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    Gain,
    Pan,
    Cutoff,
    Delay,
}

impl Target {
    pub const ALL: [Self; 4] = [Self::Gain, Self::Pan, Self::Cutoff, Self::Delay];
    pub const NAMES: [&'static str; 4] = ["Gain", "Pan", "Cutoff", "Delay send"];
    pub fn range(self) -> [f32; 2] {
        match self {
            Self::Gain => [0.0, 2.0],
            Self::Pan => [-1.0, 1.0],
            Self::Cutoff => [40.0, 18000.0],
            Self::Delay => [0.0, 0.8],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Automation {
    pub target: Target,
    pub points: Vec<[f64; 2]>,
}

impl Automation {
    pub fn value(&self, beat: f64) -> f32 {
        let i = self.points.partition_point(|p| p[0] <= beat);
        match (
            i.checked_sub(1).map(|i| self.points[i]),
            self.points.get(i).copied(),
        ) {
            (Some([a, x]), Some([b, y])) => {
                let t = (beat - a) / (b - a);
                (x + (y - x) * t * t * (3.0 - 2.0 * t)) as f32
            }
            (Some([_, value]), _) | (_, Some([_, value])) => value as f32,
            _ => 0.0,
        }
    }
}

impl Score {
    pub fn set_tempo(&mut self, bpm: f64) -> Result<(), String> {
        if !bpm.is_finite() || !(30.0..=300.0).contains(&bpm) {
            return Err("Tempo must be between 30 and 300 BPM".into());
        }
        let factor = bpm / self.bpm;
        let length = self
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .filter(|c| matches!(c.content, Content::Audio { .. }))
            .map(|c| c.start + c.length * factor)
            .fold(self.length, f64::max);
        if length > 4096.0 {
            return Err("The audio clips exceed the song length limit at this tempo".into());
        }
        for clip in self.tracks.iter_mut().flat_map(|t| &mut t.clips) {
            if matches!(clip.content, Content::Audio { .. }) {
                clip.length *= factor;
            }
        }
        self.bpm = bpm;
        self.length = length;
        Ok(())
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 2 * 1024 * 1024 {
            return Err("The score exceeds 2 MiB".into());
        }
        let score: Self = serde_json::from_str(text).map_err(|e| format!("Invalid score: {e}"))?;
        score.validate()?;
        Ok(score)
    }

    pub fn text(&self) -> String {
        serde_json::to_string(self).expect("a validated score has finite values")
    }

    pub fn validate(&self) -> Result<(), String> {
        let valid = |v: f64, lo: f64, hi: f64| v.is_finite() && v >= lo && v <= hi;
        if self.version != 1
            || !valid(self.bpm, 30.0, 300.0)
            || !valid(self.length, 1.0, 4096.0)
            || !valid(self.loop_range[0], 0.0, self.length)
            || !valid(self.loop_range[1], self.loop_range[0] + 0.01, self.length)
            || self.tracks.len() > MAX_TRACKS
        {
            return Err("Invalid score version, tempo, length, loop, or track count".into());
        }
        let mut tracks = BTreeSet::new();
        let mut clips = BTreeSet::new();
        for track in &self.tracks {
            if !tracks.insert(track.id)
                || track.name.len() > 80
                || track.root_note > 127
                || track.sample.len() > 4096
                || track.clips.len() > MAX_CLIPS
                || track.automation.len() > 4
                || !track.rave.valid()
            {
                return Err(format!("Invalid track: {}", track.name));
            }
            for (value, lo, hi) in [
                (track.gain, 0.0, 2.0),
                (track.pan, -1.0, 1.0),
                (track.attack, 0.001, 4.0),
                (track.release, 0.005, 8.0),
                (track.cutoff, 40.0, 18000.0),
                (track.drive, 1.0, 12.0),
                (track.delay_mix, 0.0, 0.8),
                (track.delay_beats, 0.125, 4.0),
            ] {
                if !valid(value.into(), lo, hi) {
                    return Err(format!("Invalid control in {}", track.name));
                }
            }
            for (i, a) in track.automation.iter().enumerate() {
                let range = a.target.range();
                if a.points.len() > 256
                    || track.automation[..i].iter().any(|b| b.target == a.target)
                    || a.points.iter().any(|p| {
                        !valid(p[0], 0.0, self.length)
                            || !valid(p[1], range[0].into(), range[1].into())
                    })
                    || a.points.windows(2).any(|p| p[0][0] >= p[1][0])
                {
                    return Err(format!("Invalid automation in {}", track.name));
                }
            }
            for clip in &track.clips {
                if !clips.insert(clip.id)
                    || clip.name.len() > 80
                    || !valid(clip.start, 0.0, self.length)
                    || !valid(clip.length, 0.01, self.length - clip.start)
                    || !valid(clip.fade_in.into(), 0.0, 60.0)
                    || !valid(clip.fade_out.into(), 0.0, 60.0)
                {
                    return Err(format!("Invalid clip: {}", clip.name));
                }
                match &clip.content {
                    Content::Notes {
                        period,
                        offset,
                        notes,
                    } => {
                        if !valid(*period, 0.01, self.length)
                            || !valid(*offset, 0.0, 4096.0)
                            || notes.len() > MAX_NOTES
                            || notes.iter().any(|n| {
                                !valid(n.start, 0.0, *period - 0.001)
                                    || !valid(n.length, 0.001, *period - n.start)
                                    || n.key > 127
                                    || !valid(n.velocity.into(), 0.001, 1.0)
                            })
                        {
                            return Err(format!("Invalid notes in {}", clip.name));
                        }
                    }
                    Content::Audio {
                        path,
                        offset_seconds,
                        rate,
                    } => {
                        if path.is_empty()
                            || path.len() > 4096
                            || !valid(*offset_seconds, 0.0, 86400.0)
                            || !valid(*rate, 0.125, 8.0)
                        {
                            return Err(format!("Invalid audio source in {}", clip.name));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn next_clip_id(&self) -> u32 {
        (1..=u32::MAX)
            .find(|id| {
                self.tracks
                    .iter()
                    .all(|t| t.clips.iter().all(|c| c.id != *id))
            })
            .unwrap()
    }
}
