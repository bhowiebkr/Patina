use geometry::{Solid, Sockets, export};
use patina_sdk::{PathKind, Ui, node, project_path};

/// Writes the solid to a `.glb`, `.obj` or `.stl` file, with the sockets in the glTF extras. It
/// writes only after a click on **Export**, or at each fire of a trigger with an order wire to
/// this node (9.4). A run after a change does not write.
#[node]
pub struct Export {
    path: String,
    request: u32,
    /// The request of the last click. The request from the graph file thus never writes.
    armed: Option<u32>,
    status: String,
}

impl Export {
    pub fn init() -> Self {
        Export { path: String::new(), request: 0, armed: None, status: "Choose a file and click Export.".into() }
    }

    pub fn process(&mut self, solid: &Solid, sockets: &Sockets, path: &str, request: u32) -> Result<(), String> {
        path.clone_into(&mut self.path);
        self.request = request;
        if self.armed != Some(request) && !patina_sdk::fired() {
            return Ok(());
        }
        self.armed = None;
        let result = write(solid, sockets, path);
        match &result {
            Ok(text) => {
                println!("{text}");
                self.status.clone_from(text);
            }
            Err(e) => self.status.clone_from(e),
        }
        result.map(drop)
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        if ui.file_path("File", &mut self.path, PathKind::Save).changed() {
            ui.set_param("path", &self.path);
        }
        if ui.button("Export").clicked() {
            let next = self.request.wrapping_add(1);
            self.armed = Some(next);
            ui.set_param("request", next);
        }
        ui.label(&self.status);
    }
}

fn write(solid: &Solid, sockets: &Sockets, path: &str) -> Result<String, String> {
    if path.is_empty() {
        return Err("Choose a .glb, .obj or .stl file name".into());
    }
    let mesh = solid.clone().checked()?.mesh();
    // A solid has no colors, so the file has no materials.
    export::write(&project_path(path), &mesh, sockets, &[])?;
    Ok(format!("Wrote {path}, {} triangles", mesh.triangles()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_click_writes() {
        let folder = std::env::temp_dir().join(format!("patina-export-node-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("box.glb");
        let text = path.to_str().unwrap();
        let (cube, sockets) = (Solid::cube(1.0, 1.0, 1.0), Sockets::default());
        let mut node = Export::init();
        // A run with the request of the graph file does not write.
        node.process(&cube, &sockets, text, 3).unwrap();
        assert!(!path.exists());
        node.armed = Some(4);
        node.process(&cube, &sockets, text, 4).unwrap();
        assert!(path.exists() && node.status.ends_with("12 triangles"), "{}", node.status);
        assert!(node.armed.is_none());
        node.armed = Some(5);
        assert!(node.process(&cube, &sockets, "", 5).is_err());
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
