//! The same base color must shade differently when roughness or metallic changes.
use forge_nodes::NativePass;
use material_lab::{Material, Operator, render::Look, world};
use patina_gpu::gpu;

#[test]
#[ignore = "Needs a GPU; verifies the PBR renderer with independent material channels"]
fn roughness_and_metallic_change_shading_without_changing_base_color() {
    let gpu = gpu().unwrap();
    let world = &world().unwrap().0;
    let mut pass = NativePass::new(&world.engine, &gpu.device, "layer", 64).unwrap();
    let mut node = Operator::new("layer").node;
    node.params.insert("color".into(), vec![2.0, 0.0, 0.65, 0.29, 0.14, 1.0, 1.0, 0.65, 0.29, 0.14, 1.0]);
    let mut look = Look::new([256, 256]).unwrap();
    let mut colors = Vec::new();
    let mut shaded = Vec::new();
    for (name, metallic, roughness) in [("dielectric", 0.0, 0.22), ("polished-metal", 1.0, 0.22), ("rough-metal", 1.0, 0.85)] {
        node.params.insert("metallic".into(), vec![metallic]);
        node.params.insert("rough_low".into(), vec![roughness]);
        node.params.insert("rough_high".into(), vec![roughness]);
        let image = pass.run(&world.engine, &gpu.device, &gpu.queue, &node, &[None; 4]).unwrap();
        let material = Material(Some(image));
        for channel in [1, 0] {
            look.light.channel = channel;
            look.dirty = true;
            look.render(&material, [256, 256]).unwrap();
            let rgba = forge_gpu::read_texture(&gpu.device, &gpu.queue, &look.target.color.texture, [256; 2], 1, 4).unwrap();
            if channel == 1 { colors.push(rgba); } else {
                if let Some(path) = std::env::var_os("PATINA_MATERIAL_PROBE") {
                    let path = std::path::PathBuf::from(path).with_file_name(format!("pbr-{name}.png"));
                    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), 256, 256);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder.write_header().unwrap().write_image_data(&rgba).unwrap();
                }
                shaded.push(rgba);
            }
        }
    }
    assert_eq!(colors[0], colors[1], "metallic must not change base color");
    assert_eq!(colors[1], colors[2], "roughness must not change base color");
    for (name, a, b) in [("metallic", 0, 1), ("roughness", 1, 2)] {
        let difference = shaded[a].iter().zip(&shaded[b]).map(|(&a, &b)| f64::from(a.abs_diff(b))).sum::<f64>() / shaded[a].len() as f64;
        assert!(difference > 1.0, "{name} has no visible shading response: {difference}");
        println!("PBR {name}: mean byte difference {difference:.3}, base color unchanged");
    }
}
