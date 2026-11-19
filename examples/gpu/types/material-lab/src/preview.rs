//! Read a small preview asynchronously. The GUI never waits for GPU completion.
use patina_gpu::{Gpu, wgpu};
use std::sync::mpsc::{Receiver, TryRecvError};

struct Pending {
    buffer: wgpu::Buffer,
    done: Receiver<Result<(), wgpu::BufferAsyncError>>,
}
#[derive(Default)]
pub struct Thumbnail {
    pending: Option<Pending>,
    pub pixels: Vec<u8>,
    pub histogram: [u32; 32],
    pub error: String,
}
impl Thumbnail {
    pub fn request(&mut self, gpu: &Gpu, texture: &wgpu::Texture) {
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("material preview readback"),
            size: 128 * 128 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(512),
                    rows_per_image: Some(128),
                },
            },
            wgpu::Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([encoder.finish()]);
        let (send, done) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = send.send(result);
        });
        self.pending = Some(Pending { buffer, done });
    }
    pub fn poll(&mut self, gpu: &Gpu) {
        let Some(pending) = &self.pending else {
            return;
        };
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        match pending.done.try_recv() {
            Ok(Ok(())) => {
                let bytes = match pending.buffer.get_mapped_range(..) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        self.error = error.to_string();
                        pending.buffer.unmap();
                        self.pending = None;
                        return;
                    }
                };
                self.pixels.resize(256 * 128 * 4, 0);
                self.histogram.fill(0);
                for y in 0..128 {
                    self.pixels[y * 1024..y * 1024 + 512]
                        .copy_from_slice(&bytes[y * 512..(y + 1) * 512]);
                    for pixel in bytes[y * 512..(y + 1) * 512].as_chunks::<4>().0.iter() {
                        if pixel[0] != 28 || pixel[1] != 28 || pixel[2] != 28 {
                            self.histogram[(usize::from(pixel[0])
                                + usize::from(pixel[1])
                                + usize::from(pixel[2]))
                                * 31
                                / 765] += 1;
                        }
                    }
                }
                drop(bytes);
                pending.buffer.unmap();
                self.pending = None;
                self.error.clear();
            }
            Ok(Err(e)) => {
                self.error = e.to_string();
                self.pending = None;
            }
            Err(TryRecvError::Disconnected) => {
                self.error = "Preview readback stopped".into();
                self.pending = None;
            }
            Err(TryRecvError::Empty) => {}
        }
    }
}
