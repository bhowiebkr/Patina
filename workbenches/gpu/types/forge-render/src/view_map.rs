use forge_core::AssetInfo;
use forge_gpu::Target;

use crate::GpuAsset;
use crate::OrbitCamera;
use crate::scene::{Light, vertex_layout};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Renders the rest-pose mesh from the asset's camera (fitted, else 3/4 framing) into a `size`-square map:
/// rg = the UV seen, b = light (0.3 to 1), a = coverage. Node thumbnails look the node's
/// output up through it, so a mesh-domain preview shows the model, not the UV atlas.
pub fn bake(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    asset: &GpuAsset,
    info: &AssetInfo,
    size: u32,
) -> Result<Target, String> {
    let attach = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
    let target = Target::new(device, "view map", [size; 2], FORMAT, attach, 1);
    let depth = Target::new(device, "view map depth", [size; 2], DEPTH, attach, 1);
    let module = forge_gpu::compile(
        device,
        "maps_view.wgsl",
        include_str!("../shaders/maps_view.wgsl"),
    )?;

    let camera = match &info.view {
        Some(view) => OrbitCamera::fitted(view),
        None => OrbitCamera::framing(info.bounds.center(), info.bounds.radius() * 0.85),
    };
    let sun = Light::for_asset(info).sun;
    let mut uniform = [0f32; 20];
    uniform[..16].copy_from_slice(&camera.view_proj(1.0).to_cols_array());
    uniform[16..19].copy_from_slice(&sun.to_array());
    let buffer = forge_gpu::uniform_buffer(device, "view map", bytemuck::cast_slice(&uniform));

    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("view map"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("view map"),
        layout: &bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("view map"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("view map"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_layout())],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        }),
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

    let (vertices, indices, index_count) = asset.buffers();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("view map"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("view map"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
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
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..index_count, 0, 0..1);
    }
    queue.submit([encoder.finish()]);
    Ok(target)
}
