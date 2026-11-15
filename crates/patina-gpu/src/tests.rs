use super::*;

/// The shared context, or `None` on a computer with no adapter, for example a Linux CI runner.
fn try_gpu() -> Option<&'static Gpu> {
    gpu().inspect_err(|e| eprintln!("skipped: {e}")).ok()
}

#[test]
fn link_joins_the_imports_and_sets_the_features() {
    let dir = std::env::temp_dir().join(format!("patina-gpu-link-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("color.wesl"), "fn red() -> f32 { return 1.0; }\n").unwrap();
    let main = "import package::color::red;\n@if(bright) const LEVEL: f32 = 2.0;\n@if(!bright) const LEVEL: f32 = 0.5;\n\
                @compute @workgroup_size(1) fn main() { let v = red() * LEVEL; }\n";
    std::fs::write(dir.join("main.wesl"), main).unwrap();
    let bright = link(&dir.join("main.wesl"), &[("bright", true)]).unwrap();
    assert!(bright.contains("2.0") && !bright.contains("0.5") && !bright.contains("import"), "{bright}");
    assert!(link(&dir.join("main.wesl"), &[]).unwrap().contains("0.5"));
    std::fs::write(dir.join("bad.wesl"), "import package::nothing::f;\n@compute @workgroup_size(1) fn main() { f(); }\n").unwrap();
    assert!(link(&dir.join("bad.wesl"), &[]).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_image_goes_to_the_gpu_and_back() {
    let Some(gpu) = try_gpu() else { return };
    let image = GpuImage::new(gpu, 3, 2);
    let pixels: Vec<u8> = (0..24).collect();
    let layout = wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(12), rows_per_image: Some(2) };
    gpu.queue.write_texture(image.texture.as_image_copy(), &pixels, layout, image.texture.size());
    assert_eq!(image.read(gpu).unwrap(), pixels);
    assert!(std::ptr::eq(gpu, try_gpu().unwrap()));
}

#[test]
fn a_bad_shader_gives_an_error() {
    let Some(gpu) = try_gpu() else { return };
    assert!(gpu.shader("good", "@compute @workgroup_size(1) fn main() {}").is_ok());
    let error = gpu.shader("bad", "fn main( {").err().unwrap();
    assert!(error.starts_with("bad: "), "{error}");
}
