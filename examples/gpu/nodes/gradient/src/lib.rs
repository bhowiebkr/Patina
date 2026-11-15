use patina_gpu::{GpuImage, gpu, link, wgpu};
use patina_sdk::{asset, node};

/// Makes a gray gradient from left to right on the GPU, with the shaders in `assets/`. Set
/// `invert` to go from white to black.
#[node]
pub fn gradient(width: u32, height: u32, invert: bool) -> Result<GpuImage, String> {
    let gpu = gpu()?;
    let wgsl = link(&asset!("gradient.wesl"), &[("invert", invert)])?;
    let module = gpu.shader("gradient.wesl", &wgsl)?;
    let image = GpuImage::new(gpu, width, height);
    let pipeline = gpu.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("gradient"),
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let view = image.texture.create_view(&Default::default());
    let group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }],
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(image.width().div_ceil(8), image.height().div_ceil(8), 1);
    }
    gpu.queue.submit([encoder.finish()]);
    Ok(image)
}
