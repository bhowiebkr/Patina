//! Hand-painted masks for Paint Mask nodes: one R32Float texture per node in the mesh maps' UV
//! layout, and the compute brush (`kernels/stamp.wgsl`) that stamps dabs into it.

use bytemuck::{Pod, Zeroable};
use forge_gpu::Target;

mod boundary;
#[cfg(test)]
mod tests;

pub const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
/// Bytes per mask texel.
pub const MASK_TEXEL: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Brush,
    Eraser,
    /// Sets the whole picked part to the strength.
    Fill,
}

#[derive(Clone, Copy, Debug)]
pub struct Brush {
    pub tool: Tool,
    /// Radius in screen pixels.
    pub radius: f32,
    /// Model units one screen pixel spans at unit distance from the eye.
    pub pixel: f32,
    pub strength: f32,
    /// 0 soft falloff from the centre, 1 hard edge.
    pub hardness: f32,
    /// Also stamps the dab mirrored across the plane where model axis `.0` (0 x, 1 y, 2 z)
    /// equals `.1`.
    pub mirror: Option<(usize, f32)>,
    pub object_lock: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BrushGpu {
    shape: [f32; 4],
    mode: [u32; 4],
    /// The mirror plane's coordinate (x).
    mirror: [f32; 4],
}

pub(crate) fn new_mask(device: &wgpu::Device, size: u32) -> Target {
    Target::new(
        device,
        "paint mask",
        [size; 2],
        MASK_FORMAT,
        wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        1,
    )
}

pub(crate) struct Stamper {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    uniform: wgpu::Buffer,
    boundary: boundary::Boundary,
}

impl Stamper {
    pub fn new(device: &wgpu::Device, source: &str) -> Result<Self, String> {
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("stamp"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture(1),
                texture(2),
                texture(3),
                texture(4),
                texture(5),
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::ReadWrite,
                        format: MASK_FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let module = forge_gpu::compile(device, "stamp.wgsl", source)?;
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("stamp"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = forge_gpu::validated(device, "stamp.wgsl", || {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("stamp"),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            })
        })?;
        let uniform =
            forge_gpu::uniform_buffer(device, "brush", bytemuck::bytes_of(&BrushGpu::zeroed()));
        Ok(Self {
            layout,
            pipeline,
            uniform,
            boundary: boundary::Boundary::new(device)?,
        })
    }

    /// Stamps one dab. `hit` is the picker's position and normal texels, `maps` the mesh maps.
    pub fn stamp(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        brush: &Brush,
        hit: [&wgpu::TextureView; 2],
        maps: [&wgpu::TextureView; 3],
        mask: &Target,
    ) {
        let size = mask.size[0];
        let tool = match brush.tool {
            Tool::Brush => 0,
            Tool::Eraser => 1,
            Tool::Fill => 2,
        };
        let (axis, plane) = brush.mirror.map_or((0, 0.0), |(a, c)| (a as u32 + 1, c));
        let gpu = BrushGpu {
            shape: [brush.radius, brush.pixel, brush.strength, brush.hardness],
            mode: [tool, size, axis, 0],
            mirror: [plane, 0.0, 0.0, 0.0],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&gpu));
        let view = |binding, view| wgpu::BindGroupEntry {
            binding,
            resource: wgpu::BindingResource::TextureView(view),
        };
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stamp"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                view(1, hit[0]),
                view(2, hit[1]),
                view(3, maps[0]),
                view(4, maps[1]),
                view(5, maps[2]),
                view(6, &mask.view),
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        let protect = brush.object_lock && brush.tool != Tool::Fill;
        if protect {
            self.boundary.backup(device, &mut encoder, mask);
        }
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups(size.div_ceil(8), size.div_ceil(8), 1);
        }
        if protect {
            self.boundary
                .restore(device, &mut encoder, hit[1], maps[2], mask);
        }
        queue.submit([encoder.finish()]);
    }
}
