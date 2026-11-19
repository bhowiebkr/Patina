//! Dock tools for the Ember sound workbench. Graph edits keep the graph as the source of truth.
use panel::{EDGE, INK, TEAL};
use patina_sdk::{Graph, Ui};

mod analysis;
mod controls;
mod export;
mod keyboard;
mod learn;
mod modulation;
pub mod parameters;
mod patterns;
mod presets;
mod sounds;
mod spectrum;
pub use analysis::{Analysis, Audio};
pub use controls::Controls;
pub use export::Export;
pub use keyboard::Keyboard;
pub use learn::Learn;
pub use modulation::Modulation;
pub use patterns::Patterns;
pub use sounds::Sounds;
#[cfg(test)]
mod tests;

pub fn width(ui: &Ui) -> f32 {
    (ui.available_size().unwrap_or([280.0, 200.0])[0] - 24.0).clamp(120.0, 4000.0)
}

fn title(ui: &mut Ui, name: &str, subtitle: &str) {
    let w = width(ui);
    let mut c = ui.canvas("", [w, 28.0]);
    c.rect([0.0, 2.0], [3.0, 18.0], TEAL, true);
    c.text([12.0, 16.0], INK, name);
    c.line([0.0, 27.0], [w, 27.0], EDGE);
    drop(c);
    ui.label(subtitle);
}

fn label(graph: &Graph, id: &str) -> String {
    format!(
        "{} / {}",
        graph
            .folder(id)
            .unwrap_or("node")
            .trim_start_matches("nodes/"),
        &id[id.len().saturating_sub(4)..]
    )
}

fn number(ui: &mut Ui, graph: &mut Graph, id: &str, key: &str, text: &str, range: [f32; 2]) {
    if !parameters::available(graph, id, graph.folder(id).unwrap_or(""), key) {
        ui.label(&format!("{text}: connected"));
        return;
    }
    let mut value = graph
        .param(id, key)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    if ui.number(text, &mut value).changed() && value.is_finite() {
        graph.set_param(
            id,
            key,
            value.clamp(f64::from(range[0]), f64::from(range[1])) as f32,
        );
    }
}

fn parameter(ui: &mut Ui, graph: &mut Graph, p: parameters::Parameter) {
    if !p.editable(graph) {
        ui.label(&format!("{}: connected or missing", p.label));
        return;
    }
    let mut value = p.read(graph).to_string().parse::<f64>().unwrap_or(0.0);
    let step = f64::from((p.range[1] - p.range[0]) / 1000.0);
    if ui.number_unit(p.label, &mut value, step, p.unit).changed() {
        p.write(graph, value as f32);
    }
}
