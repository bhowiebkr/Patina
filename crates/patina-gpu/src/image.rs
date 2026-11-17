//! An RGBA image that stays on the GPU between GPU nodes (10.2).

use std::fmt::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

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
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GpuImage read"),
            size: u64::from(stride) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
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
        let view = buffer.get_mapped_range(..).map_err(|e| e.to_string())?;
        let mut pixels = Vec::with_capacity((row * height) as usize);
        for line in view.chunks(stride as usize) {
            pixels.extend_from_slice(&line[..row as usize]);
        }
        Ok(pixels)
    }
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
        let small = GpuImage::new(gpu, width.div_ceil(scale), height.div_ceil(scale));
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
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GpuImage shrink scale"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut bytes = [0u8; 16];
        bytes[..4].copy_from_slice(&scale.to_le_bytes());
        gpu.queue.write_buffer(&uniform, 0, &bytes);
        let (from, to) = (self.texture.create_view(&Default::default()), small.texture.create_view(&Default::default()));
        let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&from) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&to) },
                wgpu::BindGroupEntry { binding: 2, resource: uniform.as_entire_binding() },
            ],
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(small.width().div_ceil(8), small.height().div_ceil(8), 1);
        }
        gpu.queue.submit([encoder.finish()]);
        Ok(([small.width(), small.height()], small.read(gpu)?))
    }
}

impl patina_sdk::Viewer for GpuImage {
    fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
        write!(out, "GpuImage {}x{}", self.width(), self.height())
    }

    fn thumbnail(&self, out: &mut patina_sdk::Thumbnail<'_>) {
        let Ok(([width, height], pixels)) = crate::gpu().and_then(|gpu| self.preview(gpu, patina_sdk::Thumbnail::MAX)) else { return };
        out.pixels(width, height).copy_from_slice(&pixels);
    }
}
