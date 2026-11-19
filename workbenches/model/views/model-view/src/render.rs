//! Draws the mesh or the scene, the grid and the wireframe into the textures of the view, with a
//! depth texture, and copies the picture into the viewport.

use geometry::{IDENTITY, Mesh, Place, Scene};
use patina_gpu::wgpu::util::DeviceExt;
use patina_gpu::{Gpu, ViewportSurface, gpu, wgpu};
use patina_sdk::Viewport;

use crate::camera::Camera;

const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;
/// The largest side of the picture, in pixels.
const MAX_SIDE: u32 = 4096;
/// The bytes of the uniform `Scene` of `scene.wgsl`: the matrix, the light and the pixel size.
const UNIFORM: usize = 96;

// The colors of the GUI guide, in sRGB.
const BACKGROUND: u32 = 0x18_1818;
/// The toon look has a lighter background, so the ink shows around dark parts.
const TOON_BACKGROUND: u32 = 0x30_3030;
const GRID: u32 = 0x45_4545;
const X_AXIS: u32 = 0xED_7B78;
const Y_AXIS: u32 = 0x75_C884;
const EDGE: u32 = 0x17_1511;
const PLAIN: u32 = 0xBB_BBB5;
/// The part colors: the light colors of the node categories.
const PARTS: [u32; 7] = [0xE3_B869, 0x79_D4C8, 0xDE_8FBA, 0xD9_A17A, 0xB9_9BDF, 0xF2_C4B8, 0x75_C884];
/// Two faces that meet with a smaller dot product of their normals get an ink line in the toon look.
const CREASE: f32 = 0.75;

/// A corner of a triangle or a line: the position, the normal and the linear color. A line has
/// a zero normal.
type Vertex = [f32; 9];

const ATTRIBUTES: [wgpu::VertexAttribute; 3] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 12, shader_location: 1 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 24, shader_location: 2 },
];

/// An instance: the columns of its frame, as `geometry::Matrix`.
const INSTANCE: [wgpu::VertexAttribute; 4] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 3 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 12, shader_location: 4 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 24, shader_location: 5 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 36, shader_location: 6 },
];

/// What the view shows: the mesh of a solid, or a scene.
pub enum Content {
    Mesh(Mesh),
    Scene(Scene),
}

/// What the view draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub grid: bool,
    pub wireframe: bool,
    pub parts: bool,
    pub toon: bool,
}

/// The GPU state of one view.
pub struct Renderer {
    surface: ViewportSurface,
    triangles: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    outlines: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    batches: Vec<Batch>,
    /// One instance that does not move the mesh, for the grid and the meshes with one place.
    identity: wgpu::Buffer,
    grid: Option<(wgpu::Buffer, u32)>,
    targets: Option<Targets>,
    blit: Option<Blit>,
    /// The camera and the options of the picture, or `None` when the picture is old.
    drawn: Option<(Camera, Options)>,
}

/// The faces, the edges and the crease lines of one mesh, and its instances, or `None` for the
/// identity instance.
struct Batch {
    faces: Option<(wgpu::Buffer, u32)>,
    edges: Option<(wgpu::Buffer, u32)>,
    creases: Option<(wgpu::Buffer, u32)>,
    instances: Option<(wgpu::Buffer, u32)>,
}

/// The textures of the picture: the samples, the picture after the resolve, and the depth.
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
    pub fn new() -> Result<Renderer, String> {
        let gpu = gpu()?;
        let device = &gpu.device;
        let module = gpu.shader("model view", include_str!("scene.wgsl"))?;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("model view"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("model view"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        // An outline pipeline draws the back faces with `vs_outline` and `fs_outline`.
        let pipeline = |topology, bias, outline: bool| {
            let entry = |name: &'static str| if outline { [name, "_outline"].concat() } else { name.to_string() };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("model view"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(&entry("vs")),
                    compilation_options: Default::default(),
                    buffers: &[
                        Some(wgpu::VertexBufferLayout {
                            array_stride: size_of::<Vertex>() as u64,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &ATTRIBUTES,
                        }),
                        Some(wgpu::VertexBufferLayout {
                            array_stride: size_of::<geometry::Matrix>() as u64,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &INSTANCE,
                        }),
                    ],
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    cull_mode: (topology == wgpu::PrimitiveTopology::TriangleList).then_some(if outline { wgpu::Face::Front } else { wgpu::Face::Back }),
                    ..Default::default()
                },
                // The faces go back a little, so the lines on them show.
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias,
                }),
                multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(&entry("fs")),
                    compilation_options: Default::default(),
                    targets: &[Some(COLOR.into())],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let face_bias = wgpu::DepthBiasState { constant: 2, slope_scale: 1.5, clamp: 0.0 };
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("model view"),
            size: UNIFORM as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("model view"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() }],
        });
        Ok(Renderer {
            surface: ViewportSurface::default(),
            triangles: pipeline(wgpu::PrimitiveTopology::TriangleList, face_bias, false),
            lines: pipeline(wgpu::PrimitiveTopology::LineList, Default::default(), false),
            outlines: pipeline(wgpu::PrimitiveTopology::TriangleList, face_bias, true),
            uniform,
            bind,
            batches: Vec::new(),
            identity: buffer(gpu, &IDENTITY, IDENTITY.len()).ok_or("no identity buffer")?.0,
            grid: None,
            targets: None,
            blit: None,
            drawn: None,
        })
    }

    /// Sends the triangles, the edges and a grid under the content to the GPU. A scene item with
    /// more than one place gets instances, and the other items go into one batch.
    pub fn upload(&mut self, content: &Content, parts: bool) -> Result<(), String> {
        let gpu = gpu()?;
        self.batches.clear();
        let reach = match content {
            Content::Mesh(mesh) => {
                let (faces, edges) = vertices(mesh, parts);
                let mut lines = Vec::new();
                creases(mesh, &Place { matrix: IDENTITY, group: 0 }, &mut lines);
                self.batches.push(Batch::new(gpu, [&faces, &edges, &lines], &[]));
                mesh.positions.iter().map(|p| p[0].abs().max(p[1].abs())).fold(1.0_f32, f32::max)
            }
            Content::Scene(scene) => {
                let (mut faces, mut edges, mut lines) = (Vec::new(), Vec::new(), Vec::new());
                for item in scene.items() {
                    let color = linear(if parts { item.color } else { PLAIN });
                    if let [place] = item.places[..] {
                        add(&item.mesh, &place, |_| color, &mut faces, &mut edges);
                        creases(&item.mesh, &place, &mut lines);
                        continue;
                    }
                    let (mut f, mut e, mut c) = (Vec::new(), Vec::new(), Vec::new());
                    let identity = Place { matrix: IDENTITY, group: 0 };
                    add(&item.mesh, &identity, |_| color, &mut f, &mut e);
                    creases(&item.mesh, &identity, &mut c);
                    let instances: Vec<f32> = item.places.iter().flat_map(|p| p.matrix).collect();
                    self.batches.push(Batch::new(gpu, [&f, &e, &c], &instances));
                }
                self.batches.push(Batch::new(gpu, [&faces, &edges, &lines], &[]));
                let [low, high] = scene.bounds();
                [low[0], low[1], high[0], high[1]].into_iter().filter(|v| v.is_finite()).fold(1.0_f32, |r, v| r.max(v.abs() as f32))
            }
        };
        self.grid = buffer(gpu, grid(reach).as_flattened(), size_of::<Vertex>() / 4);
        self.drawn = None;
        Ok(())
    }

    /// Draws the picture again when it is old, and copies it into the viewport.
    pub fn draw(&mut self, viewport: &Viewport, camera: &Camera, options: Options) -> Result<(), String> {
        if viewport.window == 0 || viewport.rect[2] == 0 || viewport.rect[3] == 0 {
            return Ok(());
        }
        let gpu = gpu()?;
        let size = [viewport.rect[2], viewport.rect[3]].map(|v| v.min(MAX_SIDE));
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            self.targets = Some(Targets::new(gpu, size));
            (self.blit, self.drawn) = (None, None);
        }
        let Some(targets) = &self.targets else { return Ok(()) };
        if self.drawn != Some((*camera, options)) {
            self.render(gpu, targets, camera, options);
            self.drawn = Some((*camera, options));
        }
        let mut error = None;
        self.surface.draw(viewport, wgpu::Color::BLACK, |pass, format| {
            if self.blit.as_ref().is_none_or(|b| b.format != format) {
                match Blit::new(gpu, format, &targets.picture) {
                    Ok(blit) => self.blit = Some(blit),
                    Err(e) => error = Some(e),
                }
            }
            if let Some(blit) = &self.blit {
                pass.set_pipeline(&blit.pipeline);
                pass.set_bind_group(0, &blit.bind, &[]);
                pass.draw(0..3, 0..1);
            }
        })?;
        error.map_or(Ok(()), Err)
    }

    fn render(&self, gpu: &Gpu, targets: &Targets, camera: &Camera, options: Options) {
        let [w, h] = targets.size;
        let light = camera.light();
        let toon = if options.toon { 1.0 } else { 0.0 };
        let pixel = [2.0 / w as f32, 2.0 / h as f32, 0.0, 0.0];
        let floats = camera.matrix(w as f32 / h as f32).into_iter().flatten().chain(light).chain([toon]).chain(pixel);
        let mut bytes = [0_u8; UNIFORM];
        for (chunk, v) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(floats) {
            *chunk = v.to_le_bytes();
        }
        gpu.queue.write_buffer(&self.uniform, 0, &bytes);
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let [r, g, b] = linear(if options.toon { TOON_BACKGROUND } else { BACKGROUND }).map(f64::from);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("model view"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.samples,
                depth_slice: None,
                resolve_target: Some(&targets.picture),
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }), store: wgpu::StoreOp::Discard },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.bind, &[]);
        pass.set_vertex_buffer(1, self.identity.slice(..));
        if let Some((grid, count)) = self.grid.as_ref().filter(|_| options.grid) {
            pass.set_pipeline(&self.lines);
            pass.set_vertex_buffer(0, grid.slice(..));
            pass.draw(0..*count, 0..1);
        }
        for batch in &self.batches {
            let (instances, count) = batch.instances.as_ref().map_or((&self.identity, 1), |(b, n)| (b, *n));
            pass.set_vertex_buffer(1, instances.slice(..));
            let draws = [
                (true, &self.triangles, &batch.faces),
                (options.toon, &self.outlines, &batch.faces),
                (options.toon && !options.wireframe, &self.lines, &batch.creases),
                (options.wireframe, &self.lines, &batch.edges),
            ];
            for (pipeline, (buffer, vertices)) in draws.into_iter().filter_map(|(on, p, b)| Some((p, b.as_ref().filter(|_| on)?))) {
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..*vertices, 0..count);
            }
        }
        drop(pass);
        gpu.queue.submit([encoder.finish()]);
    }
}

impl Targets {
    fn new(gpu: &Gpu, [width, height]: [u32; 2]) -> Targets {
        let texture = |format, samples, usage| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("model view"),
                    size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        Targets {
            size: [width, height],
            samples: texture(COLOR, SAMPLES, attach),
            picture: texture(COLOR, 1, attach | wgpu::TextureUsages::TEXTURE_BINDING),
            depth: texture(DEPTH, SAMPLES, attach),
        }
    }
}

impl Blit {
    fn new(gpu: &Gpu, format: wgpu::TextureFormat, source: &wgpu::TextureView) -> Result<Blit, String> {
        let module = gpu.shader("model view blit", include_str!("blit.wgsl"))?;
        let pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("model view blit"),
            layout: None,
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
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
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("model view blit"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(source) }],
        });
        Ok(Blit { format, pipeline, bind })
    }
}

impl Batch {
    /// A batch of the faces, the edges and the creases.
    fn new(gpu: &Gpu, vertices: [&[Vertex]; 3], instances: &[f32]) -> Batch {
        let [faces, edges, creases] = vertices.map(|v| buffer(gpu, v.as_flattened(), size_of::<Vertex>() / 4));
        Batch { faces, edges, creases, instances: buffer(gpu, instances, IDENTITY.len()) }
    }
}

/// A vertex buffer of `floats` and its count of items of `per` floats, or `None` for no items.
fn buffer(gpu: &Gpu, floats: &[f32], per: usize) -> Option<(wgpu::Buffer, u32)> {
    let count = u32::try_from(floats.len() / per).ok().filter(|&n| n > 0)?;
    let bytes: Vec<u8> = floats.iter().flat_map(|v| v.to_le_bytes()).collect();
    let usage = wgpu::BufferUsages::VERTEX;
    Some((gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("model view"), contents: &bytes, usage }), count))
}

/// The triangles with their light colors, and the three edges of each triangle as lines.
fn vertices(mesh: &Mesh, parts: bool) -> (Vec<Vertex>, Vec<Vertex>) {
    let (palette, plain) = (PARTS.map(linear), linear(PLAIN));
    let mut faces = Vec::with_capacity(mesh.positions.len());
    let mut edges = Vec::with_capacity(mesh.positions.len() * 2);
    let color = |part: u32| if parts { palette[part as usize % palette.len()] } else { plain };
    add(mesh, &Place { matrix: IDENTITY, group: 0 }, color, &mut faces, &mut edges);
    (faces, edges)
}

/// Adds the triangles of `mesh` at `place` to `faces` with the color of their part, and the
/// three edges of each triangle as lines to `edges`.
fn add(mesh: &Mesh, place: &Place, color: impl Fn(u32) -> [f32; 3], faces: &mut Vec<Vertex>, edges: &mut Vec<Vertex>) {
    let edge = linear(EDGE);
    let corners = mesh.positions.as_chunks::<3>().0.iter().zip(mesh.normals.as_chunks::<3>().0);
    for ((triangle, normals), part) in corners.zip(&mesh.parts) {
        let (color, triangle) = (color(*part), triangle.map(|p| place.point(p)));
        for (p, n) in triangle.iter().zip(normals) {
            faces.push(vertex(*p, place.normal(*n), color));
        }
        for i in 0..3 {
            edges.push(vertex(triangle[i], [0.0; 3], edge));
            edges.push(vertex(triangle[(i + 1) % 3], [0.0; 3], edge));
        }
    }
}

/// Adds the ink lines of the toon look to `lines`: each edge where two faces meet at a sharp
/// angle, and each edge with one face. The corners of a mesh are exact copies, so their bits match.
fn creases(mesh: &Mesh, place: &Place, lines: &mut Vec<Vertex>) {
    type Key = ([u32; 3], [u32; 3]);
    let ink = linear(EDGE);
    let bits = |p: [f32; 3]| p.map(f32::to_bits);
    // The normal of the first face of each edge, the count of faces, and true for a sharp angle.
    let mut sides: std::collections::HashMap<Key, ([f32; 3], u32, bool)> = std::collections::HashMap::with_capacity(mesh.positions.len());
    for &[a, b, c] in mesh.positions.as_chunks::<3>().0 {
        let (u, v) = (std::array::from_fn::<f32, 3, _>(|i| b[i] - a[i]), std::array::from_fn::<f32, 3, _>(|i| c[i] - a[i]));
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let length = n.iter().map(|x| x * x).sum::<f32>().sqrt();
        if length < 1e-12 {
            continue;
        }
        let n = n.map(|x| x / length);
        for (p, q) in [(a, b), (b, c), (c, a)] {
            let key = if bits(p) < bits(q) { (bits(p), bits(q)) } else { (bits(q), bits(p)) };
            let (first, faces, sharp) = sides.entry(key).or_insert((n, 0, false));
            *faces += 1;
            *sharp |= (0..3).map(|i| first[i] * n[i]).sum::<f32>() < CREASE;
        }
    }
    for ((p, q), (_, faces, sharp)) in sides {
        if faces != 2 || sharp {
            lines.extend([p, q].map(|p| vertex(place.point(p.map(f32::from_bits)), [0.0; 3], ink)));
        }
    }
}

/// The lines of a grid on the XY plane, with a step of a power of ten, to `reach` and past it.
fn grid(reach: f32) -> Vec<Vertex> {
    let step = 10_f32.powf(reach.log10().floor());
    let half = (reach * 1.25 / step).ceil().clamp(1.0, 50.0) as i32;
    let extent = half as f32 * step;
    let (gray, x_axis, y_axis) = (linear(GRID), linear(X_AXIS), linear(Y_AXIS));
    let mut lines = Vec::with_capacity((half as usize * 2 + 1) * 4);
    for i in -half..=half {
        let at = i as f32 * step;
        let (along_x, along_y) = if i == 0 { (x_axis, y_axis) } else { (gray, gray) };
        lines.extend([vertex([-extent, at, 0.0], [0.0; 3], along_x), vertex([extent, at, 0.0], [0.0; 3], along_x)]);
        lines.extend([vertex([at, -extent, 0.0], [0.0; 3], along_y), vertex([at, extent, 0.0], [0.0; 3], along_y)]);
    }
    lines
}

fn vertex(p: [f32; 3], n: [f32; 3], c: [f32; 3]) -> Vertex {
    [p[0], p[1], p[2], n[0], n[1], n[2], c[0], c[1], c[2]]
}

/// The linear color of an sRGB color 0xRRGGBB.
fn linear(rgb: u32) -> [f32; 3] {
    [16, 8, 0].map(|shift| {
        let c = ((rgb >> shift) & 0xff) as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_triangle_gives_three_corners_and_three_edges() {
        let mesh = geometry::Solid::cube(2.0, 2.0, 2.0).union(&geometry::Solid::cube(1.0, 1.0, 1.0).translate([0.0, 0.0, 1.5]).as_part()).mesh();
        let (faces, edges) = vertices(&mesh, true);
        assert_eq!((faces.len(), edges.len()), (mesh.triangles() * 3, mesh.triangles() * 6));
        // Two parts give two colors, and no part color gives one color.
        let colors = |faces: &[Vertex]| faces.iter().map(|v| v[6..].iter().map(|c| c.to_bits()).collect::<Vec<_>>()).collect::<std::collections::BTreeSet<_>>().len();
        assert_eq!(colors(&faces), 2);
        assert_eq!(colors(&vertices(&mesh, false).0), 1);
        // The grid reaches past the mesh, and the lines through the origin have the axis colors.
        let lines = grid(1.0);
        assert!(lines.iter().all(|v| v[2] == 0.0) && lines.iter().any(|v| v[0] >= 1.25));
        assert!(lines.iter().any(|v| v[6..] == linear(X_AXIS)));
        assert_eq!(linear(0xff_ffff), [1.0; 3]);
    }

    #[test]
    fn a_box_has_twelve_creases_and_a_round_side_has_none() {
        let count = |solid: geometry::Solid| {
            let mut lines = Vec::new();
            creases(&solid.mesh(), &Place { matrix: IDENTITY, group: 0 }, &mut lines);
            lines.len() / 2
        };
        assert_eq!(count(geometry::Solid::cube(1.0, 2.0, 3.0)), 12);
        // A cylinder of 32 sides has creases only on the two rims.
        assert_eq!(count(geometry::Solid::cylinder(1.0, 0.5, 0.5, 32)), 64);
    }

    #[test]
    fn the_shaders_are_valid() {
        for (name, source) in [("scene.wgsl", include_str!("scene.wgsl")), ("blit.wgsl", include_str!("blit.wgsl"))] {
            patina_gpu::link_source(name, source, &[]).unwrap();
        }
    }
}
