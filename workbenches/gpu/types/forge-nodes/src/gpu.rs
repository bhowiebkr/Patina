//! GPU plumbing shared by every node: bind group layouts (matching `kernels/prelude.wesl` and
//! `kernels/thumb.wgsl`), layered output textures, the thumbnail pipeline and stand-ins for
//! unbound resources.

use bytemuck::{Pod, Zeroable};
use forge_gpu::Target;

use crate::def::MAX_INPUTS;

/// Node outputs: one array layer per gray or colour output, three per material. Full float:
/// f16 steps (4.9e-4 near 0.5) through steep gradient maps cost one tested material
/// 0.135 MAE against 0.013 in f32.
pub const OUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
/// Bytes of one `OUT_FORMAT` texel in a readback.
pub const OUT_TEXEL: u32 = 16;
/// Multipass scratch, full precision for distance and accumulation passes.
pub const SCRATCH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
/// Side of a node thumbnail, in texels.
pub const THUMB: u32 = 128;
pub const THUMB_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Shared RGBA LUT plus Curve's temporary master table (`array<atomic<u32>, 5120>`).
pub const STATS_BYTES: u64 = 20480;

/// `Info` in the prelude; one per pass, at a dynamic offset.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Info {
    pub size: [u32; 4],
    pub domain: [u32; 4],
    pub inputs: [[u32; 4]; MAX_INPUTS],
    pub triplanar: [f32; 4],
    pub bounds: [f32; 4],
}

pub const INFO_SIZE: u64 = std::mem::size_of::<Info>() as u64;
/// `Info.domain.w` bit8; bit0 remains the host-image presence flag.
pub(crate) const UPDATED_INPUT_DEFAULTS: u32 = 1 << 8;
/// The `atoms` storage buffer, after the inputs (bindings 10 onwards).
pub const ATOMS_BINDING: u32 = 10 + MAX_INPUTS as u32;
/// The part table: per part, its CAD base colour (linear rgb) and colour group (w).
pub const PARTS_BINDING: u32 = ATOMS_BINDING + 1;
/// Sampled textures a node pass binds: the three mesh maps, the scratch input and every input.
pub const SAMPLED_TEXTURES: u32 = 4 + MAX_INPUTS as u32;

/// wgpu's default limits raised to what node passes bind, with `adapter`'s buffer limits so a
/// 4096 mesh-map bake can bind its 256 MiB accumulation buffers.
pub fn required_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
    let (base, max) = (wgpu::Limits::default(), adapter.limits());
    wgpu::Limits {
        max_sampled_textures_per_shader_stage: base
            .max_sampled_textures_per_shader_stage
            .max(SAMPLED_TEXTURES),
        max_buffer_size: max.max_buffer_size.max(base.max_buffer_size),
        max_storage_buffer_binding_size: max
            .max_storage_buffer_binding_size
            .max(base.max_storage_buffer_binding_size),
        ..base
    }
}

/// Bilinear reads of `OUT_FORMAT` need FLOAT32_FILTERABLE (every desktop GPU has it).
pub fn required_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    adapter.features() & wgpu::Features::FLOAT32_FILTERABLE
}

/// A device request with [`required_limits`], for tools that make their own device.
pub fn device_descriptor(adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
    wgpu::DeviceDescriptor {
        label: Some("millwright nodes"),
        required_features: required_features(adapter),
        required_limits: required_limits(adapter),
        ..Default::default()
    }
}

/// One `OUT_FORMAT` texel of a readback, from `OUT_TEXEL` bytes.
pub fn out_texel(bytes: &[u8]) -> [f32; 4] {
    let mut c = [0.0; 4];
    for (v, b) in c.iter_mut().zip(bytes.as_chunks::<4>().0) {
        *v = f32::from_le_bytes(*b);
    }
    c
}

/// A texture array with a view of the whole array and one 2D view per layer.
pub struct Layers {
    pub texture: wgpu::Texture,
    pub array: wgpu::TextureView,
    pub layers: Vec<wgpu::TextureView>,
    pub size: u32,
}

impl Layers {
    pub fn new(
        device: &wgpu::Device,
        label: &str,
        size: u32,
        count: u32,
        usage: wgpu::TextureUsages,
    ) -> Self {
        let count = count.max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.max(1),
                height: size.max(1),
                depth_or_array_layers: count,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUT_FORMAT,
            usage,
            view_formats: &[],
        });
        let array = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layers = (0..count)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        Self {
            texture,
            array,
            layers,
            size,
        }
    }
}

fn entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    ty: wgpu::BindingType,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty,
        count: None,
    }
}

fn texture(filterable: bool, dimension: wgpu::TextureViewDimension) -> wgpu::BindingType {
    wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable },
        view_dimension: dimension,
        multisampled: false,
    }
}

fn storage_texture(
    format: wgpu::TextureFormat,
    dimension: wgpu::TextureViewDimension,
) -> wgpu::BindingType {
    wgpu::BindingType::StorageTexture {
        access: wgpu::StorageTextureAccess::WriteOnly,
        format,
        view_dimension: dimension,
    }
}

/// The node kernels' bind group layout (bindings 0..21 of the prelude).
pub fn node_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    use wgpu::TextureViewDimension::{D2, D2Array};
    let c = wgpu::ShaderStages::COMPUTE;
    let mut entries = vec![
        entry(
            0,
            c,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(INFO_SIZE),
            },
        ),
        entry(
            1,
            c,
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
        ),
        entry(
            2,
            c,
            wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        ),
    ];
    entries.extend((3..7).map(|b| entry(b, c, texture(false, D2))));
    entries.push(entry(7, c, storage_texture(SCRATCH_FORMAT, D2)));
    entries.push(entry(8, c, storage_texture(OUT_FORMAT, D2Array)));
    entries.push(entry(
        9,
        c,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(STATS_BYTES),
        },
    ));
    entries.extend((0..MAX_INPUTS as u32).map(|i| entry(10 + i, c, texture(true, D2Array))));
    entries.push(entry(
        ATOMS_BINDING,
        c,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(4),
        },
    ));
    entries.push(entry(
        PARTS_BINDING,
        c,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(16),
        },
    ));
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("node"),
        entries: &entries,
    })
}

pub fn thumb_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let f = wgpu::ShaderStages::FRAGMENT;
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("thumbnail"),
        entries: &[
            entry(
                0,
                f,
                wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
            ),
            entry(1, f, texture(false, wgpu::TextureViewDimension::D2Array)),
            entry(2, f, texture(false, wgpu::TextureViewDimension::D2)),
        ],
    })
}

pub fn thumb_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    source: &str,
) -> Result<wgpu::RenderPipeline, String> {
    let module = forge_gpu::compile(device, "thumb.wgsl", source)?;
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("thumbnail"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    forge_gpu::validated(device, "thumb.wgsl", || {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("thumbnail"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(THUMB_FORMAT.into())],
            }),
            multiview_mask: None,
            cache: None,
        })
    })
}

/// Stand-ins bound where a node has no mesh maps, scratch or linked input.
pub struct Dummies {
    /// Mesh maps and scratch input.
    pub map: Target,
    pub scratch_out: Target,
    pub input: Layers,
    pub stats: wgpu::Buffer,
    pub atoms: wgpu::Buffer,
    /// A one-row part table, for engines without a target.
    pub parts: wgpu::Buffer,
    pub sampler: wgpu::Sampler,
}

impl Dummies {
    pub fn new(device: &wgpu::Device) -> Self {
        let map = Target::new(
            device,
            "no mesh map",
            [1, 1],
            SCRATCH_FORMAT,
            wgpu::TextureUsages::TEXTURE_BINDING,
            1,
        );
        let scratch_out = Target::new(
            device,
            "no scratch",
            [1, 1],
            SCRATCH_FORMAT,
            wgpu::TextureUsages::STORAGE_BINDING,
            1,
        );
        let input = Layers::new(
            device,
            "unlinked input",
            1,
            1,
            wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let stats = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("node stats"),
            size: STATS_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let atoms = atoms_buffer(device, 4);
        let parts = part_table(device, &[[0.5, 0.5, 0.5, 0.0]]);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("node input"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            map,
            scratch_out,
            input,
            stats,
            atoms,
            parts,
            sampler,
        }
    }
}

/// Two scratch textures of one size for multipass ping-pong.
pub fn scratch_pair(device: &wgpu::Device, size: u32) -> [Target; 2] {
    let usage = wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING;
    [0, 1].map(|_| Target::new(device, "node scratch", [size; 2], SCRATCH_FORMAT, usage, 1))
}

/// An `atoms` buffer of `bytes`, capped at the device's storage binding limit (accesses past the
/// end are clamped by WGSL, so an oversized node degrades instead of failing).
pub fn atoms_buffer(device: &wgpu::Device, bytes: u64) -> wgpu::Buffer {
    let limit = device.limits().max_storage_buffer_binding_size;
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("node atoms"),
        size: bytes.clamp(4, limit) / 4 * 4,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// The part table buffer (see [`PARTS_BINDING`]); at least one row.
pub fn part_table(device: &wgpu::Device, rows: &[[f32; 4]]) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    let fallback = [[0.5, 0.5, 0.5, 0.0]];
    let rows = if rows.is_empty() { &fallback[..] } else { rows };
    let bytes: Vec<u8> = rows
        .iter()
        .flatten()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("part table"),
        contents: &bytes,
        usage: wgpu::BufferUsages::STORAGE,
    })
}
