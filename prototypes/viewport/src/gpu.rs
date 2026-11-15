//! The shared GPU context and the test scene (spec 10.2). A compute pass changes one cell buffer
//! each frame, and all viewports draw that buffer. Nothing is uploaded again for a viewport.

use std::future::Future;
use std::pin::pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

pub const GRID: u32 = 96;
pub const CELLS: u32 = GRID * GRID;
/// The format of the copy path: the page reads the bytes as RGBA.
pub const COPY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// The size of the camera uniform: yaw, pitch, distance and aspect.
pub const CAMERA_BYTES: u64 = 16;

// shortcut: the file has no WESL features, so wgpu reads it as WGSL; link it with the wesl crate when it imports.
const SHADER: &str = include_str!("scene.wesl");

/// Runs a future of wgpu to its end. The native futures of wgpu are ready at the first poll.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        std::thread::yield_now();
    }
}

pub struct Gpu {
    pub adapter: String,
    pub handle: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Set by the device-lost callback.
    pub lost: Arc<AtomicBool>,
    pub scene: Scene,
}

impl Gpu {
    pub fn new(instance: &wgpu::Instance, low_power: bool) -> Result<Self, String> {
        let power_preference =
            if low_power { wgpu::PowerPreference::LowPower } else { wgpu::PowerPreference::HighPerformance };
        let options = wgpu::RequestAdapterOptions { power_preference, ..Default::default() };
        let adapter = block_on(instance.request_adapter(&options)).map_err(|e| e.to_string())?;
        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| e.to_string())?;
        let lost = Arc::new(AtomicBool::new(false));
        let flag = lost.clone();
        device.set_device_lost_callback(move |reason, message| {
            eprintln!("viewport: the GPU device is lost ({reason:?}): {message}");
            flag.store(true, Ordering::SeqCst);
        });
        // The default handler stops the process. A lost device gives errors until the loop makes a new one.
        device.on_uncaptured_error(Arc::new(|error| eprintln!("viewport: GPU error: {error}")));
        let scene = Scene::new(&device);
        let info = adapter.get_info();
        Ok(Self { adapter: format!("{} ({:?})", info.name, info.backend), handle: adapter, device, queue, lost, scene })
    }

    pub fn is_lost(&self) -> bool {
        self.lost.load(Ordering::SeqCst)
    }
}

pub struct Scene {
    cells: wgpu::Buffer,
    globals: wgpu::Buffer,
    compute: wgpu::ComputePipeline,
    compute_group: wgpu::BindGroup,
    view_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    module: wgpu::ShaderModule,
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
}

fn entry(binding: u32, visibility: wgpu::ShaderStages, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

impl Scene {
    fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let cells = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cells"),
            size: u64::from(CELLS) * 16,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compute"),
            entries: &[
                entry(0, wgpu::ShaderStages::COMPUTE, wgpu::BufferBindingType::Uniform),
                entry(1, wgpu::ShaderStages::COMPUTE, wgpu::BufferBindingType::Storage { read_only: false }),
            ],
        });
        let compute_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compute"),
            layout: &compute_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: cells.as_entire_binding() },
            ],
        });
        let compute = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("animate"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("compute"),
                bind_group_layouts: &[Some(&compute_layout)],
                immediate_size: 0,
            })),
            module: &module,
            entry_point: Some("animate"),
            compilation_options: Default::default(),
            cache: None,
        });
        let view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("view"),
            entries: &[
                entry(2, wgpu::ShaderStages::VERTEX, wgpu::BufferBindingType::Uniform),
                entry(3, wgpu::ShaderStages::VERTEX, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("view"),
            bind_group_layouts: &[Some(&view_layout)],
            immediate_size: 0,
        });
        Self { cells, globals, compute, compute_group, view_layout, pipeline_layout, module, pipelines: Vec::new() }
    }

    /// Makes the bind group of one viewport: its camera and the shared cells.
    pub fn view_group(&self, device: &wgpu::Device, camera: &wgpu::Buffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("view"),
            layout: &self.view_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 2, resource: camera.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.cells.as_entire_binding() },
            ],
        })
    }

    /// Makes the render pipeline for a target format, one time for each format.
    pub fn prepare(&mut self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        if self.pipelines.iter().any(|(f, _)| *f == format) {
            return;
        }
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.push((format, pipeline));
    }

    /// Records the compute pass that changes the cells for this frame.
    pub fn animate(&self, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, time: f32) {
        let mut globals = [0u8; 16];
        globals[..4].copy_from_slice(&time.to_le_bytes());
        globals[4..8].copy_from_slice(&GRID.to_le_bytes());
        queue.write_buffer(&self.globals, 0, &globals);
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&self.compute);
        pass.set_bind_group(0, &self.compute_group, &[]);
        pass.dispatch_workgroups(CELLS.div_ceil(64), 1, 1);
    }

    /// Draws the cells with the camera of `group`. Call `prepare` for the format first.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, format: wgpu::TextureFormat, group: &wgpu::BindGroup) {
        let Some((_, pipeline)) = self.pipelines.iter().find(|(f, _)| *f == format) else { return };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..14, 0..CELLS);
    }
}

pub fn depth_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_on_returns_a_ready_value() {
        assert_eq!(block_on(async { 7 }), 7);
    }
}
