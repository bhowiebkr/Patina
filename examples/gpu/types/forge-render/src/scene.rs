use bytemuck::{Pod, Zeroable};
use forge_core::{AssetInfo, MeshData, PartMaterial, Vertex};
use forge_gpu::{ShaderFile, Target};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::planes::{MipPipelines, Planes};
use crate::{Environment, OrbitCamera};

#[cfg(test)]
mod update_tests;

pub const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;
const PLANE: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.003,
    g: 0.003,
    b: 0.003,
    a: 1.0,
};

pub(crate) const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
    0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32, 4 => Uint32
];

pub(crate) fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &VERTEX_ATTRIBUTES,
    }
}

/// What a view is shaded under: a sun (soft when `angle` > 0), a sky dome above the horizon
/// and ground below (or the scene's environment panorama when `env` > 0), and the tone curve.
#[derive(Clone, Copy, Debug)]
pub struct Light {
    /// Direction to the sun.
    pub sun: Vec3,
    pub strength: f32,
    /// The sun's angular diameter, degrees.
    pub angle: f32,
    pub sky: [f32; 3],
    pub ground: [f32; 3],
    pub exposure: f32,
    /// ACES; otherwise a plain clip (Blender's Standard view transform, which fits were made in).
    pub aces: bool,
    /// 0 lit; else one material channel unlit: 1 base colour, 2 roughness, 3 metallic,
    /// 4 height, 5 ambient occlusion, 6 normal.
    pub channel: u32,
    /// Strength of the scene's environment panorama as ambient light; 0 uses the sky and ground.
    pub env: f32,
    /// The panorama's turn about the vertical axis, in turns.
    pub env_yaw: f32,
    pub env_pitch: f32,
}

impl Light {
    /// Panorama-only lighting used by material comparisons and previews.
    pub fn for_environment(info: &AssetInfo) -> Self {
        Self {
            strength: 0.0,
            exposure: 1.0,
            aces: true,
            env: 1.0,
            env_yaw: 0.0,
            env_pitch: 0.0,
            ..Self::for_asset(info)
        }
    }

    pub fn studio() -> Self {
        Self {
            sun: Vec3::new(0.55, 1.0, 0.35).normalize(),
            strength: 3.0,
            angle: 0.0,
            sky: [0.62, 0.62, 0.64],
            ground: [0.16, 0.15, 0.14],
            exposure: 1.0,
            aces: true,
            channel: 0,
            env: 0.0,
            env_yaw: 0.0,
            env_pitch: 0.0,
        }
    }

    /// The light the asset's reference render was fitted under, else the studio light.
    pub fn for_asset(info: &AssetInfo) -> Self {
        match &info.view {
            Some(v) => Self {
                sun: Vec3::from(v.sun).normalize(),
                strength: v.sun_strength,
                angle: v.sun_angle,
                sky: [v.sky; 3],
                ground: [v.ground; 3],
                exposure: 1.0,
                aces: false,
                channel: 0,
                env: 0.0,
                env_yaw: 0.0,
                env_pitch: 0.0,
            },
            None => Self::studio(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameGpu {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    sun: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
    misc: [f32; 4],
    env: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PartGpu {
    model: [[f32; 4]; 4],
    color: [f32; 4],
    pbr: [f32; 4],
    highlight: [f32; 4],
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub struct SceneRenderer {
    shader: ShaderFile,
    layout: wgpu::PipelineLayout,
    pipeline: wgpu::RenderPipeline,
    pub(crate) frame_layout: wgpu::BindGroupLayout,
    pub(crate) asset_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    mips: MipPipelines,
    dummy: Target,
    env_layout: wgpu::BindGroupLayout,
    env_sampler: wgpu::Sampler,
    env_bind: wgpu::BindGroup,
    env_levels: u32,
    preview_exposure: f32,
    preview_rotation: f32,
    preview_tilt: f32,
}

impl SceneRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self, String> {
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene frame"),
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
        let plane = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let asset_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene asset"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                plane(1),
                plane(2),
                plane(4),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let env_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene environment"),
            entries: &[
                plane(0),
                plane(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&frame_layout), Some(&asset_layout), Some(&env_layout)],
            immediate_size: 0,
        });
        let shader = ShaderFile::new(
            concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/scene.wgsl"),
            include_str!("../shaders/scene.wgsl"),
        );
        let module = forge_gpu::compile(device, "scene.wgsl", &shader.source())?;
        let pipeline = Self::pipeline(device, &layout, &module);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("material planes"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });
        let dummy = Target::solid(device, queue, "no material", PLANE, &[0; 8]);
        // Panoramas wrap around in longitude and stop at the poles.
        let env_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("environment"),
            address_mode_u: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let env_bind = Self::env_bind(device, &env_layout, &env_sampler, [&dummy.view; 2]);
        Ok(Self {
            shader,
            layout,
            pipeline,
            frame_layout,
            asset_layout,
            sampler,
            mips: MipPipelines::new(device)?,
            dummy,
            env_layout,
            env_sampler,
            env_bind,
            env_levels: 1,
            preview_exposure: 1.0,
            preview_rotation: 0.0,
            preview_tilt: 0.0,
        })
    }

    fn env_bind(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        [radiance, irradiance]: [&wgpu::TextureView; 2],
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("environment"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(radiance),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(irradiance),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// The panorama a `Light` with `env` > 0 lights the asset with; `None` drops it.
    pub fn set_environment(&mut self, device: &wgpu::Device, env: Option<&Environment>) {
        let d = &self.dummy.view;
        let views = env.map_or([d, d], |e| [&e.radiance, &e.irradiance]);
        self.env_bind = Self::env_bind(device, &self.env_layout, &self.env_sampler, views);
        self.env_levels = env.map_or(1, |e| e.levels);
    }

    /// A loaded panorama replaces the fitted sun and dome, including the tone curve.
    pub fn preview_light(&self, info: &AssetInfo) -> Light {
        if self.env_levels > 1 {
            Light {
                exposure: self.preview_exposure,
                env_yaw: self.preview_rotation,
                env_pitch: self.preview_tilt,
                ..Light::for_environment(info)
            }
        } else {
            Light::for_asset(info)
        }
    }

    pub fn set_preview_lighting(&mut self, exposure: f32, rotation_turns: f32, tilt_turns: f32) {
        self.preview_exposure = exposure;
        self.preview_rotation = rotation_turns;
        self.preview_tilt = tilt_turns;
    }

    fn pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        module: &wgpu::ShaderModule,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module,
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
            multisample: wgpu::MultisampleState {
                count: SAMPLES,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    /// Rebuilds the pipeline when scene.wgsl changed on disk. `Some(Err)` keeps the old one.
    pub fn hot_reload(&mut self, device: &wgpu::Device) -> Option<Result<String, String>> {
        if !self.shader.changed() {
            return None;
        }
        let result =
            forge_gpu::compile(device, "scene.wgsl", &self.shader.source()).and_then(|module| {
                forge_gpu::validated(device, "scene.wgsl", || {
                    Self::pipeline(device, &self.layout, &module)
                })
            });
        Some(result.map(|pipeline| {
            self.pipeline = pipeline;
            "scene.wgsl reloaded".to_owned()
        }))
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &View,
        camera: &OrbitCamera,
        light: &Light,
        asset: &GpuAsset,
    ) {
        let aspect = view.size[0] as f32 / view.size[1].max(1) as f32;
        let (sun, [sr, sg, sb], [gr, gg, gb]) = (light.sun, light.sky, light.ground);
        let frame = FrameGpu {
            view_proj: camera.view_proj(aspect).to_cols_array_2d(),
            eye: camera.eye().extend(1.0).to_array(),
            sun: [sun.x, sun.y, sun.z, light.strength],
            sky: [sr, sg, sb, 1.0],
            ground: [gr, gg, gb, 1.0],
            misc: [
                light.exposure,
                if light.aces { 0.0 } else { 1.0 },
                (light.angle * 0.5).to_radians().sin(),
                light.channel as f32,
            ],
            env: [
                light.env,
                light.env_yaw,
                self.env_levels.saturating_sub(1) as f32,
                light.env_pitch,
            ],
        };
        queue.write_buffer(&view.frame, 0, bytemuck::bytes_of(&frame));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("scene"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view.msaa.view,
                    depth_slice: None,
                    resolve_target: Some(&view.color.view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(CLEAR),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view.depth.view,
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
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &view.bind, &[]);
            pass.set_bind_group(1, &asset.bind, &[]);
            pass.set_bind_group(2, &self.env_bind, &[]);
            pass.set_vertex_buffer(0, asset.vertices.slice(..));
            pass.set_index_buffer(asset.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..asset.index_count, 0, 0..1);
        }
        queue.submit([encoder.finish()]);
    }
}

/// An offscreen viewport: 4x MSAA colour and depth resolved into `color`, which the UI shows.
pub struct View {
    pub color: Target,
    msaa: Target,
    depth: Target,
    frame: wgpu::Buffer,
    bind: wgpu::BindGroup,
    pub size: [u32; 2],
}

impl View {
    pub fn new(device: &wgpu::Device, scene: &SceneRenderer, size: [u32; 2]) -> Self {
        let size = [size[0].max(1), size[1].max(1)];
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let frame =
            forge_gpu::uniform_buffer(device, "frame", bytemuck::bytes_of(&FrameGpu::zeroed()));
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout: &scene.frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame.as_entire_binding(),
            }],
        });
        Self {
            color: Target::new(
                device,
                "view",
                size,
                COLOR,
                attach | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                1,
            ),
            msaa: Target::new(device, "view msaa", size, COLOR, attach, SAMPLES),
            depth: Target::new(device, "view depth", size, DEPTH, attach, SAMPLES),
            frame,
            bind,
            size,
        }
    }
}

/// One asset on the GPU: its mesh, per-part transforms and colours, and the material planes.
pub struct GpuAsset {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    parts: Vec<PartGpu>,
    parts_buffer: wgpu::Buffer,
    bind: wgpu::BindGroup,
    /// The material's mip chain, when one is set.
    planes: Option<Planes>,
}

impl GpuAsset {
    /// Share immutable geometry while owning independent poses, highlights and material planes.
    pub fn preview_copy(&self, device: &wgpu::Device, scene: &SceneRenderer) -> Self {
        let mut parts = self.parts.clone();
        for part in &mut parts {
            part.highlight = [0.; 4];
            // The copy starts with dummy planes; sampling them as a material draws black.
            part.pbr[2] = 0.;
        }
        let bytes: &[u8] = if parts.is_empty() {
            &[0; size_of::<PartGpu>()]
        } else {
            bytemuck::cast_slice(&parts)
        };
        let parts_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("preview parts"),
            contents: bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let bind = Self::bind(device, scene, &parts_buffer, [&scene.dummy.view; 3]);
        Self {
            vertices: self.vertices.clone(),
            indices: self.indices.clone(),
            index_count: self.index_count,
            parts,
            parts_buffer,
            bind,
            planes: None,
        }
    }

    pub fn part_pose(&self) -> Vec<Mat4> {
        self.parts
            .iter()
            .map(|part| Mat4::from_cols_array_2d(&part.model))
            .collect()
    }

    pub fn new(
        device: &wgpu::Device,
        scene: &SceneRenderer,
        info: &AssetInfo,
        mesh: &MeshData,
    ) -> Self {
        let parts: Vec<PartGpu> = info
            .parts
            .iter()
            .map(|p| {
                let c = p.material.base_color;
                PartGpu {
                    model: Mat4::IDENTITY.to_cols_array_2d(),
                    color: [
                        srgb_to_linear(c[0]),
                        srgb_to_linear(c[1]),
                        srgb_to_linear(c[2]),
                        c[3],
                    ],
                    pbr: [p.material.metallic, p.material.roughness, 0.0, 1.0],
                    highlight: [0.0; 4],
                }
            })
            .collect();
        let init = |label, contents, usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let vertices = init(
            "vertices",
            bytemuck::cast_slice(&mesh.vertices),
            wgpu::BufferUsages::VERTEX,
        );
        let indices = init(
            "indices",
            bytemuck::cast_slice(&mesh.indices),
            wgpu::BufferUsages::INDEX,
        );
        // A storage buffer may not be empty.
        let part_bytes: &[u8] = if parts.is_empty() {
            &[0; size_of::<PartGpu>()]
        } else {
            bytemuck::cast_slice(&parts)
        };
        let parts_buffer = init(
            "parts",
            part_bytes,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let bind = Self::bind(device, scene, &parts_buffer, [&scene.dummy.view; 3]);
        Self {
            vertices,
            indices,
            index_count: mesh.indices.len() as u32,
            parts,
            parts_buffer,
            bind,
            planes: None,
        }
    }

    fn bind(
        device: &wgpu::Device,
        scene: &SceneRenderer,
        parts: &wgpu::Buffer,
        planes: [&wgpu::TextureView; 3],
    ) -> wgpu::BindGroup {
        let [plane0, plane1, plane2] = planes;
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("asset"),
            layout: &scene.asset_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: parts.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(plane0),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(plane1),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&scene.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(plane2),
                },
            ],
        })
    }

    pub(crate) fn buffers(&self) -> (&wgpu::Buffer, &wgpu::Buffer, u32) {
        (&self.vertices, &self.indices, self.index_count)
    }

    pub(crate) fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind
    }

    fn upload(&self, queue: &wgpu::Queue) {
        if !self.parts.is_empty() {
            queue.write_buffer(&self.parts_buffer, 0, bytemuck::cast_slice(&self.parts));
        }
    }

    /// Replace geometry for deformation/UV animation without rebuilding material mip chains,
    /// part transforms, visibility, or bindings. Vertex part indices keep their existing meaning.
    pub fn set_mesh(&mut self, device: &wgpu::Device, mesh: &MeshData) {
        self.vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertices"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        self.indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("indices"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.index_count = mesh.indices.len() as u32;
    }

    /// Update sRGB base colors and metallic/roughness while retaining pose, visibility and
    /// material-plane flags. A count mismatch leaves the complete asset unchanged.
    pub fn set_part_materials(
        &mut self,
        queue: &wgpu::Queue,
        materials: &[PartMaterial],
    ) -> Result<(), String> {
        if materials.len() != self.parts.len() {
            return Err(format!(
                "Part material count mismatch: expected {}, got {}",
                self.parts.len(),
                materials.len()
            ));
        }
        for (part, material) in self.parts.iter_mut().zip(materials) {
            let c = material.base_color;
            part.color = [
                srgb_to_linear(c[0]),
                srgb_to_linear(c[1]),
                srgb_to_linear(c[2]),
                c[3],
            ];
            part.pbr[0] = material.metallic;
            part.pbr[1] = material.roughness;
        }
        self.upload(queue);
        Ok(())
    }

    pub fn set_part_highlights(
        &mut self,
        queue: &wgpu::Queue,
        colors: &[[f32; 4]],
    ) -> Result<(), String> {
        if colors.len() != self.parts.len() {
            return Err(format!(
                "Part highlight count mismatch: expected {}, got {}",
                self.parts.len(),
                colors.len()
            ));
        }
        for (part, color) in self.parts.iter_mut().zip(colors) {
            part.highlight = [
                srgb_to_linear(color[0].clamp(0.0, 1.0)),
                srgb_to_linear(color[1].clamp(0.0, 1.0)),
                srgb_to_linear(color[2].clamp(0.0, 1.0)),
                color[3].clamp(0.0, 1.0),
            ];
        }
        self.upload(queue);
        Ok(())
    }

    /// Part matrices, indexed like `AssetInfo::parts`.
    pub fn set_pose(&mut self, queue: &wgpu::Queue, pose: &[Mat4]) {
        for (part, m) in self.parts.iter_mut().zip(pose) {
            part.model = m.to_cols_array_2d();
        }
        self.upload(queue);
    }

    /// Runs `draw` with `pose` applied, then leaves the stored pose for later renders.
    pub fn with_pose(&self, queue: &wgpu::Queue, pose: &[Mat4], draw: impl FnOnce(&Self)) {
        let mut parts = self.parts.clone();
        for (part, m) in parts.iter_mut().zip(pose) {
            part.model = m.to_cols_array_2d();
        }
        if !parts.is_empty() {
            queue.write_buffer(&self.parts_buffer, 0, bytemuck::cast_slice(&parts));
        }
        draw(self);
        self.upload(queue);
    }

    pub fn set_visible(&mut self, queue: &wgpu::Queue, visible: &[bool]) {
        for (part, &v) in self.parts.iter_mut().zip(visible) {
            part.pbr[3] = if v { 1.0 } else { 0.0 };
        }
        self.upload(queue);
    }

    /// Shades every part with the material layers (base colour, rough/metal/height/ao, normal),
    /// or with the CAD colours when `None`. The layers are copied into a mip chain: call
    /// `refresh_material` after they change.
    pub fn set_material(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &SceneRenderer,
        planes: Option<[&wgpu::TextureView; 3]>,
    ) {
        let old = self.planes.take();
        self.planes = planes.map(|source| Planes::new(device, queue, &scene.mips, source, old));
        let d = &scene.dummy.view;
        let views = match &self.planes {
            Some(p) => [&p.views[0], &p.views[1], &p.views[2]],
            None => [d; 3],
        };
        self.bind = Self::bind(device, scene, &self.parts_buffer, views);
        let on = if planes.is_some() { 1.0 } else { 0.0 };
        for part in &mut self.parts {
            part.pbr[2] = on;
        }
        self.upload(queue);
    }

    /// Rebuilds the material's mip chain from the layers `set_material` was given.
    pub fn refresh_material(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if let Some(planes) = &self.planes {
            planes.build(device, queue);
        }
    }
}
