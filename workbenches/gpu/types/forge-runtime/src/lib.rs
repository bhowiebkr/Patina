//! Embeddable, UI-independent GPU material session shared by the viewer, editor and hosts.
mod authoring;
mod preview_target;
pub use preview_target::PreviewBinding;
mod document;
mod map_export;
pub mod organization;
pub mod requirements;
mod subgraphs;
mod textures;
pub use authoring::{DocumentState, SourceCandidate, SourceSet};
pub use subgraphs::add_port as add_subgraph_port;
pub use textures::{TextureEncoding, TextureImport, TextureInfo};
pub mod flatten;

use forge_core::{AssetInfo, MeshData};
use forge_nodes::{Engine, Graph, Registry};
use forge_render::{Environment, GpuAsset, Light, MeshMaps, OrbitCamera, SceneRenderer, View};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct Session {
    pub engine: Engine,
    pub graph: Graph,
    pub drafts: BTreeMap<String, String>,
    pub custom_nodes: Vec<String>,
    source_state: std::sync::Arc<SourceSet>,
    pub materials: BTreeMap<String, Graph>,
    pub flat: Graph,
    pub origins: BTreeMap<forge_nodes::NodeId, (String, forge_nodes::NodeId)>,
    pub aliases: flatten::OutputAliases,
    pub scene: SceneRenderer,
    pub ball: GpuAsset,
    pub info: AssetInfo,
    pub camera: OrbitCamera,
    pub ball_orientation: glam::Quat,
    ball_yaw: f32,
    ball_pitch: f32,
    pub exposure: f32,
    pub rotation: f32,
    pub tilt: f32,
    pub environment_name: String,
    pub resource_root: PathBuf,
    pub status: String,
    environment_bytes: std::sync::Arc<Vec<u8>>,
    environment_ext: String,
    _environment: Environment,
    _maps: MeshMaps,
    dirty: bool,
    resources: std::sync::Arc<BTreeMap<String, Vec<u8>>>,
    _mesh: MeshData,
    preview: Option<preview_target::Current>,
}

impl Session {
    pub fn mesh_maps(&self) -> &MeshMaps {
        &self._maps
    }
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self, String> {
        let info: AssetInfo = serde_json::from_str(include_str!("../assets/ball/asset.json"))
            .map_err(|e| e.to_string())?;
        let mesh = MeshData::from_bytes(
            Path::new("embedded-ball"),
            include_bytes!("../assets/ball/mesh.bin"),
        )
        .map_err(|e| e.to_string())?;
        let mut scene = SceneRenderer::new(device, queue)?;
        let studio = include_bytes!("../assets/studio.hdr").to_vec();
        let environment = load_environment(device, queue, &studio, "hdr")?;
        scene.set_environment(device, Some(&environment));
        let ball = GpuAsset::new(device, &scene, &info, &mesh);
        let bake = ball_bake()?;
        let maps = MeshMaps::from_bytes(device, queue, &ball, &info, &mesh, 2048, &bake)?
            .ok_or("Embedded ball bake does not match its mesh")?;
        let registry = Registry::embedded();
        let graph = forge_nodes::blank_material(&registry, "No material loaded");
        let mut engine = Engine::with_registry(device, registry)?;
        configure_engine(device, &mut engine, &maps, &info);
        let camera = OrbitCamera::for_asset(&info);
        let mut session = Self {
            engine,
            graph: graph.clone(),
            drafts: BTreeMap::new(),
            custom_nodes: Vec::new(),
            source_state: Default::default(),
            flat: graph,
            origins: BTreeMap::new(),
            aliases: BTreeMap::new(),
            materials: BTreeMap::new(),
            scene,
            ball,
            info,
            camera,
            ball_orientation: glam::Quat::IDENTITY,
            ball_yaw: 0.0,
            ball_pitch: 0.0,
            exposure: 1.0,
            rotation: 0.0,
            tilt: 0.0,
            environment_name: "Forge studio".into(),
            resource_root: PathBuf::from("material"),
            status: "Drop a material PNG, or use Open PNG".into(),
            environment_bytes: studio.into(),
            environment_ext: "hdr".into(),
            _environment: environment,
            _maps: maps,
            dirty: true,
            resources: Default::default(),
            _mesh: mesh,
            preview: None,
        };
        session.remember_sources();
        session.changed()?;
        session.evaluate(device, queue)?;
        Ok(session)
    }

    pub fn light(&self) -> Light {
        Light {
            exposure: self.exposure,
            env_yaw: self.rotation / 360.0,
            env_pitch: self.tilt / 360.0,
            ..self
                .preview
                .as_ref()
                .map_or_else(|| Light::for_environment(&self.info), |p| p.light)
        }
    }

    /// Rotate the model, never the camera or the environment. Mesh maps stay in object space.
    pub fn rotate_ball(&mut self, queue: &wgpu::Queue, dx: f32, dy: f32) {
        self.ball_yaw = (self.ball_yaw + dx + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        self.ball_pitch = (self.ball_pitch + dy).clamp(-1.45, 1.45);
        let local =
            glam::Quat::from_euler(glam::EulerRot::YXZ, self.ball_yaw, self.ball_pitch, 0.0);
        self.set_ball_orientation(
            queue,
            self.camera.tilt * local * self.camera.tilt.conjugate(),
        );
    }

    pub fn set_ball_orientation(&mut self, queue: &wgpu::Queue, orientation: glam::Quat) {
        self.ball_orientation = orientation;
        let local = self.camera.tilt.conjugate() * orientation * self.camera.tilt;
        let (yaw, pitch, _) = local.to_euler(glam::EulerRot::YXZ);
        self.ball_yaw = yaw;
        self.ball_pitch = pitch;
        let pose = self.ball_pose(orientation);
        self.ball.set_pose(queue, &pose);
    }

    fn ball_pose(&self, orientation: glam::Quat) -> Vec<glam::Mat4> {
        let pivot = self
            .info
            .view
            .as_ref()
            .map_or_else(|| self.info.bounds.center(), |v| glam::Vec3::from(v.target));
        let model = glam::Mat4::from_translation(pivot)
            * glam::Mat4::from_quat(orientation)
            * glam::Mat4::from_translation(-pivot);
        self.preview.as_ref().map_or_else(
            || vec![model; self.info.parts.len()],
            |preview| preview.pose.iter().map(|base| model * *base).collect(),
        )
    }

    fn rebuild_flat(&mut self) -> Result<(), String> {
        subgraphs::validate_documents(self.engine.registry(), &self.graph, &self.materials)?;
        let flat = flatten::flatten_with_registry(
            self.engine.registry(),
            "document",
            &self.graph,
            &|id| self.materials.get(id),
        );
        if !flat.errors.is_empty() {
            return Err(flat.errors.join("\n"));
        }
        validate_graph(self.engine.registry(), &flat.graph)?;
        self.flat = flat.graph;
        self.origins = flat.origins;
        self.aliases = flat.aliases;
        Ok(())
    }

    pub fn changed(&mut self) -> Result<(), String> {
        if std::iter::once(&self.graph)
            .chain(self.materials.values())
            .flat_map(|g| &g.nodes)
            .any(|n| {
                self.engine
                    .registry()
                    .get(&n.kind)
                    .is_some_and(|d| !self.source_state.files.contains_key(&d.path))
            })
        {
            self.remember_sources();
        }
        self.rebuild_flat()?;
        self.engine.graph_changed(&self.flat);
        self.dirty = true;
        Ok(())
    }

    /// Retain only the output currently inspected by the host, without changing the document.
    pub fn keep_inspected_output(&mut self, node: Option<forge_nodes::NodeId>) {
        let node = node.filter(|id| self.flat.node(*id).is_some());
        self.engine.keep(node);
        if node.is_some_and(|id| self.engine.node_texture(id).is_none()) {
            // The evaluator recomputes a kept output if the memory budget released it.
            self.dirty = true;
        }
    }

    pub fn resolve_output(
        &self,
        owner: Option<&str>,
        node: forge_nodes::NodeId,
        output: &str,
    ) -> Option<(forge_nodes::NodeId, String)> {
        let flat = if let Some(owner) = owner {
            self.origins
                .iter()
                .find(|(_, (reference, id))| reference == owner && *id == node)
                .map(|(flat, _)| *flat)?
        } else {
            node
        };
        let value = flatten::resolve(&self.aliases, flat, output)?;
        self.flat.node(value.0)?;
        Some(value)
    }

    pub fn parameter_changed(
        &mut self,
        reference: Option<&str>,
        id: forge_nodes::NodeId,
    ) -> Result<(), String> {
        self.rebuild_flat()?;
        if let Some(reference) = reference {
            for (flat_id, (owner, source)) in &self.origins {
                if owner == reference && *source == id {
                    self.engine.mark_dirty(&self.flat, *flat_id);
                }
            }
        } else {
            self.engine.mark_dirty(&self.flat, id);
        }
        self.dirty = true;
        Ok(())
    }

    pub fn enable_catalog(&mut self, _device: &wgpu::Device) -> Result<(), String> {
        self.engine.enable_catalog()?;
        Ok(())
    }

    /// Browser picking is one exact ray/mesh test at gesture start; it never renders pixels on the CPU.
    pub fn hit_mesh(&self, at: [f32; 2], aspect: f32) -> bool {
        use glam::{Mat4, Vec3, Vec4};
        let inverse = self.camera.view_proj(aspect).inverse();
        let unproject = |z| {
            let p = inverse * Vec4::new(at[0] * 2.0 - 1.0, 1.0 - at[1] * 2.0, z, 1.0);
            p.truncate() / p.w
        };
        let pivot = self
            .info
            .view
            .as_ref()
            .map_or_else(|| self.info.bounds.center(), |v| Vec3::from(v.target));
        let model = Mat4::from_translation(pivot)
            * Mat4::from_quat(self.ball_orientation)
            * Mat4::from_translation(-pivot);
        let inv_model = model.inverse();
        let origin = inv_model.transform_point3(unproject(0.0));
        let direction = inv_model
            .transform_vector3(unproject(1.0) - unproject(0.0))
            .normalize();
        for tri in self._mesh.indices.chunks_exact(3) {
            let a = Vec3::from(self._mesh.vertices[tri[0] as usize].position);
            let b = Vec3::from(self._mesh.vertices[tri[1] as usize].position);
            let c = Vec3::from(self._mesh.vertices[tri[2] as usize].position);
            let e1 = b - a;
            let e2 = c - a;
            let p = direction.cross(e2);
            let determinant = e1.dot(p);
            if determinant.abs() < 1e-8 {
                continue;
            }
            let s = origin - a;
            let u = s.dot(p) / determinant;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = s.cross(e1);
            let v = direction.dot(q) / determinant;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            if e2.dot(q) / determinant > 0.0 {
                return true;
            }
        }
        false
    }

    pub fn is_evaluated(&self) -> bool {
        !self.dirty
    }

    pub fn evaluate(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<bool, String> {
        if !self.dirty {
            return Ok(false);
        }
        self.validate_preview_maps(&self.flat)?;
        self.engine.evaluate(device, queue, &self.flat);
        let errors = self.engine.errors();
        if !errors.is_empty() {
            return Err(errors.join("\n"));
        }
        self.ball
            .set_material(device, queue, &self.scene, self.engine.output(&self.flat));
        self.dirty = false;
        Ok(true)
    }

    pub fn set_environment(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: Vec<u8>,
        extension: &str,
        name: String,
    ) -> Result<(), String> {
        let env = load_environment(device, queue, &bytes, extension)?;
        self._environment = env;
        self.apply_preview_environment(device);
        self.environment_bytes = bytes.into();
        self.environment_ext = extension.into();
        self.environment_name = name;
        Ok(())
    }

    pub fn render(&self, device: &wgpu::Device, queue: &wgpu::Queue, view: &View) {
        self.scene
            .render(device, queue, view, &self.camera, &self.light(), &self.ball);
    }

    pub fn thumbnail(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Vec<u8>, String> {
        let view = self.thumbnail_target_at(device, queue, 1024);
        forge_gpu::read_texture(device, queue, &view.color.texture, [1024, 1024], 1, 4)
    }

    pub fn thumbnail_target(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> View {
        self.thumbnail_target_at(device, queue, 1024)
    }

    pub fn thumbnail_target_at(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        side: u32,
    ) -> View {
        let view = View::new(device, &self.scene, [side, side]);
        let camera = OrbitCamera::for_asset(&self.info);
        // Covers use the resting pose a reopened material has, never a dragged preview.
        let pose = self.ball_pose(glam::Quat::IDENTITY);
        self.ball.with_pose(queue, &pose, |ball| {
            self.scene
                .render(device, queue, &view, &camera, &self.light(), ball);
        });
        view
    }

    pub fn replace_sources(
        &mut self,
        device: &wgpu::Device,
        files: BTreeMap<String, String>,
    ) -> Result<(), String> {
        let registry = Registry::from_files(files);
        if !registry.errors().is_empty() {
            return Err(registry
                .errors()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"));
        }
        let mut engine = Engine::with_registry(device, registry)?;
        self.configure_engine(device, &mut engine);
        engine.set_source_root(&self.resource_root);
        engine.set_resources(self.resources.clone());
        self.engine = engine;
        self.remember_sources();
        self.changed()
    }
}

pub fn ball_bake() -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    flate2::read::ZlibDecoder::new(&include_bytes!("../assets/ball/maps.bin.zlib")[..])
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}

fn configure_engine(device: &wgpu::Device, engine: &mut Engine, maps: &MeshMaps, info: &AssetInfo) {
    engine.set_budget(if cfg!(target_arch = "wasm32") {
        128 << 20
    } else {
        512 << 20
    });
    let rows: Vec<[f32; 4]> = info
        .parts
        .iter()
        .map(|p| {
            [
                p.material.base_color[0],
                p.material.base_color[1],
                p.material.base_color[2],
                0.0,
            ]
        })
        .collect();
    engine.set_part_table(device, &rows);
    engine.set_mesh_maps(
        &maps.position.view,
        &maps.normal.view,
        &maps.extras.view,
        &maps.view.view,
        maps.size,
        &info.bounds,
    );
}

pub fn validate_graph(registry: &Registry, graph: &Graph) -> Result<(), String> {
    if graph.nodes.iter().filter(|n| n.kind == "output").count() != 1 {
        return Err("A material needs exactly one Output node".into());
    }
    validate_links(registry, graph)
}

pub(crate) fn validate_links(registry: &Registry, graph: &Graph) -> Result<(), String> {
    if graph.version != forge_nodes::VERSION {
        return Err("Unsupported graph version".into());
    }
    if graph.size > 13
        || graph.nodes.iter().any(|n| match n.size {
            Some(forge_nodes::Size::Absolute(size)) => size.iter().any(|v| *v > 13),
            Some(forge_nodes::Size::Relative(size)) => size.iter().any(|v| !(-13..=13).contains(v)),
            None => false,
        })
    {
        return Err(
            "Authored graph and node resolutions must stay within 1 to 8192 pixels"
                .into(),
        );
    }
    let mut check = graph.clone();
    check.links.clear();
    let mut inputs = std::collections::BTreeSet::new();
    for l in &graph.links {
        if !inputs.insert((l.to, l.input.clone())) {
            return Err("Multiple links to one input".into());
        }
        check
            .connect(registry, l.from, &l.output, l.to, &l.input)
            .map_err(str::to_owned)?;
    }
    for node in &graph.nodes {
        if registry.get(&node.kind).is_none() {
            return Err(format!("Unknown node {}", node.kind));
        }
    }
    Ok(())
}

fn load_environment(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bytes: &[u8],
    extension: &str,
) -> Result<Environment, String> {
    let path = PathBuf::from(format!("environment.{extension}"));
    let raster = forge_sources::bitmap_bytes(bytes, &path, 4096).map_err(|e| e.to_string())?;
    let rgba = match raster.pixels {
        forge_sources::Pixels::F32(v) => v,
        _ => return Err("Choose an HDR or EXR environment".into()),
    };
    Environment::new(device, queue, raster.width, raster.height, &rgba)
}
