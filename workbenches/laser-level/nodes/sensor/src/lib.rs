use measurement::{
    Calibration, Frame,
    core::camera::{ModeScaling, Sensor},
    settings,
};
use measurement_ui::{PINK, field, panel, place};
use patina_sdk::Ui;
#[patina_sdk::node(domain=frame,free_layout,color="#71344f")]
pub struct SensorScale {
    sensor: Sensor,
    relative: f64,
    scale: f64,
}
impl SensorScale {
    pub fn init() -> Self {
        Self { sensor: Sensor::default(), relative: 0.01, scale: 3.0 }
    }
    pub fn process(
        &mut self,
        frame: &Frame,
        pitch: f64,
        width: u32,
        height: u32,
        cropped: bool,
        uncertainty: f64,
    ) -> Result<Calibration, String> {
        if !settings::range(pitch, 0.01, 100.0)
            || width == 0
            || height == 0
            || width > 32768
            || height > 32768
            || !settings::range(uncertainty, 0.0, 1.0)
        {
            return Err("Invalid sensor dimensions, pitch, or uncertainty".into());
        }
        self.sensor = Sensor {
            name: "Workbench sensor".into(),
            pixel_pitch_um: pitch,
            width_px: width,
            height_px: height,
            scaling: if cropped { ModeScaling::Cropped } else { ModeScaling::Scaled },
        };
        self.relative = uncertainty;
        let scale = frame.image.as_ref().map(|im| {
            let mut s = self.sensor.scale(im.width as u32, im.height as u32);
            s.invert = frame.orientation.invert();
            s.uncertainty_rel = uncertainty;
            self.scale = s.um_per_px;
            s
        });
        Ok(Calibration { sensor: self.sensor.clone(), scale })
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        panel(ui, [328.0, 362.0], "SENSOR / CALIBRATION", "FULL SENSOR AND ACQUIRED MODE", PINK);
        field(ui, [16.0, 78.0], "Pixel pitch", "pitch", &mut self.sensor.pixel_pitch_um, 0.01, "um");
        field(ui, [16.0, 115.0], "Full sensor width", "width", &mut self.sensor.width_px, 1, "px");
        field(ui, [16.0, 152.0], "Full sensor height", "height", &mut self.sensor.height_px, 1, "px");
        let mut mode = usize::from(self.sensor.scaling == ModeScaling::Cropped);
        place(ui, [16.0, 191.0]);
        if ui.tabs("Smaller modes", &mut mode, &["Scaled", "Cropped"]).changed() {
            ui.set_param("cropped", mode == 1);
        }
        field(ui, [16.0, 231.0], "Relative uncertainty", "uncertainty", &mut self.relative, 0.001, "fraction");
        place(ui, [16.0, 280.0]);
        ui.label(&format!("Effective scale: {:.5} um / frame pixel", self.scale));
        place(ui, [16.0, 316.0]);
        ui.label("Use the sensor data sheet for the actual pitch.");
        ui.pin("frame", [0.0, 280.0]);
        ui.pin("out", [328.0, 280.0]);
    }
}
