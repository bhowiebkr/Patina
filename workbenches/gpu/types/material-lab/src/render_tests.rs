use super::*;
use forge_gpu::Target;

#[test]
#[ignore = "Needs the material GPU adapter"]
fn display_blit_keeps_srgb_bytes_and_image_orientation() {
    let gpu = gpu().unwrap();
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let source = Target::new(
        &gpu.device,
        "display test",
        [3, 2],
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        1,
    );
    let rgba: Vec<u8> = [
        [0, 0, 0, 255],
        [255, 255, 255, 255],
        [128, 64, 32, 255],
        [10, 190, 80, 255],
        [230, 40, 110, 255],
        [2, 90, 255, 255],
    ]
    .concat();
    gpu.queue.write_texture(
        source.texture.as_image_copy(),
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(12),
            rows_per_image: Some(2),
        },
        wgpu::Extent3d {
            width: 3,
            height: 2,
            depth_or_array_layers: 1,
        },
    );
    for format in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ] {
        let blit = Blit::new(gpu, format, &source.view).unwrap();
        let out = Target::new(
            &gpu.device,
            "display output",
            [3, 2],
            format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            1,
        );
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &out.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&blit.pipeline);
            pass.set_bind_group(0, &blit.bind, &[]);
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([encoder.finish()]);
        let pixels =
            forge_gpu::read_texture(&gpu.device, &gpu.queue, &out.texture, [3, 2], 1, 4).unwrap();
        assert!(
            pixels.iter().zip(&rgba).all(|(&a, &b)| a.abs_diff(b) <= 1),
            "{format:?}: {pixels:?}"
        );
    }
    assert!(patina_gpu::block_on(scope.pop()).is_none());
}
