use super::*;

#[test]
fn object_guard_protects_brush_eraser_and_mirrored_dabs_with_the_original_kernel() {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&crate::device_descriptor(&adapter))).unwrap();
    let texture = |name: &str, size, values: &[[f32; 4]]| {
        let target = Target::new(
            &device,
            name,
            size,
            wgpu::TextureFormat::Rgba32Float,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            1,
        );
        forge_gpu::write_texture(
            &queue,
            &target.texture,
            size,
            16,
            bytemuck::cast_slice(values),
        );
        target
    };
    let position = texture("positions", [2, 2], &[[0.1, 0.0, 0.0, 1.0]; 4]);
    let normal = texture("normals", [2, 2], &[[0.0, 1.0, 0.0, 0.0]; 4]);
    let extras = texture(
        "two neighboring object IDs",
        [2, 2],
        &[
            [0.0; 4],
            [0.0, 0.0, 1.0, 0.0],
            [0.0; 4],
            [0.0, 0.0, 1.0, 0.0],
        ],
    );
    let hit = texture("hit", [1, 1], &[[0.0, 0.0, 0.0, 1.0]]);
    let hit_normal = texture("hit object zero", [1, 1], &[[0.0, 1.0, 0.0, 0.0]]);
    let mask = new_mask(&device, 2);
    let mut stamper = Stamper::new(&device, include_str!("../../kernels/stamp.wgsl")).unwrap();
    let mut brush = Brush {
        tool: Tool::Brush,
        radius: 100.0,
        pixel: 1.0,
        strength: 1.0,
        hardness: 1.0,
        mirror: None,
        object_lock: true,
    };
    let write = |values: &[f32]| {
        forge_gpu::write_texture(
            &queue,
            &mask.texture,
            [2, 2],
            4,
            bytemuck::cast_slice(values),
        )
    };
    let read = || {
        forge_gpu::read_texture(&device, &queue, &mask.texture, [2, 2], 1, 4)
            .unwrap()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect::<Vec<_>>()
    };
    write(&[0.1, 0.2, 0.3, 0.4]);
    stamper.stamp(
        &device,
        &queue,
        &brush,
        [&hit.view, &hit_normal.view],
        [&position.view, &normal.view, &extras.view],
        &mask,
    );
    assert_eq!(read(), [1.0, 0.2, 1.0, 0.4]);
    brush.object_lock = false;
    stamper.stamp(
        &device,
        &queue,
        &brush,
        [&hit.view, &hit_normal.view],
        [&position.view, &normal.view, &extras.view],
        &mask,
    );
    assert_eq!(read(), [1.0; 4]);
    brush.object_lock = true;
    brush.tool = Tool::Eraser;
    brush.mirror = Some((0, 0.0));
    write(&[0.8; 4]);
    stamper.stamp(
        &device,
        &queue,
        &brush,
        [&hit.view, &hit_normal.view],
        [&position.view, &normal.view, &extras.view],
        &mask,
    );
    assert_eq!(read(), [0.0, 0.8, 0.0, 0.8]);
    brush.tool = Tool::Fill;
    brush.object_lock = false;
    brush.strength = 0.0;
    write(&[0.8; 4]);
    stamper.stamp(
        &device,
        &queue,
        &brush,
        [&hit.view, &hit_normal.view],
        [&position.view, &normal.view, &extras.view],
        &mask,
    );
    assert_eq!(read(), [0.0, 0.8, 0.0, 0.8]);
    let miss = texture("miss", [1, 1], &[[0.0, 0.0, 0.0, -1.0]]);
    brush.tool = Tool::Brush;
    brush.object_lock = true;
    let before = read();
    stamper.stamp(
        &device,
        &queue,
        &brush,
        [&miss.view, &hit_normal.view],
        [&position.view, &normal.view, &extras.view],
        &mask,
    );
    assert_eq!(read(), before);
}
