use super::*;

fn binding(
    session: &Session,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    id: &str,
    size: u32,
    scale: f32,
) -> Result<PreviewBinding, String> {
    let mut info = session.info.clone();
    info.id = id.into();
    info.atlas_size = size;
    info.bounds.min[0] *= scale;
    info.bounds.max[0] *= scale;
    let mut mesh = session._mesh.clone();
    for vertex in &mut mesh.vertices {
        vertex.position[0] *= scale;
    }
    let gpu = GpuAsset::new(device, &session.scene, &info, &mesh);
    let maps = MeshMaps::bake(device, queue, &gpu, &info, &mesh, size, false)?;
    let table = info
        .parts
        .iter()
        .enumerate()
        .map(|(i, part)| {
            let [r, g, b, _] = part.material.base_color;
            // Distinct explicit rows detect a rebuild falling back to AssetInfo defaults.
            [r * 0.5, g * 0.7, b * 0.9, i as f32]
        })
        .collect();
    let light = Light {
        exposure: 1.75,
        env_yaw: 0.125,
        ..Light::for_environment(&info)
    };
    let camera = OrbitCamera::for_asset(&info);
    Ok(PreviewBinding {
        info,
        mesh,
        maps,
        gpu,
        camera,
        environment: Some(session._environment.clone()),
        environment_name: "Independent studio".into(),
        light,
        part_table: table,
    })
}

fn output(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    engine: &Engine,
    graph: &Graph,
) -> Result<Vec<u8>, String> {
    let (texture, size) = engine.output_texture(graph).ok_or("No output")?;
    forge_gpu::read_texture(device, queue, texture, [size; 2], 3, forge_nodes::OUT_TEXEL)
}

#[test]
fn covers_show_the_reference_ball_at_rest() -> Result<(), Box<dyn std::error::Error>> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&forge_nodes::device_descriptor(&adapter)))?;
    let mut session = Session::new(&device, &queue)?;
    session.new_material(&device, &queue, "Cover proof")?;
    session.graph.size = 5;
    session.changed()?;
    session.evaluate(&device, &queue)?;
    let rest = session.thumbnail(&device, &queue)?;

    session.rotate_ball(&queue, 1.2, 0.5);
    let dragged = session.ball_orientation;
    assert_eq!(
        session.thumbnail(&device, &queue)?,
        rest,
        "A dragged preview must not change the cover"
    );
    assert_eq!(session.ball_orientation, dragged);

    let asset = binding(&session, &device, &queue, "shelf-asset", 64, 1.)?;
    session.bind_preview(&device, asset);
    session.evaluate(&device, &queue)?;
    let view = session.cover(&device, &queue)?;
    let cover = forge_gpu::read_texture(&device, &queue, &view.color.texture, view.size, 1, 4)?;
    assert_eq!(cover, rest, "An asset preview must not reach the cover");
    assert!(session.preview_is_current());
    assert_eq!(session.info.id, "shelf-asset");
    assert_eq!(session.export_target(&device, &queue)?.1, 64);
    session.reference_preview(&device);
    assert_eq!(session.ball_orientation, dragged);
    Ok(())
}

#[test]
fn targets_survive_source_undo_and_export_without_changing_document_ownership()
-> Result<(), Box<dyn std::error::Error>> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&forge_nodes::device_descriptor(&adapter)))?;
    let mut session = Session::new(&device, &queue)?;
    session.new_material(&device, &queue, "Preview binding proof")?;
    session.graph.size = 5;
    let base = session
        .graph
        .nodes
        .iter()
        .find(|n| n.kind == "material")
        .ok_or("Base missing")?
        .id;
    let color = session.graph.add("noise", [-200., 0.]);
    session
        .graph
        .connect(session.engine.registry(), color, "output", base, "metallic")?;
    session.changed()?;
    let original_id = session.info.id.clone();
    let original = session.snapshot();
    let first = binding(&session, &device, &queue, "first-asset", 64, 1.)?;

    // Independent evaluator, scene and material assignment use the same read-only inputs.
    let mut registry = Registry::from_files(session.engine.registry().files().clone());
    registry.set_linked(session.engine.registry().linked_sources().clone());
    let mut engine = Engine::with_registry(&device, registry)?;
    super::super::configure_engine(&device, &mut engine, &first.maps, &first.info);
    engine.set_part_table(&device, &first.part_table);
    engine.evaluate(&device, &queue, &session.flat);
    assert!(engine.errors().is_empty(), "{:?}", engine.errors());
    let expected = output(&device, &queue, &engine, &session.flat)?;
    let mut scene = SceneRenderer::new(&device, &queue)?;
    scene.set_environment(&device, first.environment.as_ref());
    let mut gpu = first.gpu.preview_copy(&device, &scene);
    gpu.set_material(&device, &queue, &scene, engine.output(&session.flat));
    let view = View::new(&device, &scene, [320, 240]);
    scene.render(&device, &queue, &view, &first.camera, &first.light, &gpu);
    let reference_view =
        forge_gpu::read_texture(&device, &queue, &view.color.texture, view.size, 1, 4)?;

    session.bind_preview(&device, first);
    assert_eq!(
        session.snapshot(),
        original,
        "Selecting a preview must not edit the document"
    );
    session.evaluate(&device, &queue)?;
    assert_eq!(session.mesh_maps().size, 64);
    assert_eq!(
        output(&device, &queue, &session.engine, &session.flat)?,
        expected
    );
    let actual_view = View::new(&device, &session.scene, [320, 240]);
    session.render(&device, &queue, &actual_view);
    let actual_view = forge_gpu::read_texture(
        &device,
        &queue,
        &actual_view.color.texture,
        actual_view.size,
        1,
        4,
    )?;
    // Declared before this check: max one 8-bit level, mean <= 0.01 levels per channel.
    let differences: Vec<_> = actual_view
        .iter()
        .zip(reference_view)
        .map(|(a, b)| a.abs_diff(b))
        .collect();
    assert!(differences.iter().all(|difference| *difference <= 1));
    assert!(differences.iter().map(|d| *d as usize).sum::<usize>() * 100 <= differences.len());
    session.exposure = 2.5;
    assert_eq!(
        session.snapshot(),
        original,
        "Asset display settings are view state"
    );
    assert_eq!(session.package()?.manifest.exposure, original.lighting[0]);

    let source = "//! name: Preview probe\n//! category: Custom\n//! output: output Color \"Color\"\nfn eval(p: vec2<i32>) -> Out { return Out(vec4<f32>(0.25, 0.5, 0.75, 1.0)); }\n";
    session.apply_source(
        &device,
        &queue,
        "nodes/custom/preview_binding_probe.wesl",
        source,
    )?;
    assert_eq!(session.info.id, "first-asset");
    assert_eq!(
        output(&device, &queue, &session.engine, &session.flat)?,
        expected,
        "Source replacement must retain maps and the part table"
    );
    session.restore(&device, &queue, original.clone())?;
    assert_eq!(session.info.id, "first-asset");
    assert_eq!(
        output(&device, &queue, &session.engine, &session.flat)?,
        expected
    );
    assert_eq!(session.snapshot(), original);

    let second = binding(&session, &device, &queue, "second-asset", 128, 0.5)?;
    session.bind_preview(&device, second);
    session.evaluate(&device, &queue)?;
    assert_eq!(session.info.id, "second-asset");
    assert_eq!(session.export_target(&device, &queue)?.1, 128);
    assert_eq!(session.snapshot(), original);
    // Traced maps are required explicitly; neutral geometry maps cannot impersonate a bake.
    let ao = session.graph.add("surface", [-200., 200.]);
    session
        .graph
        .connect(session.engine.registry(), ao, "output", base, "roughness")?;
    session.changed()?;
    assert!(
        session
            .evaluate(&device, &queue)
            .err()
            .is_some_and(|error| error.contains("no traced bake"))
    );
    assert!(
        session
            .export_target(&device, &queue)
            .err()
            .is_some_and(|error| error.contains("no traced bake"))
    );
    session.reference_preview(&device);
    assert_eq!(session.info.id, original_id);
    session.evaluate(&device, &queue)?;
    assert_eq!(session.export_target(&device, &queue)?.1, 2048);
    assert!(!session.preview_is_current());
    Ok(())
}
