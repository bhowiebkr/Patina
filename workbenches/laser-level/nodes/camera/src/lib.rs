use measurement::{
    Frame,
    acquisition::Camera as Device,
    camera,
    core::camera::{CameraSettings, ControlMode, Mains},
    settings,
};
use measurement_ui::{AMBER, panel, place};
use patina_sdk::Ui;

#[patina_sdk::node(domain=frame, free_layout, color="#655219")]
pub struct Camera {
    device: Device,
    settings: CameraSettings,
    enabled: bool,
    devices: Vec<String>,
    advanced: bool,
}
impl Camera {
    pub fn init() -> Self {
        Self {
            device: Device::default(),
            settings: CameraSettings::default(),
            enabled: true,
            devices: vec![camera::synthetic::NAME.into()],
            advanced: false,
        }
    }
    pub fn process(&mut self, settings: &str, enabled: bool) -> Result<Frame, String> {
        self.settings = measurement::settings::camera(settings)?;
        self.enabled = enabled;
        if let Some(device) = &self.settings.device
            && !self.devices.contains(device)
        {
            self.devices.push(device.clone());
        }
        self.device.update(settings, enabled)
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        let extra = if self.advanced {
            self.device.controls.iter().map(|c| if c.auto.is_some() { 64.0 } else { 32.0 }).sum::<f32>()
                + 64.0
                + self.device.refused.len() as f32 * 24.0
        } else {
            0.0
        };
        panel(ui, [328.0, 548.0 + extra], "CAMERA / ACQUISITION", "RAW SENSOR / MANUAL CONTROL", AMBER);
        let mut selected = self.devices.iter().position(|n| Some(n) == self.settings.device.as_ref()).unwrap_or(0);
        place(ui, [16.0, 76.0]);
        if ui.dropdown("Device", &mut selected, &self.devices.iter().map(String::as_str).collect::<Vec<_>>()).changed()
        {
            self.settings.device = Some(self.devices[selected].clone());
            self.settings.mode = None;
            ui.set_param("settings", settings::encode(&self.settings));
        }
        place(ui, [16.0, 110.0]);
        if ui.button("Rescan cameras").clicked() {
            self.devices = camera::devices().into_iter().map(|d| d.name).collect();
        }
        place(ui, [180.0, 110.0]);
        if ui.toggle("Capture", &mut self.enabled).changed() {
            ui.set_param("enabled", self.enabled);
        }
        let labels: Vec<String> = self
            .device
            .modes
            .iter()
            .map(|m| format!("{} x {} / {} / {} fps", m.width, m.height, m.format.name(), m.fps))
            .collect();
        let mut mode = self.device.modes.iter().position(|m| Some(m) == self.settings.mode.as_ref()).unwrap_or(0);
        place(ui, [16.0, 150.0]);
        if ui.dropdown("Mode", &mut mode, &labels.iter().map(String::as_str).collect::<Vec<_>>()).changed()
            && mode < self.device.modes.len()
        {
            self.settings.mode = Some(self.device.modes[mode]);
            ui.set_param("settings", settings::encode(&self.settings));
        }
        let mut mains = usize::from(self.settings.mains == Mains::Hz60);
        place(ui, [16.0, 186.0]);
        if ui.tabs("Mains", &mut mains, &["50 Hz", "60 Hz"]).changed() {
            self.settings.mains = if mains == 0 { Mains::Hz50 } else { Mains::Hz60 };
            ui.set_param("settings", settings::encode(&self.settings));
        }
        place(ui, [16.0, 225.0]);
        if ui.button("Lock for measuring").clicked() {
            self.settings.controls =
                measurement::core::camera::for_measuring(&self.device.controls, self.settings.mains);
            ui.set_param("settings", settings::encode(&self.settings));
        }
        place(ui, [16.0, 270.0]);
        ui.label(&self.device.status);
        let mut histogram = [0.0; 256];
        if let Some(im) = &self.device.frame.image {
            for &v in &im.data {
                histogram[v as usize] += 1.0;
            }
        }
        let max = histogram.iter().copied().fold(1.0, f32::max);
        place(ui, [16.0, 300.0]);
        ui.plot_axes("Sensor histogram", &histogram, 0.0..=max, 0.0..=255.0, ["level", "pixels"]);
        place(ui, [16.0, 470.0]);
        ui.label(&format!("{} capture frames dropped", self.device.dropped));
        place(ui, [16.0, 502.0]);
        ui.section("Camera controls / readback", &mut self.advanced, |ui| {
            for info in &self.device.controls {
                let mut value = info.value;
                if ui.number(info.id.label(), &mut value).changed() {
                    self.settings.set(info.id, ControlMode::Manual(info.snap(value)));
                    ui.set_param("settings", settings::encode(&self.settings));
                }
                if let Some(mut auto) = info.auto
                    && ui.toggle(&format!("{} automatic", info.id.label()), &mut auto).changed()
                {
                    self.settings.set(info.id, if auto { ControlMode::Auto } else { ControlMode::Manual(info.value) });
                    ui.set_param("settings", settings::encode(&self.settings));
                }
            }
            if self.device.controls.is_empty() {
                ui.label("No controls reported by this camera");
            }
            for refusal in &self.device.refused {
                ui.label(refusal);
            }
        });
        ui.pin("out", [328.0, 270.0]);
    }
}
