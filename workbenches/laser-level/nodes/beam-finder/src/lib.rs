use measurement::{
    Beam, Frame,
    acquisition::Finder,
    core::{beam::FinderConfig, frame::Roi},
    settings,
};
use measurement_ui::{TEAL, beam::Preview, panel, place};
use patina_sdk::Ui;
#[patina_sdk::node(domain=frame,free_layout,full_gui,color="#175c59")]
pub struct BeamFinder {
    finder: Finder,
    preview: Preview,
    config: FinderConfig,
}
impl BeamFinder {
    pub fn init() -> Self {
        Self { finder: Finder::default(), preview: Preview::default(), config: FinderConfig::default() }
    }
    pub fn process(&mut self, frame: &Frame, settings: &str) -> Result<Beam, String> {
        self.config = measurement::settings::finder(settings)?;
        self.finder.update(frame, settings)
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        panel(ui, [472.0, 730.0], "BEAM FINDER / SUBPIXEL POSITION", "IMAGE, PROFILE, AND SEARCH REGION", TEAL);
        place(ui, [16.0, 78.0]);
        match &self.finder.output.fix {
            Ok(f) => ui.label(&format!(
                "{:.3} px +/- {:.3} / {} strips / SNR {:.1}",
                f.position_px, f.uncertainty_px, f.strips_used, f.snr
            )),
            Err(e) => ui.label(e.text()),
        }
        place(ui, [16.0, 106.0]);
        ui.label("The source finder removes background and rejects strip outliers.");
        if let Some(roi) = self.preview.ui(ui, &self.finder.output) {
            self.config.roi = Some(roi);
            ui.set_param("settings", settings::encode(&self.config));
        }
        let mut strips = self.config.strips as u32;
        place(ui, [16.0, 458.0]);
        if ui.number("Strips", &mut strips).changed() {
            self.config.strips = strips.clamp(3, 128) as usize;
            ui.set_param("settings", settings::encode(&self.config));
        }
        place(ui, [16.0, 492.0]);
        if ui.number("Least signal / noise", &mut self.config.min_snr).changed() {
            ui.set_param("settings", settings::encode(&self.config));
        }
        place(ui, [16.0, 526.0]);
        if ui.number("Centroid half-window / sigma", &mut self.config.window_sigmas).changed() {
            ui.set_param("settings", settings::encode(&self.config));
        }
        place(ui, [16.0, 560.0]);
        if ui.number("Background starts / sigma", &mut self.config.background_sigmas).changed() {
            ui.set_param("settings", settings::encode(&self.config));
        }
        let im = self.finder.output.frame.image.as_ref();
        let r = self.config.roi.unwrap_or(Roi {
            x: 0,
            y: 0,
            width: im.map_or(640, |f| f.width),
            height: im.map_or(480, |f| f.height),
        });
        let mut v = [r.x as u32, r.y as u32, r.width as u32, r.height as u32];
        place(ui, [16.0, 597.0]);
        ui.columns(190.0, |ui| {
            for (i, name) in ["ROI X / px", "ROI Y / px", "ROI width / px", "ROI height / px"].iter().enumerate() {
                ui.column(|ui| {
                    if ui.number(name, &mut v[i]).changed() {
                        self.config.roi = Some(Roi {
                            x: v[0] as usize,
                            y: v[1] as usize,
                            width: v[2] as usize,
                            height: v[3] as usize,
                        });
                        ui.set_param("settings", settings::encode(&self.config));
                    }
                });
            }
        });
        place(ui, [16.0, 687.0]);
        if ui.button("Full sensor").clicked() {
            self.config.roi = None;
            ui.set_param("settings", settings::encode(&self.config));
        }
        ui.pin("frame", [0.0, 260.0]);
        ui.pin("out", [472.0, 260.0]);
    }
}
