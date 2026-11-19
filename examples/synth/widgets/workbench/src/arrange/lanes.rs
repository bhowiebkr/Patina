use super::*;
use patina_sdk::{Automation as UiAutomation, Clip as UiClip, Lane, LaneKind, Timeline};
use synth_dsp::song::{Automation, Target};

impl Arrange {
    pub(super) fn lanes(&mut self, ui: &mut Ui, live: Option<&Arrangement>) -> bool {
        let pages = self.score.tracks.len().div_ceil(8).max(1);
        self.track_page = self.track_page.min(pages - 1);
        if pages > 1 {
            let labels: Vec<_> = (0..pages)
                .map(|i| {
                    format!(
                        "Tracks {} - {}",
                        i * 8 + 1,
                        ((i + 1) * 8).min(self.score.tracks.len())
                    )
                })
                .collect();
            ui.tabs(
                "Track pages",
                &mut self.track_page,
                &labels.iter().map(String::as_str).collect::<Vec<_>>(),
            );
        }
        let start = self.track_page * 8;
        let spb = 60.0 / self.score.bpm;
        let mut time = self.timeline(live);
        time.lanes = self
            .score
            .tracks
            .iter()
            .skip(start)
            .take(8)
            .enumerate()
            .map(|(ti, track)| Lane {
                name: track.name.clone(),
                mute: track.mute,
                solo: track.solo,
                kind: LaneKind::Clips(
                    track
                        .clips
                        .iter()
                        .map(|clip| {
                            let (offset, source, samples) = match &clip.content {
                                Content::Notes { offset, .. } => (*offset, 0.0, Vec::new()),
                                Content::Audio {
                                    offset_seconds,
                                    rate,
                                    ..
                                } => {
                                    let audio = live
                                        .and_then(|song| song.tracks.get(start + ti))
                                        .and_then(|t| t.audio.iter().find(|a| a.clip == clip.id));
                                    (
                                        *offset_seconds / spb / rate,
                                        audio.map_or(0.0, |a| a.sample.seconds() / spb / rate),
                                        audio.map_or_else(Vec::new, |a| a.sample.peaks.clone()),
                                    )
                                }
                            };
                            UiClip {
                                name: clip.name.clone(),
                                start: clip.start as f32,
                                length: clip.length as f32,
                                offset: offset as f32,
                                source: source as f32,
                                samples,
                                color: match &clip.content {
                                    Content::Audio { .. } => panel::TEAL,
                                    _ => [panel::AMBER, panel::VIOLET, panel::TEAL, panel::PINK]
                                        [(start + ti) % 4],
                                },
                            }
                        })
                        .collect(),
                ),
            })
            .collect();
        time.selected = self
            .selected_clip()
            .filter(|_| self.track >= start && self.track < start + 8)
            .map(|ci| [self.track - start, ci]);
        let before = (time.playhead, time.playing);
        if !ui
            .beat_timeline("Arrangement / bars and beats", &mut time)
            .changed()
        {
            return false;
        }
        self.transport(live, &time, before);
        let mut changed = self.loop_edit(&time);
        if let Some([lane, clip]) = time.selected {
            self.track = start + lane;
            self.clip = self.score.tracks[self.track].clips.get(clip).map(|c| c.id);
        }
        for (track, lane) in self.score.tracks.iter_mut().skip(start).zip(&time.lanes) {
            changed |= track.mute != lane.mute || track.solo != lane.solo;
            track.mute = lane.mute;
            track.solo = lane.solo;
            if let LaneKind::Clips(clips) = &lane.kind {
                for (clip, shown) in track.clips.iter_mut().zip(clips) {
                    // Preserve untouched f64 values when the f32 widget rounds them.
                    if clip.start as f32 != shown.start {
                        clip.start = shown.start.into();
                        changed = true;
                    }
                    if clip.length as f32 != shown.length {
                        clip.length = shown.length.into();
                        changed = true;
                    }
                    match &mut clip.content {
                        Content::Notes { offset, .. } if *offset as f32 != shown.offset => {
                            *offset = shown.offset.into();
                            changed = true;
                        }
                        Content::Audio {
                            offset_seconds,
                            rate,
                            ..
                        } if (*offset_seconds / spb / *rate) as f32 != shown.offset => {
                            *offset_seconds = f64::from(shown.offset) * spb * *rate;
                            changed = true;
                        }
                        _ => {}
                    }
                }
            }
        }
        changed
    }

    fn timeline(&self, live: Option<&Arrangement>) -> Timeline {
        Timeline {
            length: self.score.length as f32,
            playhead: live.map_or(0.0, |s| s.transport.beat(s.samples_per_beat()) as f32),
            playing: live.is_some_and(|s| s.transport.playing()),
            looping: self.score.looping,
            loop_range: self.score.loop_range.map(|v| v as f32),
            snap: self.snap,
            ..Default::default()
        }
    }

    fn transport(&mut self, live: Option<&Arrangement>, time: &Timeline, before: (f32, bool)) {
        self.snap = time.snap;
        if let Some(song) = live {
            if before.0 != time.playhead {
                song.transport.seek(time.playhead.into());
            }
            if before.1 != time.playing {
                song.transport.play(time.playing);
            }
        }
    }

    fn loop_edit(&mut self, time: &Timeline) -> bool {
        let changed = self.score.looping != time.looping
            || self.score.loop_range.map(|v| v as f32) != time.loop_range;
        if changed {
            self.score.looping = time.looping;
            self.score.loop_range = time.loop_range.map(f64::from);
        }
        changed
    }

    pub(super) fn automation(&mut self, ui: &mut Ui, live: Option<&Arrangement>) -> bool {
        if self.score.tracks.is_empty() {
            return false;
        }
        ui.dropdown("Automation target", &mut self.automation, &Target::NAMES);
        let target = Target::ALL[self.automation];
        let track = &self.score.tracks[self.track];
        let mut time = self.timeline(live);
        time.lanes.push(Lane {
            name: format!("{} / {}", track.name, Target::NAMES[self.automation]),
            mute: track.mute,
            solo: track.solo,
            kind: LaneKind::Automation(UiAutomation {
                range: target.range(),
                unit: if target == Target::Cutoff { "Hz" } else { "" }.into(),
                points: track
                    .automation
                    .iter()
                    .find(|a| a.target == target)
                    .map_or_else(Vec::new, |a| {
                        a.points.iter().map(|p| p.map(|v| v as f32)).collect()
                    }),
            }),
        });
        let old = time.lanes[0].clone();
        let before = (time.playhead, time.playing);
        if !ui.beat_timeline("Track automation", &mut time).changed() {
            return false;
        }
        self.transport(live, &time, before);
        let mut changed = self.loop_edit(&time);
        let track = &mut self.score.tracks[self.track];
        changed |= track.mute != time.lanes[0].mute || track.solo != time.lanes[0].solo;
        track.mute = time.lanes[0].mute;
        track.solo = time.lanes[0].solo;
        if old.kind != time.lanes[0].kind
            && let LaneKind::Automation(a) = &time.lanes[0].kind
        {
            let track = &mut self.score.tracks[self.track];
            track.automation.retain(|a| a.target != target);
            let mut points: Vec<_> = a.points.iter().map(|p| p.map(f64::from)).collect();
            points.dedup_by(|a, b| a[0] == b[0]);
            track.automation.push(Automation { target, points });
            changed = true;
        }
        changed
    }
}
