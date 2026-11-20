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
    pub fn process(&mut self, simulated: bool, feed: f64, lift: f64, dwell: f64, zero_every: u32) -> Motion {
        self.motion =
            Motion { simulated, feed_mm_min: feed, lift_mm: lift, dwell_s: dwell, zero_every: zero_every as usize };
        self.motion
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        panel(ui, [360.0, 350.0], "SCAN CONTROLLER / LOCAL", "SIMULATED MOTION / NO MACHINE CONNECTION", AMBER);
        place(ui, [16.0, 78.0]);
        if ui.toggle("Enable local simulation", &mut self.motion.simulated).changed() {
            ui.set_param("simulated", self.motion.simulated);
        }
        field(ui, [16.0, 118.0], "Travel feed", "feed", &mut self.motion.feed_mm_min, 100.0, "mm/min");
        field(ui, [16.0, 156.0], "Lift clearance", "lift", &mut self.motion.lift_mm, 1.0, "mm");
        field(ui, [16.0, 194.0], "Settling wait", "dwell", &mut self.motion.dwell_s, 0.1, "s");
        let mut every = self.motion.zero_every as u32;
        field(ui, [16.0, 232.0], "Zero every", "zero_every", &mut every, 1, "stations");
        place(ui, [16.0, 280.0]);
        ui.label("Lift. Travel. Lower. Settle. Sample.");
        place(ui, [16.0, 308.0]);
        ui.label("LinuxCNC will retain its real-time motion loop.");
        ui.pin("out", [360.0, 118.0]);
    }
}
