//! Embeds every `kernels/**/*.wesl` and `*.wgsl` as `KERNELS: &[(&str, &str)]` (path relative to
//! `kernels/`, source), the fallback when the source tree is not beside the exe.

use std::path::{Path, PathBuf};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "wesl" || e == "wgsl") {
            out.push(path);
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=kernels");
    let root =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()).join("kernels");
    let mut files = Vec::new();
    collect(&root, &mut files);
    files.sort();
    let mut code = String::from("pub static KERNELS: &[(&str, &str)] = &[\n");
    for path in &files {
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let rel = rel.to_string_lossy().replace('\\', "/");
        let abs = path.to_string_lossy().replace('\\', "/");
        code.push_str(&format!("    ({rel:?}, include_str!({abs:?})),\n"));
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("kernels.rs");
    if let Err(e) = std::fs::write(&out, code) {
        println!("cargo:warning=kernels.rs: {e}");
    }
}
