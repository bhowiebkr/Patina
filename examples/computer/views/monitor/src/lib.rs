use frame::{Frame, PALETTE, SIZE};
use patina_gpu::{ViewportSurface, block_on, gpu, wgpu};
use patina_sdk::{Graph, Ui, Viewport, view};

/// The color around the screen.
const BEZEL: wgpu::Color = wgpu::Color { r: 0.035, g: 0.035, b: 0.04, a: 1.0 };

const EXTENT: wgpu::Extent3d = wgpu::Extent3d { width: SIZE as u32, height: SIZE as u32, depth_or_array_layers: 1 };

/// Draws each pixel of the screen as a lit cell with a dark gap, and makes the corners darker.
/// The 32 is `SIZE`.
const SHADER: &str = "
@group(0) @binding(0) var screen: texture_2d<f32>;

struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> }

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return Out(vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0), uv);
}

@fragment
fn fs(in: Out) -> @location(0) vec4<f32> {
    let p = in.uv * 32.0;
    let lit = max(textureLoad(screen, min(vec2<u32>(p), vec2<u32>(31u)), 0).rgb, vec3<f32>(0.04));
    let f = fract(p);
    let cell = step(0.1, f.x) * step(0.1, f.y);
    let d = in.uv - 0.5;
    return vec4<f32>(lit * mix(0.3, 1.0, cell) * (1.0 - dot(d, d) * 0.9), 1.0);
}
";

/// The screen of the computer on the real GPU (spec 15.8). The view draws the frame that goes into
/// the first **screen** node of the graph.
#[view]
pub struct Monitor {
    surface: ViewportSurface,
    pipe: Option<Pipe>,
    /// The frame in the texture. None after a new texture.
    shown: Option<Frame>,
    pixels: [u8; SIZE * SIZE * 4],
    error: String,
}

/// The GPU objects of the screen, for one texture format of the window.
struct Pipe {
    format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    texture: wgpu::Texture,
    group: wgpu::BindGroup,
}

impl Monitor {
    pub fn init() -> Self {
        Monitor { surface: ViewportSurface::default(), pipe: None, shown: None, pixels: [0; SIZE * SIZE * 4], error: String::new() }
    }

    pub fn ui(&mut self, ui: &mut Ui, graph: &mut Graph) {
        let viewport = ui.viewport("Monitor", 320.0);
        // The arrow keys, W, A, S, D and Space play the game, as on the pad of the joystick.
        pad::press(&viewport.keys);
        let frame = input(graph);
        let [x, y, side] = square(&viewport);
        let Monitor { surface, pipe, shown, pixels, error } = self;
        let mut failed = None;
        let drawn = surface.draw(&viewport, BEZEL, |pass, format| {
            let Some(frame) = frame else { return };
            let pipe = match &*pipe {
                Some(p) if p.format == format => p,
                _ => match Pipe::new(format) {
                    Ok(p) => {
                        *shown = None;
                        &*pipe.insert(p)
                    }
                    Err(e) => {
                        failed = Some(e);
                        return;
                    }
                },
            };
            if shown.as_ref() != Some(frame) {
                for (rgba, &color) in pixels.as_chunks_mut::<4>().0.iter_mut().zip(&frame.0) {
                    let [r, g, b] = PALETTE[usize::from(color % 16)];
                    *rgba = [r, g, b, 255];
                }
                let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(EXTENT.width * 4), rows_per_image: None };
                match gpu() {
                    Ok(gpu) => gpu.queue.write_texture(pipe.texture.as_image_copy(), &pixels[..], layout, EXTENT),
                    Err(e) => {
                        failed = Some(e);
                        return;
                    }
                }
                *shown = Some(frame.clone());
            }
            pass.set_viewport(x, y, side, side, 0.0, 1.0);
            pass.set_pipeline(&pipe.pipeline);
            pass.set_bind_group(0, &pipe.group, &[]);
            pass.draw(0..3, 0..1);
        });
        *error = drawn.err().or(failed).unwrap_or_default();
        match frame {
            Some(frame) => ui.label(&format!("{frame:?}. Click the monitor and push the arrows to play.")),
            None => ui.label("No frame yet. Connect a frame to the screen node, and run the graph."),
        }
        if !error.is_empty() {
            ui.label(error);
        }
    }
}

impl Pipe {
    fn new(format: wgpu::TextureFormat) -> Result<Pipe, String> {
        let gpu = gpu()?;
        let module = gpu.shader("monitor", SHADER)?;
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let target = Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL });
        let pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("monitor"),
            layout: None,
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs"), compilation_options: Default::default(), targets: &[target] }),
            multiview_mask: None,
            cache: None,
        });
        // The colors of the palette are sRGB. A sRGB window encodes them again.
        let pixel_format = if format.is_srgb() { wgpu::TextureFormat::Rgba8UnormSrgb } else { wgpu::TextureFormat::Rgba8Unorm };
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("monitor pixels"),
            size: EXTENT,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: pixel_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("monitor"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&texture.create_view(&Default::default())) }],
        });
        match block_on(scope.pop()) {
            Some(error) => Err(format!("monitor: {error}")),
            None => Ok(Pipe { format, pipeline, texture, group }),
        }
    }
}

/// The frame on the input of the first screen node. None before the first frame.
fn input(graph: &Graph) -> Option<&Frame> {
    let screen = graph.nodes().find(|id| graph.folder(id) == Some("nodes/screen"))?;
    let (from, _) = graph.wires().find(|(_, to)| to.split_once('.') == Some((screen, "frame")))?;
    let (id, key) = from.split_once('.')?;
    graph.value::<Frame>(id, key)
}

/// The square of the screen in the area that `ViewportSurface::draw` uses: x, y and the side.
/// When the area is large enough, each pixel of the screen gets the same number of pixels.
fn square(viewport: &Viewport) -> [f32; 3] {
    let ([ww, wh], [x, y, w, h]) = (viewport.window_size, viewport.rect);
    let (w, h) = (w.min(ww.saturating_sub(x)), h.min(wh.saturating_sub(y)));
    let size = SIZE as u32;
    let side = match w.min(h) {
        side if side >= size => side / size * size,
        side => side,
    };
    [((w - side) / 2) as f32, ((h - side) / 2) as f32, side as f32]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_is_a_centered_square_inside_the_area_in_the_window() {
        let at = |window_size, rect| square(&Viewport { window: 1, window_size, rect, ..Viewport::default() });
        assert_eq!(at([1000, 800], [10, 20, 400, 300]), [56.0, 6.0, 288.0]);
        // The window cuts the area: 100 x 50 shows.
        assert_eq!(at([110, 70], [10, 20, 400, 300]), [34.0, 9.0, 32.0]);
        assert_eq!(at([1000, 800], [0, 0, 20, 30]), [0.0, 5.0, 20.0]);
    }

    /// The shader and the pipeline are valid, on a computer with a GPU.
    #[test]
    fn the_pipeline_builds_for_the_window_formats() {
        if gpu().is_err() {
            return;
        }
        for format in [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8UnormSrgb] {
            Pipe::new(format).unwrap();
        }
    }
}
