use patina_sdk::{Graph, Ui};
use synth_dsp::song::{
    Arrangement, Clip, Content, Instrument, MAX_TRACKS, Note, Score, Track, demo,
};
mod details;
mod lanes;
mod piano;
mod routing;
#[cfg(test)]
mod tests;

pub struct Arrange {
    target: String,
    saved: String,
    score: Score,
    page: usize,
    track: usize,
    clip: Option<u32>,
    note: usize,
    track_page: usize,
    automation: usize,
    snap: f32,
    split_at: f64,
    path: String,
    status: String,
    piano: piano::Piano,
}

impl Default for Arrange {
    fn default() -> Self {
        Self {
            target: String::new(),
            saved: String::new(),
            score: demo(),
            page: 0,
            track: 0,
            clip: None,
            note: 0,
            track_page: 0,
            automation: 0,
            snap: 0.25,
            split_at: 0.0,
            path: String::new(),
            status: String::new(),
            piano: Default::default(),
        }
    }
}

impl Arrange {
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        let Some(target) = graph
            .nodes()
            .find(|id| graph.folder(id) == Some("nodes/arrangement"))
            .map(str::to_owned)
        else {
            ui.label("Open the song graph to arrange notes, drums, and recorded audio.");
            return;
        };
        let text = graph.param(&target, "score").unwrap_or("");
        if graph.wires().any(|(_, to)| to == format!("{target}.score")) {
            ui.label("The score input is connected. Edit its source node.");
            return;
        }
        if self.target != target || self.saved != text {
            match if text.is_empty() {
                Ok(demo())
            } else {
                Score::parse(text)
            } {
                Ok(score) => {
                    self.score = score;
                    self.saved = text.into();
                    self.target = target;
                    self.status.clear();
                }
                Err(e) => {
                    ui.label(&e);
                    return;
                }
            }
        }
        self.track = self.track.min(self.score.tracks.len().saturating_sub(1));
        let live = graph.value::<Arrangement>(&self.target, "out").cloned();
        if let Some(song) = &live
            && !song.error.is_empty()
        {
            ui.label(&format!(
                "{}. The last valid score stays active.",
                song.error
            ));
        }
        let mut changed = false;
        ui.columns(170.0, |ui| {
            ui.column(|ui| {
                let mut bpm = self.score.bpm;
                if ui.number_unit("Tempo", &mut bpm, 1.0, "BPM").changed() {
                    match self.score.set_tempo(bpm) {
                        Ok(()) => changed = true,
                        Err(e) => self.status = e,
                    }
                }
            });
            ui.column(|ui| {
                let mut bars = self.score.length / 4.0;
                if ui
                    .number_unit("Song length", &mut bars, 1.0, "bars")
                    .changed()
                {
                    let minimum = self
                        .score
                        .tracks
                        .iter()
                        .flat_map(|t| &t.clips)
                        .map(|c| c.start + c.length)
                        .fold(1.0_f64, f64::max);
                    self.score.length = (bars * 4.0).clamp(minimum, 4096.0);
                    self.score.loop_range[1] = self.score.loop_range[1].min(self.score.length);
                    changed = true;
                }
            });
            ui.column(|ui| {
                if self.page == 0 || self.page == 3 {
                    ui.label(&format!(
                        "EMBER / {} tracks / stereo",
                        self.score.tracks.len()
                    ));
                } else if let Some(song) = &live {
                    if ui
                        .button(if song.transport.playing() {
                            "Pause song"
                        } else {
                            "Play song"
                        })
                        .clicked()
                    {
                        song.transport.play(!song.transport.playing());
                    }
                } else {
                    ui.label("Run the graph to start its audio clock.");
                }
            });
        });
        ui.tabs(
            "Song tools",
            &mut self.page,
            &["Arrange", "Piano roll", "Mixer", "Automation"],
        );
        changed |= match self.page {
            0 => self.lanes(ui, live.as_ref()),
            1 => self.piano_page(ui),
            2 => self.mixer(ui, live.as_ref()),
            _ => self.automation(ui, live.as_ref()),
        };
        ui.label("");
        changed |= self.selection(ui);
        if self.page == 0 {
            changed |= self.clip_tools(ui);
        }
        ui.columns(180.0, |ui| {
            ui.column(|ui| {
                if self.track > 0 && ui.button("Move track up").clicked() {
                    self.score.tracks.swap(self.track, self.track - 1);
                    self.track -= 1;
                    changed = true;
                }
            });
            ui.column(|ui| {
                if self.track + 1 < self.score.tracks.len()
                    && ui.button("Move track down").clicked()
                {
                    self.score.tracks.swap(self.track, self.track + 1);
                    self.track += 1;
                    changed = true;
                }
            });
            ui.column(|ui| {
                if self.score.tracks.len() > 1 && ui.button("Remove selected track").clicked() {
                    match routing::remove(graph, &self.target, self.score.tracks[self.track].id) {
                        Ok(()) => {
                            self.score.tracks.remove(self.track);
                            self.track = self.track.min(self.score.tracks.len() - 1);
                            self.clip = None;
                            changed = true;
                        }
                        Err(e) => self.status = e,
                    }
                }
            });
        });
        ui.columns(180.0, |ui| {
            ui.column(|ui| {
                if ui.button("Reload WAV files").clicked() {
                    let revision = graph
                        .param(&self.target, "reload")
                        .and_then(|s| s.parse::<u32>().ok())
                        .unwrap_or(0);
                    graph.set_param(&self.target, "reload", revision.wrapping_add(1));
                }
            });
            ui.column(|ui| {
                if self.score.tracks.len() < MAX_TRACKS && ui.button("Add track").clicked() {
                    let id = (1..=u32::MAX)
                        .find(|id| self.score.tracks.iter().all(|t| t.id != *id))
                        .unwrap();
                    match routing::add(graph, &self.target, id) {
                        Ok(()) => {
                            self.score
                                .tracks
                                .push(Track::new(id, "New track", Instrument::Lead));
                            self.track = self.score.tracks.len() - 1;
                            changed = true;
                        }
                        Err(e) => self.status = e,
                    }
                }
            });
            ui.column(|ui| {
                if ui.button("Select track node").clicked()
                    && let Some(track) = self.score.tracks.get(self.track)
                {
                    let id = graph
                        .nodes()
                        .find(|id| {
                            graph.folder(id) == Some("nodes/song-track")
                                && graph.param(id, "track").and_then(|s| s.parse::<u32>().ok())
                                    == Some(track.id)
                        })
                        .map(str::to_owned);
                    if let Some(id) = id {
                        graph.select([id.as_str()]);
                    }
                }
            });
            ui.column(|ui| {
                ui.label(
                    "Save keeps the score. Undo restores edits. Export uses the saved song graph.",
                );
            });
        });
        if changed {
            match self.score.validate() {
                Ok(()) => {
                    self.saved = self.score.text();
                    graph.set_param(&self.target, "score", &self.saved);
                    self.status.clear();
                }
                Err(error) => {
                    self.status = error;
                    let text = graph.param(&self.target, "score").unwrap_or("");
                    self.score = if text.is_empty() {
                        demo()
                    } else {
                        Score::parse(text).expect("the displayed score was validated")
                    };
                }
            }
        }
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
    }

    fn selection(&mut self, ui: &mut Ui) -> bool {
        if self.score.tracks.is_empty() {
            return false;
        }
        let names: Vec<_> = self.score.tracks.iter().map(|t| t.name.as_str()).collect();
        let old = self.track;
        ui.dropdown("Track", &mut self.track, &names);
        if old != self.track {
            self.clip = None;
            self.note = 0;
        }
        let track = &self.score.tracks[self.track];
        if track.clips.is_empty() {
            self.clip = None;
            return false;
        }
        let mut selected = self
            .clip
            .and_then(|id| track.clips.iter().position(|c| c.id == id))
            .unwrap_or(0);
        let names: Vec<_> = track.clips.iter().map(|c| c.name.as_str()).collect();
        ui.dropdown("Clip", &mut selected, &names);
        self.clip = Some(track.clips[selected].id);
        false
    }

    fn selected_clip(&self) -> Option<usize> {
        self.score
            .tracks
            .get(self.track)?
            .clips
            .iter()
            .position(|c| Some(c.id) == self.clip)
    }

    fn new_notes(&mut self) {
        let id = self.score.next_clip_id();
        let track = &mut self.score.tracks[self.track];
        let start = track
            .clips
            .iter()
            .map(|c| c.start + c.length)
            .fold(0.0_f64, f64::max)
            .min(self.score.length - 4.0);
        track.clips.push(Clip {
            id,
            name: "New notes".into(),
            start: start.max(0.0),
            length: 4.0_f64.min(self.score.length),
            fade_in: 0.005,
            fade_out: 0.005,
            content: Content::Notes {
                period: 4.0_f64.min(self.score.length),
                offset: 0.0,
                notes: vec![Note {
                    start: 0.0,
                    length: 0.5,
                    key: 60,
                    velocity: 0.8,
                }],
            },
        });
        self.clip = Some(id);
    }
}

fn number8(ui: &mut Ui, label: &str, value: &mut u8, range: [u32; 2]) -> bool {
    let mut number = u32::from(*value);
    let changed = ui.slider(label, &mut number, range[0]..=range[1]).changed();
    *value = number.clamp(range[0], range[1]) as u8;
    changed
}
