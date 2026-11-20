use measurement::{ResultData, core::units::Unit, storage};
use measurement_ui::{AMBER, panel, place};
use patina_sdk::{PathKind, Ui};
#[patina_sdk::node(domain=frame,free_layout,color="#655219")]
pub struct Export {
    path: String,
    format: u32,
    unit: u32,
    request: u32,
    armed: Option<u32>,
    status: String,
    data: ResultData,
}
impl Export {
    pub fn init() -> Self {
        Self {
            path: "survey.csv".into(),
            format: 0,
            unit: 0,
            request: 0,
            armed: None,
            status: "Choose a new file, then export.".into(),
            data: Default::default(),
        }
    }
    pub fn process(
        &mut self,
        results: &ResultData,
        path: &str,
        format: u32,
        unit: u32,
        request: u32,
    ) -> Result<String, String> {
        self.path = path.into();
        self.format = format;
        self.unit = unit;
        self.request = request;
        self.data = results.clone();
        if self.armed != Some(request) {
            return Ok(self.status.clone());
        }
        self.armed = None;
        let result: Result<String, String> = (|| {
            let data = results.result.as_ref().ok_or("No surface results to export")?;
            if path.trim().is_empty() {
                return Err("Choose an export file".into());
            }
            let text = if format == 1 {
                data.json(&results.name, results.correction)
            } else {
                data.csv(Unit::ALL[unit.min(2) as usize])
            };
            storage::export(&patina_sdk::project_path(path), &text)?;
            Ok(format!("Exported {} stations", data.points.len()))
        })();
        self.status = match result {
            Ok(s) => s,
            Err(e) => format!("Export failed: {e}"),
        };
        Ok(self.status.clone())
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        panel(ui, [360.0, 306.0], "EXPORT / SURVEY DATA", "CSV FOR TABLES. JSON FOR SCRIPTS.", AMBER);
        place(ui, [16.0, 78.0]);
        if ui.file_path("File", &mut self.path, PathKind::Save).changed() {
            ui.set_param("path", &self.path);
        }
        let mut format = self.format as usize;
        place(ui, [16.0, 119.0]);
        if ui.tabs("Format", &mut format, &["CSV", "JSON"]).changed() {
            ui.set_param("format", format);
        }
        let mut unit = self.unit as usize;
        place(ui, [16.0, 161.0]);
        if ui.tabs("Height unit", &mut unit, &["um", "mm", "in"]).changed() {
            ui.set_param("unit", unit);
        }
        place(ui, [16.0, 204.0]);
        if ui.button("Export new file").clicked() {
            let next = self.request.wrapping_add(1);
            self.armed = Some(next);
            ui.set_param("request", next);
        }
        place(ui, [16.0, 249.0]);
        ui.label(&self.status);
        place(ui, [16.0, 280.0]);
        ui.label("New files only. JSON heights use um.");
        ui.pin("results", [0.0, 119.0]);
        ui.pin("out", [360.0, 249.0]);
    }
}
