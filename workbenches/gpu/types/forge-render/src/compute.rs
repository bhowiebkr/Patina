//! Bind group layout entries, pipelines and dispatches for the bake's compute passes.

fn entry(binding: u32, ty: wgpu::BindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty,
        count: None,
    }
}

pub fn uniform(binding: u32, size: usize, dynamic: bool) -> wgpu::BindGroupLayoutEntry {
    entry(
        binding,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: wgpu::BufferSize::new(size as u64),
        },
    )
}

pub fn storage(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    entry(
        binding,
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
    )
}

pub fn texture(binding: u32) -> wgpu::BindGroupLayoutEntry {
    entry(
        binding,
        wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
    )
}

pub fn storage_texture(binding: u32, format: wgpu::TextureFormat) -> wgpu::BindGroupLayoutEntry {
    entry(
        binding,
        wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
    )
}

pub fn pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    bgl: &wgpu::BindGroupLayout,
    entry_point: &str,
) -> wgpu::ComputePipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(entry_point),
        bind_group_layouts: &[Some(bgl)],
        immediate_size: 0,
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry_point),
        layout: Some(&layout),
        module,
        entry_point: Some(entry_point),
        compilation_options: Default::default(),
        cache: None,
    })
}

/// Runs `pipeline` over a `size` x `size` map (8x8 workgroups) in a submit of its own.
pub fn dispatch(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &wgpu::ComputePipeline,
    bind: &wgpu::BindGroup,
    offsets: &[u32],
    size: u32,
) {
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mesh maps"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("mesh maps"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind, offsets);
        pass.dispatch_workgroups(size.div_ceil(8), size.div_ceil(8), 1);
    }
    queue.submit([encoder.finish()]);
}

pub fn texture_entry(binding: u32, target: &forge_gpu::Target) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::TextureView(&target.view),
    }
}

pub fn buffer_entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

/// One parameter block per batch in a uniform buffer, each at its own dynamic offset; returns
/// the buffer and the offsets.
pub fn batches<T: bytemuck::Pod>(
    device: &wgpu::Device,
    label: &str,
    blocks: &[T],
) -> (wgpu::Buffer, Vec<u32>) {
    let size = size_of::<T>();
    let align = device.limits().min_uniform_buffer_offset_alignment as usize;
    let stride = size.next_multiple_of(align);
    let mut bytes = vec![0u8; stride * blocks.len().max(1)];
    for (chunk, block) in bytes.chunks_exact_mut(stride).zip(blocks) {
        chunk[..size].copy_from_slice(bytemuck::bytes_of(block));
    }
    let offsets = (0..blocks.len()).map(|i| (i * stride) as u32).collect();
    (forge_gpu::uniform_buffer(device, label, &bytes), offsets)
}

/// Binds one `size`-byte block of a buffer from `batches`.
pub fn block_entry(binding: u32, buffer: &wgpu::Buffer, size: usize) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer,
            offset: 0,
            size: wgpu::BufferSize::new(size as u64),
        }),
    }
}

/// A storage buffer holding `contents`, which must not be empty.
pub fn storage_buffer(device: &wgpu::Device, label: &str, contents: &[u8]) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage: wgpu::BufferUsages::STORAGE,
    })
}
