//! Image-based lighting: an equirectangular HDR panorama prefiltered on the GPU into a radiance
//! mip chain (GGX, one perceptual roughness per mip) and a diffuse irradiance map, the two
//! textures Bevy's environment map light reads. Built once per panorama, never in the edit loop.

use bytemuck::{Pod, Zeroable};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const RADIANCE: [u32; 2] = [1024, 512];
const LEVELS: u32 = 8;
const IRRADIANCE: [u32; 2] = [64, 32];
/// The irradiance sums every texel of the first source mip at most this wide.
const IRRADIANCE_SOURCE: u32 = 128;
const SAMPLES: u32 = 512;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    size: [u32; 2],
    level: u32,
    levels: u32,
    source_size: [u32; 2],
    mip: u32,
    samples: u32,
}

/// A panorama's prefiltered radiance (`levels` mips) and irradiance.
#[derive(Clone)]
pub struct Environment {
    pub(crate) radiance: wgpu::TextureView,
    pub(crate) irradiance: wgpu::TextureView,
    pub(crate) levels: u32,
}

fn mip_count(size: [u32; 2]) -> u32 {
    32 - size[0].max(size[1]).max(1).leading_zeros()
}

fn mip_size(size: [u32; 2], level: u32) -> [u32; 2] {
    [(size[0] >> level).max(1), (size[1] >> level).max(1)]
}

fn texture(
    device: &wgpu::Device,
    label: &str,
    size: [u32; 2],
    levels: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn mip_view(texture: &wgpu::Texture, level: u32) -> wgpu::TextureView {
    texture.create_view(&wgpu::TextureViewDescriptor {
        base_mip_level: level,
        mip_level_count: Some(1),
        ..Default::default()
    })
}

struct Passes<'a> {
    device: &'a wgpu::Device,
    module: wgpu::ShaderModule,
    encoder: wgpu::CommandEncoder,
}

impl Passes<'_> {
    /// One dispatch of `entry` over `params.size`; `bindings` are (binding, resource) pairs
    /// besides the parameters.
    fn run(&mut self, entry: &str, params: Params, bindings: &[(u32, wgpu::BindingResource)]) {
        let pipeline = self
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &self.module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            });
        let buffer = forge_gpu::uniform_buffer(self.device, entry, bytemuck::bytes_of(&params));
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }];
        entries.extend(
            bindings
                .iter()
                .map(|(binding, resource)| wgpu::BindGroupEntry {
                    binding: *binding,
                    resource: resource.clone(),
                }),
        );
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(entry),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut pass = self
            .encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(entry),
                timestamp_writes: None,
            });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(params.size[0].div_ceil(8), params.size[1].div_ceil(8), 1);
    }
}

impl Environment {
    /// Prefilters a `width` x `height` panorama of linear RGBA f32 pixels, top row first.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        rgba: &[f32],
    ) -> Result<Self, String> {
        let size = [width, height];
        let max = device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > max || height > max {
            return Err(format!("panorama {width}x{height}: unsupported size"));
        }
        if rgba.len() != (width * height * 4) as usize {
            return Err(format!("panorama {width}x{height}: {} floats", rgba.len()));
        }
        let usage = wgpu::TextureUsages::TEXTURE_BINDING;
        let store = usage | wgpu::TextureUsages::STORAGE_BINDING;
        let raw = texture(
            device,
            "panorama f32",
            size,
            1,
            wgpu::TextureFormat::Rgba32Float,
            usage | wgpu::TextureUsages::COPY_DST,
        );
        queue.write_texture(
            raw.as_image_copy(),
            bytemuck::cast_slice(rgba),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 16),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let mips = mip_count(size);
        let source = texture(device, "panorama", size, mips, FORMAT, store);
        let radiance = texture(device, "env radiance", RADIANCE, LEVELS, FORMAT, store);
        let irradiance = texture(device, "env irradiance", IRRADIANCE, 1, FORMAT, store);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("panorama"),
            address_mode_u: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let module = forge_gpu::compile(
            device,
            "environment.wgsl",
            include_str!("../shaders/environment.wgsl"),
        )?;
        // Pipeline and dispatch errors come back as an error, not a device panic.
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut passes = Passes {
            device,
            module,
            encoder: device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("environment"),
            }),
        };
        let params = |size: [u32; 2]| Params {
            size,
            level: 0,
            levels: LEVELS,
            source_size: [width, height],
            mip: 0,
            samples: SAMPLES,
        };
        let raw_view = raw.create_view(&Default::default());
        let mip0 = mip_view(&source, 0);
        passes.run(
            "load",
            params(size),
            &[
                (1, wgpu::BindingResource::TextureView(&raw_view)),
                (4, wgpu::BindingResource::TextureView(&mip0)),
            ],
        );
        for level in 1..mips {
            let (from, to) = (mip_view(&source, level - 1), mip_view(&source, level));
            passes.run(
                "down",
                params(mip_size(size, level)),
                &[
                    (2, wgpu::BindingResource::TextureView(&from)),
                    (4, wgpu::BindingResource::TextureView(&to)),
                ],
            );
        }
        let all = source.create_view(&Default::default());
        for level in 0..LEVELS {
            let to = mip_view(&radiance, level);
            passes.run(
                "radiance",
                Params {
                    level,
                    ..params(mip_size(RADIANCE, level))
                },
                &[
                    (2, wgpu::BindingResource::TextureView(&all)),
                    (3, wgpu::BindingResource::Sampler(&sampler)),
                    (4, wgpu::BindingResource::TextureView(&to)),
                ],
            );
        }
        let mip = (0..mips)
            .find(|&m| mip_size(size, m)[0] <= IRRADIANCE_SOURCE)
            .unwrap_or(mips - 1);
        let to = mip_view(&irradiance, 0);
        passes.run(
            "irradiance",
            Params {
                mip,
                ..params(IRRADIANCE)
            },
            &[
                (2, wgpu::BindingResource::TextureView(&all)),
                (4, wgpu::BindingResource::TextureView(&to)),
            ],
        );
        queue.submit([passes.encoder.finish()]);
        if let Some(error) = forge_gpu::finish_scope(scope, "environment.wgsl") {
            return Err(error);
        }
        Ok(Self {
            radiance: radiance.create_view(&Default::default()),
            irradiance: irradiance.create_view(&Default::default()),
            levels: LEVELS,
        })
    }
}
