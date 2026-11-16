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

impl GpuImage {
    /// A copy that fits in `max` x `max` pixels, for a node GUI or a viewer (10.2).
    // shortcut: it reads the full image and shrinks it on the CPU, shrink on the GPU when large images get slow.
    pub fn preview(&self, gpu: &Gpu, max: u32) -> Result<([u32; 2], Vec<u8>), String> {
        Ok(shrink(&self.read(gpu)?, [self.width(), self.height()], max))
    }
}

/// Shrinks RGBA pixels to fit in `max` x `max`, with the mean of each block. It keeps the
/// aspect ratio and does not enlarge.
pub(crate) fn shrink(pixels: &[u8], size: [u32; 2], max: u32) -> ([u32; 2], Vec<u8>) {
    let [w, h] = size;
    let scale = w.max(h).div_ceil(max.max(1)).max(1);
    let (to_w, to_h) = (w.div_ceil(scale), h.div_ceil(scale));
    let mut out = Vec::with_capacity((to_w * to_h * 4) as usize);
    for y in 0..to_h {
        for x in 0..to_w {
            let (mut sum, mut count) = ([0u32; 4], 0);
            for sy in y * scale..((y + 1) * scale).min(h) {
                for sx in x * scale..((x + 1) * scale).min(w) {
                    let i = ((sy * w + sx) * 4) as usize;
                    for (s, &p) in sum.iter_mut().zip(&pixels[i..i + 4]) {
                        *s += u32::from(p);
                    }
                    count += 1;
                }
            }
            out.extend(sum.map(|s| (s / count) as u8));
        }
    }
    ([to_w, to_h], out)
}

impl patina_sdk::Viewer for GpuImage {
    fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
        write!(out, "GpuImage {}x{}", self.width(), self.height())
    }
}
