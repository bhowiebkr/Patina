//! Evaluates a graph on the GPU: one compute kernel per node definition (its WESL file plus
//! generated accessors, linked with the prelude and `lib/` items it uses), a layered output
//! texture per node taken from a pool while held (see `run`), and a thumbnail of each node's
//! own output (only Output shows the finished material).

use std::collections::{BTreeMap, BTreeSet};

use forge_gpu::Target;

use crate::def::{Domain, MAX_PASSES, Socket};
use crate::gpu::{self, Dummies, INFO_SIZE, Layers, THUMB, THUMB_FORMAT};
use crate::graph::{Graph, Node, NodeId, Size};
use crate::host::Hosts;
use crate::paint::{self, Brush, Stamper};
use crate::registry::Registry;

/// Side of texture-domain node images.
pub const TEXTURE_SIZE: u32 = 512;

mod kernels;
mod run;
mod native;
pub use native::{NativeImage, NativePass};

use kernels::Kernel;

struct Maps {
    pos: wgpu::TextureView,
    nrm: wgpu::TextureView,
    extras: wgpu::TextureView,
    view: wgpu::TextureView,
    size: u32,
    /// The mesh's bounding box centre (xyz) and largest side (w).
    bounds: [f32; 4],
}

/// What a node's GPU resources were made for; any change rebuilds them.
#[derive(Clone, PartialEq, Eq)]
struct Shape {
    kind: String,
    mesh: bool,
    size: u32,
    layers: u32,
    slots: usize,
    multipass: bool,
    atoms: u32,
}

struct NodeGpu {
    shape: Shape,
    /// Taken from the pool when the node runs; released after its last reader when held
    /// outputs pass the budget.
    out: Option<Layers>,
    values: wgpu::Buffer,
    /// One `Info` per pass, `stride` apart.
    info: wgpu::Buffer,
    thumb: Target,
    thumb_params: wgpu::Buffer,
}

/// One graph's node resources, parked while another graph is evaluated: each open material
/// keeps its own, so switching between them re-evaluates nothing.
#[derive(Default)]
pub struct GraphState {
    nodes: BTreeMap<NodeId, NodeGpu>,
    masks: BTreeMap<NodeId, Target>,
    dirty: BTreeSet<NodeId>,
    hosts: Hosts,
    /// The engine epoch the nodes were made in; older ones read mesh maps that are gone.
    epoch: u64,
}

impl GraphState {
    /// The parked Output node's layers, as `Engine::output`.
    pub fn output(&self, graph: &Graph) -> Option<[&wgpu::TextureView; 3]> {
        let layers = &self.nodes.get(&graph.output()?)?.out.as_ref()?.layers;
        Some([layers.first()?, layers.get(1)?, layers.get(2)?])
    }

    /// The parked painted masks (saving a material that is not the one evaluated).
    pub fn masks(&self) -> &BTreeMap<NodeId, Target> {
        &self.masks
    }

    /// The parked painted masks, handed over and forgotten (switching assets).
    pub fn take_masks(&mut self) -> BTreeMap<NodeId, Target> {
        std::mem::take(&mut self.masks)
    }
}

pub struct Engine {
    // Patina shares this engine across loaded clusters, so revisions stay unique after a reload.
    native_revision: std::sync::atomic::AtomicU64,
    registry: Registry,
    kernels: BTreeMap<String, Kernel>,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    thumb_layout: wgpu::BindGroupLayout,
    thumb_pipeline: wgpu::RenderPipeline,
    dummies: Dummies,
    stride: u64,
    maps: Option<Maps>,
    /// The target's part table (see `set_part_table`).
    parts: Option<wgpu::Buffer>,
    scratch: BTreeMap<u32, [Target; 2]>,
    /// `atoms` buffers by (size, u32s per pixel), shared like scratch.
    atoms: BTreeMap<(u32, u32), wgpu::Buffer>,
    nodes: BTreeMap<NodeId, NodeGpu>,
    /// Released node outputs, reused by the next nodes that run.
    pool: run::Pool,
    /// Bytes of node outputs held past which read ones are released (see `set_budget`).
    budget: u64,
    /// Nodes whose outputs are never released (see `keep`).
    kept: BTreeSet<NodeId>,
    /// Paint Mask nodes' painted masks, kept across graph resets (they are the user's work).
    masks: BTreeMap<NodeId, Target>,
    stamper: Stamper,
    dirty: BTreeSet<NodeId>,
    generation: u64,
    /// A failed GPU completion wait, reported alongside shader/load diagnostics.
    evaluation_error: Option<String>,
    hosts: Hosts,
    /// Bumps when the mesh maps or part table change, which parked graph states then miss.
    epoch: u64,
    root: std::path::PathBuf,
    resources: std::sync::Arc<BTreeMap<String, Vec<u8>>>,
}

fn socket_code(socket: Socket) -> u32 {
    match socket {
        Socket::Gray => 0,
        Socket::Color => 1,
        Socket::Material => 2,
    }
}

/// A texture-domain node's side. Images are square: a non-square size runs at its longer
/// side, which inputs read by UV like any other resample.
fn texture_side(graph: &Graph, node: &Node) -> u32 {
    let [w, h] = node.size.map_or([graph.size; 2], |s| s.log2(graph.size));
    1 << w.max(h)
}

/// A UV-space node's side: the mesh map's, moved by a relative size (an imported mask
/// generator's smaller records keep their ratio to the output).
fn uv_side(node: &Node, map_size: u32) -> u32 {
    match node.size {
        Some(Size::Relative(d)) => {
            let log2 = map_size.max(1).ilog2() as i32 + d[0].max(d[1]);
            1 << log2.clamp(0, 13)
        }
        _ => map_size,
    }
}

/// How `to` reads an input made by `from`: load, resample, or triplanar onto the mesh.
fn read_mode(from: &Shape, to: &Shape) -> u32 {
    if to.mesh && !from.mesh {
        2
    } else if from.size != to.size {
        1
    } else {
        0
    }
}

fn texture_entry(binding: u32, view: &wgpu::TextureView) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::TextureView(view),
    }
}

impl Engine {
    pub fn new(device: &wgpu::Device) -> Result<Self, String> {
        Self::with_registry(device, Registry::new())
    }

    pub fn with_registry(device: &wgpu::Device, registry: Registry) -> Result<Self, String> {
        let layout = gpu::node_layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("node"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let thumb_layout = gpu::thumb_layout(device);
        let thumb_pipeline = gpu::thumb_pipeline(device, &thumb_layout, registry.thumb())?;
        let stamper = Stamper::new(device, registry.stamp())?;
        let align = u64::from(device.limits().min_uniform_buffer_offset_alignment).max(1);
        Ok(Self {
            native_revision: std::sync::atomic::AtomicU64::new(1),
            registry,
            kernels: BTreeMap::new(),
            layout,
            pipeline_layout,
            thumb_layout,
            thumb_pipeline,
            dummies: Dummies::new(device),
            stride: INFO_SIZE.div_ceil(align) * align,
            maps: None,
            parts: None,
            scratch: BTreeMap::new(),
            atoms: BTreeMap::new(),
            nodes: BTreeMap::new(),
            pool: run::Pool::default(),
            budget: run::BUDGET,
            kept: BTreeSet::new(),
            masks: BTreeMap::new(),
            stamper,
            dirty: BTreeSet::new(),
            generation: 0,
            evaluation_error: None,
            hosts: Hosts::default(),
            epoch: 0,
            root: std::path::PathBuf::new(),
            resources: Default::default(),
        })
    }

    pub fn set_resources(&mut self, resources: std::sync::Arc<BTreeMap<String, Vec<u8>>>) {
        self.resources = resources.clone();
        self.hosts.set_resources(resources);
    }

    /// Catalog discovery is UI-only: retain pipelines, evaluated textures and dirty state.
    pub fn enable_catalog(&mut self) -> Result<(), String> {
        self.registry.enable_catalog()
    }

    pub fn into_registry(self) -> Registry {
        self.registry
    }
    pub fn accept_registry(&mut self, registry: Registry, graph: &Graph) {
        let changes_existing = self
            .registry
            .files()
            .iter()
            .any(|(p, s)| registry.files().get(p) != Some(s));
        self.registry = registry;
        if changes_existing {
            self.kernels.clear();
            self.graph_changed(graph);
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Why `id` shows nothing: its file or text failed to load, or its kernel to compile.
    pub fn node_error(&self, graph: &Graph, id: NodeId) -> Option<&str> {
        let node = graph.node(id)?;
        self.hosts.error(id).or_else(|| {
            let (key, _) = self.kernel_key(node);
            self.kernels.get(&key).and_then(|k| k.error.as_deref())
        })
    }

    /// Relative file paths in nodes (bitmaps, fonts, SVGs) resolve against `root`.
    pub fn set_source_root(&mut self, root: &std::path::Path) {
        root.clone_into(&mut self.root);
        self.hosts.set_root(root);
    }

    /// Swaps the current graph's resources with `state`'s, so another graph can be evaluated.
    /// Resources made before the mesh maps last changed are dropped and rebuilt.
    pub fn swap_state(&mut self, state: &mut GraphState) {
        std::mem::swap(&mut self.nodes, &mut state.nodes);
        std::mem::swap(&mut self.masks, &mut state.masks);
        std::mem::swap(&mut self.dirty, &mut state.dirty);
        std::mem::swap(&mut self.hosts, &mut state.hosts);
        if state.epoch != self.epoch {
            self.nodes.clear();
            self.dirty.clear();
        }
        state.epoch = self.epoch;
        self.hosts.set_root(&self.root);
        self.hosts.set_resources(self.resources.clone());
        self.generation += 1;
    }

    /// Bumps whenever node textures are created or dropped: re-register thumbnails and rebind
    /// the output layers when it changes.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The baked mesh maps of the asset the material is evaluated on; mesh-domain nodes take
    /// their size. `view` holds (u, v, light, coverage) per pixel of the preview camera; mesh
    /// thumbnails show the node through it. `bounds` is the mesh's bounding box, which
    /// `mesh_unit_position` normalises positions by.
    pub fn set_mesh_maps(
        &mut self,
        pos: &wgpu::TextureView,
        nrm: &wgpu::TextureView,
        extras: &wgpu::TextureView,
        view: &wgpu::TextureView,
        size: u32,
        bounds: &forge_core::Bounds,
    ) {
        let c = bounds.center();
        self.maps = Some(Maps {
            pos: pos.clone(),
            nrm: nrm.clone(),
            extras: extras.clone(),
            view: view.clone(),
            size,
            bounds: [c.x, c.y, c.z, bounds.largest_side().max(1e-6)],
        });
        self.reset();
    }

    /// Sets the target's part table: per part (in part order), its CAD base colour (linear rgb)
    /// and its colour group index (w), read by `mesh_cad_color` and `mesh_cad_group`.
    pub fn set_part_table(&mut self, device: &wgpu::Device, rows: &[[f32; 4]]) {
        self.parts = Some(gpu::part_table(device, rows));
        self.reset();
    }

    /// Drops every node's textures (use when switching graphs); painted masks stay until
    /// `take_masks` or a sync without their node. Parked graph states drop theirs when next
    /// swapped in.
    pub fn reset(&mut self) {
        self.epoch += 1;
        self.nodes.clear();
        self.pool.clear();
        self.dirty.clear();
        self.generation += 1;
    }

    /// A parameter of `id` changed: it and everything downstream re-evaluate.
    pub fn mark_dirty(&mut self, graph: &Graph, id: NodeId) {
        self.dirty.extend(graph.downstream(id));
    }

    /// Links or nodes were added or removed.
    pub fn graph_changed(&mut self, graph: &Graph) {
        self.dirty.extend(graph.nodes.iter().map(|n| n.id));
    }

    /// Each node's resolved shape, in evaluation order: mesh when the node asks for UVs, else
    /// domain from its definition or, when it inherits, mesh if any linked input is.
    fn shapes(&self, graph: &Graph, map_size: u32) -> BTreeMap<NodeId, Shape> {
        let mut shapes: BTreeMap<NodeId, Shape> = BTreeMap::new();
        for id in graph.order() {
            let Some(node) = graph.node(id) else {
                continue;
            };
            let Some(def) = self.registry.get(&node.kind) else {
                continue;
            };
            let mesh = node.uv
                || match def.domain {
                    Domain::Mesh => true,
                    Domain::Texture => false,
                    Domain::Inherit => {
                        def.is_output()
                            || graph
                                .links
                                .iter()
                                .any(|l| l.to == id && shapes.get(&l.from).is_some_and(|s| s.mesh))
                    }
                };
            let shape = Shape {
                kind: node.kind.clone(),
                mesh,
                size: if node.uv {
                    uv_side(node, map_size)
                } else if mesh {
                    map_size
                } else {
                    texture_side(graph, node)
                },
                layers: def.layers(),
                slots: def.slots(),
                multipass: def.multipass(),
                atoms: def.atoms,
            };
            shapes.insert(id, shape);
        }
        shapes
    }

    fn node_gpu(&self, device: &wgpu::Device, shape: Shape) -> NodeGpu {
        let values = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("node values"),
            size: (shape.slots.max(4) * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let info = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("node info"),
            size: u64::from(MAX_PASSES) * self.stride,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let thumb = Target::new(
            device,
            "thumbnail",
            [THUMB; 2],
            THUMB_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            1,
        );
        let layer = self
            .registry
            .get(&shape.kind)
            .and_then(|d| d.outputs.first())
            .map_or(0, |o| o.layer);
        let thumb_params = forge_gpu::uniform_buffer(
            device,
            "thumbnail",
            bytemuck::bytes_of(&[u32::from(shape.mesh), layer, THUMB, 0]),
        );
        NodeGpu {
            shape,
            out: None,
            values,
            info,
            thumb,
            thumb_params,
        }
    }

    /// Creates, rebuilds and drops node resources to match `graph` and compiles the kernels it
    /// uses.
    fn sync(&mut self, device: &wgpu::Device, graph: &Graph) {
        let Some(map_size) = self.maps.as_ref().map(|m| m.size) else {
            return;
        };
        let mut shapes = self.shapes(graph, map_size);
        for shape in shapes.values_mut() {
            shape.size = shape.size.min(device.limits().max_texture_dimension_2d);
        }
        let before = self.nodes.len();
        self.nodes
            .retain(|id, gpu| shapes.get(id) == Some(&gpu.shape));
        let mut changed = self.nodes.len() != before;
        let painted: BTreeSet<NodeId> = graph
            .nodes
            .iter()
            .filter(|n| self.registry.get(&n.kind).is_some_and(|d| d.paint))
            .map(|n| n.id)
            .collect();
        let masks = self.masks.len();
        self.masks
            .retain(|id, mask| painted.contains(id) && mask.size[0] == map_size);
        changed |= self.masks.len() != masks;
        for id in &painted {
            if !self.masks.contains_key(id) {
                self.masks.insert(*id, paint::new_mask(device, map_size));
                self.dirty.extend(graph.downstream(*id));
                changed = true;
            }
        }
        let mut used = BTreeSet::new();
        for node in graph.nodes.iter().filter(|n| shapes.contains_key(&n.id)) {
            let (key, code) = self.kernel_key(node);
            if !self.kernels.contains_key(&key) {
                let kernel = self.compile(device, &node.kind, code.as_deref());
                self.kernels.insert(key.clone(), kernel);
            }
            used.insert(key);
        }
        self.kernels
            .retain(|key, _| !key.contains('#') || used.contains(key));
        for (id, shape) in &shapes {
            if shape.multipass && !self.scratch.contains_key(&shape.size) {
                self.scratch
                    .insert(shape.size, gpu::scratch_pair(device, shape.size));
            }
            let key = (shape.size, shape.atoms);
            if shape.atoms > 0 && !self.atoms.contains_key(&key) {
                let bytes = u64::from(shape.size).pow(2) * u64::from(shape.atoms) * 4;
                self.atoms.insert(key, gpu::atoms_buffer(device, bytes));
            }
            if !self.nodes.contains_key(id) {
                let gpu = self.node_gpu(device, shape.clone());
                self.nodes.insert(*id, gpu);
                self.dirty.extend(graph.downstream(*id));
                changed = true;
            }
        }
        if changed {
            self.generation += 1;
        }
    }

    /// The Output node's layers: base colour (a: opacity); roughness, metallic, height, AO;
    /// normal. Colours are sRGB-encoded.
    pub fn output(&self, graph: &Graph) -> Option<[&wgpu::TextureView; 3]> {
        let layers = &self.nodes.get(&graph.output()?)?.out.as_ref()?.layers;
        Some([layers.first()?, layers.get(1)?, layers.get(2)?])
    }

    /// The Output node's texture array and side, for export.
    pub fn output_texture(&self, graph: &Graph) -> Option<(&wgpu::Texture, u32)> {
        self.node_texture(graph.output()?)
    }

    /// A node's output texture array and side.
    pub fn node_texture(&self, id: NodeId) -> Option<(&wgpu::Texture, u32)> {
        let out = self.nodes.get(&id)?.out.as_ref()?;
        Some((&out.texture, out.size))
    }

    pub fn thumbnail(&self, id: NodeId) -> Option<&wgpu::TextureView> {
        self.nodes.get(&id).map(|n| &n.thumb.view)
    }

    pub fn thumbnails(&self) -> impl Iterator<Item = (NodeId, &wgpu::TextureView)> {
        self.nodes.iter().map(|(id, n)| (*id, &n.thumb.view))
    }

    /// Reads `id`'s thumbnail back: `THUMB` squared RGBA8 pixels, sRGB, row 0 first.
    pub fn read_thumbnail(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: NodeId,
    ) -> Option<Result<Vec<u8>, String>> {
        let thumb = &self.nodes.get(&id)?.thumb;
        Some(forge_gpu::read_texture(
            device,
            queue,
            &thumb.texture,
            thumb.size,
            1,
            4,
        ))
    }

    /// The Paint Mask nodes of `graph`, in graph order.
    pub fn paint_nodes(&self, graph: &Graph) -> Vec<NodeId> {
        graph
            .nodes
            .iter()
            .filter(|n| self.registry.get(&n.kind).is_some_and(|d| d.paint))
            .map(|n| n.id)
            .collect()
    }

    /// Stamps one brush dab into `node`'s mask at the picked point (`hit`: the picker's
    /// position and normal texels) and re-evaluates what reads it. One submit per dab.
    pub fn stamp(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        graph: &Graph,
        node: NodeId,
        brush: &Brush,
        hit: [&wgpu::TextureView; 2],
    ) {
        self.sync(device, graph);
        let (Some(maps), Some(mask)) = (&self.maps, self.masks.get(&node)) else {
            return;
        };
        self.stamper.stamp(
            device,
            queue,
            brush,
            hit,
            [&maps.pos, &maps.nrm, &maps.extras],
            mask,
        );
        self.mark_dirty(graph, node);
    }

    /// Side of the painted masks (the mesh maps' size).
    pub fn mask_size(&self) -> Option<u32> {
        self.maps.as_ref().map(|m| m.size)
    }

    /// Replaces `node`'s mask with `values` (row 0 first, `mask_size` squared); false when the
    /// node has no mask yet or the size differs.
    pub fn set_mask(
        &mut self,
        queue: &wgpu::Queue,
        graph: &Graph,
        node: NodeId,
        values: &[f32],
    ) -> bool {
        let Some(mask) = self.masks.get(&node) else {
            return false;
        };
        let size = mask.size[0];
        if values.len() != (size * size) as usize {
            return false;
        }
        forge_gpu::write_texture(
            queue,
            &mask.texture,
            mask.size,
            paint::MASK_TEXEL,
            bytemuck::cast_slice(values),
        );
        self.mark_dirty(graph, node);
        true
    }

    /// Reads `node`'s mask back (row 0 first).
    pub fn read_mask(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        node: NodeId,
    ) -> Option<Result<Vec<f32>, String>> {
        let mask = self.masks.get(&node)?;
        Some(
            forge_gpu::read_texture(
                device,
                queue,
                &mask.texture,
                mask.size,
                1,
                paint::MASK_TEXEL,
            )
            .map(|bytes| {
                bytes
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect()
            }),
        )
    }

    /// Hands the painted masks over (switching assets) and forgets them.
    pub fn take_masks(&mut self) -> BTreeMap<NodeId, Target> {
        std::mem::take(&mut self.masks)
    }

    /// Restores masks taken earlier for the same asset (sizes must match the mesh maps).
    pub fn put_masks(&mut self, graph: &Graph, masks: BTreeMap<NodeId, Target>) {
        for id in masks.keys() {
            self.dirty.extend(graph.downstream(*id));
        }
        self.masks = masks;
    }

    /// Ensures the graph's resources exist (so new Paint Mask nodes get their masks).
    pub fn prepare(&mut self, device: &wgpu::Device, graph: &Graph) {
        self.sync(device, graph);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kernel compiles and the preset evaluates without a validation error (skipped on a
    /// machine without a GPU adapter).
    #[test]
    #[allow(clippy::unwrap_used)]
    fn kernels_compile_and_preset_evaluates() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&gpu::device_descriptor(&adapter))).unwrap();
        let mut engine = Engine::new(&device).unwrap();
        assert_eq!(engine.compile_all(&device), Vec::<String>::new());
        let usage = wgpu::TextureUsages::TEXTURE_BINDING;
        let map = |label, format| Target::new(&device, label, [64; 2], format, usage, 1);
        let pos = map("pos", wgpu::TextureFormat::Rgba32Float);
        let nrm = map("nrm", wgpu::TextureFormat::Rgba16Float);
        let extras = map("extras", wgpu::TextureFormat::Rgba16Float);
        let bounds = forge_core::Bounds {
            min: [-1.0; 3],
            max: [1.0; 3],
        };
        engine.set_mesh_maps(&pos.view, &nrm.view, &extras.view, &pos.view, 64, &bounds);
        let graph = crate::sample_material();
        let ran = forge_gpu::validated(&device, "evaluate", || {
            engine.evaluate(&device, &queue, &graph)
        });
        assert_eq!(ran, Ok(true));
        assert!(engine.output(&graph).is_some());
        assert!(!engine.evaluate(&device, &queue, &graph), "nothing dirty");
    }

    /// Surface's Near fact turns the baked near-other-part map (extras alpha) around.
    #[test]
    #[allow(clippy::unwrap_used)]
    fn surface_near_is_one_where_the_baked_map_reads_touching() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&gpu::device_descriptor(&adapter))).unwrap();
        let usage = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST;
        let map = |label, format| Target::new(&device, label, [64; 2], format, usage, 1);
        let pos = map("pos", wgpu::TextureFormat::Rgba32Float);
        let nrm = map("nrm", wgpu::TextureFormat::Rgba16Float);
        let extras = map("extras", wgpu::TextureFormat::Rgba16Float);
        let bounds = forge_core::Bounds {
            min: [-1.0; 3],
            max: [1.0; 3],
        };
        let registry = Registry::embedded();
        let mut graph = crate::blank_material(&registry, "near");
        let near = graph.add("surface", [0.0; 2]);
        graph
            .node_mut(near)
            .unwrap()
            .params
            .insert("fact".to_owned(), vec![7.0]);
        let material = graph
            .nodes
            .iter()
            .find(|node| node.kind == "material")
            .unwrap()
            .id;
        graph
            .connect(&registry, near, "output", material, "roughness")
            .unwrap();
        // Half-float alpha 1, 0.25 and 0.
        for (alpha, mask) in [(0x3c00u16, 0.0f32), (0x3400, 0.75), (0, 1.0)] {
            let texels = vec![[0, 0, 0, alpha]; 64 * 64];
            let bytes: Vec<u8> = texels
                .iter()
                .flatten()
                .flat_map(|c: &u16| c.to_le_bytes())
                .collect();
            forge_gpu::write_texture(&queue, &extras.texture, [64; 2], 8, &bytes);
            let mut engine = Engine::new(&device).unwrap();
            engine.set_mesh_maps(&pos.view, &nrm.view, &extras.view, &pos.view, 64, &bounds);
            let ran = forge_gpu::validated(&device, "evaluate", || {
                engine.evaluate(&device, &queue, &graph)
            });
            assert_eq!(ran, Ok(true));
            let (texture, size) = engine.node_texture(near).unwrap();
            let out = forge_gpu::read_texture(&device, &queue, texture, [size; 2], 1, 16).unwrap();
            let value = f32::from_le_bytes([out[0], out[1], out[2], out[3]]);
            assert!((value - mask).abs() < 1e-3, "alpha {alpha:#x}: {value}");
        }
    }
}
