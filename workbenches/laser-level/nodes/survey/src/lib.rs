use measurement::{
    Plan, Reading,
    survey::{Motion, Snapshot, Survey as Runtime},
};
use patina_sdk::Ui;
#[patina_sdk::node(domain=frame,free_layout,full_gui,color="#175c59")]
pub struct Survey {
    runtime: Runtime,
}
impl Survey {
    pub fn init() -> Self {
        Self { runtime: Runtime::default() }
    }
    pub fn process(
        &mut self,
        reading: &Reading,
        plan: &Plan,
        motion: &Motion,
        options: &str,
        request: &str,
    ) -> Result<Snapshot, String> {
        self.runtime.update(reading, plan, motion, options, request)
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        let edits = measurement_ui::survey::controls(ui, &self.runtime.shown);
        if let Some(request) = edits.request {
            ui.set_param("request", request);
        }
        if let Some(options) = edits.options {
            ui.set_param("options", options);
        }
    }
}
