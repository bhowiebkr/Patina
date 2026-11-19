use super::*;
use patina_sdk::PathKind;
use synth_dsp::song::{MAX_CLIPS, wav::Sample};

impl Arrange {
    pub(super) fn clip_tools(&mut self, ui: &mut Ui) -> bool {
        if self.score.tracks.is_empty() {
            return false;
        }
        let mut changed = false;
        if self.score.tracks[self.track].clips.len() < MAX_CLIPS
            && ui.button("Add note clip").clicked()
        {
            self.new_notes();
            changed = true;
        }
        ui.columns(240.0, |ui| {
            ui.column(|ui| {
                ui.file_path("Audio clip / WAV", &mut self.path, PathKind::Open);
            });
            ui.column(|ui| {
                if ui.button("Import WAV clip").clicked() {
                    match self.import() {
                        Ok(()) => changed = true,
                        Err(error) => self.status = error,
                    }
                }
            });
        });
        let Some(ci) = self.selected_clip() else {
            return changed;
        };
        let clip = &mut self.score.tracks[self.track].clips[ci];
        if self.split_at <= clip.start || self.split_at >= clip.start + clip.length {
            self.split_at = clip.start + clip.length * 0.5;
        }
        changed |= ui.text_field("Clip name", &mut clip.name).changed();
        ui.columns(170.0, |ui| {
            ui.column(|ui| {
                changed |= edit(
                    ui,
                    "Start",
                    &mut clip.start,
                    0.0,
                    self.score.length - clip.length,
                    "beats",
                );
            });
            ui.column(|ui| {
                changed |= edit(
                    ui,
                    "Length",
                    &mut clip.length,
                    0.01,
                    self.score.length - clip.start,
                    "beats",
                );
            });
            match &mut clip.content {
                Content::Notes { period, offset, .. } => {
                    ui.column(|ui| {
                        changed |=
                            edit(ui, "Pattern loop", period, 0.01, self.score.length, "beats");
                    });
                    ui.column(|ui| {
                        changed |= edit(ui, "Note offset", offset, 0.0, 4096.0, "beats");
                    });
                }
                Content::Audio {
                    offset_seconds,
                    rate,
                    ..
                } => {
                    ui.column(|ui| {
                        changed |=
                            edit(ui, "Source offset", offset_seconds, 0.0, 86400.0, "seconds");
                    });
                    ui.column(|ui| {
                        let previous = *rate;
                        if edit(ui, "Playback rate", rate, 0.125, 8.0, "x") {
                            clip.length = (clip.length * previous / *rate).min(4096.0 - clip.start);
                            changed = true;
                        }
                    });
                    ui.column(|ui| {
                        changed |= ui
                            .slider("Fade in / seconds", &mut clip.fade_in, 0.0..=2.0)
                            .changed();
                    });
                    ui.column(|ui| {
                        changed |= ui
                            .slider("Fade out / seconds", &mut clip.fade_out, 0.0..=2.0)
                            .changed();
                    });
                }
            }
        });
        self.score.length = self.score.length.max(clip.start + clip.length);
        ui.number_unit("Split position", &mut self.split_at, 0.25, "beats");
        ui.columns(150.0, |ui| {
            ui.column(|ui| {
                if ui.button("Duplicate clip").clicked() {
                    let id = self.score.next_clip_id();
                    let mut copy = self.score.tracks[self.track].clips[ci].clone();
                    copy.id = id;
                    copy.start += copy.length;
                    if copy.start + copy.length <= 4096.0
                        && self.score.tracks[self.track].clips.len() < MAX_CLIPS
                    {
                        self.score.length = self.score.length.max(copy.start + copy.length);
                        self.score.tracks[self.track].clips.push(copy);
                        self.clip = Some(id);
                        changed = true;
                    }
                }
            });
            ui.column(|ui| {
                if ui.button("Split clip").clicked()
                    && self.score.tracks[self.track].clips.len() < MAX_CLIPS
                {
                    let id = self.score.next_clip_id();
                    let clip = &mut self.score.tracks[self.track].clips[ci];
                    if self.split_at >= clip.start + 0.01
                        && self.split_at <= clip.start + clip.length - 0.01
                    {
                        let mut right = clip.clone();
                        right.id = id;
                        clip.length = self.split_at - clip.start;
                        right.start = self.split_at;
                        right.length -= clip.length;
                        clip.fade_out = 0.0;
                        right.fade_in = 0.0;
                        advance(&mut right.content, clip.length, self.score.bpm);
                        self.score.tracks[self.track].clips.push(right);
                        self.clip = Some(id);
                        changed = true;
                    }
                }
            });
            ui.column(|ui| {
                if ui.button("Delete clip").clicked() {
                    self.score.tracks[self.track].clips.remove(ci);
                    self.clip = None;
                    changed = true;
                }
            });
        });
        if let Some(ci) = self.selected_clip() {
            let names: Vec<_> = self.score.tracks.iter().map(|t| t.name.as_str()).collect();
            let mut destination = self.track;
            if ui
                .dropdown("Move clip to track", &mut destination, &names)
                .changed()
                && destination != self.track
                && self.score.tracks[destination].clips.len() < MAX_CLIPS
            {
                let clip = self.score.tracks[self.track].clips.remove(ci);
                self.score.tracks[destination].clips.push(clip);
                self.track = destination;
                changed = true;
            }
        }
        changed
    }

    fn import(&mut self) -> Result<(), String> {
        if self.score.tracks[self.track].clips.len() >= MAX_CLIPS {
            return Err("This track has 64 clips".into());
        }
        let path = patina_sdk::project_path(&self.path);
        let sample = Sample::load(&path)?;
        let id = self.score.next_clip_id();
        let track = &mut self.score.tracks[self.track];
        let start = track
            .clips
            .iter()
            .map(|c| c.start + c.length)
            .fold(0.0_f64, f64::max);
        let length = sample.seconds() * self.score.bpm / 60.0;
        if start + length > 4096.0 || length < 0.01 {
            return Err("The WAV duration does not fit the song".into());
        }
        let source = import_path(&path)?;
        self.path = source.clone();
        track.clips.push(Clip {
            id,
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .chars()
                .take(80)
                .collect(),
            start,
            length,
            fade_in: 0.005,
            fade_out: 0.005,
            content: Content::Audio {
                path: source,
                offset_seconds: 0.0,
                rate: 1.0,
            },
        });
        self.score.length = self.score.length.max(start + length);
        self.clip = Some(id);
        Ok(())
    }

    pub(super) fn mixer(&mut self, ui: &mut Ui, live: Option<&Arrangement>) -> bool {
        let mut changed = false;
        ui.columns(220.0, |ui| {
            for (index, track) in self.score.tracks.iter_mut().enumerate() {
                ui.column(|ui| {
                    ui.label(&track.name);
                    if let Some(song) = live {
                        let meter = &song.transport.meters[index];
                        let [left, right] = meter.read();
                        ui.progress(
                            &format!("L / {:.1} dBFS", 20.0 * left.max(0.00001).log10()),
                            left.clamp(0.0, 1.0),
                        );
                        ui.progress(
                            &format!("R / {:.1} dBFS", 20.0 * right.max(0.00001).log10()),
                            right.clamp(0.0, 1.0),
                        );
                        let stolen = meter.stolen.load(std::sync::atomic::Ordering::Relaxed);
                        if stolen > 0 {
                            ui.label(&format!("32 voices / {stolen} voice replacements"));
                        }
                    }
                    changed |= ui.text_field("Track name", &mut track.name).changed();
                    let mut instrument = Instrument::ALL
                        .iter()
                        .position(|i| *i == track.instrument)
                        .unwrap_or(0);
                    if ui
                        .dropdown("Instrument", &mut instrument, &Instrument::NAMES)
                        .changed()
                    {
                        track.instrument = Instrument::ALL[instrument];
                        changed = true;
                    }
                    if track.instrument == Instrument::Sampler {
                        let mut path = track.sample.clone();
                        if ui
                            .file_path("Sampler WAV", &mut path, PathKind::Open)
                            .changed()
                        {
                            let source = patina_sdk::project_path(&path);
                            let imported = if path.is_empty() {
                                Ok(String::new())
                            } else {
                                Sample::load(&source).and_then(|_| import_path(&source))
                            };
                            match imported {
                                Ok(path) => {
                                    track.sample = path;
                                    changed = true;
                                }
                                Err(e) => self.status = e,
                            }
                        }
                        changed |= number8(ui, "Root MIDI note", &mut track.root_note, [0, 127]);
                    }
                    changed |= ui.slider("Gain", &mut track.gain, 0.0..=2.0).changed();
                    changed |= ui.slider("Pan", &mut track.pan, -1.0..=1.0).changed();
                    changed |= ui.toggle("Mute", &mut track.mute).changed();
                    changed |= ui.toggle("Solo", &mut track.solo).changed();
                    changed |= ui
                        .slider("Attack / seconds", &mut track.attack, 0.001..=4.0)
                        .changed();
                    changed |= ui
                        .slider("Release / seconds", &mut track.release, 0.005..=8.0)
                        .changed();
                    changed |= ui
                        .slider("Cutoff / Hz", &mut track.cutoff, 40.0..=18000.0)
                        .changed();
                    changed |= ui.slider("Drive", &mut track.drive, 1.0..=12.0).changed();
                    changed |= ui
                        .slider("Delay send", &mut track.delay_mix, 0.0..=0.8)
                        .changed();
                    changed |= ui
                        .slider("Delay / beats", &mut track.delay_beats, 0.125..=4.0)
                        .changed();
                });
            }
        });
        changed
    }
}

fn edit(ui: &mut Ui, label: &str, value: &mut f64, low: f64, high: f64, unit: &str) -> bool {
    let changed = ui.number_unit(label, value, 0.25, unit).changed();
    *value = value.clamp(low, high.max(low));
    changed
}

fn advance(content: &mut Content, beats: f64, bpm: f64) {
    match content {
        Content::Notes { offset, .. } => *offset += beats,
        Content::Audio {
            offset_seconds,
            rate,
            ..
        } => *offset_seconds += beats * 60.0 / bpm * *rate,
    }
}

fn import_path(source: &std::path::Path) -> Result<String, String> {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = patina_sdk::project_path("")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    let directory = root.join("types/synth-dsp/samples");
    if source.starts_with(&directory) {
        return Ok(source
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/"));
    }
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let relative = format!("types/synth-dsp/samples/audio-{stamp}.wav");
    crate::export::copy_new(&source, &root.join(&relative))?;
    Ok(relative)
}
