use geometry::export;
use guns::{Gun, look};
use patina_sdk::{PathKind, Ui, node, project_path};

/// Writes the gun to `<name>.glb` in the folder, with the `muzzle` and `grip` sockets, and its
/// stats to `<name>.txt`. The `.glb` file has the look of the 3D view: one primitive for each
/// material of each part, with a glTF material for each color. It writes only after a click on **Export**, or at each fire of a
/// trigger with an order wire to this node (9.4). A run after a change does not write.
#[node(color = "#3a3c40")]
pub struct ExportGun {
    folder: String,
    name: String,
    request: u32,
    /// The request of the last click. The request from the graph file thus never writes.
    armed: Option<u32>,
    status: String,
}

impl ExportGun {
    pub fn init() -> Self {
        ExportGun { folder: String::new(), name: String::new(), request: 0, armed: None, status: "Choose a folder and click Export.".into() }
    }

    pub fn process(&mut self, gun: &Gun, folder: &str, name: &str, request: u32) -> Result<(), String> {
        folder.clone_into(&mut self.folder);
        name.clone_into(&mut self.name);
        self.request = request;
        if self.armed != Some(request) && !patina_sdk::fired() {
            return Ok(());
        }
        self.armed = None;
        let result = write(gun, folder, name);
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
        if ui.file_path("Folder", &mut self.folder, PathKind::Folder).changed() {
            ui.set_param("folder", &self.folder);
        }
        if ui.button("Export").clicked() {
            let next = self.request.wrapping_add(1);
            self.armed = Some(next);
            ui.set_param("request", next);
        }
        ui.label(&self.status);
    }
}

fn write(gun: &Gun, folder: &str, name: &str) -> Result<String, String> {
    if folder.is_empty() {
        return Err("Choose a folder".into());
    }
    let name = if name.trim().is_empty() { "gun" } else { name.trim() };
    if name.contains(['/', '\\', ':', '.']) {
        return Err(format!("{name:?} is not a file name: write letters, digits, - and _"));
    }
    let (mesh, colors) = look(gun).bake(None);
    if mesh.triangles() == 0 {
        return Err("The gun has no parts".into());
    }
    let root = project_path(folder);
    std::fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    export::write(&root.join(format!("{name}.glb")), &mesh, &gun.sockets, &colors)?;
    let text = root.join(format!("{name}.txt"));
    std::fs::write(&text, gun.report()).map_err(|e| format!("{}: {e}", text.display()))?;
    Ok(format!("Wrote {name}.glb and {name}.txt to {folder}, {} triangles", mesh.triangles()))
}

#[cfg(test)]
mod tests {
    use geometry::Seed;
    use guns::{Pick, generate};

    use super::*;

    #[test]
    fn only_a_click_writes_the_model_and_the_stats() {
        let folder = std::env::temp_dir().join(format!("patina-export-gun-{}", std::process::id()));
        let text = folder.to_str().unwrap();
        let gun = generate(Seed(5), Pick::Random, Pick::Random, Pick::Random, Pick::Random).unwrap();
        let mut node = ExportGun::init();
        node.process(&gun, text, "", 3).unwrap();
        assert!(!folder.exists());
        node.armed = Some(4);
        node.process(&gun, text, "", 4).unwrap();
        assert!(node.status.starts_with("Wrote gun.glb"), "{}", node.status);
        let glb = std::fs::read(folder.join("gun.glb")).unwrap();
        assert!(glb.windows(12).any(|w| w == b"\"materials\":"), "the material colors give glTF materials");
        assert_eq!(std::fs::read_to_string(folder.join("gun.txt")).unwrap(), gun.report());
        node.armed = Some(5);
        assert!(node.process(&gun, text, "../up", 5).is_err());
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
