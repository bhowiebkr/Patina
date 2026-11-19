//! Finds the surface under the pointer on the GPU (`shaders/pick.wgsl`): the asset is drawn
//! into a 1x1 target centred on the pointer, leaving the hit's rest-pose position, eye
//! distance, normal and part in two texels the paint brush reads directly, with no readback.

use forge_gpu::Target;
use glam::{Mat4, Vec4};

use crate::OrbitCamera;
use crate::scene::{self, GpuAsset, SceneRenderer};

const HIT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct Picker {
    pipeline: wgpu::RenderPipeline,
    frame: wgpu::Buffer,
    bind: wgpu::BindGroup,
    /// Rest position (xyz) and eye distance (w, -1 when nothing is under the pointer).
    pub position: Target,
    /// Rest normal (xyz) and part index (w).
    pub normal: Target,
    depth: Target,
}

impl Picker {
    pub fn new(device: &wgpu::Device, scene: &SceneRenderer) -> Result<Self, String> {
        let module = forge_gpu::compile(device, "pick.wgsl", include_str!("../shaders/pick.wgsl"))?;
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pick"),
            bind_group_layouts: &[Some(&scene.frame_layout), Some(&scene.asset_layout)],
            immediate_size: 0,
        });
        let target = Some(wgpu::ColorTargetState {
            format: HIT,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        });
        let pipeline = forge_gpu::validated(device, "pick.wgsl", || {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("pick"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(scene::vertex_layout())],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[target.clone(), target],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        })?;
        let frame = forge_gpu::uniform_buffer(device, "pick frame", &[0; 80]);
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pick frame"),
            layout: &scene.frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame.as_entire_binding(),
            }],
        });
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC;
        Ok(Self {
            pipeline,
            frame,
            bind,
            position: Target::new(device, "pick position", [1; 2], HIT, usage, 1),
            normal: Target::new(device, "pick normal", [1; 2], HIT, usage, 1),
            depth: Target::new(
                device,
                "pick depth",
                [1; 2],
                DEPTH,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
                1,
            ),
        })
    }

    /// One 16-byte readback at gesture start distinguishes the actual mesh from background.
    pub fn hit_test(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &OrbitCamera,
        aspect: f32,
        at: [f32; 2],
        asset: &GpuAsset,
    ) -> Result<bool, String> {
        self.pick(device, queue, camera, aspect, at, asset);
        let bytes = forge_gpu::read_texture(device, queue, &self.position.texture, [1, 1], 1, 16)?;
        let distance = f32::from_le_bytes(
            bytes[12..16]
                .try_into()
                .map_err(|_| "Invalid pick readback")?,
        );
        Ok(distance >= 0.0)
    }

    /// Picks the surface at `at` (0..1 across the view, y down) of a view with `aspect`.
    pub fn pick(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &OrbitCamera,
        aspect: f32,
        at: [f32; 2],
        asset: &GpuAsset,
    ) {
        let (cx, cy) = (at[0] * 2.0 - 1.0, 1.0 - at[1] * 2.0);
        let shift = Mat4::from_cols(Vec4::X, Vec4::Y, Vec4::Z, Vec4::new(-cx, -cy, 0.0, 1.0));
        let mut frame = [0f32; 20];
        frame[..16].copy_from_slice(&(shift * camera.view_proj(aspect)).to_cols_array());
        frame[16..19].copy_from_slice(&camera.eye().to_array());
        queue.write_buffer(&self.frame, 0, bytemuck::cast_slice(&frame));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("pick"),
        });
        {
            let attachment = |view, clear| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })
            };
            let miss = wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: -1.0,
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pick"),
                color_attachments: &[
                    attachment(&self.position.view, miss),
                    attachment(&self.normal.view, wgpu::Color::TRANSPARENT),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let (vertices, indices, count) = asset.buffers();
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_bind_group(1, asset.bind_group(), &[]);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..count, 0, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}
