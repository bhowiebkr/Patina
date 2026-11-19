//! The material planes with a mip chain, so the viewport reads them filtered at any distance: a
//! 20482 map on a 768 px view aliases into sparkles without one. The chain is rebuilt on the GPU
//! from the node output after every change.

use crate::compute::pipeline;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const LAYERS: u32 = 3;

/// The two passes that build a chain, made once per device.
pub(crate) struct MipPipelines {
    first_layout: wgpu::BindGroupLayout,
    first: wgpu::ComputePipeline,
    down_layout: wgpu::BindGroupLayout,
    down: wgpu::ComputePipeline,
}

impl MipPipelines {
    pub(crate) fn new(device: &wgpu::Device) -> Result<Self, String> {
        let module = forge_gpu::compile(device, "mips.wgsl", include_str!("../shaders/mips.wgsl"))?;
        let texture = |binding, view_dimension| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension,
                multisampled: false,
            },
            count: None,
        };
        let store = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::StorageTexture {
                access: wgpu::StorageTextureAccess::WriteOnly,
                format: FORMAT,
                view_dimension: wgpu::TextureViewDimension::D2Array,
            },
            count: None,
        };
        let d2 = wgpu::TextureViewDimension::D2;
        let first_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mips first"),
            entries: &[texture(0, d2), texture(1, d2), texture(2, d2), store(3)],
        });
        let down_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mips down"),
            entries: &[texture(4, wgpu::TextureViewDimension::D2Array), store(5)],
        });
        Ok(Self {
            first: pipeline(device, &module, &first_layout, "first"),
            down: pipeline(device, &module, &down_layout, "down"),
            first_layout,
            down_layout,
        })
    }
}

/// The node output's three layers copied into a mipmapped texture array.
pub(crate) struct Planes {
    texture: wgpu::Texture,
    /// One view per layer over every mip, for the scene shader.
    pub(crate) views: [wgpu::TextureView; 3],
    /// Per pass: its pipeline, bind group and the size it writes.
    passes: Vec<(wgpu::ComputePipeline, wgpu::BindGroup, u32)>,
}

impl Planes {
    /// Allocates the chain for `source` (three same-sized layers, mip 0 read), reusing `old`'s
    /// texture when the size matches, and builds it.
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mips: &MipPipelines,
        source: [&wgpu::TextureView; 3],
        old: Option<Planes>,
    ) -> Self {
        let size = source[0].texture().width().max(1);
        let texture = match old {
            Some(old) if old.texture.width() == size => old.texture,
            _ => device.create_texture(&wgpu::TextureDescriptor {
                label: Some("material mips"),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: LAYERS,
                },
                mip_level_count: size.ilog2() + 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            }),
        };
        let level = |mip| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("material mip"),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                base_mip_level: mip,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        fn view(binding: u32, view: &wgpu::TextureView) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(view),
            }
        }
        let top = level(0);
        let first = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mips first"),
            layout: &mips.first_layout,
            entries: &[
                view(0, source[0]),
                view(1, source[1]),
                view(2, source[2]),
                view(3, &top),
            ],
        });
        let mut passes = vec![(mips.first.clone(), first, size)];
        for mip in 1..texture.mip_level_count() {
            let (above, below) = (level(mip - 1), level(mip));
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mips down"),
                layout: &mips.down_layout,
                entries: &[view(4, &above), view(5, &below)],
            });
            passes.push((mips.down.clone(), bind, (size >> mip).max(1)));
        }
        let views = [0, 1, 2].map(|layer| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("material plane"),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });
        let planes = Self {
            texture,
            views,
            passes,
        };
        planes.build(device, queue);
        planes
    }

    /// Recopies the source layers and rebuilds every mip.
    pub(crate) fn build(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("material mips"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("material mips"),
                timestamp_writes: None,
            });
            for (i, (pipeline, bind, size)) in self.passes.iter().enumerate() {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, bind, &[]);
                let layers = if i == 0 { 1 } else { LAYERS };
                pass.dispatch_workgroups(size.div_ceil(8), size.div_ceil(8), layers);
            }
        }
        queue.submit([encoder.finish()]);
    }
}
