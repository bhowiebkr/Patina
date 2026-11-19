use crate::{label, number, parameters::available, width};
use panel::{AMBER, DARK, EDGE, INK, MUTED, PINK, TEAL};
use patina_sdk::{Graph, Pointer, Ui};
use synth_dsp::{NAMES, Sequence, parse_pattern, pattern_text};

#[derive(Default)]
pub struct Patterns {
    target: String,
    page: usize,
    step: usize,
    detail: bool,
}

impl Patterns {
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        let ids: Vec<_> = graph
            .nodes()
            .filter(|id| graph.folder(id) == Some("nodes/sequence"))
            .map(str::to_owned)
            .collect();
        if ids.is_empty() {
            ui.label("Add a sequence node to edit a phrase.");
            return;
        }
        if let Some(id) = graph.selected().find(|id| ids.iter().any(|n| n == id)) {
            self.target = id.to_owned();
        }
        let mut selected = ids.iter().position(|id| *id == self.target).unwrap_or(0);
        let names: Vec<_> = ids.iter().map(|id| label(graph, id)).collect();
        ui.columns(150.0, |ui| {
            ui.column(|ui| {
                ui.dropdown(
                    "Sequencer",
                    &mut selected,
                    &names.iter().map(String::as_str).collect::<Vec<_>>(),
                );
            });
            self.target = ids[selected].clone();
            ui.column(|ui| {
                let mut playing = graph.param(&self.target, "playing") == Some("true");
                if available(graph, &self.target, "nodes/sequence", "playing")
                    && ui.toggle("Phrase playing", &mut playing).changed()
                {
                    graph.set_param(&self.target, "playing", playing);
                }
            });
            ui.column(|ui| number(ui, graph, &self.target, "bpm", "Tempo / BPM", [40.0, 220.0]));
            ui.column(|ui| {
                ui.toggle("Pitch editor", &mut self.detail);
            });
        });
        let mut notes = parse_pattern(graph.param(&self.target, "pattern").unwrap_or(""));
        let mut drums = ["kick", "snare", "hat"].map(|key| {
            graph
                .param(&self.target, key)
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(0)
        });
        let position = graph
            .value::<Sequence>(&self.target, "out")
            .map(|s| s.position.step());
        let (before, before_drums) = (notes, drums);
        let w = width(ui);
        let count = if w < 600.0 { 8 } else { 16 };
        if count == 8 {
            ui.tabs("Steps", &mut self.page, &["01 - 08", "09 - 16"]);
        }
        let start = if count == 8 { self.page * 8 } else { 0 };
        let cell = (w - 42.0) / count as f32;
        let pitch_rows = if self.detail { 8 } else { 1 };
        let y_drums = 22.0 + pitch_rows as f32 * 22.0;
        let mut c = ui.canvas("Phrase steps and drum hits", [w, y_drums + 68.0]);
        c.grab();
        for event in c.pointer() {
            if let Pointer::Down([x, y]) = *event
                && x >= 42.0
                && x < w
                && y >= 22.0
                && y < y_drums + 66.0
            {
                let step = start + ((x - 42.0) / cell) as usize;
                self.step = step;
                if y < y_drums && available(graph, &self.target, "nodes/sequence", "pattern") {
                    let note = if self.detail {
                        7 - ((y - 22.0) / 22.0) as i8
                    } else {
                        (notes[step] + 2) % 9 - 1
                    };
                    notes[step] = if self.detail && notes[step] == note {
                        -1
                    } else {
                        note
                    };
                } else if y >= y_drums {
                    let lane = ((y - y_drums) / 22.0) as usize;
                    if available(
                        graph,
                        &self.target,
                        "nodes/sequence",
                        ["kick", "snare", "hat"][lane],
                    ) {
                        drums[lane] ^= 1 << step;
                    }
                }
            }
        }
        for i in 0..count {
            let step = start + i;
            let x = 42.0 + i as f32 * cell;
            c.text(
                [x + 4.0, 14.0],
                if position == Some(step) { AMBER } else { MUTED },
                &format!("{:02}", step + 1),
            );
            for row in 0..pitch_rows {
                let y = 22.0 + row as f32 * 22.0;
                let on = if self.detail {
                    notes[step] == 7 - row as i8
                } else {
                    notes[step] >= 0
                };
                c.rect(
                    [x, y],
                    [cell - 2.0, 20.0],
                    if on { AMBER } else { DARK },
                    true,
                );
                if !self.detail {
                    c.text(
                        [x + 3.0, y + 14.0],
                        if on { DARK } else { MUTED },
                        usize::try_from(notes[step])
                            .ok()
                            .and_then(|n| NAMES.get(n))
                            .copied()
                            .unwrap_or("--"),
                    );
                }
            }
            for (lane, bits) in drums.iter().enumerate() {
                c.rect(
                    [x, y_drums + lane as f32 * 22.0],
                    [cell - 2.0, 20.0],
                    if bits & (1 << step) != 0 {
                        [AMBER, PINK, TEAL][lane]
                    } else {
                        EDGE
                    },
                    true,
                );
            }
            if self.step == step {
                c.line([x, 20.0], [x + cell - 2.0, 20.0], INK);
            }
        }
        c.text([0.0, 36.0], MUTED, "NOTE");
        for (i, name) in ["KICK", "SNARE", "HAT"].iter().enumerate() {
            c.text([0.0, y_drums + 15.0 + i as f32 * 22.0], MUTED, name);
        }
        drop(c);
        if notes != before {
            graph.set_param(&self.target, "pattern", pattern_text(&notes));
        }
        for (i, key) in ["kick", "snare", "hat"].iter().enumerate() {
            if drums[i] != before_drums[i] {
                graph.set_param(&self.target, key, drums[i]);
            }
        }
        ui.label("Click notes to change pitch. Click drum cells to toggle hits. Phrase play is separate from engine Run.");
    }
}
