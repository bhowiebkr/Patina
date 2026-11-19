//! Patina adapter: execute one kernel with explicit GPU inputs and no second graph scheduler.

use super::{Engine, NodeGpu, Shape, socket_code, texture_entry};
use crate::def::{MAX_INPUTS, Socket};
use crate::gpu::{self, ATOMS_BINDING, INFO_SIZE, Info, Layers, PARTS_BINDING};
use crate::graph::Node;
use forge_gpu::Target;
use std::sync::{Arc, atomic::Ordering};

/// An immutable published output. A changed run creates new storage for retained readers.
#[derive(Clone)]
pub struct NativeImage {
    pub planes: Arc<Layers>,
    pub revision: u64,
    pub layer: u32,
    pub socket: Socket,
}

impl NativeImage {
    pub fn output(&self, layer: u32, socket: Socket) -> Self {
        Self {
            layer,
            socket,
            ..self.clone()
        }
    }
}

/// Per-node state. Scratch and uniforms never alias another concurrently running node.
pub struct NativePass {
    gpu: NodeGpu,
    pipeline: wgpu::ComputePipeline,
    scratch: [Target; 2],
    stats: wgpu::Buffer,
    last_values: Vec<f32>,
    last_inputs: [u64; MAX_INPUTS],
    last_defaults: u32,
    output: Option<NativeImage>,
}

impl NativePass {
    pub fn new(
        engine: &Engine,
        device: &wgpu::Device,
        kind: &str,
        size: u32,
    ) -> Result<Self, String> {
        Self::with_source(engine, device, kind, size, None)
    }

    pub fn with_source(
        engine: &Engine,
        device: &wgpu::Device,
        kind: &str,
        size: u32,
        source: Option<&str>,
    ) -> Result<Self, String> {
        let def = engine.registry.get(kind).ok_or("Unknown material kernel")?;
        if size == 0 || size > 2048 || !size.is_power_of_two() {
            return Err("Material size must be a power of two from 1 to 2048".into());
        }
        if def.paint || def.host.is_some() || def.atoms != 0 || def.code_param().is_some() {
            return Err("This kernel needs host resources outside the native core adapter".into());
        }
        let pipeline = if let Some(source) = source.filter(|s| !s.lines().eq(def.source.lines())) {
            let header = |s: &str| {
                s.lines()
                    .filter(|line| line.starts_with("//!"))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            if header(source) != header(&def.source) {
                return Err(
                    "Keep the kernel's port and parameter declarations. Change its shader body."
                        .into(),
                );
            }
            let linked = crate::link::link(
                engine.registry.modules_for(kind),
                &def.path,
                source,
                "",
                &crate::generate(def),
            )?;
            let module = forge_gpu::compile(device, &def.path, &linked.wgsl)?;
            forge_gpu::validated(device, &def.path, || {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(&def.path),
                    layout: Some(&engine.pipeline_layout),
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                })
            })?
        } else {
            let compiled = engine.compile(device, kind, None);
            compiled
                .pipeline
                .ok_or_else(|| compiled.error.unwrap_or_default())?
        };
        let shape = Shape {
            kind: kind.into(),
            mesh: true,
            size,
            layers: def.layers(),
            slots: def.slots(),
            multipass: def.multipass(),
            atoms: 0,
        };
        let gpu = engine.node_gpu(device, shape);
        let scratch = gpu::scratch_pair(device, if def.multipass() { size } else { 1 });
        let stats = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("native material stats"),
            size: gpu::STATS_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Self {
            gpu,
            pipeline,
            scratch,
            stats,
            last_values: Vec::new(),
            last_inputs: [0; MAX_INPUTS],
            last_defaults: 0,
            output: None,
        })
    }

    pub fn thumbnail(&self) -> &wgpu::Texture {
        &self.gpu.thumb.texture
    }

    pub fn run(
        &mut self,
        engine: &Engine,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        node: &Node,
        sources: &[Option<&NativeImage>],
    ) -> Result<NativeImage, String> {
        if node.kind != self.gpu.shape.kind {
            return Err("Material kernel kind changed".into());
        }
        let def = engine
            .registry
            .get(&node.kind)
            .ok_or("Missing material definition")?;
        if sources.len() != def.inputs.len() {
            return Err("Material input count does not match the kernel".into());
        }
        let maps = engine.maps.as_ref().ok_or("No mesh maps")?;
        let values: Vec<f32> = def
            .params
            .iter()
            .flat_map(|p| node.value(&engine.registry, &p.id))
            .collect();
        if values.iter().any(|v| !v.is_finite()) {
            return Err("Material parameters must be finite".into());
        }
        let mut keys = [0; MAX_INPUTS];
        let mut table = [[0; 4]; MAX_INPUTS];
        let mut views = [&engine.dummies.input.array; MAX_INPUTS];
        for (i, source) in sources.iter().enumerate() {
            if let Some(source) = source {
                if !def.inputs[i].socket.accepts(source.socket)
                    || source.layer + source.socket.layers() > source.planes.layers.len() as u32
                {
                    return Err(format!(
                        "{}: incompatible input {}",
                        node.kind, def.inputs[i].id
                    ));
                }
                keys[i] = source.revision.wrapping_mul(128)
                    + u64::from(source.layer * 4 + socket_code(source.socket));
                table[i] = [
                    1,
                    source.layer,
                    u32::from(source.planes.size != self.gpu.shape.size),
                    socket_code(source.socket),
                ];
                views[i] = &source.planes.array;
            }
        }
        if self.last_values == values
            && self.last_inputs == keys
            && self.last_defaults == node.defaults_version
        {
            if let Some(output) = &self.output {
                return Ok(output.clone());
            }
        }
        let size = self.gpu.shape.size;
        let output = Arc::new(Layers::new(
            device,
            &node.kind,
            size,
            def.layers(),
            wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        ));
        if !values.is_empty() {
            queue.write_buffer(&self.gpu.values, 0, bytemuck::cast_slice(&values));
        }
        let passes = def.pass_count(size, &values);
        let defaults = u32::from(
            node.defaults_version >= def.input_defaults_version.unwrap_or(def.defaults_version)
                && def.inputs.iter().any(|i| i.legacy_default.is_some()),
        ) * gpu::UPDATED_INPUT_DEFAULTS;
        let mut infos = vec![0; (u64::from(passes) * engine.stride) as usize];
        for pass in 0..passes {
            let info = Info {
                size: [size, size, pass, passes],
                domain: [1, maps.size, def.layers(), defaults],
                inputs: table,
                triplanar: [1.0, 4.0, 0.0, 0.0],
                bounds: maps.bounds,
            };
            let at = (u64::from(pass) * engine.stride) as usize;
            infos[at..at + INFO_SIZE as usize].copy_from_slice(bytemuck::bytes_of(&info));
        }
        queue.write_buffer(&self.gpu.info, 0, &infos);
        let binds = self.binds(engine, device, &output, &views);
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.clear_buffer(&self.stats, 0, None);
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            for i in 0..passes {
                pass.set_bind_group(
                    0,
                    &binds[i as usize % 2],
                    &[(u64::from(i) * engine.stride) as u32],
                );
                pass.dispatch_workgroups(size.div_ceil(8), size.div_ceil(8), 1);
            }
        }
        let thumb = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("native material thumbnail"),
            layout: &engine.thumb_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.gpu.thumb_params.as_entire_binding(),
                },
                texture_entry(1, &output.array),
                texture_entry(2, &maps.view),
            ],
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("native material thumbnail"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.gpu.thumb.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&engine.thumb_pipeline);
            pass.set_bind_group(0, &thumb, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
        let out = NativeImage {
            planes: output,
            revision: engine.native_revision.fetch_add(1, Ordering::Relaxed),
            layer: 0,
            socket: def.outputs.first().ok_or("Kernel has no output")?.socket,
        };
        self.last_values = values;
        self.last_inputs = keys;
        self.last_defaults = node.defaults_version;
        self.output = Some(out.clone());
        Ok(out)
    }

    fn binds(
        &self,
        engine: &Engine,
        device: &wgpu::Device,
        out: &Layers,
        views: &[&wgpu::TextureView; MAX_INPUTS],
    ) -> [wgpu::BindGroup; 2] {
        let maps = engine.maps.as_ref().expect("run checked mesh maps");
        std::array::from_fn(|parity| {
            let mut entries = vec![
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.gpu.info,
                        offset: 0,
                        size: wgpu::BufferSize::new(INFO_SIZE),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.gpu.values.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&engine.dummies.sampler),
                },
                texture_entry(3, &maps.pos),
                texture_entry(4, &maps.nrm),
                texture_entry(5, &maps.extras),
                texture_entry(6, &self.scratch[parity].view),
                texture_entry(7, &self.scratch[1 - parity].view),
                texture_entry(8, &out.array),
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: self.stats.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: ATOMS_BINDING,
                    resource: engine.dummies.atoms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: PARTS_BINDING,
                    resource: engine
                        .parts
                        .as_ref()
                        .unwrap_or(&engine.dummies.parts)
                        .as_entire_binding(),
                },
            ];
            entries.extend(
                views
                    .iter()
                    .enumerate()
                    .map(|(i, view)| texture_entry(10 + i as u32, view)),
            );
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("native material"),
                layout: &engine.layout,
                entries: &entries,
            })
        })
    }
}
