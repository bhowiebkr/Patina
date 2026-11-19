use wgpu::util::DeviceExt;

/// A 2D texture with its default view.
#[derive(Clone)]
pub struct Target {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub size: [u32; 2],
}

impl Target {
    pub fn new(
        device: &wgpu::Device,
        label: &str,
        size: [u32; 2],
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        samples: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size[0].max(1),
                height: size[1].max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: samples,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[format.remove_srgb_suffix()],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            size,
        }
    }

    /// A view that reads the stored bytes without sRGB decoding, for egui, which treats every
    /// texture as gamma-encoded: an sRGB target registered as is shows far too dark.
    pub fn display_view(&self) -> wgpu::TextureView {
        self.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.texture.format().remove_srgb_suffix()),
            ..Default::default()
        })
    }

    /// A 1x1 texture filled with `rgba` (texture-binding use), for unbound inputs.
    pub fn solid(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        format: wgpu::TextureFormat,
        bytes: &[u8],
    ) -> Self {
        let t = Self::new(
            device,
            label,
            [1, 1],
            format,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            1,
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &t.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes.len() as u32),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        t
    }
}

pub fn uniform_buffer(device: &wgpu::Device, label: &str, bytes: &[u8]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytes,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    })
}

/// Copies `layers` layers of `texture` (`size` texels, `texel` bytes each) back to the CPU,
/// rows packed tight, layer after layer, row 0 first. Blocks until the copy is done.
pub fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    size: [u32; 2],
    layers: u32,
    texel: u32,
) -> Result<Vec<u8>, String> {
    read_texture_region(
        device,
        queue,
        texture,
        wgpu::Origin3d::ZERO,
        size,
        layers,
        texel,
    )
}

/// Reads a bounded texture region, allowing large exports without one oversized buffer.
pub fn read_texture_region(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    origin: wgpu::Origin3d,
    size: [u32; 2],
    layers: u32,
    texel: u32,
) -> Result<Vec<u8>, String> {
    let (tight, row) = region_layout(
        texture,
        origin,
        size,
        layers,
        texel,
        device.limits().max_buffer_size,
    )?;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(row) * u64::from(size[1]) * u64::from(layers),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            origin,
            ..texture.as_image_copy()
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(size[1]),
            },
        },
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: layers,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let fail = |e: &dyn std::fmt::Display| format!("readback: {e}");
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| fail(&e))?;
    rx.recv().map_err(|e| fail(&e))?.map_err(|e| fail(&e))?;
    let bytes = slice.get_mapped_range().map_err(|e| fail(&e))?;
    let out = bytes
        .chunks_exact(row as usize)
        .flat_map(|r| &r[..tight as usize])
        .copied()
        .collect();
    drop(bytes);
    buffer.unmap();
    Ok(out)
}

/// Uploads tight rows of `texel`-byte texels into a single-layer `texture`.
pub fn write_texture(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    size: [u32; 2],
    texel: u32,
    bytes: &[u8],
) {
    queue.write_texture(
        texture.as_image_copy(),
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size[0] * texel),
            rows_per_image: Some(size[1]),
        },
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );
}

/// Browser readback yields to the WebGPU event loop instead of blocking it.
#[cfg(target_arch = "wasm32")]
pub async fn read_texture_async(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    size: [u32; 2],
    layers: u32,
    texel: u32,
) -> Result<Vec<u8>, String> {
    read_texture_region_async(
        device,
        queue,
        texture,
        wgpu::Origin3d::ZERO,
        size,
        layers,
        texel,
    )
    .await
}

#[cfg(target_arch = "wasm32")]
pub async fn read_texture_region_async(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    origin: wgpu::Origin3d,
    size: [u32; 2],
    layers: u32,
    texel: u32,
) -> Result<Vec<u8>, String> {
    let (tight, row) = region_layout(
        texture,
        origin,
        size,
        layers,
        texel,
        device.limits().max_buffer_size,
    )?;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("browser PNG readback"),
        size: u64::from(row) * u64::from(size[1]) * u64::from(layers),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            origin,
            ..texture.as_image_copy()
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(size[1]),
            },
        },
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: layers,
        },
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = futures_channel::oneshot::channel();
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    rx.await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let bytes = slice.get_mapped_range().map_err(|e| e.to_string())?;
    let out = bytes
        .chunks_exact(row as usize)
        .flat_map(|r| &r[..tight as usize])
        .copied()
        .collect();
    drop(bytes);
    buffer.unmap();
    Ok(out)
}

fn region_layout(
    texture: &wgpu::Texture,
    origin: wgpu::Origin3d,
    size: [u32; 2],
    layers: u32,
    texel: u32,
    limit: u64,
) -> Result<(u32, u32), String> {
    if size.contains(&0)
        || layers == 0
        || texel == 0
        || origin
            .x
            .checked_add(size[0])
            .is_none_or(|x| x > texture.width())
        || origin
            .y
            .checked_add(size[1])
            .is_none_or(|y| y > texture.height())
        || origin
            .z
            .checked_add(layers)
            .is_none_or(|z| z > texture.depth_or_array_layers())
    {
        return Err("Readback region is outside the texture".into());
    }
    let tight = size[0]
        .checked_mul(texel)
        .ok_or("Readback row size overflow")?;
    let row = tight
        .checked_next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        .ok_or("Readback alignment overflow")?;
    if u64::from(row) * u64::from(size[1]) * u64::from(layers) > limit {
        return Err("Readback exceeds the device buffer limit; use smaller regions".into());
    }
    Ok((tight, row))
}
