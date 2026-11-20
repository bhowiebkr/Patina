use measurement::{Frame, core::frame::Orientation};
use measurement_ui::{TEAL, panel, place};
use patina_sdk::Ui;
#[patina_sdk::node(domain=frame,free_layout,color="#175c59")]
pub struct Turn {
    orientation: Orientation,
}
impl Turn {
    pub fn init() -> Self {
        Self { orientation: Orientation::default() }
    }
    pub fn process(&mut self, frame: &Frame, turns: u32, mirror: bool) -> Frame {
        self.orientation = Orientation { quarter_turns: (turns % 4) as u8, mirrored: mirror };
        Frame { orientation: self.orientation, ..frame.clone() }
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        panel(ui, [268.0, 220.0], "ORIENTATION", "SENSOR TO WORLD", TEAL);
        place(ui, [16.0, 78.0]);
        let mut turn = self.orientation.quarter_turns as usize;
        if ui.tabs("Clockwise rotation", &mut turn, &["0", "90", "180", "270"]).changed() {
            ui.set_param("turns", turn);
        }
        place(ui, [16.0, 115.0]);
        if ui.toggle("Mirror", &mut self.orientation.mirrored).changed() {
            ui.set_param("mirror", self.orientation.mirrored);
        }
        place(ui, [16.0, 153.0]);
        ui.label(&format!("Reads across {:?} beam", self.orientation.axis()));
        place(ui, [16.0, 182.0]);
        ui.label(if self.orientation.invert() { "Height sign: inverted" } else { "Height sign: positive" });
        ui.pin("frame", [0.0, 115.0]);
        ui.pin("out", [268.0, 115.0]);
    }
}
