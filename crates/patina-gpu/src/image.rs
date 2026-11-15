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

impl patina_sdk::Viewer for GpuImage {
    fn text(&self, out: &mut patina_sdk::Text<'_>) -> fmt::Result {
        write!(out, "GpuImage {}x{}", self.width(), self.height())
    }
}
