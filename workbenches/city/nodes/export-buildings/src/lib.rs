use geometry::{Scene, Sockets, export};
use patina_sdk::{PathKind, Ui, node, project_path};

/// Writes each building of the scene to its own file `building-<group>.glb` in the folder, with
/// the center of its base at the origin. It writes only after a click on **Export**, or at each
/// fire of a trigger with an order wire to this node (9.4). A run after a change does not write.
#[node]
pub struct ExportBuildings {
    folder: String,
    request: u32,
    /// The request of the last click. The request from the graph file thus never writes.
    armed: Option<u32>,
    status: String,
}

impl ExportBuildings {
    pub fn init() -> Self {
        ExportBuildings { folder: String::new(), request: 0, armed: None, status: "Choose a folder and click Export.".into() }
    }

    pub fn process(&mut self, scene: &Scene, folder: &str, request: u32) -> Result<(), String> {
        folder.clone_into(&mut self.folder);
        self.request = request;
        if self.armed != Some(request) && !patina_sdk::fired() {
            return Ok(());
        }
        self.armed = None;
        let result = write(scene, folder);
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

fn write(scene: &Scene, folder: &str) -> Result<String, String> {
    if folder.is_empty() {
        return Err("Choose a folder".into());
    }
    let groups = scene.groups();
    if groups.is_empty() {
        return Err("The scene has no buildings".into());
    }
    let root = project_path(folder);
    std::fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    let mut triangles = 0;
    for &group in &groups {
        let (mut mesh, colors) = scene.bake(Some(group));
        let (mut low, mut high) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for p in &mesh.positions {
            for i in 0..3 {
                (low[i], high[i]) = (low[i].min(p[i]), high[i].max(p[i]));
            }
        }
        let base = [(low[0] + high[0]) / 2.0, (low[1] + high[1]) / 2.0, low[2]];
        for p in &mut mesh.positions {
            *p = std::array::from_fn(|i| p[i] - base[i]);
        }
        export::write(&root.join(format!("building-{group}.glb")), &mesh, &Sockets::default(), &colors)?;
        triangles += mesh.triangles();
    }
    Ok(format!("Wrote {} buildings to {folder}, {triangles} triangles", groups.len()))
}

#[cfg(test)]
mod tests {
    use city::{Plot, Plots, Terrain};
    use geometry::Seed;

    use super::*;

    #[test]
    fn only_a_click_writes_one_file_for_each_building() {
        let folder = std::env::temp_dir().join(format!("patina-export-buildings-{}", std::process::id()));
        let text = folder.to_str().unwrap();
        let lot = |x: f64| Plot { corners: vec![[x, 0.0], [x + 16.0, 0.0], [x + 16.0, 20.0], [x, 20.0]], fronts: vec![Some(2); 4], floors: 2, ..Plot::default() };
        let plans = city::buildings(&Plots(vec![lot(0.0), lot(30.0)]), &Terrain::default(), Seed(1), 3.0, 2.0).unwrap();
        let scene = city::walls(&plans);
        let mut node = ExportBuildings::init();
        node.process(&scene, text, 3).unwrap();
        assert!(!folder.exists());
        node.armed = Some(4);
        node.process(&scene, text, 4).unwrap();
        assert!(node.status.starts_with("Wrote 2 buildings"), "{}", node.status);
        assert!(folder.join("building-1.glb").exists() && folder.join("building-2.glb").exists());
        let glb = std::fs::read(folder.join("building-1.glb")).unwrap();
        assert!(glb.windows(12).any(|w| w == b"\"materials\":"), "the item colors give materials");
        node.armed = Some(5);
        assert!(node.process(&Scene::default(), text, 5).is_err());
        std::fs::remove_dir_all(&folder).unwrap();
    }
}
