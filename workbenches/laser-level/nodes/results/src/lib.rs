use measurement::{ResultData, results::Processor, survey::Snapshot};
use patina_sdk::Ui;
#[patina_sdk::node(domain=frame,free_layout,full_gui,color="#59366f")]
pub struct Results {
    processor: Processor,
    display: measurement_ui::results::Display,
    interpolate: bool,
}
impl Results {
    pub fn init() -> Self {
        Self { processor: Processor::default(), display: Default::default(), interpolate: true }
    }
    pub fn process(&mut self, survey: &Snapshot, interpolate: bool) -> ResultData {
        self.interpolate = interpolate;
        self.processor.update(survey, interpolate)
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        self.display.ui(ui, &self.processor.output);
        measurement_ui::place(ui, [16.0, 784.0]);
        if ui.toggle("Interpolate between zero visits", &mut self.interpolate).changed() {
            ui.set_param("interpolate", self.interpolate);
        }
        ui.pin("survey", [0.0, 120.0]);
        ui.pin("out", [480.0, 120.0]);
    }
}
