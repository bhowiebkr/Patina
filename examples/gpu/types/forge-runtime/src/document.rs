use super::Session;
use forge_nodes::{Engine, Graph, Registry};
use forge_png::Material;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

impl Session {
    pub fn open(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &Path,
        editing: bool,
    ) -> Result<(), String> {
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        {
            let material = forge_png::load(path).map_err(|e| e.to_string())?;
            self.open_material(device, queue, material, editing)?;
        } else if editing
            && path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        {
            self.graph = Graph::load(path).map_err(|e| e.to_string())?;
            self.materials.clear();
            let parent = path.parent().unwrap_or(Path::new("."));
            self.resource_root = if parent.file_name().is_some_and(|n| n == "materials") {
                parent.parent().unwrap_or(parent)
            } else {
                parent
            }
            .to_owned();
            let mut pending: Vec<String> = references(&self.graph);
            while let Some(id) = pending.pop() {
                if self.materials.contains_key(&id) {
                    continue;
                }
                if id.contains(['/', '\\']) || id.contains("..") || self.materials.len() >= 64 {
                    return Err("Invalid material reference".into());
                }
                let graph = Graph::load(&parent.join(format!("{id}.graph.json")))
                    .map_err(|e| e.to_string())?;
                pending.extend(references(&graph));
                self.materials.insert(id, graph);
            }
            let mut engine = Engine::new(device)?;
            self.configure_engine(device, &mut engine);
            engine.set_source_root(&self.resource_root);
            self.engine = engine;
            self.changed()?;
        } else {
            return Err("Open a WESL Forge material PNG".into());
        }
        self.evaluate(device, queue)?;
        self.status = format!("Opened {}", forge_core::display_path(path));
        Ok(())
    }

    pub fn open_material(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        material: Material,
        editing: bool,
    ) -> Result<(), String> {
        material.validate().map_err(|e| e.to_string())?;
        let m = &material.manifest;
        let mut files = if editing {
            Registry::embedded().files().clone()
        } else {
            BTreeMap::new()
        };
        files.extend(m.sources.clone());
        let mut registry = Registry::from_files(files);
        registry.set_linked(m.linked_wgsl.clone());
        registry.validate_pinned()?;
        super::subgraphs::validate_documents(&registry, &m.graph, &m.materials)?;
        let mut engine = Engine::with_registry(device, registry)?;
        let cache = PathBuf::from("material");
        let resources: std::sync::Arc<BTreeMap<String, Vec<u8>>> = std::sync::Arc::new(
            m.resources
                .iter()
                .map(|r| (r.path.clone(), material.binaries[&r.sha256].clone()))
                .collect(),
        );
        engine.set_resources(resources.clone());
        self.configure_engine(device, &mut engine);
        engine.set_source_root(&cache);
        if let Some(id) = &m.environment {
            let r = m
                .resources
                .iter()
                .find(|r| &r.path == id)
                .ok_or("Missing environment")?;
            let ext = Path::new(id)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("hdr");
            self.set_environment(
                device,
                queue,
                material.binaries[&r.sha256].clone(),
                ext,
                "Material environment".into(),
            )?;
        }
        self.drafts = m.drafts.clone();
        self.custom_nodes = m.custom_nodes.clone();
        self.graph = m.graph.clone();
        self.materials = m.materials.clone();
        self.engine = engine;
        self.remember_sources();
        self.resource_root = cache;
        self.resources = resources;
        self.set_document_lighting([m.exposure, m.rotation, m.tilt]);
        self.changed()?;
        self.evaluate(device, queue)?;
        Ok(())
    }

    pub fn package(&self) -> Result<Material, String> {
        let registry = self.engine.registry();
        let mut files = BTreeMap::new();
        let mut linked = BTreeMap::new();
        for g in std::iter::once(&self.graph).chain(self.materials.values()) {
            let (f, s) = registry.package_sources(g)?;
            files.extend(f);
            linked.extend(s);
        }
        for path in &self.custom_nodes {
            if let Some(def) = registry.defs().find(|d| &d.path == path) {
                let mut probe = Graph::default();
                probe.add(&def.id, [0., 0.]);
                let (f, s) = registry.package_sources(&probe)?;
                files.extend(f);
                linked.extend(s);
            }
        }
        let mut package = Material::new(self.graph.clone(), files, linked);
        package.manifest.drafts = self.drafts.clone();
        package.manifest.custom_nodes = self.custom_nodes.clone();
        package.manifest.materials = self.materials.clone();
        if self.graph.interface.is_some() || self.materials.values().any(|g| g.interface.is_some())
        {
            package.manifest.version = 2;
        }
        [
            package.manifest.exposure,
            package.manifest.rotation,
            package.manifest.tilt,
        ] = self.document_lighting();
        let mut graphs: Vec<(Option<String>, Graph)> = vec![(None, package.manifest.graph.clone())];
        graphs.extend(
            package
                .manifest
                .materials
                .iter()
                .map(|(id, g)| (Some(id.clone()), g.clone())),
        );
        for (id, mut graph) in graphs {
            for node in &mut graph.nodes {
                for key in ["path", "font"] {
                    if let Some(path) = node.text.get_mut(key).filter(|s| !s.is_empty()) {
                        let disk = self.resource_root.join(&*path);
                        let bytes = match self.resources.get(&*path) {
                            Some(bytes) => bytes.clone(),
                            None => std::fs::read(&disk)
                                .map_err(|e| format!("{}: {e}", disk.display()))?,
                        };
                        let ext = PathBuf::from(&*path)
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("bin")
                            .to_owned();
                        *path = package.add_resource(bytes, &ext);
                    }
                }
            }
            if let Some(id) = id {
                package.manifest.materials.insert(id, graph);
            } else {
                package.manifest.graph = graph;
            }
        }
        package.manifest.environment = Some(package.add_resource(
            self.environment_bytes.as_ref().clone(),
            &self.environment_ext,
        ));
        package.validate().map_err(|e| e.to_string())?;
        Ok(package)
    }

    pub fn save(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &Path,
    ) -> Result<(), String> {
        self.evaluate(device, queue)?;
        let material = self.package()?;
        let target = self.cover(device, queue)?;
        let [width, height] = target.size;
        let preview =
            forge_gpu::read_texture(device, queue, &target.color.texture, target.size, 1, 4)?;
        forge_png::save(path, &material, width, height, &preview).map_err(|e| e.to_string())?;
        self.status = format!(
            "Saved self-contained material: {}",
            forge_core::display_path(path)
        );
        Ok(())
    }
}

fn references(graph: &Graph) -> Vec<String> {
    graph
        .nodes
        .iter()
        .filter_map(|n| forge_nodes::subgraph::reference(n).map(str::to_owned))
        .collect()
}
