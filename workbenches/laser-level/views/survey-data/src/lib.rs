use measurement::{ResultData, core::units::Unit, settings, survey::Snapshot};
use patina_sdk::{Graph, Ui, view};

#[view]
pub struct SurveyData {
    page: usize,
    unit: usize,
    selected: Option<usize>,
    visit: Option<usize>,
    file: String,
}
impl SurveyData {
    pub fn init() -> Self {
        Self { page: 0, unit: 0, selected: None, visit: None, file: String::new() }
    }
    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        let survey_id = graph.nodes().find(|id| graph.folder(id) == Some("nodes/survey")).map(str::to_owned);
        let result_id = graph.nodes().find(|id| graph.folder(id) == Some("nodes/results")).map(str::to_owned);
        let Some(id) = survey_id else {
            ui.label("Open Manual Survey or Automated Survey.");
            return;
        };
        let snapshot = graph.value::<Snapshot>(&id, "out").cloned();
        let result = result_id.and_then(|id| graph.value::<ResultData>(&id, "out").cloned());
        let Some(s) = snapshot else {
            ui.label("Run the graph to read the survey data.");
            return;
        };
        ui.columns(160.0, |ui| {
            ui.column(|ui| {
                ui.tabs("Survey data", &mut self.page, &["Stations", "Raw visits", "Session"]);
            });
            ui.column(|ui| {
                ui.tabs("Height unit", &mut self.unit, &["um", "mm", "in"]);
            });
            ui.column(|ui| {
                ui.label(&format!("Run {} / {} saved visits", s.run + 1, s.run().map_or(0, |r| r.visits.len()),));
            });
        });
        if self.page == 0 {
            if let Some(data) = result {
                self.selected = data.result.as_ref().and_then(|r| r.points.iter().position(|p| p.station == s.station));
                if measurement_ui::results::table(ui, &data, Unit::ALL[self.unit], &mut self.selected)
                    && let Some(p) = self.selected.and_then(|i| data.result.as_ref()?.points.get(i))
                {
                    let mut options = s.options.clone();
                    options.station = p.station;
                    graph.set_param(&id, "options", settings::encode(&options));
                }
            } else {
                ui.label("Connect a Results node to the survey.");
            }
        } else if self.page == 1 {
            measurement_ui::survey::visits(ui, &s, &mut self.visit);
        } else {
            ui.label(&format!("Session file: {}", s.options.file));
            ui.label(
                "Each kept sample saves the frame readings and measurement settings. Raw camera images are not stored.",
            );
            ui.label("Run history stays in the session file. Results show the current run.");
            ui.label("The automated graph simulates movement. It has no machine connection.");
            if ui.button("Save session").clicked() {
                graph.set_param(&id, "request", s.command("save"));
            }
            if ui.button("Select survey node").clicked() {
                graph.select([id.as_str()]);
            }
            if self.file.is_empty() {
                self.file = s.options.file.clone();
            }
            ui.file_path("Session file", &mut self.file, patina_sdk::PathKind::Save);
            if ui.button("Open / new session").clicked() {
                let mut options = s.options.clone();
                options.file = self.file.clone();
                graph.set_param(&id, "options", settings::encode(&options));
            }
        }
        if !s.error.is_empty() {
            ui.label(&s.error);
        }
    }
}
