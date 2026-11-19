use material_lab::{BakeTarget, Mask, Material, Operator, bake, export, object, render::Look};
use patina_gpu::gpu;
use std::sync::Arc;

#[test]
#[ignore = "Needs a GPU; rebakes the original sphere at 2048, compares maps, renders, and exports"]
fn object_bake_material_render_and_export() {
    let gpu = gpu().unwrap();
    let scope = gpu
        .device
        .push_error_scope(patina_gpu::wgpu::ErrorFilter::Validation);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../forge-runtime/assets/ball/asset.json");
    let geometry = object::load(&path).unwrap();
    assert_eq!(geometry.mesh.indices.len() / 3, 82228);
    assert_eq!(geometry.info.parts.len(), 1);
    assert!(bake(&geometry, 123, false).is_err());
    let start = std::time::Instant::now();
    let target = BakeTarget(Some(bake(&geometry, 128, true).unwrap()));
    let context = &target.0.as_ref().unwrap().0;
    assert!(context.maps.baked);
    assert!(!context.cached_maps);
    assert_eq!(context.maps.size, 2048);
    let reference = forge_render::MeshMaps::bake(
        &gpu.device,
        &gpu.queue,
        &context.ball,
        &geometry.info,
        &geometry.mesh,
        2048,
        true,
    )
    .unwrap();
    for (actual, expected) in [
        (&context.maps.normal, &reference.normal),
        (&context.maps.extras, &reference.extras),
    ] {
        assert_eq!(
            forge_gpu::read_texture(&gpu.device, &gpu.queue, &actual.texture, [2048; 2], 1, 8)
                .unwrap(),
            forge_gpu::read_texture(&gpu.device, &gpu.queue, &expected.texture, [2048; 2], 1, 8)
                .unwrap(),
            "Node bake must match Surface's direct baker"
        );
    }
    drop(reference);
    let mut surface = Operator::new("surface");
    let ao = surface
        .run_in(&target, &[("fact", 0.0), ("gain", 1.0)], &[])
        .unwrap();
    let bytes =
        forge_gpu::read_texture(&gpu.device, &gpu.queue, &ao.planes.texture, [128; 2], 1, 16)
            .unwrap();
    let occupied = bytes
        .as_chunks::<16>()
        .0
        .iter()
        .filter(|p| f32::from_le_bytes(p[..4].try_into().unwrap()) > 0.01)
        .count();
    assert!(
        occupied > 100,
        "traced enclosure map is neutral: {occupied}"
    );
    let mask = Mask(Some(ao));
    let mut layer = Operator::new("layer");
    layer
        .gradient("[2,0,0.45,0.19,0.07,1,1,0.75,0.35,0.13,1]")
        .unwrap();
    let params = [("metallic", 1.0), ("rough_low", 0.25), ("rough_high", 0.65)];
    let image = layer
        .run_in(&target, &params, &[None, None, mask.0.as_ref(), None])
        .unwrap();
    let cached = layer
        .run_in(&target, &params, &[None, None, mask.0.as_ref(), None])
        .unwrap();
    assert!(Arc::ptr_eq(&image.planes, &cached.planes));
    let mut finish = Operator::new("finish");
    let image = finish
        .run(&[("depth", 0.01), ("occlusion", 0.6)], &[Some(&image)])
        .unwrap();
    assert!(Arc::ptr_eq(&image.target, target.0.as_ref().unwrap()));
    let material = Material(Some(image));
    let mut look = Look::new([320, 320]).unwrap();
    look.render(&material, [320, 320]).unwrap();
    let rgba = forge_gpu::read_texture(
        &gpu.device,
        &gpu.queue,
        &look.target.color.texture,
        [320; 2],
        1,
        4,
    )
    .unwrap();
    let visible = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] > 50 && p[1] > 20)
        .count();
    if let Some(path) = std::env::var_os("PATINA_MATERIAL_PROBE") {
        for channel in [0, 1, 2, 3, 5, 6] {
            look.light.channel = channel;
            look.dirty = true;
            look.render(&material, [320, 320]).unwrap();
            let data = forge_gpu::read_texture(
                &gpu.device,
                &gpu.queue,
                &look.target.color.texture,
                [320; 2],
                1,
                4,
            )
            .unwrap();
            let path = std::path::PathBuf::from(&path)
                .with_file_name(format!("sphere-channel-{channel}.png"));
            let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), 320, 320);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&data)
                .unwrap();
        }
    }
    assert!(
        visible > 1000,
        "object preview has only {visible} lit pixels"
    );
    let files = export::files(&material).unwrap();
    assert_eq!(files.len(), 9);
    for (name, bytes) in &files {
        if !name.ends_with(".png") {
            continue;
        }
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        assert_eq!([reader.info().width, reader.info().height], [128, 128]);
        assert_eq!(
            reader.info().bit_depth,
            if name == "Height.png" {
                png::BitDepth::Sixteen
            } else {
                png::BitDepth::Eight
            }
        );
        let mut data = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut data).unwrap();
    }
    let second = BakeTarget(Some(bake(&geometry, 256, false).unwrap()));
    assert!(
        layer
            .run_in(&second, &params, &[None, None, mask.0.as_ref(), None])
            .is_err()
    );
    let next = surface.run_in(&second, &[("fact", 0.0)], &[]).unwrap();
    assert_eq!(next.planes.size, 256);
    assert!(!Arc::ptr_eq(&next.target, target.0.as_ref().unwrap()));
    if let Some(path) = std::env::var_os("PATINA_MATERIAL_PROBE") {
        let path = std::path::PathBuf::from(path).with_file_name("sphere-bake.png");
        let mut encoder = png::Encoder::new(std::fs::File::create(&path).unwrap(), 320, 320);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&rgba)
            .unwrap();
        let zip = path.with_extension("zip");
        if zip.exists() {
            std::fs::remove_file(&zip).unwrap();
        }
        assert_eq!(export::write(&material, &zip).unwrap(), 8);
    }
    assert!(patina_gpu::block_on(scope.pop()).is_none());
    println!(
        "Sphere workflow: 82228 triangles, 2048 maps match direct Surface bake, {occupied} enclosure texels, 8 PBR PNGs; {:.3} s",
        start.elapsed().as_secs_f64()
    );
}
