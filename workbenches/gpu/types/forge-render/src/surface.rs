//! The surface under each texel of the UV atlas, for the occlusion and curvature bakers: the BVH
//! triangle there and the barycentrics on it (`shaders/surface.wgsl`), so a baker reads the exact
//! point and the interpolated vertex normal from the triangle (`surface_at` in trace.wgsl) rather
//! than from interpolated maps. `draw` rasterises at one of four sample points a quarter texel off
//! the centre; a baker that averages all four supersamples 2x2
//! (casting 4 times the initial amount of rays).

use bytemuck::{Pod, Zeroable};
use forge_core::MeshData;
use forge_gpu::Target;
use wgpu::util::DeviceExt;

use crate::bvh::Bvh;
use crate::compute;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
/// Sample points per texel.
pub const SAMPLES: u32 = 4;

const CORNER_ATTRIBUTES: [wgpu::VertexAttribute; 3] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Uint32, 2 => Uint32];
const SHIFT_ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![3 => Float32x2];

/// One triangle corner: its UV, its triangle's BVH slot and which corner it is (0..3).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Corner {
    uv: [f32; 2],
    slot: u32,
    corner: u32,
}

pub struct Surface {
    /// x = BVH triangle, yz = barycentrics on its e1 and e2, w = coverage (1 covered, 0.5 filled
    /// from a neighbour, 0 empty).
    pub map: Target,
    raw: Target,
    corners: wgpu::Buffer,
    shifts: wgpu::Buffer,
    count: u32,
    raster: wgpu::RenderPipeline,
    dilate: wgpu::ComputePipeline,
    dilate_bind: wgpu::BindGroup,
    size: u32,
}

impl Surface {
    pub fn new(
        device: &wgpu::Device,
        bvh: &Bvh,
        mesh: &MeshData,
        size: u32,
    ) -> Result<Self, String> {
        let corners: Vec<Corner> = bvh
            .order
            .iter()
            .enumerate()
            .flat_map(|(slot, &t)| {
                [0u32, 1, 2].map(|k| Corner {
                    uv: mesh.vertices[mesh.indices[t as usize * 3 + k as usize] as usize].uv,
                    slot: slot as u32,
                    corner: k,
                })
            })
            .collect();
        let count = corners.len() as u32;
        // Vertex buffers can't be empty.
        let corners = if corners.is_empty() {
            vec![Corner::zeroed()]
        } else {
            corners
        };
        // A quarter texel is half a texel's 2/size width in clip space; clip y runs up, v down.
        let q = 0.5 / size.max(1) as f32;
        let shifts = [[-q, q], [q, q], [-q, -q], [q, -q]];
        let vertex_buffer = |label, contents| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage: wgpu::BufferUsages::VERTEX,
            })
        };
        let corners = vertex_buffer("surface corners", bytemuck::cast_slice(&corners));
        let shifts = vertex_buffer("surface shifts", bytemuck::cast_slice(&shifts));

        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let raw = Target::new(device, "raw surface map", [size; 2], FORMAT, attach, 1);
        let out = wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING;
        let map = Target::new(device, "surface map", [size; 2], FORMAT, out, 1);

        let module = forge_gpu::compile(
            device,
            "surface.wgsl",
            include_str!("../shaders/surface.wgsl"),
        )?;
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("surface raster"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let raster = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("surface raster"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<Corner>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &CORNER_ATTRIBUTES,
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: 8,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &SHIFT_ATTRIBUTES,
                    }),
                ],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("surface dilate"),
            entries: &[compute::texture(0), compute::storage_texture(1, FORMAT)],
        });
        let dilate = compute::pipeline(device, &module, &bgl, "dilate");
        let dilate_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("surface dilate"),
            layout: &bgl,
            entries: &[
                compute::texture_entry(0, &raw),
                compute::texture_entry(1, &map),
            ],
        });
        Ok(Self {
            map,
            raw,
            corners,
            shifts,
            count,
            raster,
            dilate,
            dilate_bind,
            size,
        })
    }

    /// Fills `map` for sample point `sample` (0..SAMPLES).
    pub fn draw(&self, device: &wgpu::Device, queue: &wgpu::Queue, sample: u32) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("surface"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("surface raster"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.raw.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.raster);
            pass.set_vertex_buffer(0, self.corners.slice(..));
            pass.set_vertex_buffer(1, self.shifts.slice(u64::from(sample % SAMPLES) * 8..));
            pass.draw(0..self.count, 0..1);
        }
        queue.submit([encoder.finish()]);
        compute::dispatch(
            device,
            queue,
            &self.dilate,
            &self.dilate_bind,
            &[],
            self.size,
        );
    }
}
