//! In-place animated geometry/material updates must retain the complete existing asset state.
use super::*;

#[test]
fn animated_updates_keep_pose_planes_visibility_and_reject_count_mismatch() {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let scene = SceneRenderer::new(&device, &queue).unwrap();
    let info = AssetInfo {
        parts: vec![forge_core::PartInfo::default(); 2],
        ..Default::default()
    };
    let mut mesh = MeshData {
        vertices: vec![Vertex::default(); 3],
        indices: vec![0, 1, 2],
    };
    let mut asset = GpuAsset::new(&device, &scene, &info, &mesh);
    let plane = Target::new(
        &device,
        "test material",
        [4; 2],
        PLANE,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        1,
    );
    asset.set_material(&device, &queue, &scene, Some([&plane.view; 3]));
    let bound = asset.bind.clone();
    let pose = [
        Mat4::from_translation(Vec3::X),
        Mat4::from_translation(Vec3::Y),
    ];
    asset.set_pose(&queue, &pose);
    asset.set_visible(&queue, &[true, false]);
    let highlights = [[1.0, 0.5, 0.0, 0.65], [0.0; 4]];
    asset.set_part_highlights(&queue, &highlights).unwrap();
    let before = bytemuck::cast_slice::<PartGpu, u8>(&asset.parts).to_vec();
    assert!(
        asset
            .set_part_materials(&queue, &[PartMaterial::default()])
            .is_err()
    );
    assert_eq!(before, bytemuck::cast_slice::<PartGpu, u8>(&asset.parts));
    assert!(asset.set_part_highlights(&queue, &[]).is_err());
    assert_eq!(before, bytemuck::cast_slice::<PartGpu, u8>(&asset.parts));
    let material = PartMaterial {
        base_color: [0.5, 0.25, 0.75, 0.4],
        metallic: 0.8,
        roughness: 0.2,
        ..Default::default()
    };
    asset.set_part_materials(&queue, &[material; 2]).unwrap();
    mesh.vertices[0].uv = [0.25, 0.75];
    mesh.vertices[0].position = [1., 2., 3.];
    mesh.indices.extend([2, 1, 0]);
    asset.set_mesh(&device, &mesh);
    assert_eq!(asset.index_count, 6);
    assert_eq!(asset.parts[0].model, pose[0].to_cols_array_2d());
    assert_eq!(asset.parts[1].model, pose[1].to_cols_array_2d());
    assert_eq!(asset.parts[0].pbr, [0.8, 0.2, 1., 1.]);
    assert_eq!(asset.parts[1].pbr, [0.8, 0.2, 1., 0.]);
    assert_eq!(
        asset.parts[0].color,
        [
            srgb_to_linear(0.5),
            srgb_to_linear(0.25),
            srgb_to_linear(0.75),
            0.4
        ]
    );
    assert_eq!(asset.bind, bound);
    assert!(asset.planes.is_some());
    assert_eq!(
        asset.parts[0].highlight,
        [1.0, srgb_to_linear(0.5), 0.0, 0.65]
    );
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
}

#[test]
fn highlights_affect_only_selected_pixels_and_preserve_picking() {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let scene = SceneRenderer::new(&device, &queue).unwrap();
    let info = AssetInfo {
        parts: vec![forge_core::PartInfo::default(); 2],
        ..Default::default()
    };
    let mut mesh = MeshData::default();
    for part in 0..2 {
        let left = -0.9 + part as f32;
        let start = mesh.vertices.len() as u32;
        for position in [
            [left, -0.5, 0.0],
            [left + 0.8, -0.5, 0.0],
            [left + 0.8, 0.5, 0.0],
            [left, 0.5, 0.0],
        ] {
            mesh.vertices.push(Vertex {
                position,
                normal: [0.0, 0.0, 1.0],
                part,
                ..Default::default()
            });
        }
        mesh.indices
            .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
    }
    let mut asset = GpuAsset::new(&device, &scene, &info, &mesh);
    let view = View::new(&device, &scene, [64; 2]);
    let mut camera = OrbitCamera::framing(Vec3::ZERO, 1.0);
    camera.yaw = 0.0;
    camera.pitch = 0.0;
    camera.distance = 4.0;
    let light = Light {
        channel: 1,
        ..Light::studio()
    };
    let render = |asset: &GpuAsset| {
        scene.render(&device, &queue, &view, &camera, &light, asset);
        forge_gpu::read_texture(&device, &queue, &view.color.texture, [64; 2], 1, 4).unwrap()
    };
    let before = render(&asset);
    asset
        .set_part_highlights(&queue, &[[0.0; 4], [1.0, 0.4, 0.0, 0.65]])
        .unwrap();
    let after = render(&asset);
    let left = (32 * 64 + 17) * 4;
    let right = (32 * 64 + 47) * 4;
    assert_eq!(before[left..left + 4], after[left..left + 4]);
    assert_ne!(before[right..right + 4], after[right..right + 4]);
    let picker = crate::Picker::new(&device, &scene).unwrap();
    assert!(
        picker
            .hit_test(&device, &queue, &camera, 1.0, [47.5 / 64.0, 0.5], &asset)
            .unwrap()
    );
    let picked =
        forge_gpu::read_texture(&device, &queue, &picker.normal.texture, [1; 2], 1, 16).unwrap();
    assert_eq!(f32::from_le_bytes(picked[12..16].try_into().unwrap()), 1.0);
    asset.set_part_highlights(&queue, &[[0.0; 4]; 2]).unwrap();
    assert_eq!(before, render(&asset));
}
