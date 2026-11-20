use measurement::{Beam, Calibration, Reading};
use measurement_ui::{INK, MUTED, PINK, RED, digits, panel, place};
use patina_sdk::{Mode, Ui};
#[patina_sdk::node(domain=frame,free_layout,color="#71344f")]
pub struct Calibrated {
    reading: Reading,
}
impl Calibrated {
    pub fn init() -> Self {
        Self { reading: Reading::default() }
    }
    pub fn process(&mut self, beam: &Beam, calibration: &Calibration) -> Reading {
        self.reading = Reading { beam: beam.clone(), calibration: calibration.clone() };
        self.reading.clone()
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        panel(ui, [410.0, 218.0], "CALIBRATED READING", "ABSOLUTE BEAM POSITION / NOT SURFACE HEIGHT", PINK);
        if ui.mode() == Mode::Large {
            ui.label(&format!("{:?}", self.reading.position()));
            return;
        }
        place(ui, [16.0, 76.0]);
        let mut c = ui.canvas("Calibrated position and uncertainty", [378.0, 125.0]);
        match self.reading.position() {
            Some((v, u)) => {
                digits(&mut c, [4.0, 2.0], &format!("{v:.3}"), PINK);
                c.text([304.0, 36.0], INK, "um");
                c.text([4.0, 65.0], INK, format_args!("Within-frame uncertainty +/- {u:.3} um"));
                if let Ok(f) = &self.reading.beam.fix {
                    c.text(
                        [4.0, 91.0],
                        MUTED,
                        format_args!(
                            "Tilt {:.3} mm/m / {}",
                            f.tilt * 1000.0,
                            if f.warnings.any() { "CHECK BEAM WARNINGS" } else { "VALID" }
                        ),
                    );
                }
            }
            None => {
                c.text([4.0, 32.0], RED, "NO VALID READING");
                c.text([4.0, 65.0], MUTED, "Check the camera, beam, and calibration.");
            }
        }
        ui.pin("beam", [0.0, 100.0]);
        ui.pin("calibration", [0.0, 165.0]);
        ui.pin("out", [410.0, 100.0]);
    }
}
