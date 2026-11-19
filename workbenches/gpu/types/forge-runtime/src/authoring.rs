//! Document snapshots and transactional authoring. Candidate engines replace the active one only on success.
use super::*;
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceSet {
    pub files: BTreeMap<String, String>,
    pub linked: BTreeMap<String, String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentState {
    pub graph: Graph,
    pub materials: BTreeMap<String, Graph>,
    pub drafts: BTreeMap<String, String>,
    pub custom_nodes: Vec<String>,
    pub sources: Arc<SourceSet>,
    pub resources: Arc<BTreeMap<String, Vec<u8>>>,
    pub resource_root: PathBuf,
    pub lighting: [f32; 3],
    pub environment: Arc<Vec<u8>>,
    pub environment_ext: String,
    pub environment_name: String,
}
/// A prepared implementation cannot enter a session until device validation succeeds.
pub struct SourceCandidate {
    engine: Engine,
    base: DocumentState,
    flat: Graph,
    kind: String,
    path: String,
    existed: bool,
    validated: bool,
}
impl SourceCandidate {
    /// Browser hosts await actual WebGPU validation before offering the candidate for acceptance.
    pub async fn validate(mut self, device: &wgpu::Device) -> Result<Self, String> {
        let mut graph = self.flat.clone();
        graph.add(&self.kind, [0., 0.]);
        self.engine
            .validate_graph_kernels_async(device, &graph)
            .await?;
        self.validated = true;
        Ok(self)
    }
}
impl Session {
    pub fn snapshot(&self) -> DocumentState {
        DocumentState {
            graph: self.graph.clone(),
            materials: self.materials.clone(),
            drafts: self.drafts.clone(),
            custom_nodes: self.custom_nodes.clone(),
            sources: self.source_state.clone(),
            resources: self.resources.clone(),
            resource_root: self.resource_root.clone(),
            lighting: self.document_lighting(),
            environment: self.environment_bytes.clone(),
            environment_ext: self.environment_ext.clone(),
            environment_name: self.environment_name.clone(),
        }
    }
    pub(crate) fn remember_sources(&mut self) {
        self.source_state = Arc::new(SourceSet {
            files: self.engine.registry().files().clone(),
            linked: self.engine.registry().linked_sources().clone(),
        });
    }
    pub fn restore(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        state: DocumentState,
    ) -> Result<(), String> {
        let reg = Registry::from_files(state.sources.files.clone());
        subgraphs::validate_documents(&reg, &state.graph, &state.materials)?;
        let flat = flatten::flatten_with_registry(&reg, "document", &state.graph, &|id| {
            state.materials.get(id)
        });
        if !flat.errors.is_empty() {
            return Err(flat.errors.join("\n"));
        }
        validate_graph(&reg, &flat.graph)?;
        let render_changed = !same_evaluation(&self.flat, &flat.graph);
        let mut replacement = None;
        if self.source_state != state.sources || self.resources != state.resources {
            let mut reg = reg;
            reg.set_linked(state.sources.linked.clone());
            let mut candidate = Engine::with_registry(device, reg)?;
            self.configure_engine(device, &mut candidate);
            candidate.set_source_root(&state.resource_root);
            candidate.set_resources(state.resources.clone());
            candidate.evaluate(device, queue, &flat.graph);
            let errors = candidate.errors();
            if !errors.is_empty() {
                return Err(errors.join("\n"));
            }
            replacement = Some(candidate);
        }
        if self.environment_bytes != state.environment
            || self.environment_ext != state.environment_ext
        {
            self.set_environment(
                device,
                queue,
                state.environment.as_ref().clone(),
                &state.environment_ext,
                state.environment_name.clone(),
            )?;
        }
        let replaced = replacement.is_some();
        if let Some(engine) = replacement {
            self.engine = engine;
        }
        self.set_document_lighting(state.lighting);
        self.graph = state.graph;
        self.materials = state.materials;
        self.drafts = state.drafts;
        self.custom_nodes = state.custom_nodes;
        self.source_state = state.sources;
        self.resources = state.resources;
        self.resource_root = state.resource_root;
        self.rebuild_flat()?;
        if replaced {
            self.ball
                .set_material(device, queue, &self.scene, self.engine.output(&self.flat));
            self.dirty = false;
        } else if render_changed {
            self.engine.graph_changed(&self.flat);
            self.dirty = true;
            self.evaluate(device, queue)?;
        }
        Ok(())
    }
    pub fn new_material(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: &str,
    ) -> Result<(), String> {
        let registry = Registry::embedded();
        let graph = forge_nodes::blank_material(&registry, name);
        let mut engine = Engine::with_registry(device, registry)?;
        self.configure_engine(device, &mut engine);
        self.graph = graph;
        self.materials.clear();
        self.drafts.clear();
        self.custom_nodes.clear();
        self.resources = Default::default();
        self.resource_root = PathBuf::from("material");
        self.engine = engine;
        self.set_document_lighting([1., 0., 0.]);
        self.set_environment(
            device,
            queue,
            include_bytes!("../assets/studio.hdr").to_vec(),
            "hdr",
            "Forge studio".into(),
        )?;
        self.set_ball_orientation(queue, glam::Quat::IDENTITY);
        self.remember_sources();
        self.changed()?;
        self.evaluate(device, queue)?;
        Ok(())
    }
    pub fn apply_source(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &str,
        text: &str,
    ) -> Result<(), String> {
        let mut candidate = self.prepare_source(device, path, text)?;
        candidate.engine.validate_kind(device, &candidate.kind)?;
        candidate
            .engine
            .validate_graph_kernels(device, &candidate.flat)?;
        candidate.validated = true;
        self.accept_source(device, queue, candidate)
    }
    pub fn prepare_source(
        &self,
        device: &wgpu::Device,
        path: &str,
        text: &str,
    ) -> Result<SourceCandidate, String> {
        if path.starts_with("nodes/subgraphs/forge_subgraph_") {
            return Err("Edit generated subgraph declarations through the graph interface".into());
        }
        if !path.starts_with("nodes/")
            || !path.ends_with(".wesl")
            || path.contains("..")
            || path.contains('\\')
        {
            return Err("Node paths must be relative nodes/*.wesl paths".into());
        }
        let existed = self.engine.registry().files().contains_key(path);
        let mut files = self.engine.registry().files().clone();
        files.insert(path.into(), text.into());
        let mut reg = Registry::from_files(files);
        if !reg.errors().is_empty() {
            return Err(reg
                .errors()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"));
        }
        let kind = reg
            .defs()
            .find(|d| d.path == path)
            .ok_or("Node declaration was not registered")?
            .id
            .clone();
        // Retain every unchanged node's pinned linked implementation.
        let mut linked = self.engine.registry().linked_sources().clone();
        linked.retain(|key, _| key.split('\n').next() != Some(kind.as_str()));
        reg.set_linked(linked);
        subgraphs::validate_documents(&reg, &self.graph, &self.materials)?;
        let mut probe = Graph::default();
        probe.add(&kind, [0., 0.]);
        // Resolve and validate complete generated WGSL before making a GPU pipeline (also on web).
        reg.package_sources(&probe)?;
        validate_graph(&reg, &self.flat)?;
        reg.package_sources(&self.flat)?;
        let mut candidate = Engine::with_registry(device, reg)?;
        self.configure_engine(device, &mut candidate);
        candidate.set_resources(self.resources.clone());
        candidate.set_source_root(&self.resource_root);
        Ok(SourceCandidate {
            engine: candidate,
            base: self.snapshot(),
            flat: self.flat.clone(),
            kind,
            path: path.into(),
            existed,
            validated: false,
        })
    }
    pub fn accept_source(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        candidate: SourceCandidate,
    ) -> Result<(), String> {
        if !candidate.validated {
            return Err("Node candidate has not passed device validation".into());
        }
        if self.snapshot() != candidate.base {
            return Err("Material changed while compiling; compile the latest draft again".into());
        }
        let SourceCandidate {
            engine,
            kind,
            path,
            existed,
            ..
        } = candidate;
        self.engine
            .accept_registry(engine.into_registry(), &self.flat);
        if !self.custom_nodes.iter().any(|p| p == &path) {
            self.custom_nodes.push(path.clone());
        }
        self.drafts.remove(&path);
        self.remember_sources();
        if existed {
            self.changed()?;
            self.evaluate(device, queue)?;
        }
        self.status = format!("Compiled {kind}; available in the node menu");
        Ok(())
    }
    pub fn import_resource(&mut self, bytes: Vec<u8>, extension: &str) -> String {
        let ext: String = extension
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(8)
            .collect();
        let path = format!(
            "resources/{}.{}",
            forge_png::hash(&bytes),
            if ext.is_empty() { "bin" } else { &ext }
        );
        Arc::make_mut(&mut self.resources).insert(path.clone(), bytes);
        path
    }
}

// Presentation and authoring metadata never invalidates evaluated node textures.
pub(crate) fn same_evaluation(a: &Graph, b: &Graph) -> bool {
    a.size == b.size
        && a.triplanar == b.triplanar
        && a.links == b.links
        && a.nodes.len() == b.nodes.len()
        && a.nodes.iter().zip(&b.nodes).all(|(a, b)| {
            a.id == b.id
                && a.kind == b.kind
                && a.params == b.params
                && a.text == b.text
                && a.size == b.size
                && a.uv == b.uv
        })
}
