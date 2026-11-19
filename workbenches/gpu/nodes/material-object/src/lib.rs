use material_lab::object::{self, Object};
use patina_sdk::{PathKind, Ui};

/// Loads a UV-mapped OBJ or a Millwright asset.json and mesh.bin pair.
#[patina_sdk::node(free_layout, full_gui, color = "#79533b")]
pub struct MaterialObject {
    path: String,
    loaded_path: String,
    revision: u32,
    object: Object,
    status: String,
}

impl MaterialObject {
    pub fn init() -> Self {
        Self {
            path: String::new(),
            loaded_path: String::new(),
            revision: 0,
            object: Object::default(),
            status: "Choose an object, then Run.".into(),
        }
    }
    pub fn process(&mut self, path: &str, reload: u32) -> Result<Object, String> {
        self.path = path.into();
        if self.object.0.is_none() || self.loaded_path != path || self.revision != reload {
            let result = object::load(&patina_sdk::project_path(path));
            match result {
                Ok(g) => {
                    self.status = format!(
                        "{} triangles / {} parts",
                        g.mesh.indices.len() / 3,
                        g.info.parts.len()
                    );
                    self.object = Object(Some(g));
                    self.loaded_path = path.into();
                    self.revision = reload;
                }
                Err(e) => {
                    self.status = e.clone();
                    return Err(e);
                }
            }
        }
        Ok(self.object.clone())
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        material_panel::workflow::frame(ui, "OBJECT / UV MESH", "OBJ OR MILLWRIGHT ASSET", 248.0);
        ui.pin("out", [340.0, 224.0]);
        ui.pin("path", [0.0, 84.0]);
        ui.pin("reload", [0.0, 132.0]);
        ui.place([16.0, 72.0]);
        if ui
            .file_path("Object file", &mut self.path, PathKind::Open)
            .changed()
        {
            ui.set_param("path", &self.path);
        }
        ui.place([16.0, 118.0]);
        if ui.button("Reload object").clicked() {
            ui.set_param("reload", self.revision.wrapping_add(1));
        }
        ui.place([16.0, 162.0]);
        ui.label("Use triangles and a non-overlapping UV atlas.");
        ui.place([16.0, 184.0]);
        ui.label("OBJ material files and textures are not imported.");
        ui.place([16.0, 214.0]);
        ui.label(&self.status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn object_loading_reuses_success_and_preserves_it_after_an_error() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../types/forge-runtime/assets/ball/asset.json");
        let path = path.to_str().unwrap();
        let mut node = MaterialObject::init();
        let first = node.process(path, 0).unwrap().0.unwrap();
        let cached = node.process(path, 0).unwrap().0.unwrap();
        assert!(std::sync::Arc::ptr_eq(&first, &cached));
        assert!(node.process("missing-object.obj", 0).is_err());
        assert!(std::sync::Arc::ptr_eq(
            &first,
            node.object.0.as_ref().unwrap()
        ));
        let reloaded = node.process(path, 1).unwrap().0.unwrap();
        assert!(!std::sync::Arc::ptr_eq(&first, &reloaded));
    }
}
