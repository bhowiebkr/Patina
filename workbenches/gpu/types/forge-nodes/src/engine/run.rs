//! Evaluation: records the dirty nodes in graph order, binding each to its inputs as it goes,
//! and keeps node textures near a memory budget. A node's output returns to a pool once every
//! node of the run that reads it has run, while the textures held exceed the budget; the next
//! run that needs a released output recomputes it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{Engine, Maps, NodeGpu, read_mode, socket_code, texture_entry};
use crate::def::{Def, MAX_INPUTS};
use crate::gpu::{ATOMS_BINDING, INFO_SIZE, Info, Layers, OUT_TEXEL, PARTS_BINDING};
use crate::graph::{Graph, NodeId};

/// Nodes recorded per submit.
const CHUNK: usize = 32;

/// Default bytes of node textures held after a run (see `Engine::set_budget`).
pub(super) const BUDGET: u64 = 4 << 30;

fn out_bytes(size: u32, layers: u32) -> u64 {
    u64::from(size).pow(2) * u64::from(layers) * u64::from(OUT_TEXEL)
}

fn held(out: &Layers) -> u64 {
    out_bytes(out.size, out.layers.len() as u32)
}

/// Released node textures by (side, layers), reused before new ones are made.
#[derive(Default)]
pub(super) struct Pool {
    free: BTreeMap<(u32, u32), Vec<Layers>>,
}

impl Pool {
    /// A pooled texture of this shape, else a new one, made after dropping pooled ones until at
    /// most `room` bytes stay with it.
    fn take(&mut self, device: &wgpu::Device, size: u32, layers: u32, room: u64) -> Layers {
        if let Some(out) = self.free.get_mut(&(size, layers)).and_then(Vec::pop) {
            return out;
        }
        self.trim(room.saturating_sub(out_bytes(size, layers)));
        let usage = wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC;
        Layers::new(device, "node output", size, layers, usage)
    }

    fn put(&mut self, out: Layers) {
        let key = (out.size, out.layers.len() as u32);
        self.free.entry(key).or_default().push(out);
    }

    /// Drops pooled textures, largest first, until at most `room` bytes stay.
    fn trim(&mut self, room: u64) {
        let mut bytes: u64 = self.free.values().flatten().map(held).sum();
        while bytes > room {
            let Some(mut largest) = self.free.last_entry() else {
                return;
            };
            if let Some(out) = largest.get_mut().pop() {
                bytes -= held(&out);
            }
            if largest.get().is_empty() {
                largest.remove();
            }
        }
    }

    pub(super) fn clear(&mut self) {
        self.free.clear();
    }
}

impl Engine {
    /// Bytes of node textures an evaluation keeps; past it, outputs that every reader has
    /// consumed are released (default 4 GiB).
    pub fn set_budget(&mut self, bytes: u64) {
        self.budget = bytes;
    }

    /// Nodes whose outputs are never released, such as the one the 2D view shows. A kept node
    /// without an output runs at the next evaluation.
    pub fn keep(&mut self, ids: impl IntoIterator<Item = NodeId>) {
        self.kept = ids.into_iter().collect();
    }

    /// Runs every dirty node, and the released outputs they read, and redraws their
    /// thumbnails. Returns true when anything ran.
    pub fn evaluate(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, graph: &Graph) -> bool {
        self.evaluation_error = None;
        for id in self.hosts.sync(device, queue, &self.registry, graph) {
            self.dirty.extend(graph.downstream(id));
        }
        self.sync(device, graph);
        if self.maps.is_none() {
            return false;
        }
        let missing: Vec<NodeId> = self
            .kept
            .iter()
            .filter(|id| self.nodes.get(id).is_some_and(|g| g.out.is_none()))
            .copied()
            .collect();
        self.dirty.extend(missing);
        if self.dirty.is_empty() {
            return false;
        }
        let order = graph.order();
        let run = self.run_set(graph, &order);
        let mut readers: BTreeMap<NodeId, usize> = BTreeMap::new();
        for l in graph.links.iter().filter(|l| run.contains(&l.to)) {
            *readers.entry(l.from).or_default() += 1;
        }
        let output = graph.output();
        let pinned = |id: NodeId, kept: &BTreeSet<NodeId>| Some(id) == output || kept.contains(&id);
        let mut live: u64 = self
            .nodes
            .values()
            .filter_map(|g| g.out.as_ref())
            .map(held)
            .sum();
        // Outputs nothing in this run reads go first, then each as its last reader runs.
        let mut idle: VecDeque<NodeId> = self
            .nodes
            .iter()
            .filter(|(id, g)| g.out.is_some() && !run.contains(id) && !readers.contains_key(id))
            .map(|(id, _)| *id)
            .filter(|id| !pinned(*id, &self.kept))
            .collect();
        let mut encoder = None;
        let mut recorded = 0;
        let mut ran = false;
        for &id in order.iter().filter(|id| run.contains(id)) {
            if self.runnable(graph, id)
                && let Some(gpu) = self.nodes.get_mut(&id)
            {
                if gpu.out.is_none() {
                    let room = self.budget.saturating_sub(live);
                    let out = self
                        .pool
                        .take(device, gpu.shape.size, gpu.shape.layers, room);
                    live += held(&out);
                    gpu.out = Some(out);
                    if Some(id) == output {
                        self.generation += 1;
                    }
                }
                let encoder = encoder.get_or_insert_with(|| {
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("material"),
                    })
                });
                ran |= self.record(device, queue, encoder, graph, id).is_some();
                recorded += 1;
            }
            if !readers.contains_key(&id) && !pinned(id, &self.kept) {
                idle.push_back(id);
            }
            for l in graph.links.iter().filter(|l| l.to == id) {
                if let Some(n) = readers.get_mut(&l.from) {
                    *n -= 1;
                    if *n == 0 && !pinned(l.from, &self.kept) {
                        idle.push_back(l.from);
                    }
                }
            }
            while live > self.budget
                && let Some(free) = idle.pop_front()
            {
                if let Some(out) = self.nodes.get_mut(&free).and_then(|g| g.out.take()) {
                    live -= held(&out);
                    self.pool.put(out);
                }
            }
            if recorded == CHUNK
                && let Some(done) = encoder.take()
            {
                queue.submit([done.finish()]);
                // Dropped outputs remain alive in submitted commands. Bound that backlog
                // natively before recording more allocations; no texture is read back.
                #[cfg(not(target_arch = "wasm32"))]
                if let Err(error) = device.poll(wgpu::PollType::wait_indefinitely()) {
                    self.evaluation_error = Some(format!("material GPU completion: {error}"));
                    return ran;
                }
                recorded = 0;
            }
        }
        if let Some(done) = encoder {
            queue.submit([done.finish()]);
        }
        self.dirty.clear();
        self.pool.trim(self.budget.saturating_sub(live));
        ran
    }

    /// The dirty nodes plus every source they read whose output was released, recursively.
    fn run_set(&self, graph: &Graph, order: &[NodeId]) -> BTreeSet<NodeId> {
        let mut run: BTreeSet<NodeId> = self
            .dirty
            .iter()
            .filter(|id| self.nodes.contains_key(id))
            .copied()
            .collect();
        for id in order.iter().rev() {
            if !run.contains(id) {
                continue;
            }
            for l in graph.links.iter().filter(|l| l.to == *id) {
                if self.nodes.get(&l.from).is_some_and(|g| g.out.is_none()) {
                    run.insert(l.from);
                }
            }
        }
        run
    }

    /// The node has a definition and a compiled kernel; one without stays unconnected for its
    /// readers.
    fn runnable(&self, graph: &Graph, id: NodeId) -> bool {
        graph.node(id).is_some_and(|node| {
            self.registry.get(&node.kind).is_some()
                && self
                    .kernels
                    .get(&self.kernel_key(node).0)
                    .is_some_and(|k| k.pipeline.is_some())
        })
    }

    /// Records one node's passes and its thumbnail into `encoder`.
    fn record(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        graph: &Graph,
        id: NodeId,
    ) -> Option<()> {
        let maps = self.maps.as_ref()?;
        let node = graph.node(id)?;
        let def = self.registry.get(&node.kind)?;
        let (key, _) = self.kernel_key(node);
        let pipeline = self.kernels.get(&key)?.pipeline.as_ref()?;
        let gpu = self.nodes.get(&id)?;
        let out = gpu.out.as_ref()?;
        let (inputs, views) = self.inputs(graph, id, gpu, def);
        let binds = self.binds(device, id, gpu, out, &views, maps);
        let values: Vec<f32> = def
            .params
            .iter()
            .flat_map(|p| node.value(&self.registry, &p.id))
            .collect();
        if !values.is_empty() {
            queue.write_buffer(&gpu.values, 0, bytemuck::cast_slice(&values));
        }
        let size = gpu.shape.size;
        let passes = def.pass_count(size, &values);
        let hosted = u32::from(self.hosts.view(id).is_some());
        // Leave old pinned shaders' domain.w unchanged unless the definition opts in.
        let defaults = u32::from(
            node.defaults_version >= def.input_defaults_version.unwrap_or(def.defaults_version)
                && def.inputs.iter().any(|i| i.legacy_default.is_some()),
        ) * crate::gpu::UPDATED_INPUT_DEFAULTS;
        let mut infos = Vec::with_capacity((u64::from(passes) * self.stride) as usize);
        for pass in 0..passes {
            let info = Info {
                size: [size, size, pass, passes],
                domain: [
                    u32::from(gpu.shape.mesh),
                    maps.size,
                    gpu.shape.layers,
                    hosted | defaults,
                ],
                inputs,
                triplanar: [graph.triplanar[0], graph.triplanar[1], 0.0, 0.0],
                bounds: maps.bounds,
            };
            infos.extend_from_slice(bytemuck::bytes_of(&info));
            infos.resize((u64::from(pass + 1) * self.stride) as usize, 0);
        }
        queue.write_buffer(&gpu.info, 0, &infos);
        if def.stats {
            encoder.clear_buffer(&self.dummies.stats, 0, None);
        }
        if let Some(atoms) = self.atoms.get(&(size, def.atoms)) {
            encoder.clear_buffer(atoms, 0, None);
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(&def.path),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            let groups = size.div_ceil(8);
            for i in 0..passes {
                let bind = &binds[i as usize % binds.len()];
                pass.set_bind_group(0, bind, &[(u64::from(i) * self.stride) as u32]);
                pass.dispatch_workgroups(groups, groups, 1);
            }
        }
        let view_map = if gpu.shape.mesh {
            &maps.view
        } else {
            &self.dummies.map.view
        };
        let thumb_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("thumbnail"),
            layout: &self.thumb_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu.thumb_params.as_entire_binding(),
                },
                texture_entry(1, &out.array),
                texture_entry(2, view_map),
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("thumbnail"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &gpu.thumb.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.thumb_pipeline);
        pass.set_bind_group(0, &thumb_bind, &[]);
        pass.draw(0..3, 0..1);
        Some(())
    }

    /// The node's input table and the views its input slots read: each linked source with an
    /// output, else the dummy.
    fn inputs<'a>(
        &'a self,
        graph: &Graph,
        id: NodeId,
        gpu: &NodeGpu,
        def: &Def,
    ) -> ([[u32; 4]; MAX_INPUTS], [&'a wgpu::TextureView; MAX_INPUTS]) {
        let mut inputs = [[0u32; 4]; MAX_INPUTS];
        let mut views = [&self.dummies.input.array; MAX_INPUTS];
        for (slot, input) in def.inputs.iter().enumerate().take(MAX_INPUTS) {
            let source = graph.source(id, &input.id).and_then(|(from, output)| {
                let src = self.nodes.get(&from)?;
                let view = &src.out.as_ref()?.array;
                let out = self.registry.get(&src.shape.kind)?.output(output)?;
                input.socket.accepts(out.socket).then_some((src, out, view))
            });
            if let Some((src, out, view)) = source {
                inputs[slot] = [
                    1,
                    out.layer,
                    read_mode(&src.shape, &gpu.shape),
                    socket_code(out.socket),
                ];
                views[slot] = view;
            }
        }
        (inputs, views)
    }

    /// One bind group per scratch parity (multipass nodes swap the pair each pass).
    fn binds(
        &self,
        device: &wgpu::Device,
        id: NodeId,
        gpu: &NodeGpu,
        out: &Layers,
        views: &[&wgpu::TextureView; MAX_INPUTS],
        maps: &Maps,
    ) -> Vec<wgpu::BindGroup> {
        let parts = self.parts.as_ref().unwrap_or(&self.dummies.parts);
        let scratch = gpu
            .shape
            .multipass
            .then(|| self.scratch.get(&gpu.shape.size))
            .flatten();
        let parities: Vec<(&wgpu::TextureView, &wgpu::TextureView)> =
            match (scratch, self.masks.get(&id)) {
                (Some([a, b]), _) => vec![(&a.view, &b.view), (&b.view, &a.view)],
                (None, Some(mask)) => vec![(&mask.view, &self.dummies.scratch_out.view)],
                (None, None) => {
                    let read = self.hosts.view(id).unwrap_or(&self.dummies.map.view);
                    vec![(read, &self.dummies.scratch_out.view)]
                }
            };
        parities
            .into_iter()
            .map(|(read, write)| {
                let mut entries = vec![
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &gpu.info,
                            offset: 0,
                            size: wgpu::BufferSize::new(INFO_SIZE),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: gpu.values.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.dummies.sampler),
                    },
                    texture_entry(3, &maps.pos),
                    texture_entry(4, &maps.nrm),
                    texture_entry(5, &maps.extras),
                    texture_entry(6, read),
                    texture_entry(7, write),
                    texture_entry(8, &out.array),
                    wgpu::BindGroupEntry {
                        binding: 9,
                        resource: self.dummies.stats.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: ATOMS_BINDING,
                        resource: self
                            .atoms
                            .get(&(gpu.shape.size, gpu.shape.atoms))
                            .unwrap_or(&self.dummies.atoms)
                            .as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: PARTS_BINDING,
                        resource: parts.as_entire_binding(),
                    },
                ];
                entries.extend(
                    views
                        .iter()
                        .enumerate()
                        .map(|(i, view)| texture_entry(10 + i as u32, view)),
                );
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&gpu.shape.kind),
                    layout: &self.layout,
                    entries: &entries,
                })
            })
            .collect()
    }
}
