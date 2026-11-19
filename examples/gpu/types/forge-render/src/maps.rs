use std::path::Path;

use forge_core::{AssetInfo, MeshData};
use forge_gpu::Target;

use crate::GpuAsset;
use crate::bvh;
use crate::curvature::Curvature;
use crate::occlusion::{EXTRAS, Occlusion};
use crate::scene::vertex_layout;
use crate::surface::{self, Surface};
use crate::view_map;
mod neutral;

const POSITION: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
pub(crate) const NORMAL: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const PART: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;
/// The view map's side: twice the node thumbnail, which supersamples it 2x2.
const VIEW: u32 = 256;
/// Bytes per texel of the traced maps (`NORMAL`, `EXTRAS`).
pub(crate) const TEXEL: u32 = 8;
/// Bumped when the bake changes, so older caches miss.
pub(crate) const MAGIC: &[u8; 8] = b"MWMAPS03";

/// Mesh data baked into the UV atlas, the inputs every material node reads.
/// `position`: object-space xyz, w = coverage (1 inside a chart, 0.5 dilated, 0 empty).
/// `normal`: unit normal xyz, w = curvature (0.5 flat, brighter convex, darker concave).
/// `extras`: r = ambient occlusion (1 open), g = thickness (0 thin, 1 thick),
/// b = part index (into `AssetInfo::parts`), a = near other part (0 touching a different part,
/// 1 far from every other part or none in the mesh, see `occlusion.rs`).
/// `view`: the mesh from the 3/4 preview camera (see `view_map::bake`), for node thumbnails.
#[derive(Clone)]
pub struct MeshMaps {
    pub position: Target,
    pub normal: Target,
    pub extras: Target,
    pub view: Target,
    pub size: u32,
    /// AO, thickness, curvature and near other part were traced; otherwise they are neutral
    /// (1, 1, 0.5, 1).
    pub baked: bool,
}

#[derive(Clone, Copy, Default)]
pub struct BakeOptions<'a> {
    pub trace: bool,
    pub ao_isolated_parts: &'a [u32],
}

impl MeshMaps {
    /// Where `save` keeps them in an asset's folder.
    pub const FILE: &'static str = "maps.bin";

    /// Rasterises the rest-pose mesh in UV space (see `raster`). With `trace`, traces AO,
    /// thickness, curvature and near other part against `mesh` from the exact surface point at
    /// each of `surface::SAMPLES` sample points per texel (`occlusion.rs`, `curvature.rs`),
    /// averaging them; without, they stay neutral until the user bakes them.
    pub fn bake(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        asset: &GpuAsset,
        info: &AssetInfo,
        mesh: &MeshData,
        size: u32,
        trace: bool,
    ) -> Result<Self, String> {
        Self::bake_with_options(
            device,
            queue,
            asset,
            info,
            mesh,
            size,
            BakeOptions {
                trace,
                ao_isolated_parts: &[],
            },
        )
    }

    pub fn bake_with_options(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        asset: &GpuAsset,
        info: &AssetInfo,
        mesh: &MeshData,
        size: u32,
        options: BakeOptions<'_>,
    ) -> Result<Self, String> {
        if options
            .ao_isolated_parts
            .iter()
            .any(|&part| part as usize >= info.parts.len())
        {
            return Err("AO isolation references an unknown mesh part".into());
        }
        let [position, normal, part] = raster(device, queue, asset, size)?;
        let (normal, extras) = if options.trace {
            let mut tree = bvh::Bvh::build(mesh);
            let isolated: std::collections::BTreeSet<_> =
                options.ao_isolated_parts.iter().copied().collect();
            for triangle in &mut tree.tris {
                let part = triangle.v0[3] as u32;
                triangle.e1[3] = if isolated.contains(&part) {
                    (part + 1) as f32
                } else {
                    0.0
                };
            }
            let bvh = tree.upload(device);
            let surface = Surface::new(device, &tree, mesh, size)?;
            let occlusion =
                Occlusion::new(device, &bvh, &info.bounds, [&surface.map, &part], size)?;
            let curvature =
                Curvature::new(device, &bvh, &info.bounds, [&surface.map, &normal], size)?;
            for sample in 0..surface::SAMPLES {
                surface.draw(device, queue, sample);
                occlusion.accumulate(device, queue, sample);
                curvature.accumulate(device, queue, sample);
            }
            let extras = occlusion.resolve(device, queue);
            let normal = curvature.resolve(device, queue);
            (normal, extras)
        } else {
            neutral::maps(device, queue, [&normal, &part], size)?
        };
        let view = view_map::bake(device, queue, asset, info, VIEW)?;
        Ok(Self {
            position,
            normal,
            extras,
            view,
            size,
            baked: options.trace,
        })
    }

    /// The traced maps `save` wrote to `path` for this mesh and size, or None when it holds
    /// none for them.
    pub fn load(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        asset: &GpuAsset,
        info: &AssetInfo,
        mesh: &MeshData,
        size: u32,
        path: &Path,
    ) -> Result<Option<Self>, String> {
        let Ok(bytes) = std::fs::read(path) else {
            return Ok(None);
        };
        Self::from_bytes(device, queue, asset, info, mesh, size, &bytes)
    }

    /// Loads an embedded bake without a file-system dependency.
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        asset: &GpuAsset,
        info: &AssetInfo,
        mesh: &MeshData,
        size: u32,
        bytes: &[u8],
    ) -> Result<Option<Self>, String> {
        let texels = (size as usize).pow(2) * TEXEL as usize;
        let body = bytes
            .strip_prefix(MAGIC)
            .and_then(|b| b.strip_prefix(&key(mesh, size).to_le_bytes()));
        let Some((normal_bytes, extras_bytes)) = body
            .filter(|b| b.len() == 2 * texels)
            .map(|b| b.split_at(texels))
        else {
            return Ok(None);
        };
        let [position, ..] = raster(device, queue, asset, size)?;
        let usage = wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST;
        let normal = Target::new(device, "normal map", [size; 2], NORMAL, usage, 1);
        let extras = Target::new(device, "extras map", [size; 2], EXTRAS, usage, 1);
        forge_gpu::write_texture(queue, &normal.texture, [size; 2], TEXEL, normal_bytes);
        forge_gpu::write_texture(queue, &extras.texture, [size; 2], TEXEL, extras_bytes);
        let view = view_map::bake(device, queue, asset, info, VIEW)?;
        Ok(Some(Self {
            position,
            normal,
            extras,
            view,
            size,
            baked: true,
        }))
    }

    /// Writes the traced maps to `path` for `load`, keyed on `mesh` and the size.
    pub fn save(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &MeshData,
        path: &Path,
    ) -> Result<(), String> {
        let read = |t: &Target| {
            forge_gpu::read_texture(device, queue, &t.texture, [self.size; 2], 1, TEXEL)
        };
        let (normal, extras) = (read(&self.normal)?, read(&self.extras)?);
        let mut out = Vec::with_capacity(16 + normal.len() + extras.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&key(mesh, self.size).to_le_bytes());
        out.extend_from_slice(&normal);
        out.extend_from_slice(&extras);
        forge_core::error::write_bytes(path, &out).map_err(|e| e.to_string())
    }
}

/// The mesh and map size the traced maps were made for: any rebuild that changes the mesh
/// misses.
pub(crate) fn key(mesh: &MeshData, size: u32) -> u64 {
    forge_core::fnv1a(&[&mesh.to_bytes(), &size.to_le_bytes()])
}

/// Rasterises the rest-pose mesh in UV space, then dilates each chart a few texels so
/// bilinear sampling at seams never reads empty texels: position, normal and part maps.
fn raster(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    asset: &GpuAsset,
    size: u32,
) -> Result<[Target; 3], String> {
    let attach = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
    let raw_pos = Target::new(device, "raw position map", [size; 2], POSITION, attach, 1);
    let raw_nrm = Target::new(device, "raw normal map", [size; 2], NORMAL, attach, 1);
    let raw_part = Target::new(device, "raw part map", [size; 2], PART, attach, 1);
    // Copy source too, for the Maps tab's readback.
    let out = wgpu::TextureUsages::STORAGE_BINDING
        | wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::COPY_SRC;
    let position = Target::new(device, "position map", [size; 2], POSITION, out, 1);
    let normal = Target::new(device, "normal map", [size; 2], NORMAL, out, 1);
    let part = Target::new(device, "part map", [size; 2], PART, out, 1);

    let raster = forge_gpu::compile(
        device,
        "maps_raster.wgsl",
        include_str!("../shaders/maps_raster.wgsl"),
    )?;
    let dilate = forge_gpu::compile(
        device,
        "maps_dilate.wgsl",
        include_str!("../shaders/maps_dilate.wgsl"),
    )?;

    let raster_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("maps raster"),
        bind_group_layouts: &[],
        immediate_size: 0,
    });
    let raster_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("maps raster"),
        layout: Some(&raster_layout),
        vertex: wgpu::VertexState {
            module: &raster,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_layout())],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &raster,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[
                Some(wgpu::ColorTargetState {
                    format: POSITION,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
                Some(wgpu::ColorTargetState {
                    format: NORMAL,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
                Some(wgpu::ColorTargetState {
                    format: PART,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
            ],
        }),
        multiview_mask: None,
        cache: None,
    });

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
    let storage = |binding, format| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    };
    let dilate_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("maps dilate"),
        entries: &[
            texture(0),
            texture(1),
            storage(2, POSITION),
            storage(3, NORMAL),
            texture(4),
            storage(5, PART),
        ],
    });
    let dilate_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("maps dilate"),
        bind_group_layouts: &[Some(&dilate_bgl)],
        immediate_size: 0,
    });
    let dilate_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("maps dilate"),
        layout: Some(&dilate_layout),
        module: &dilate,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let views = [
        &raw_pos.view,
        &raw_nrm.view,
        &position.view,
        &normal.view,
        &raw_part.view,
        &part.view,
    ];
    let entries: Vec<wgpu::BindGroupEntry> = views
        .iter()
        .enumerate()
        .map(|(i, view)| wgpu::BindGroupEntry {
            binding: i as u32,
            resource: wgpu::BindingResource::TextureView(view),
        })
        .collect();
    let dilate_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("maps dilate"),
        layout: &dilate_bgl,
        entries: &entries,
    });

    let (vertices, indices, index_count) = asset.buffers();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mesh maps"),
    });
    {
        let clear = |view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("maps raster"),
            color_attachments: &[
                clear(&raw_pos.view),
                clear(&raw_nrm.view),
                clear(&raw_part.view),
            ],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&raster_pipeline);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..index_count, 0, 0..1);
    }
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("maps dilate"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&dilate_pipeline);
        pass.set_bind_group(0, &dilate_bind, &[]);
        pass.dispatch_workgroups(size.div_ceil(8), size.div_ceil(8), 1);
    }
    queue.submit([encoder.finish()]);
    Ok([position, normal, part])
}
