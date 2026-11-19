use super::{Content, Score, Transport, demo, wav::Sample};
use patina_sdk::{Text, Viewer};
use std::{
    collections::BTreeMap,
    fmt::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct Arrangement {
    pub score: Arc<Score>,
    pub tracks: Arc<Vec<CompiledTrack>>,
    pub transport: Arc<Transport>,
    pub revision: u64,
    pub error: Arc<String>,
}

impl Arrangement {
    pub fn samples_per_beat(&self) -> f64 {
        48000.0 * 60.0 / self.score.bpm
    }
    pub fn frame(&self, beat: f64) -> u64 {
        (beat * self.samples_per_beat()).round() as u64
    }
}

impl Viewer for Arrangement {
    fn text(&self, out: &mut Text<'_>) -> fmt::Result {
        write!(
            out,
            "{} tracks / {:.0} BPM / {:.0} bars",
            self.score.tracks.len(),
            self.score.bpm,
            self.score.length / 4.0
        )
    }
}

#[derive(Clone, Debug)]
pub struct CompiledTrack {
    pub notes: Vec<NoteEvent>,
    pub audio: Vec<AudioEvent>,
    pub sample: Option<Arc<Sample>>,
}

#[derive(Clone, Copy, Debug)]
pub struct NoteEvent {
    pub start: u64,
    pub end: u64,
    pub offset: u64,
    pub key: u8,
    pub velocity: f32,
}

#[derive(Clone, Debug)]
pub struct AudioEvent {
    pub clip: u32,
    pub start: u64,
    pub end: u64,
    pub offset: f64,
    pub step: f64,
    pub fade_in: f32,
    pub fade_out: f32,
    pub sample: Arc<Sample>,
}

pub struct Compiler {
    text: String,
    cache: BTreeMap<PathBuf, Arc<Sample>>,
    pub arrangement: Arrangement,
    pub error: String,
    force: bool,
}

impl Default for Compiler {
    fn default() -> Self {
        let score = demo();
        let tracks = compile(&score, Path::new(""), &mut BTreeMap::new())
            .expect("the built-in arrangement is valid");
        Self {
            text: String::new(),
            cache: BTreeMap::new(),
            error: String::new(),
            force: false,
            arrangement: Arrangement {
                score: Arc::new(score),
                tracks: Arc::new(tracks),
                transport: Arc::new(Transport::default()),
                revision: 1,
                error: Arc::new(String::new()),
            },
        }
    }
}

impl Compiler {
    pub fn reload(&mut self) {
        self.text.clear();
        self.cache.clear();
        self.force = true;
    }
    /// Parses and loads only when the saved document changes. The frame thread owns this work.
    pub fn process(&mut self, text: &str, root: &Path) -> Arrangement {
        if text != self.text || self.force {
            let force = self.force;
            self.force = false;
            self.text.clear();
            self.text.push_str(text);
            let result = if text.is_empty() {
                Ok(demo())
            } else {
                Score::parse(text)
            };
            match result.and_then(|score| {
                compile(&score, root, &mut self.cache).map(|tracks| (score, tracks))
            }) {
                Ok((score, tracks)) => {
                    if force || timing_changed(&self.arrangement.score, &score) {
                        self.arrangement.revision += 1;
                    }
                    self.arrangement.score = Arc::new(score);
                    self.arrangement.tracks = Arc::new(tracks);
                    self.error.clear();
                    self.cache.retain(|_, sample| Arc::strong_count(sample) > 1);
                }
                Err(error) => {
                    self.error = error;
                }
            }
            self.arrangement.error = Arc::new(self.error.clone());
        }
        self.arrangement.clone()
    }
}

fn timing_changed(a: &Score, b: &Score) -> bool {
    a.bpm != b.bpm
        || a.length != b.length
        || a.looping != b.looping
        || a.loop_range != b.loop_range
        || a.tracks.len() != b.tracks.len()
        || a.tracks.iter().any(|old| {
            let Some(new) = b.tracks.iter().find(|t| t.id == old.id) else {
                return true;
            };
            old.instrument != new.instrument
                || old.sample != new.sample
                || old.root_note != new.root_note
                || old.clips.len() != new.clips.len()
                || old.clips.iter().zip(&new.clips).any(|(a, b)| {
                    a.id != b.id
                        || a.start != b.start
                        || a.length != b.length
                        || a.content != b.content
                })
        })
}

fn asset(
    path: &str,
    root: &Path,
    cache: &mut BTreeMap<PathBuf, Arc<Sample>>,
) -> Result<Arc<Sample>, String> {
    let path = root.join(path);
    if let Some(sample) = cache.get(&path) {
        return Ok(sample.clone());
    }
    let sample = Sample::load(&path)?;
    let used: usize = cache.values().map(|s| s.frames.len() * 8).sum();
    if used + sample.frames.len() * 8 > 256 * 1024 * 1024 {
        return Err("Loaded samples exceed the 256 MiB song limit".into());
    }
    let sample = Arc::new(sample);
    cache.insert(path, sample.clone());
    Ok(sample)
}

pub fn compile(
    score: &Score,
    root: &Path,
    cache: &mut BTreeMap<PathBuf, Arc<Sample>>,
) -> Result<Vec<CompiledTrack>, String> {
    score.validate()?;
    let spb = 48000.0 * 60.0 / score.bpm;
    let frame = |beat: f64| (beat * spb).round() as u64;
    let mut result = Vec::with_capacity(score.tracks.len());
    let mut total_notes = 0;
    for track in &score.tracks {
        let mut compiled = CompiledTrack {
            notes: Vec::new(),
            audio: Vec::new(),
            sample: None,
        };
        if !track.sample.is_empty() {
            compiled.sample = Some(asset(&track.sample, root, cache)?);
        }
        for clip in &track.clips {
            match &clip.content {
                Content::Notes {
                    period,
                    offset,
                    notes,
                } => {
                    let first = (offset / period).floor() as u32;
                    let last = ((offset + clip.length) / period).ceil() as u32;
                    if u64::from(last - first) * notes.len() as u64 + total_notes > 200_000 {
                        return Err("The score exceeds 200000 scheduled notes".into());
                    }
                    for repeat in first..last {
                        for note in notes {
                            let start =
                                clip.start + f64::from(repeat) * period + note.start - offset;
                            let end = (start + note.length).min(clip.start + clip.length);
                            let clamped = start.max(clip.start);
                            if end <= clamped {
                                continue;
                            }
                            compiled.notes.push(NoteEvent {
                                start: frame(clamped),
                                end: frame(end),
                                offset: frame(clamped - start),
                                key: note.key,
                                velocity: note.velocity,
                            });
                            total_notes += 1;
                        }
                    }
                }
                Content::Audio {
                    path,
                    offset_seconds,
                    rate,
                } => {
                    let sample = asset(path, root, cache)?;
                    let seconds = clip.length * 60.0 / score.bpm;
                    if offset_seconds + seconds * rate > sample.seconds() + 1.0 / 48000.0 {
                        return Err(format!(
                            "{} extends past the end of its WAV source",
                            clip.name
                        ));
                    }
                    compiled.audio.push(AudioEvent {
                        clip: clip.id,
                        start: frame(clip.start),
                        end: frame(clip.start + clip.length),
                        offset: offset_seconds * f64::from(sample.rate),
                        step: rate * f64::from(sample.rate) / 48000.0,
                        fade_in: clip.fade_in * 48000.0,
                        fade_out: clip.fade_out * 48000.0,
                        sample,
                    });
                }
            }
        }
        compiled.notes.sort_by_key(|n| n.start);
        compiled.audio.sort_by_key(|a| a.start);
        result.push(compiled);
    }
    Ok(result)
}
