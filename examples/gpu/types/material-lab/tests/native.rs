use forge_nodes::{Engine, NativeImage, NativePass, Registry, Size};
use material_lab::{Material, catalog, recipe, render::Look, world};
use patina_gpu::{gpu, wgpu};
use std::{collections::BTreeMap, time::Instant};

#[test]
#[ignore = "Needs a GPU with float32 filtering; run explicitly for material integration"]
fn native_copper_matches_surface_and_reuses_unchanged_outputs() {
    let started = Instant::now();
    let gpu = gpu().expect("GPU required for this integration check");
    println!("Adapter: {:?}", gpu.adapter.get_info());
    let world = &world().expect("shared material context").0;
    let maps = world.mesh_maps();
    println!("Context: {:.3} s", started.elapsed().as_secs_f64());
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let graph = &recipe().manifest.graph;
    let mut passes = BTreeMap::new();
    let mut values: BTreeMap<forge_nodes::NodeId, NativeImage> = BTreeMap::new();
    let start = Instant::now();
    for id in graph.order() {
        let node = graph.node(id).unwrap();
        let def = catalog().get(&node.kind).unwrap();
        let mut pass = NativePass::new(&world.engine, &gpu.device, &node.kind, 512).unwrap();
        let inputs: Vec<_> = def
            .inputs
            .iter()
            .map(|input| {
                graph
                    .source(id, &input.id)
                    .map(|(from, _)| values.get(&from).unwrap())
            })
            .collect();
        let image = pass
            .run(&world.engine, &gpu.device, &gpu.queue, node, &inputs)
            .unwrap();
        let again = pass
            .run(&world.engine, &gpu.device, &gpu.queue, node, &inputs)
            .unwrap();
        assert_eq!(image.revision, again.revision, "unchanged {}", node.title);
        values.insert(id, image);
        passes.insert(id, pass);
    }
    let final_image = values.get(&graph.output().unwrap()).unwrap();
    let native = forge_gpu::read_texture(
        &gpu.device,
        &gpu.queue,
        &final_image.planes.texture,
        [512; 2],
        3,
        16,
    )
    .unwrap();
    let native_seconds = start.elapsed().as_secs_f64();
    let order = graph.order();
    let inputs: Vec<Vec<_>> = order.iter().map(|id| {
        let node = graph.node(*id).unwrap();
        catalog().get(&node.kind).unwrap().inputs.iter().map(|input| {
            graph.source(*id, &input.id).map(|(from, _)| &values[&from])
        }).collect()
    }).collect();
    let cached = Instant::now();
    for _ in 0..100 {
        for (id, sources) in order.iter().zip(&inputs) {
            let image = passes.get_mut(id).unwrap().run(
                &world.engine, &gpu.device, &gpu.queue, graph.node(*id).unwrap(), sources,
            ).unwrap();
            assert_eq!(image.revision, values[id].revision);
        }
    }
    println!("Cached 28-node evaluation: {:.3} ms mean over 100 runs", cached.elapsed().as_secs_f64() * 10.0);
    let mut registry = Registry::from_files(catalog().files().clone());
    registry.set_linked(recipe().manifest.linked_wgsl.clone());
    let mut oracle = Engine::with_registry(&gpu.device, registry).unwrap();
    oracle.set_mesh_maps(
        &maps.position.view,
        &maps.normal.view,
        &maps.extras.view,
        &maps.view.view,
        maps.size,
        &world.info.bounds,
    );
    let mut reference = graph.clone();
    for node in &mut reference.nodes {
        node.uv = true;
        node.size = Some(Size::Relative([-2; 2]));
    }
    oracle.graph_changed(&reference);
    assert!(oracle.evaluate(&gpu.device, &gpu.queue, &reference));
    assert!(oracle.errors().is_empty(), "{:?}", oracle.errors());
    let (texture, size) = oracle.node_texture(reference.output().unwrap()).unwrap();
    assert_eq!(size, 512);
    let expected =
        forge_gpu::read_texture(&gpu.device, &gpu.queue, texture, [512; 2], 3, 16).unwrap();
    assert_eq!(native.len(), expected.len());
    let mut maximum = 0.0f32;
    for (a, b) in native.chunks_exact(4).zip(expected.chunks_exact(4)) {
        let a = f32::from_le_bytes(a.try_into().unwrap());
        let b = f32::from_le_bytes(b.try_into().unwrap());
        assert!(
            a.is_finite() && b.is_finite(),
            "non-finite material channel"
        );
        maximum = maximum.max((a - b).abs());
    }
    assert!(
        maximum <= 0.000001,
        "native/reference maximum float difference: {maximum}"
    );
    for kind in ["cells", "flow", "read"] {
        let node = material_lab::Operator::new(kind).node;
        let def = catalog().get(kind).unwrap();
        let mut pass = NativePass::new(&world.engine, &gpu.device, kind, 64).unwrap();
        pass.run(
            &world.engine,
            &gpu.device,
            &gpu.queue,
            &node,
            &vec![None; def.inputs.len()],
        )
        .unwrap();
    }
    let original = &catalog().get("noise").unwrap().source;
    let edited = original.replace("const CONTRAST: f32 = 1.2;", "const CONTRAST: f32 = 0.8;");
    assert_ne!(original, &edited);
    let mut edited_pass =
        NativePass::with_source(&world.engine, &gpu.device, "noise", 512, Some(&edited)).unwrap();
    let noise = graph.nodes.iter().find(|n| n.kind == "noise").unwrap();
    let changed = edited_pass
        .run(&world.engine, &gpu.device, &gpu.queue, noise, &[])
        .unwrap();
    let read_noise = |image: &NativeImage| {
        forge_gpu::read_texture(
            &gpu.device,
            &gpu.queue,
            &image.planes.texture,
            [512; 2],
            1,
            16,
        )
        .unwrap()
    };
    assert_ne!(
        read_noise(&changed),
        read_noise(&values[&noise.id]),
        "shader edits must affect GPU output"
    );
    assert!(
        NativePass::with_source(
            &world.engine,
            &gpu.device,
            "noise",
            512,
            Some("invalid shader")
        )
        .is_err()
    );
    let material = Material(Some(final_image.clone()));
    let mut look = Look::new([640, 640]).unwrap();
    look.render(&material, [640, 640]).unwrap();
    let rgba = forge_gpu::read_texture(
        &gpu.device,
        &gpu.queue,
        &look.target.color.texture,
        [640; 2],
        1,
        4,
    )
    .unwrap();
    assert!(
        rgba.chunks_exact(4).any(|p| p[0] > 80 && p[1] > 40),
        "preview must contain the shaded material"
    );
    let mut reference_asset = world.ball.preview_copy(&gpu.device, &world.scene);
    reference_asset.set_material(&gpu.device, &gpu.queue, &world.scene, oracle.output(&reference));
    let reference_view = forge_render::View::new(&gpu.device, &world.scene, [640, 640]);
    world.scene.render(&gpu.device, &gpu.queue, &reference_view, &world.camera, &world.light(), &reference_asset);
    let expected_render = forge_gpu::read_texture(&gpu.device, &gpu.queue, &reference_view.color.texture, [640; 2], 1, 4).unwrap();
    assert_eq!(rgba, expected_render, "Patina preview must match Surface's PBR render path");
    println!("PBR preview parity: all 640 x 640 RGBA pixels match Surface's render path");
    if let Some(path) = std::env::var_os("PATINA_MATERIAL_PROBE") {
        let file = std::fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(file, 640, 640);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&rgba)
            .unwrap();
    }
    assert!(
        patina_gpu::block_on(scope.pop()).is_none(),
        "GPU validation errors"
    );
    println!(
        "Native copper: 28 nodes, 42 wires, 512 x 512, {native_seconds:.3} s including first shader compiles and readback"
    );
    println!("Reference parity: maximum channel difference {maximum}");
}
