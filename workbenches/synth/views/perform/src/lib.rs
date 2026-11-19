use panel::{AMBER, DARK, Dial, EDGE, INK, Knob, MUTED, PINK, TEAL, XyPad, rounded, screen};
use patina_sdk::{Graph, Ui, view};

use workbench::parameters::*;
#[cfg(test)]
mod tests;

const SIZE: [f32; 2] = [592.0, 690.0];

/// A performance surface for Ember's main patch. All sound edits use the graph's undo path.
#[view]
pub struct Perform {
    page: usize,
    precise: usize,
    dials: [Dial; 4],
    xy: XyPad,
}

impl Perform {
    pub fn init() -> Self {
        Self {
            page: 0,
            precise: 0,
            dials: Default::default(),
            xy: XyPad::default(),
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        if graph.folder(CONSOLE) != Some("nodes/console") {
            ui.label("Open Ember's main graph to use this performance view.");
            ui.label("It controls the existing instrument nodes in that graph.");
            return;
        }
        self.background(ui, graph);
        self.scenes(ui, graph);
        ui.place([24.0, 112.0]);
        if ui
            .tabs(
                "Instrument page",
                &mut self.page,
                &["Perform", "Tone", "Envelope", "Echo", "Learn"],
            )
            .changed()
        {
            self.precise = 0;
            self.dials = Default::default();
            self.xy = XyPad::default();
        }
        let parameters: &[Parameter] = match self.page {
            1 => &TONE,
            2 => &ENVELOPE,
            3 => &ECHO,
            _ => &PERFORMANCE,
        };
        if self.page == 4 {
            self.learn(ui, graph);
            return;
        }
        for (i, parameter) in parameters.iter().take(4).enumerate() {
            self.dial(ui, graph, i, *parameter);
        }
        match self.page {
            1 => self.tone(ui, graph),
            2 => envelope(ui, graph),
            3 => echo(ui),
            _ => self.performance(ui, graph),
        }
        self.exact(ui, graph, parameters);
    }

    fn background(&self, ui: &mut Ui, graph: &Graph) {
        let mut c = ui.canvas("Ember performance surface", SIZE);
        rounded(&mut c, [0.0, 0.0, SIZE[0], SIZE[1]], 6.0, [30, 30, 30]);
        c.rect([24.0, 23.0], [4.0, 23.0], AMBER, true);
        c.text([39.0, 31.0], INK, "EMBER / PERFORMANCE");
        c.text(
            [39.0, 50.0],
            MUTED,
            "PLAY THE INSTRUMENT. EXPLORE THE PATCH.",
        );
        c.line([24.0, 98.0], [568.0, 98.0], EDGE);
        c.line([24.0, 526.0], [568.0, 526.0], EDGE);
        c.line([24.0, 651.0], [568.0, 651.0], EDGE);
        c.text(
            [24.0, 675.0],
            MUTED,
            "Run starts audio. Save keeps changes. Undo restores values.",
        );
        if self.page != 4 && graph.param(CONSOLE, "scene").is_some_and(|s| s != "0") {
            c.text(
                [24.0, 322.0],
                AMBER,
                "Sound controls below edit PHRASE. Select it to hear changes.",
            );
        } else if self.page == 0 {
            c.text(
                [24.0, 322.0],
                TEAL,
                "PHRASE FILTER / X: CUTOFF / Y: RESONANCE",
            );
        }
    }

    fn scenes(&self, ui: &mut Ui, graph: &mut Graph) {
        ui.place([24.0, 65.0]);
        if available(graph, CONSOLE, "nodes/console", "scene") {
            let mut scene = graph
                .param(CONSOLE, "scene")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0)
                .min(2);
            if ui
                .tabs("Scene", &mut scene, &["PHRASE", "ATMOSPHERE", "IMPACT"])
                .changed()
            {
                graph.set_param(CONSOLE, "scene", scene);
            }
        } else {
            ui.label("Scene is controlled by a connection.");
        }
    }

    fn dial(&mut self, ui: &mut Ui, graph: &mut Graph, i: usize, p: Parameter) {
        let at = [24.0 + i as f32 * 144.0, 155.0];
        if !p.editable(graph) {
            ui.place(at);
            ui.label("Unavailable");
            return;
        }
        let mut value = p.read(graph);
        if self.dials[i].edit(
            ui,
            &mut value,
            Knob {
                at,
                key: p.key,
                label: p.label,
                range: p.range,
                color: p.color,
                logarithmic: p.logarithmic,
                unit: p.unit,
            },
        ) {
            p.write(graph, value);
            self.precise = i;
        }
    }

    fn performance(&mut self, ui: &mut Ui, graph: &mut Graph) {
        toggle(
            ui,
            graph,
            [24.0, 278.0],
            SEQUENCE,
            "nodes/sequence",
            "playing",
            "Phrase playing",
        );
        toggle(
            ui,
            graph,
            [320.0, 278.0],
            OUTPUT,
            "nodes/output",
            "audible",
            "Audio output enabled",
        );
        if CUTOFF.editable(graph) && RESONANCE.editable(graph) {
            let cutoff = CUTOFF.read(graph);
            let resonance = RESONANCE.read(graph);
            let mut value = [
                (cutoff / 60.0).ln() / (16000.0_f32 / 60.0).ln(),
                resonance / 0.95,
            ];
            ui.place([24.0, 342.0]);
            if self.xy.show(ui, &mut value) {
                CUTOFF.write(graph, 60.0 * (16000.0_f32 / 60.0).powf(value[0]));
                RESONANCE.write(graph, value[1] * 0.95);
                self.precise = 4;
            }
        } else {
            ui.place([24.0, 350.0]);
            ui.label("The filter is missing or a connection controls its parameters.");
        }
    }

    fn tone(&self, ui: &mut Ui, graph: &mut Graph) {
        ui.place([24.0, 278.0]);
        if available(graph, TIMBRE, "nodes/timbre", "shape") {
            let mut shape = graph
                .param(TIMBRE, "shape")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0)
                .min(3);
            if ui
                .tabs(
                    "Oscillator shape",
                    &mut shape,
                    &["SINE", "SAW", "PULSE", "TRIANGLE"],
                )
                .changed()
            {
                graph.set_param(TIMBRE, "shape", shape);
            }
        } else {
            ui.label("Oscillator shape is missing or controlled by a connection.");
        }
        notes(
            ui,
            &[
                "DETUNE separates two pitches. Listen for slow beating.",
                "SUB adds a sine one octave below the main tone.",
                "MOTION moves the cutoff through a range of octaves.",
                "LFO RATE sets the number of modulation cycles each second.",
                "Try SINE, then SAW. Listen for the extra upper harmonics.",
            ],
        );
    }

    fn exact(&mut self, ui: &mut Ui, graph: &mut Graph, parameters: &[Parameter]) {
        self.precise = self.precise.min(parameters.len() - 1);
        let mut labels = [""; 6];
        for (label, p) in labels.iter_mut().zip(parameters) {
            *label = p.label;
        }
        ui.place([24.0, 536.0]);
        ui.dropdown(
            "Exact control",
            &mut self.precise,
            &labels[..parameters.len()],
        );
        let p = parameters[self.precise];
        ui.place([24.0, 572.0]);
        if !p.editable(graph) {
            ui.label("This control is missing or has a connected input.");
            return;
        }
        // Keep f32 representation noise out of the numeric readout.
        let mut value = (f64::from(p.read(graph)) * 1_000_000.0).round() / 1_000_000.0;
        let step = if p.range[1] > 10.0 { 1.0 } else { 0.001 };
        if ui.number_unit(p.label, &mut value, step, p.unit).changed() {
            p.write(graph, value as f32);
        }
        ui.place([24.0, 610.0]);
        if ui.button("Reset this control").clicked() {
            p.write(graph, p.default);
        }
        ui.place([220.0, 610.0]);
        if ui.button("Select its node").clicked() {
            graph.select([p.node]);
        }
    }

    fn learn(&self, ui: &mut Ui, graph: &mut Graph) {
        let lessons = [
            (
                TIMBRE,
                "1. Hear harmonics",
                "Select SINE, then SAW on Tone. Listen for brightness.",
            ),
            (
                CONTOUR,
                "2. Shape a note",
                "Raise ATTACK on Envelope. Each note fades in.",
            ),
            (
                COLOR,
                "3. Sweep a filter",
                "Move the XY pad left. High frequencies fall away.",
            ),
            (
                SPACE,
                "4. Hear the room",
                "Raise FEEDBACK on Echo. The repeats last longer.",
            ),
            (
                SEQUENCE,
                "5. Make a phrase",
                "Return to Graph. Edit the sequencer's pitch and drum cells.",
            ),
        ];
        for (i, (node, title, instruction)) in lessons.iter().enumerate() {
            let y = 163.0 + i as f32 * 68.0;
            ui.place([24.0, y]);
            if ui.button(title).clicked() && graph.folder(node).is_some() {
                graph.select([*node]);
            }
            ui.place([24.0, y + 29.0]);
            ui.label(instruction);
        }
        ui.place([24.0, 550.0]);
        ui.label("Select PHRASE for these lessons. The buttons select a node.");
        ui.place([24.0, 580.0]);
        ui.label("Open Graph to see the selected node and its connections.");
        ui.place([24.0, 610.0]);
        ui.label("Hold the phrase to stop its score. Use Stop to stop all audio.");
    }
}

fn toggle(
    ui: &mut Ui,
    graph: &mut Graph,
    at: [f32; 2],
    node: &str,
    folder: &str,
    key: &str,
    label: &str,
) {
    ui.place(at);
    if available(graph, node, folder, key) {
        let mut value = graph
            .param(node, key)
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);
        if ui.toggle(label, &mut value).changed() {
            graph.set_param(node, key, value);
        }
    } else {
        ui.label("Control unavailable");
    }
}

fn notes(ui: &mut Ui, lines: &[&str]) {
    ui.place([24.0, 342.0]);
    let mut c = ui.canvas("Listening notes", [544.0, 172.0]);
    rounded(&mut c, [0.0, 0.0, 544.0, 172.0], 4.0, DARK);
    for (i, line) in lines.iter().enumerate() {
        c.text([16.0, 27.0 + i as f32 * 28.0], MUTED, line);
    }
}

fn envelope(ui: &mut Ui, graph: &Graph) {
    ui.place([24.0, 278.0]);
    ui.label("PHRASE / envelope preview with a 0.4 s sustain hold");
    ui.place([24.0, 342.0]);
    let mut c = ui.canvas("Envelope shape preview", [544.0, 172.0]);
    screen(&mut c, [0.0, 0.0, 544.0, 172.0]);
    let [a, d, s, r] = ENVELOPE.map(|p| p.read(graph));
    let total = a + d + 0.4 + r;
    let x = |time| 18.0 + 508.0 * time / total;
    let points = [
        [18.0, 138.0],
        [x(a), 22.0],
        [x(a + d), 138.0 - s * 116.0],
        [x(a + d + 0.4), 138.0 - s * 116.0],
        [526.0, 138.0],
    ];
    for pair in points.windows(2) {
        c.line(pair[0], pair[1], PINK);
    }
    c.text([18.0, 15.0], MUTED, "LEVEL / 0 TO 1");
    c.text([18.0, 159.0], MUTED, "0 s");
    c.text([386.0, 159.0], MUTED, &format!("{total:.3} s total"));
}

fn echo(ui: &mut Ui) {
    ui.place([24.0, 278.0]);
    ui.label("PHRASE / delay time uses seconds");
    notes(
        ui,
        &[
            "TIME sets the gap between echoes.",
            "FEEDBACK returns each echo to the delay.",
            "WET sets the amount of delayed sound.",
            "Use exact entry for a precise delay time.",
            "The delay keeps its tail when the phrase stops.",
        ],
    );
}
