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
    let lib = "import patina_gpu::image::{luma, inside};\n\
               @compute @workgroup_size(1) fn main(@builtin(global_invocation_id) id: vec3u) {\n\
               if inside(id, vec2u(4u)) { let v = luma(vec3f(1.0)); } }\n";
    std::fs::write(dir.join("lib.wesl"), lib).unwrap();
    let linked = link(&dir.join("lib.wesl"), &[]).unwrap();
    assert!(linked.contains("0.7152") && !linked.contains("import"), "{linked}");
    std::fs::write(dir.join("shade.wesl"), "// A bad shade.\nfn shade(t: f32) -> f32 { let x: u32 = t; return t; }\n").unwrap();
    std::fs::write(dir.join("use.wesl"), "import package::shade::shade;\n@compute @workgroup_size(1) fn main() { let v = shade(1.0); }\n").unwrap();
    let error = link(&dir.join("use.wesl"), &[]).unwrap_err();
    assert!(error.starts_with("shade.wesl line 2, in `shade`: "), "{error}");
    std::fs::write(dir.join("own.wesl"), "\n\n@compute @workgroup_size(1)\nfn main() { let x: u32 = 1.5; }\n").unwrap();
    let error = link(&dir.join("own.wesl"), &[]).unwrap_err();
    assert!(error.starts_with("own.wesl line 4, in `main`: "), "{error}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn declared_finds_the_name_of_a_top_level_declaration() {
    use crate::link::declared;
    assert_eq!(declared("@if(invert) fn shade(t: f32) -> vec3f {"), Some("shade"));
    assert_eq!(declared("@compute @workgroup_size(8, 8) fn main() {"), Some("main"));
    assert_eq!(declared("@group(0) @binding(0) var image: texture_2d<f32>;"), Some("image"));
    assert_eq!(declared("var<private> count: u32;"), Some("count"));
    assert_eq!(declared("public fn luma(c: vec3f) -> f32 {"), Some("luma"));
    assert_eq!(declared("struct Light {"), Some("Light"));
    assert_eq!(declared("    const x = 1;"), None);
    assert_eq!(declared("import package::shade::shade;"), None);
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

#[test]
fn shrink_takes_the_mean_of_each_block_and_keeps_the_ratio() {
    let pixels: Vec<u8> = [0u8, 100, 200, 50].iter().flat_map(|&v| [v, v, v, 255]).collect();
    let (size, out) = crate::image::shrink(&pixels, [4, 1], 2);
    assert_eq!((size, out), ([2, 1], vec![50, 50, 50, 255, 125, 125, 125, 255]));
    let (size, _) = crate::image::shrink(&[0; 5 * 3 * 4], [5, 3], 2);
    assert_eq!(size, [2, 1]);
    assert_eq!(crate::image::shrink(&pixels, [4, 1], 8).0, [4, 1]);
}
