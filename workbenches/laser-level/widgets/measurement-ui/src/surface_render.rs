//! Depth-tested, antialiased height-field drawing through Patina's native viewport.
use crate::surface_camera::Camera;
use measurement::surface::{Mesh, Vertex};
use patina_gpu::wgpu::util::DeviceExt;
use patina_gpu::{Gpu, ViewportSurface, gpu, wgpu};
use patina_sdk::Viewport;

const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const ATTRIBUTES: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x3];

pub struct Renderer {
    surface: ViewportSurface,
    triangles: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    faces: Option<(wgpu::Buffer, u32)>,
    grid: Option<(wgpu::Buffer, u32)>,
    targets: Option<Targets>,
    blit: Option<Blit>,
    drawn: Option<Camera>,
}
struct Targets {
    size: [u32; 2],
    samples: wgpu::TextureView,
    picture: wgpu::TextureView,
    depth: wgpu::TextureView,
}
struct Blit {
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
}

impl Renderer {
    pub fn new() -> Result<Self, String> {
        let g = gpu()?;
        let d = &g.device;
        let module = g.shader("measured surface", include_str!("surface.wesl"))?;
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("measured surface"),
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
        let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |topology| {
            d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("measured surface"),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 36,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &ATTRIBUTES,
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(COLOR.into())],
                }),
                primitive: wgpu::PrimitiveState { topology, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState { count: 4, ..Default::default() },
                multiview_mask: None,
                cache: None,
            })
        };
        let uniform = d.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }],
        });
        Ok(Self {
            surface: ViewportSurface::default(),
            triangles: pipeline(wgpu::PrimitiveTopology::TriangleList),
            lines: pipeline(wgpu::PrimitiveTopology::LineList),
            uniform,
            bind,
            faces: None,
            grid: None,
            targets: None,
            blit: None,
            drawn: None,
        })
    }
    pub fn upload(&mut self, mesh: &Mesh) -> Result<(), String> {
        let g = gpu()?;
        self.faces = buffer(g, &mesh.faces);
        let [lo, hi] = mesh.bounds;
        let mut grid = Vec::new();
        let vertex = |p: [f32; 3], color: [f32; 3]| [p[0], p[1], p[2], 0., 0., 0., color[0], color[1], color[2]];
        for i in 0..=4 {
            let t = i as f32 / 4.;
            let x = lo[0] + (hi[0] - lo[0]) * t;
            let y = lo[1] + (hi[1] - lo[1]) * t;
            for p in [[x, lo[1], lo[2]], [x, hi[1], lo[2]], [lo[0], y, lo[2]], [hi[0], y, lo[2]]] {
                grid.push(vertex(p, [0.24; 3]));
            }
        }
        for (end, color) in [
            ([hi[0], lo[1], lo[2]], [0.85, 0.4, 0.4]),
            ([lo[0], hi[1], lo[2]], [0.4, 0.8, 0.5]),
            ([lo[0], lo[1], hi[2]], [0.7, 0.6, 0.9]),
        ] {
            grid.extend([vertex(lo, color), vertex(end, color)]);
        }
        if mesh.faces.is_empty() {
            let radius = ((hi[0] - lo[0]).max(hi[1] - lo[1]) * 0.004).max(0.02);
            for p in &mesh.points {
                for axis in 0..2 {
                    let mut a = *p;
                    let mut b = *p;
                    a[axis] -= radius;
                    b[axis] += radius;
                    grid.extend([vertex(a, [0.9, 0.75, 0.4]), vertex(b, [0.9, 0.75, 0.4])]);
                }
            }
        }
        self.grid = buffer(g, &grid);
        self.drawn = None;
        Ok(())
    }
    pub fn draw(&mut self, viewport: &Viewport, camera: &Camera) -> Result<(), String> {
        if viewport.window == 0 {
            return Ok(());
        }
        if viewport.rect[2] == 0 || viewport.rect[3] == 0 {
            return self.surface.draw(viewport, wgpu::Color::BLACK, |_, _| {});
        }
        let g = gpu()?;
        let size = [viewport.rect[2], viewport.rect[3]].map(|v| v.min(4096));
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            self.targets = Some(Targets::new(g, size));
            self.blit = None;
            self.drawn = None;
        }
        let t = self.targets.as_ref().unwrap();
        if self.drawn != Some(*camera) {
            self.render(g, t, camera);
            self.drawn = Some(*camera);
        }
        let mut error = None;
        self.surface.draw(viewport, wgpu::Color::BLACK, |pass, format| {
            if self.blit.as_ref().is_none_or(|b| b.format != format) {
                match Blit::new(g, format, &t.picture) {
                    Ok(b) => self.blit = Some(b),
                    Err(e) => error = Some(e),
                }
            }
            if let Some(b) = &self.blit {
                pass.set_pipeline(&b.pipeline);
                pass.set_bind_group(0, &b.bind, &[]);
                pass.draw(0..3, 0..1);
            }
        })?;
        error.map_or(Ok(()), Err)
    }
    fn render(&self, g: &Gpu, t: &Targets, camera: &Camera) {
        let floats =
            camera.matrix(t.size[0] as f32 / t.size[1] as f32).into_iter().flatten().chain(camera.light()).chain([0.]);
        let mut bytes = [0_u8; 80];
        for (out, f) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(floats) {
            *out = f.to_le_bytes();
        }
        g.queue.write_buffer(&self.uniform, 0, &bytes);
        let mut encoder = g.device.create_command_encoder(&Default::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("measured surface"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &t.samples,
                depth_slice: None,
                resolve_target: Some(&t.picture),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.006, g: 0.006, b: 0.006, a: 1. }),
                    store: wgpu::StoreOp::Discard,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &t.depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.bind, &[]);
        for (pipeline, buffer) in [(&self.triangles, &self.faces), (&self.lines, &self.grid)] {
            if let Some((v, count)) = buffer {
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, v.slice(..));
                pass.draw(0..*count, 0..1);
            }
        }
        drop(pass);
        g.queue.submit([encoder.finish()]);
    }
}
impl Targets {
    fn new(g: &Gpu, size: [u32; 2]) -> Self {
        let texture = |format, sample_count, usage| {
            g.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        Self {
            size,
            samples: texture(COLOR, 4, attach),
            picture: texture(COLOR, 1, attach | wgpu::TextureUsages::TEXTURE_BINDING),
            depth: texture(DEPTH, 4, attach),
        }
    }
}
impl Blit {
    fn new(g: &Gpu, format: wgpu::TextureFormat, picture: &wgpu::TextureView) -> Result<Self, String> {
        let module = g.shader("surface blit", include_str!("blit.wesl"))?;
        let pipeline = g.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(if format.is_srgb() { "linear" } else { "encoded" }),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let bind = g.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(picture) }],
        });
        Ok(Self { format, pipeline, bind })
    }
}
fn buffer(g: &Gpu, vertices: &[Vertex]) -> Option<(wgpu::Buffer, u32)> {
    if vertices.is_empty() {
        return None;
    }
    let bytes: Vec<_> = vertices.as_flattened().iter().flat_map(|v| v.to_le_bytes()).collect();
    Some((
        g.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: &bytes,
            usage: wgpu::BufferUsages::VERTEX,
        }),
        vertices.len() as u32,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn depth_pipeline_draws_offscreen_without_a_window() {
        let g = gpu().unwrap();
        let scope = g.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut r = Renderer::new().unwrap();
        let mesh = Mesh {
            faces: vec![
                [0., 0., 0., 0., 0., 1., 1., 0., 0.],
                [1., 0., 0., 0., 0., 1., 0., 1., 0.],
                [0., 1., 0., 0., 0., 1., 0., 0., 1.],
            ],
            bounds: [[0.; 3], [1.; 3]],
            ..Default::default()
        };
        r.upload(&mesh).unwrap();
        let targets = Targets::new(g, [256, 256]);
        let mut c = Camera::default();
        c.frame(mesh.bounds);
        r.render(g, &targets, &c);
        assert!(patina_gpu::block_on(scope.pop()).is_none());
    }
}
