//! The timeline widget of a node GUI or a view (spec 4.3): a transport, a time ruler, clip lanes
//! and automation lanes on one time axis.

use super::{Response, Ui, push_clean, push_color, push_columns, push_numbers};
use std::fmt::Write;

/// The limits of a timeline, so that the widget lines stay fast to send.
const MAX_LANES: usize = 16;
const MAX_CLIPS: usize = 64;
const MAX_POINTS: usize = 256;
/// The waveform columns of a clip.
const CLIP_COLUMNS: usize = 64;
/// The shortest clip in seconds, so that a trim cannot make a clip with no length.
const MIN_CLIP: f32 = 0.01;

/// A time ruler with lanes of clips and automation points, a playhead and a loop, in seconds.
/// `Ui::timeline` edits it, and `Timeline::advance` moves the playhead.
#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    /// The length of the ruler.
    pub length: f32,
    pub playhead: f32,
    pub playing: bool,
    /// With the loop on, `advance` plays `loop_range` again and again.
    pub looping: bool,
    pub loop_range: [f32; 2],
    /// The time step of a drag. 0 is no snap.
    pub snap: f32,
    pub lanes: Vec<Lane>,
    /// The selected clip or point: the lane and the index in the lane.
    pub selected: Option<[usize; 2]>,
}

impl Default for Timeline {
    fn default() -> Timeline {
        Timeline {
            length: 4.0,
            playhead: 0.0,
            playing: false,
            looping: false,
            loop_range: [0.0, 4.0],
            snap: 0.1,
            lanes: Vec::new(),
            selected: None,
        }
    }
}

/// A row of a timeline, with a name and mute and solo buttons.
#[derive(Clone, Debug, PartialEq)]
pub struct Lane {
    pub name: String,
    pub mute: bool,
    pub solo: bool,
    pub kind: LaneKind,
}

/// The content of a lane.
#[derive(Clone, Debug, PartialEq)]
pub enum LaneKind {
    Clips(Vec<Clip>),
    Automation(Automation),
}

/// A part of a source, for example a sound, at a time of a timeline. The user moves it and trims
/// its two ends.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub name: String,
    pub color: [u8; 3],
    /// The time of the start on the timeline.
    pub start: f32,
    /// The time that the clip plays.
    pub length: f32,
    /// The trim at the start: the time in the source where the clip starts.
    pub offset: f32,
    /// The length of the source. A trim cannot go past it. 0 is a source with no end.
    pub source: f32,
    /// The samples of the whole source, for the waveform. The clip shows the part between its trims.
    pub samples: Vec<f32>,
}

/// The values of a parameter in time, for example the cutoff of a filter.
#[derive(Clone, Debug, PartialEq)]
pub struct Automation {
    /// The lowest and the highest value, for example `[500.0, 5000.0]`.
    pub range: [f32; 2],
    /// The unit of the values, for example `Hz`. The editor shows 1000 Hz as 1 kHz.
    pub unit: String,
    /// The time and the value of each point, in the order of time.
    pub points: Vec<[f32; 2]>,
}

impl Automation {
    /// The value at `time`: a smooth curve between two points, and the value of the nearest
    /// point before the first point and after the last point. With no points, the lowest value.
    pub fn value(&self, time: f32) -> f32 {
        let i = self.points.partition_point(|p| p[0] <= time);
        match (
            i.checked_sub(1).map(|i| self.points[i]),
            self.points.get(i).copied(),
        ) {
            (Some([t0, v0]), Some([t1, v1])) => {
                let t = (time - t0) / (t1 - t0);
                v0 + (v1 - v0) * t * t * (3.0 - 2.0 * t)
            }
            (Some([_, v]), None) | (None, Some([_, v])) => v,
            (None, None) => self.range[0],
        }
    }
}

impl Timeline {
    /// The loop range inside the ruler. A node can set a `length` below the default range.
    fn loop_span(&self) -> [f32; 2] {
        self.loop_range.map(|v| v.clamp(0.0, self.length.max(0.0)))
    }

    /// Moves the playhead by `seconds` when the timeline plays. With the loop on, the playhead
    /// goes back to the loop start at the loop end. Without it, the play stops at the ruler end.
    pub fn advance(&mut self, seconds: f32) {
        if !self.playing || !seconds.is_finite() {
            return;
        }
        let [start, end] = self.loop_span();
        let was = self.playhead;
        self.playhead += seconds.max(0.0);
        if self.looping && end > start && was < end && self.playhead >= end {
            self.playhead = start + (self.playhead - end) % (end - start);
        } else if self.playhead >= self.length {
            self.playhead = self.length;
            self.playing = false;
        }
    }

    /// True when the lane plays: it is not muted, and no lane is solo or this lane is solo.
    pub fn audible(&self, lane: usize) -> bool {
        self.lanes
            .get(lane)
            .is_some_and(|l| !l.mute && (l.solo || !self.lanes.iter().any(|l| l.solo)))
    }

    /// The number of clips or points of lane `lane`.
    fn count(&self, lane: usize) -> usize {
        match self.lanes.get(lane).map(|l| &l.kind) {
            Some(LaneKind::Clips(clips)) => clips.len(),
            Some(LaneKind::Automation(a)) => a.points.len(),
            None => 0,
        }
    }

    fn clips(&mut self, lane: usize) -> Option<&mut Vec<Clip>> {
        match &mut self.lanes.get_mut(lane)?.kind {
            LaneKind::Clips(clips) => Some(clips),
            LaneKind::Automation(_) => None,
        }
    }

    fn automation(&mut self, lane: usize) -> Option<&mut Automation> {
        match &mut self.lanes.get_mut(lane)?.kind {
            LaneKind::Automation(a) => Some(a),
            LaneKind::Clips(_) => None,
        }
    }

    /// Applies a user edit, and returns false when the edit names nothing or changes nothing.
    fn apply(&mut self, edit: &str) -> bool {
        let before = self.clone();
        let mut words = edit.split(' ');
        let kind = words.next().unwrap_or("");
        let n: Vec<f32> = words
            .map(|w| w.parse().ok().filter(|v: &f32| v.is_finite()))
            .collect::<Option<_>>()
            .unwrap_or_default();
        let index = |i: usize| {
            n.get(i)
                .filter(|v| v.fract() == 0.0 && **v >= 0.0)
                .map_or(usize::MAX, |v| *v as usize)
        };
        let length = self.length;
        match (kind, n.len()) {
            ("play", 1) => self.playing = n[0] != 0.0,
            ("seek", 1) => self.playhead = n[0].clamp(0.0, length),
            ("loop", 1) => self.looping = n[0] != 0.0,
            ("snap", 1) => self.snap = n[0].clamp(0.0, length),
            ("range", 2) => {
                let [a, b] = [
                    n[0].min(n[1]).clamp(0.0, length),
                    n[0].max(n[1]).clamp(0.0, length),
                ];
                if b > a {
                    self.loop_range = [a, b];
                }
            }
            ("mute", 2) => {
                if let Some(lane) = self.lanes.get_mut(index(0)) {
                    lane.mute = n[1] != 0.0;
                }
            }
            ("solo", 2) => {
                if let Some(lane) = self.lanes.get_mut(index(0)) {
                    lane.solo = n[1] != 0.0;
                }
            }
            ("select", 2) => {
                if index(1) < self.count(index(0)) {
                    self.selected = Some([index(0), index(1)]);
                }
            }
            ("clip", 5) => {
                if let Some(clip) = self
                    .clips(index(0))
                    .and_then(|clips| clips.get_mut(index(1)))
                {
                    let end = if clip.source > 0.0 {
                        clip.source
                    } else {
                        f32::INFINITY
                    };
                    clip.offset = n[4].clamp(0.0, (end - MIN_CLIP).max(0.0));
                    clip.length = n[3].clamp(MIN_CLIP, (end - clip.offset).max(MIN_CLIP));
                    clip.start = n[2].clamp(0.0, (length - clip.length).max(0.0));
                    self.selected = Some([index(0), index(1)]);
                }
            }
            ("point", 4) => {
                let i = index(1);
                if let Some(a) = self.automation(index(0)).filter(|a| i < a.points.len()) {
                    // A point stays between its neighbors, so the indices keep the order of time.
                    let low = if i > 0 { a.points[i - 1][0] } else { 0.0 };
                    let high = a.points.get(i + 1).map_or(length, |p| p[0]).max(low);
                    a.points[i] = [
                        n[2].clamp(low, high),
                        n[3].clamp(a.range[0], a.range[1].max(a.range[0])),
                    ];
                    self.selected = Some([index(0), i]);
                }
            }
            ("add", 3) => {
                if let Some(a) = self
                    .automation(index(0))
                    .filter(|a| a.points.len() < MAX_POINTS)
                {
                    let p = [
                        n[1].clamp(0.0, length),
                        n[2].clamp(a.range[0], a.range[1].max(a.range[0])),
                    ];
                    let i = a.points.partition_point(|q| q[0] <= p[0]);
                    a.points.insert(i, p);
                    self.selected = Some([index(0), i]);
                }
            }
            ("remove", 2) => {
                let i = index(1);
                if let Some(a) = self.automation(index(0)).filter(|a| i < a.points.len()) {
                    a.points.remove(i);
                    self.selected = None;
                }
            }
            _ => {}
        }
        *self != before
    }
}

impl Ui {
    /// Shows `timeline`: a transport with **Play**, **Pause**, **Rewind**, the time, **Loop** and
    /// **Snap**, a ruler, and the lanes under it. The user drags a clip to move it, an end of a
    /// clip to trim it and a point to move it. A double click on an automation lane adds a point.
    /// The node moves the playhead with `Timeline::advance` while the timeline plays.
    pub fn timeline(&mut self, text: &str, timeline: &mut Timeline) -> Response {
        self.timeline_ruler(text, timeline, 0)
    }

    /// Shows quarter-note beats in groups of four. The data and edit times use beats.
    pub fn beat_timeline(&mut self, text: &str, timeline: &mut Timeline) -> Response {
        self.timeline_ruler(text, timeline, 4)
    }

    fn timeline_ruler(&mut self, text: &str, timeline: &mut Timeline, beats: u8) -> Response {
        let event = self.push("timeline", text);
        let changed = event.is_some_and(|e| timeline.apply(e));
        let t = &*timeline;
        let _ = write!(
            self.tree,
            "\t{} {} {} {} {} {} {}",
            t.length,
            t.playhead,
            u8::from(t.playing),
            u8::from(t.looping),
            t.loop_span()[0],
            t.loop_span()[1],
            t.snap
        );
        if beats > 0 {
            let _ = write!(self.tree, " {beats}");
        }
        self.tree.push('\t');
        if let Some([lane, i]) = t.selected {
            let _ = write!(self.tree, "{lane} {i}");
        }
        for lane in t.lanes.iter().take(MAX_LANES) {
            let (mute, solo) = (u8::from(lane.mute), u8::from(lane.solo));
            match &lane.kind {
                LaneKind::Clips(clips) => {
                    let _ = write!(
                        self.tree,
                        "\tclips {mute} {solo} {} ",
                        clips.len().min(MAX_CLIPS)
                    );
                    push_clean(&mut self.tree, &lane.name);
                    for clip in clips.iter().take(MAX_CLIPS) {
                        let _ = write!(
                            self.tree,
                            "\t{} {} {} {} ",
                            clip.start, clip.length, clip.offset, clip.source
                        );
                        push_color(&mut self.tree, clip.color);
                        let samples = clip_samples(clip);
                        let columns = samples.len().min(CLIP_COLUMNS);
                        let _ = write!(self.tree, " {columns}");
                        if columns > 0 {
                            self.tree.push(' ');
                            push_columns(&mut self.tree, samples, columns);
                        }
                        self.tree.push(' ');
                        push_clean(&mut self.tree, &clip.name);
                    }
                }
                LaneKind::Automation(a) => {
                    let _ = write!(
                        self.tree,
                        "\tauto {mute} {solo} {} {} ",
                        a.range[0], a.range[1]
                    );
                    push_clean(&mut self.tree, &lane.name);
                    self.tree.push('\t');
                    push_clean(&mut self.tree, &a.unit);
                    self.tree.push('\t');
                    push_numbers(
                        &mut self.tree,
                        a.points.iter().take(MAX_POINTS).flatten().copied(),
                    );
                }
            }
        }
        Response {
            clicked: false,
            changed,
        }
    }
}

/// The samples of the source between the two trims of `clip`.
fn clip_samples(clip: &Clip) -> &[f32] {
    let n = clip.samples.len() as f32;
    if clip.source.is_nan() || clip.source <= 0.0 {
        return &clip.samples;
    }
    let at = |time: f32| ((time / clip.source).clamp(0.0, 1.0) * n) as usize;
    &clip.samples[at(clip.offset)..at(clip.offset + clip.length).max(at(clip.offset))]
}

#[cfg(test)]
mod musical_tests {
    use super::*;
    #[test]
    fn a_musical_ruler_keeps_beat_edits_and_leaves_the_seconds_format_compatible() {
        let mut time = Timeline {
            length: 128.0,
            snap: 0.25,
            ..Default::default()
        };
        let mut ui = Ui::new(2, "0 seek 32.25");
        assert!(ui.beat_timeline("Song", &mut time).changed());
        assert_eq!(time.playhead, 32.25);
        assert!(ui.tree().contains("128 32.25 0 0 0 4 0.25 4\t"));
        let mut ui = Ui::new(2, "");
        ui.timeline("Time", &mut time);
        assert!(ui.tree().contains("128 32.25 0 0 0 4 0.25\t"));
    }
}
