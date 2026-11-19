use material_lab::Material;
use patina_sdk::{PathKind, Ui};

/// Writes finished PBR passes only after the user presses Export passes.
#[patina_sdk::node(free_layout, full_gui, color = "#79533b")]
pub struct MaterialExport {
    path: String,
    request: u32,
    armed: Option<u32>,
    status: String,
}

impl MaterialExport {
    pub fn init() -> Self {
        Self {
            path: String::new(),
            request: 0,
            armed: None,
            status: "Choose a new ZIP filename.".into(),
        }
    }
    pub fn process(&mut self, material: &Material, path: &str, request: u32) -> Result<(), String> {
        self.path = path.into();
        self.request = request;
        if self.armed == Some(request) {
            self.armed = None;
            match material_lab::export::write(material, &patina_sdk::project_path(path)) {
                Ok(count) => self.status = format!("Saved {count} PBR passes."),
                Err(e) => {
                    self.status = e.clone();
                    return Err(e);
                }
            }
        }
        Ok(())
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        material_panel::workflow::frame(
            ui,
            "EXPORT / PBR PASSES",
            "UV ATLAS / PNG MAPS IN A ZIP",
            286.0,
        );
        ui.pin("material", [0.0, 78.0]);
        ui.pin("path", [0.0, 115.0]);
        ui.pin("request", [0.0, 161.0]);
        ui.place([16.0, 68.0]);
        ui.label("FINISHED MATERIAL");
        ui.place([16.0, 100.0]);
        if ui
            .file_path("ZIP file", &mut self.path, PathKind::Save)
            .changed()
        {
            ui.set_param("path", &self.path);
        }
        ui.place([16.0, 146.0]);
        if ui.button("Export passes").clicked() {
            let next = self.request.wrapping_add(1);
            self.armed = Some(next);
            ui.set_param("request", next);
        }
        ui.place([16.0, 190.0]);
        ui.label("Color / Roughness / Metallic / AO / ORM");
        ui.place([16.0, 213.0]);
        ui.label("Normal +Y / Height 16-bit / Opacity");
        ui.place([16.0, 251.0]);
        ui.label(&self.status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_saved_request_never_arms_an_export() {
        let mut node = MaterialExport::init();
        let material = Material::default();
        assert!(
            node.process(&material, "must-not-be-written.zip", 42)
                .is_ok()
        );
        node.armed = Some(43);
        assert!(
            node.process(&material, "must-not-be-written.zip", 43)
                .is_err()
        );
        assert!(node.armed.is_none());
        assert!(
            node.process(&material, "must-not-be-written.zip", 43)
                .is_ok()
        );
    }
}
