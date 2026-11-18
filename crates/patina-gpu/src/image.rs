//! An RGBA image that stays on the GPU between GPU nodes (10.2).

use std::fmt::{self, Write};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, MutexGuard, PoisonError};

use crate::Gpu;

/// An RGBA image with 8 bits for each channel, on the GPU. A shader reads it as a texture or a
/// storage texture, and writes it as a storage texture.
#[derive(Clone, Debug)]
pub struct GpuImage {
    pub texture: wgpu::Texture,
}

impl GpuImage {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    /// A new image. Its pixels are 0.
    pub fn new(gpu: &Gpu, width: u32, height: u32) -> GpuImage {
        let usage = wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GpuImage"),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage,
            view_formats: &[],
        });
        GpuImage { texture }
    }

    pub fn width(&self) -> u32 {
        self.texture.width()
    }

    pub fn height(&self) -> u32 {
        self.texture.height()
    }

    /// Copies the pixels to the CPU: 4 bytes for each pixel, row after row. Use it only for a
    /// CPU node or a viewer (10.2), because it waits for the GPU.
    pub fn read(&self, gpu: &Gpu) -> Result<Vec<u8>, String> {
        let (width, height) = (self.width(), self.height());
        let row = width * 4;
        let stride = row.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let size = u64::from(stride) * u64::from(height);
        let spare = take(&mut spares(gpu).reads, |b| b.size() == size);
        let buffer = spare.unwrap_or_else(|| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("GpuImage read"),
                size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
        });
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: Some(height) },
            },
            self.texture.size(),
        );
        let index = gpu.queue.submit([encoder.finish()]);
        // 0: waiting, 1: mapped, 2: failed.
        let state = Arc::new(AtomicU8::new(0));
        let mapped = state.clone();
        buffer.map_async(wgpu::MapMode::Read, .., move |r| mapped.store(if r.is_ok() { 1 } else { 2 }, Ordering::SeqCst));
        gpu.device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: None }).map_err(|e| e.to_string())?;
        // The device is shared: a poll on another thread can run the callback after this poll returns.
        while state.load(Ordering::SeqCst) == 0 {
            std::thread::yield_now();
            gpu.device.poll(wgpu::PollType::Poll).map_err(|e| e.to_string())?;
        }
        if state.load(Ordering::SeqCst) != 1 {
            return Err("Patina could not read the image from the GPU".to_owned());
        }
        let mut pixels = Vec::with_capacity((row * height) as usize);
        for line in buffer.get_mapped_range(..).map_err(|e| e.to_string())?.chunks(stride as usize) {
            pixels.extend_from_slice(&line[..row as usize]);
        }
        buffer.unmap();
        give(&mut spares(gpu).reads, buffer);
        Ok(pixels)
    }
}

/// The most objects of each kind that the previews and the reads keep for the next call.
const SPARES: usize = 4;

/// The objects of the last previews and reads, so that a call of the same size in each frame
/// makes no new GPU objects (10.2).
#[derive(Default)]
pub(crate) struct Spares {
    reads: Vec<wgpu::Buffer>,
    shrinks: Vec<Shrink>,
}

fn spares(gpu: &Gpu) -> MutexGuard<'_, Spares> {
    gpu.spares.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Takes an object that `fits` out of `list`. Another thread can use the other objects meanwhile.
fn take<T>(list: &mut Vec<T>, fits: impl Fn(&T) -> bool) -> Option<T> {
    list.iter().position(fits).map(|i| list.swap_remove(i))
}

/// Keeps `item` for the next call, and drops an old object when `list` is full.
fn give<T>(list: &mut Vec<T>, item: T) {
    if list.len() >= SPARES {
        list.remove(0);
    }
    list.push(item);
}

/// The small image, the scale and the bind group of the previews of `source`. The bind group
/// keeps `source`, so a spare keeps one old image until the next preview of its size.
struct Shrink {
    small: GpuImage,
    scale: u32,
    uniform: wgpu::Buffer,
    source: wgpu::Texture,
    group: wgpu::BindGroup,
}

impl Shrink {
    fn new(gpu: &Gpu, pipeline: &wgpu::ComputePipeline, source: &wgpu::Texture, scale: u32, [width, height]: [u32; 2]) -> Shrink {
        let small = GpuImage::new(gpu, width, height);
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GpuImage shrink scale"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut bytes = [0u8; 16];
        bytes[..4].copy_from_slice(&scale.to_le_bytes());
        gpu.queue.write_buffer(&uniform, 0, &bytes);
        let group = bind(gpu, pipeline, source, &small, &uniform);
        Shrink { small, scale, uniform, source: source.clone(), group }
    }
}

fn bind(gpu: &Gpu, pipeline: &wgpu::ComputePipeline, source: &wgpu::Texture, small: &GpuImage, uniform: &wgpu::Buffer) -> wgpu::BindGroup {
    let (from, to) = (source.create_view(&Default::default()), small.texture.create_view(&Default::default()));
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&from) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&to) },
            wgpu::BindGroupEntry { binding: 2, resource: uniform.as_entire_binding() },
        ],
    })
}

/// Writes the mean of each block of `scale` x `scale` pixels of `source` to one pixel of `small`.
/// The integer mean gives the same bytes on each GPU.
const SHRINK: &str = "
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var small: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> scale: vec4u;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3u) {
    if any(id.xy >= textureDimensions(small)) {
        return;
    }
    let start = id.xy * scale.x;
    let end = min(start + scale.x, textureDimensions(source));
    var sum = vec4u(0u);
    for (var y = start.y; y < end.y; y++) {
        for (var x = start.x; x < end.x; x++) {
            sum += vec4u(round(textureLoad(source, vec2u(x, y), 0) * 255.0));
        }
    }
    let count = (end.x - start.x) * (end.y - start.y);
    textureStore(small, id.xy, vec4f(sum / count) / 255.0);
}
";

impl GpuImage {
    /// A copy that fits in `max` x `max` pixels, for a node GUI or a viewer (10.2). Each pixel is
    /// the mean of a block. It keeps the aspect ratio and does not enlarge. The GPU shrinks the
    /// image, so only the small copy comes to the CPU.
    pub fn preview(&self, gpu: &Gpu, max: u32) -> Result<([u32; 2], Vec<u8>), String> {
        let [width, height] = [self.width(), self.height()];
        let scale = width.max(height).div_ceil(max.max(1)).max(1);
        if scale == 1 {
            return Ok(([width, height], self.read(gpu)?));
        }
        let size = [width.div_ceil(scale), height.div_ceil(scale)];
        let pipeline = gpu
            .shrink
            .get_or_init(|| {
                let module = gpu.shader("GpuImage shrink", SHRINK)?;
                let desc = wgpu::ComputePipelineDescriptor {
                    label: Some("GpuImage shrink"),
                    layout: None,
                    module: &module,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                };
                Ok(gpu.device.create_compute_pipeline(&desc))
            })
            .as_ref()
            .map_err(Clone::clone)?;
        let spare = take(&mut spares(gpu).shrinks, |s| s.scale == scale && [s.small.width(), s.small.height()] == size);
        let mut shrink = spare.unwrap_or_else(|| Shrink::new(gpu, pipeline, &self.texture, scale, size));
        if shrink.source != self.texture {
            shrink.group = bind(gpu, pipeline, &self.texture, &shrink.small, &shrink.uniform);
            shrink.source = self.texture.clone();
        }
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &shrink.group, &[]);
            pass.dispatch_workgroups(size[0].div_ceil(8), size[1].div_ceil(8), 1);
        }
        gpu.queue.submit([encoder.finish()]);
        let pixels = shrink.small.read(gpu)?;
        give(&mut spares(gpu).shrinks, shrink);
        Ok((size, pixels))
    }
}

impl patina_sdk::Viewer for GpuImage {
    fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
        write!(out, "{} x {}", self.width(), self.height())
    }

    fn thumbnail(&self, out: &mut patina_sdk::Thumbnail<'_>) {
        let Ok(([width, height], pixels)) = crate::gpu().and_then(|gpu| self.preview(gpu, patina_sdk::Thumbnail::MAX)) else { return };
        out.pixels(width, height).copy_from_slice(&pixels);
    }
}
