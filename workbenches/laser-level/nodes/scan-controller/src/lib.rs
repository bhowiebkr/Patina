use measurement::survey::Motion;
use measurement_ui::{AMBER, field, panel, place};
use patina_sdk::Ui;
#[patina_sdk::node(domain=frame,free_layout,color="#655219")]
pub struct Controller {
    motion: Motion,
}
impl Controller {
    pub fn init() -> Self {
        Self { motion: Motion::default() }
    }
    #[allow(clippy::too_many_arguments)] // Each argument is a saved node parameter.
    pub fn process(
        &mut self,
        simulated: bool,
        feed: f64,
        lift: f64,
        dwell: f64,
        zero_every: u32,
        object: u32,
        relief: f64,
    ) -> Motion {
        self.motion = Motion {
            simulated,
            feed_mm_min: feed,
            lift_mm: lift,
            dwell_s: dwell,
            zero_every: zero_every as usize,
            object,
            relief_um: relief,
        };
        self.motion
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        panel(ui, [360.0, 470.0], "SCAN CONTROLLER / LOCAL", "SIMULATED MOTION / NO MACHINE CONNECTION", AMBER);
        place(ui, [16.0, 78.0]);
        if ui.toggle("Enable local simulation", &mut self.motion.simulated).changed() {
            ui.set_param("simulated", self.motion.simulated);
        }
        field(ui, [16.0, 118.0], "Travel feed", "feed", &mut self.motion.feed_mm_min, 100.0, "mm/min");
        field(ui, [16.0, 156.0], "Lift clearance", "lift", &mut self.motion.lift_mm, 1.0, "mm");
        field(ui, [16.0, 194.0], "Settling wait", "dwell", &mut self.motion.dwell_s, 0.1, "s");
        let mut every = self.motion.zero_every as u32;
        field(ui, [16.0, 232.0], "Zero every", "zero_every", &mut every, 1, "stations");
        place(ui, [16.0, 275.0]);
        let mut object = self.motion.object as usize;
        if ui.dropdown("Test object", &mut object, &["Surface plate", "Coin relief"]).changed() {
            ui.set_param("object", object);
            if object == 1 && self.motion.relief_um < 1.0 {
                ui.set_param("relief", 800.0);
            }
        }
        field(ui, [16.0, 313.0], "Coin relief scale", "relief", &mut self.motion.relief_um, 50.0, "um");
        place(ui, [16.0, 355.0]);
        ui.label("Coin: 26 mm diameter on a 30 x 30 mm bed.");
        place(ui, [16.0, 385.0]);
        ui.label("Use 41 x 41 stations at 0.75 mm spacing.");
        place(ui, [16.0, 415.0]);
        ui.label("Lift. Travel. Lower. Settle. Sample.");
        ui.pin("out", [360.0, 118.0]);
    }
}
