use super::*;
use panel::{AMBER, DARK, EDGE, INK, MUTED, TEAL};
use patina_sdk::{Key, Pointer};
use synth_dsp::song::MAX_NOTES;

pub(super) struct Piano {
    octave: i32,
    left: f64,
    bars: f64,
    drag: Option<Drag>,
    clip: Option<u32>,
}

impl Default for Piano {
    fn default() -> Self {
        Self {
            octave: 4,
            left: 0.0,
            bars: 1.0,
            drag: None,
            clip: None,
        }
    }
}

struct Drag {
    original: Note,
    preview: Note,
    anchor: [f32; 2],
    resize: bool,
    new: bool,
}

impl Arrange {
    pub(super) fn piano_page(&mut self, ui: &mut Ui) -> bool {
        let Some(ci) = self.selected_clip() else {
            ui.label("Select a note clip below.");
            return false;
        };
        let clip = &mut self.score.tracks[self.track].clips[ci];
        let Content::Notes { period, notes, .. } = &mut clip.content else {
            ui.label(
                "This clip contains recorded audio. Use Arrange to trim it and set its fades.",
            );
            return false;
        };
        if self.piano.clip != Some(clip.id) {
            self.piano.clip = Some(clip.id);
            self.piano.drag = None;
            self.piano.octave = notes
                .iter()
                .map(|n| i32::from(n.key / 12) - 1)
                .min()
                .unwrap_or(4)
                .clamp(-1, 8);
            self.piano.left = 0.0;
        }
        self.piano
            .draw(ui, notes, *period, &mut self.note, self.snap.max(0.0625))
    }
}

impl Piano {
    pub(super) fn draw(
        &mut self,
        ui: &mut Ui,
        notes: &mut Vec<Note>,
        period: f64,
        selected: &mut usize,
        snap: f32,
    ) -> bool {
        ui.columns(160.0, |ui| {
            ui.column(|ui| {
                ui.slider("Lowest octave", &mut self.octave, -1..=8);
            });
            ui.column(|ui| {
                ui.number_unit("Visible pattern", &mut self.bars, 1.0, "bars");
                self.bars = self.bars.clamp(0.25, (period / 4.0).max(0.25));
            });
            ui.column(|ui| {
                ui.number_unit("First beat", &mut self.left, 1.0, "beats");
                self.left = self.left.clamp(0.0, (period - self.bars * 4.0).max(0.0));
            });
        });
        let span = (self.bars * 4.0).min(period);
        let w = crate::width(ui);
        let h = (ui.available_size().unwrap_or([800.0, 500.0])[1] - 240.0).clamp(216.0, 432.0);
        let low = ((self.octave + 1) * 12) as u8;
        let rows = (128 - u16::from(low)).min(24);
        let row = h / f32::from(rows);
        let x = |beat: f64| 52.0 + ((beat - self.left) / span) as f32 * (w - 52.0);
        let y = |key: u8| 24.0 + (f32::from(rows - 1) - (f32::from(key) - f32::from(low))) * row;
        let at = |p: [f32; 2]| -> (f64, u8) {
            let beat = self.left + f64::from((p[0] - 52.0) / (w - 52.0)) * span;
            let beat = (beat / f64::from(snap)).round() * f64::from(snap);
            let key = (f32::from(low + rows as u8 - 1) - ((p[1] - 24.0) / row).floor())
                .clamp(0.0, 127.0) as u8;
            (beat, key)
        };
        let mut changed = false;
        let mut c = ui.canvas(
            "Chromatic piano roll / drag notes / drag right edge to resize / Delete removes",
            [w, h + 70.0],
        );
        c.grab();
        for event in c.pointer() {
            match *event {
                Pointer::Down(point)
                    if point[0] >= 52.0
                        && point[0] < w
                        && point[1] >= 24.0
                        && point[1] < h + 24.0 =>
                {
                    let found = notes.iter().rposition(|n| {
                        point[0] >= x(n.start)
                            && point[0] <= x(n.start + n.length)
                            && point[1] >= y(n.key)
                            && point[1] < y(n.key) + row
                    });
                    let (beat, key) = at(point);
                    let note = found.map_or_else(
                        || Note {
                            start: beat.clamp(0.0, (period - f64::from(snap)).max(0.0)),
                            length: f64::from(snap).min(period),
                            key,
                            velocity: 0.8,
                        },
                        |i| notes[i].clone(),
                    );
                    if let Some(i) = found {
                        *selected = i;
                    }
                    let resize = found.is_some()
                        && point[0]
                            > x(note.start + note.length)
                                - ((x(note.start + note.length) - x(note.start)) * 0.25).min(7.0);
                    self.drag = Some(Drag {
                        original: note.clone(),
                        preview: note,
                        anchor: point,
                        resize,
                        new: found.is_none(),
                    });
                }
                Pointer::Move(point) | Pointer::Up(point) => {
                    if let Some(drag) = &mut self.drag {
                        let delta = (f64::from((point[0] - drag.anchor[0]) / (w - 52.0)) * span
                            / f64::from(snap))
                        .round()
                            * f64::from(snap);
                        if drag.resize {
                            drag.preview.length = (drag.original.length + delta)
                                .clamp(0.001, period - drag.original.start);
                        } else {
                            drag.preview.start = (drag.original.start + delta)
                                .clamp(0.0, period - drag.original.length);
                            drag.preview.key = (i32::from(drag.original.key)
                                - ((point[1] - drag.anchor[1]) / row).round() as i32)
                                .clamp(0, 127) as u8;
                        }
                    }
                    if matches!(event, Pointer::Up(_))
                        && let Some(drag) = self.drag.take()
                    {
                        if drag.new && notes.len() < MAX_NOTES {
                            notes.push(drag.preview);
                            *selected = notes.len() - 1;
                            changed = true;
                        } else if !drag.new
                            && let Some(note) = notes.get_mut(*selected)
                        {
                            changed |= *note != drag.preview;
                            *note = drag.preview;
                        }
                    }
                }
                _ => {}
            }
        }
        for event in c.keys() {
            if let Key::Down(key) = event {
                if key == "Delete" || key == "Backspace" {
                    if *selected < notes.len() {
                        notes.remove(*selected);
                        changed = true;
                    }
                } else if let Some(note) = notes.get_mut(*selected) {
                    let before = note.clone();
                    match key.as_str() {
                        "ArrowLeft" => note.start = (note.start - f64::from(snap)).max(0.0),
                        "ArrowRight" => {
                            note.start = (note.start + f64::from(snap)).min(period - note.length)
                        }
                        "ArrowUp" => note.key = note.key.saturating_add(1).min(127),
                        "ArrowDown" => note.key = note.key.saturating_sub(1),
                        _ => {}
                    }
                    changed |= before != *note;
                }
            }
        }
        c.rect([0.0, 0.0], [w, h + 70.0], DARK, true);
        for r in 0..rows {
            let key = u16::from(low) + rows - 1 - r;
            let top = 24.0 + f32::from(r) * row;
            let black = matches!(key % 12, 1 | 3 | 6 | 8 | 10);
            c.rect(
                [0.0, top],
                [w, row - 1.0],
                if black { [25, 25, 25] } else { [34, 34, 34] },
                true,
            );
            c.rect(
                [0.0, top],
                [48.0, row - 1.0],
                if black { [46, 46, 46] } else { [155, 155, 150] },
                true,
            );
            if key % 12 == 0 {
                c.text(
                    [5.0, top + row - 1.0],
                    DARK,
                    &format!("C{}", i32::from(key / 12) - 1),
                );
            }
        }
        for i in 0..=((span / f64::from(snap)).ceil() as usize).min(4096) {
            let beat = self.left + i as f64 * f64::from(snap);
            let px = x(beat);
            c.line(
                [px, 24.0],
                [px, h + 24.0],
                if beat.fract() < 0.001 {
                    EDGE
                } else {
                    [43, 43, 43]
                },
            );
            if beat % 4.0 < 0.001 {
                c.text(
                    [px + 3.0, 15.0],
                    MUTED,
                    &format!("{}", (beat / 4.0) as u32 + 1),
                );
            }
        }
        for (i, stored) in notes.iter().enumerate() {
            let n = self
                .drag
                .as_ref()
                .filter(|d| !d.new && i == *selected)
                .map_or(stored, |d| &d.preview);
            draw_note(
                &mut c,
                n,
                i == *selected,
                [
                    x(n.start),
                    y(n.key),
                    x(n.start + n.length) - x(n.start),
                    row,
                ],
                [w, h],
            );
        }
        if let Some(d) = &self.drag
            && d.new
        {
            draw_note(
                &mut c,
                &d.preview,
                true,
                [
                    x(d.preview.start),
                    y(d.preview.key),
                    x(d.preview.start + d.preview.length) - x(d.preview.start),
                    row,
                ],
                [w, h],
            );
        }
        c.text([4.0, h + 52.0], MUTED, "VEL");
        drop(c);
        if !notes.is_empty() {
            *selected = (*selected).min(notes.len() - 1);
            let mut index = *selected as u32 + 1;
            if ui.number("Note number", &mut index).changed() {
                *selected = (index.saturating_sub(1) as usize).min(notes.len() - 1);
            }
            let note = &mut notes[*selected];
            ui.columns(140.0, |ui| {
                ui.column(|ui| {
                    changed |= ui
                        .number_unit("Note start", &mut note.start, 0.25, "beats")
                        .changed();
                    note.start = note.start.clamp(0.0, period - 0.001);
                });
                ui.column(|ui| {
                    changed |= ui
                        .number_unit("Note length", &mut note.length, 0.25, "beats")
                        .changed();
                    note.length = note.length.clamp(0.001, period - note.start);
                });
                ui.column(|ui| {
                    changed |= number8(ui, "MIDI pitch", &mut note.key, [0, 127]);
                });
                ui.column(|ui| {
                    changed |= ui
                        .slider("Velocity", &mut note.velocity, 0.001..=1.0)
                        .changed();
                });
            });
        }
        ui.label("Click to add. Drag to move. Drag the right edge to resize. Arrow keys move the selected note. Delete removes it.");
        changed
    }
}

fn draw_note(
    c: &mut patina_sdk::Canvas,
    note: &Note,
    selected: bool,
    rect: [f32; 4],
    size: [f32; 2],
) {
    let [x, y, width, row] = rect;
    let [w, h] = size;
    if y < 24.0 || y >= h + 24.0 || x + width <= 52.0 || x >= w {
        return;
    }
    let left = x.max(52.0);
    let width = (x + width).min(w) - left;
    c.rect(
        [left, y + 1.0],
        [width.max(1.0), row - 2.0],
        if selected { AMBER } else { TEAL },
        true,
    );
    if selected {
        c.rect([left, y + 1.0], [width.max(1.0), row - 2.0], INK, false);
    }
    c.rect(
        [left, h + 64.0 - note.velocity * 32.0],
        [3.0, note.velocity * 32.0],
        if selected { AMBER } else { TEAL },
        true,
    );
}
