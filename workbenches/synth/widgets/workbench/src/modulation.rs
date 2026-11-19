use crate::{label, number, parameter, parameters::*, title};
use patina_sdk::{Graph, Ui};

#[derive(Default)]
pub struct Modulation {
    filter: usize,
    row: Option<usize>,
    source: usize,
    target: usize,
    add: bool,
    saved_motion: f32,
}

fn target_pin(folder: &str) -> Option<&'static str> {
    match folder {
        "nodes/svf" | "nodes/vca" => Some("cv"),
        "nodes/vco" => Some("pitch"),
        _ => None,
    }
}

fn source_node(folder: &str) -> bool {
    matches!(
        folder,
        "nodes/lfo" | "nodes/adsr" | "nodes/sample-hold" | "nodes/attenuverter" | "nodes/slew"
    )
}

fn endpoint(graph: &Graph, pin: &str) -> String {
    let (id, port) = pin.rsplit_once('.').unwrap_or((pin, ""));
    format!("{} / {port}", label(graph, id))
}

impl Modulation {
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        title(ui, "EMBER / MODULATION", "Inspect the real signal routes");
        ui.tabs("Route filter", &mut self.filter, &["All", "Selected"]);
        if graph.folder(COLOR) == Some("nodes/color")
            && (self.filter == 0 || graph.selected().any(|id| id == COLOR))
        {
            ui.label("Internal LFO to filter cutoff");
            parameter(ui, graph, TONE[2]);
            parameter(ui, graph, TONE[3]);
            if TONE[2].editable(graph)
                && ui
                    .button(if TONE[2].read(graph) == 0.0 {
                        "Enable filter motion"
                    } else {
                        "Mute filter motion"
                    })
                    .clicked()
            {
                let current = TONE[2].read(graph);
                if current > 0.0 {
                    self.saved_motion = current;
                }
                TONE[2].write(
                    graph,
                    if current > 0.0 {
                        0.0
                    } else {
                        self.saved_motion.max(0.1)
                    },
                );
            }
            if ui.button("Locate filter controls").clicked() {
                graph.select([COLOR]);
            }
        }
        let routes: Vec<_> = graph
            .wires()
            .filter(|(from, to)| {
                let source = from.rsplit_once('.').map(|p| p.0).unwrap_or("");
                let (target, pin) = to.rsplit_once('.').unwrap_or(("", ""));
                let control = target_pin(graph.folder(target).unwrap_or("")) == Some(pin)
                    || source_node(graph.folder(source).unwrap_or(""))
                    || matches!(
                        graph.folder(source),
                        Some("nodes/control" | "nodes/constant")
                    );
                control
                    && (self.filter == 0 || graph.selected().any(|id| id == source || id == target))
            })
            .map(|(a, b)| (a.to_owned(), b.to_owned()))
            .collect();
        let rows: Vec<String> = routes
            .iter()
            .flat_map(|(a, b)| [endpoint(graph, a), endpoint(graph, b)])
            .collect();
        ui.table("Patch routes", &["Source", "Target"], &rows, &mut self.row);
        if let Some((from, to)) = self.row.and_then(|i| routes.get(i)) {
            let source = from.rsplit_once('.').unwrap().0;
            let target = to.rsplit_once('.').unwrap().0;
            ui.label(&format!("From {}", endpoint(graph, from)));
            ui.label(&format!("To {}", endpoint(graph, to)));
            if matches!(graph.folder(target), Some("nodes/svf" | "nodes/vco")) {
                number(ui, graph, target, "depth", "Depth / octaves", [-8.0, 8.0]);
            }
            ui.label(
                "Negative depth reverses polarity. Zero depth mutes a depth-controlled route.",
            );
            if ui.button("Locate source").clicked() {
                graph.select([source]);
            }
            if ui.button("Locate target").clicked() {
                graph.select([target]);
            }
            if ui.button("Remove route").clicked() {
                graph.disconnect(from, to);
                self.row = None;
            }
        } else if routes.is_empty() {
            ui.label("No external modulation routes match this filter.");
        }
        ui.section("Connect modulation", &mut self.add, |ui| {
            connect(ui, graph, &mut self.source, &mut self.target)
        });
        ui.label("The amplitude envelope shapes audio directly. It has no separate modulation-depth control.");
    }
}

fn connect(ui: &mut Ui, graph: &mut Graph, source: &mut usize, target: &mut usize) {
    let sources: Vec<_> = graph
        .nodes()
        .filter(|id| source_node(graph.folder(id).unwrap_or("")))
        .map(str::to_owned)
        .collect();
    let targets: Vec<_> = graph
        .nodes()
        .filter(|id| target_pin(graph.folder(id).unwrap_or("")).is_some())
        .map(str::to_owned)
        .collect();
    if sources.is_empty() || targets.is_empty() {
        ui.label("Add an LFO or ADSR and an SVF, VCO, or VCA to connect a CV route.");
        return;
    }
    *source = (*source).min(sources.len() - 1);
    *target = (*target).min(targets.len() - 1);
    let source_names: Vec<_> = sources.iter().map(|id| label(graph, id)).collect();
    let target_names: Vec<_> = targets.iter().map(|id| label(graph, id)).collect();
    ui.dropdown(
        "Source",
        source,
        &source_names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    ui.dropdown(
        "Target",
        target,
        &target_names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let from = format!("{}.out", sources[*source]);
    let to = format!(
        "{}.{}",
        targets[*target],
        target_pin(graph.folder(&targets[*target]).unwrap()).unwrap()
    );
    if graph.wires().any(|(_, end)| end == to) {
        ui.label("The target already has a source. Remove its route before connecting another.");
    } else if ui.button("Connect route").clicked() {
        graph.connect(&from, &to);
    }
}
