use std::fmt::Write;
use std::path::Path;

fn main() {
    tauri_build::build();
    embed_page();
}

/// Writes `page.rs`: the files of `frontend/dist` in the binary, so the editor needs no source tree (spec 13).
fn embed_page() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("frontend/dist");
    println!("cargo:rerun-if-changed={}", dist.display());
    let mut files = Vec::new();
    if dist.is_dir() {
        list(&dist, "", &mut files);
    }
    files.sort();
    let mut code = String::from("static PAGE: &[(&str, &[u8])] = &[\n");
    for (name, path) in &files {
        writeln!(code, "    ({name:?}, include_bytes!({path:?})),").unwrap();
    }
    code.push_str("];\n");
    let out = Path::new(&std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR")).join("page.rs");
    std::fs::write(out, code).expect("the build script writes page.rs");
}

fn list(dir: &Path, prefix: &str, files: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).expect("frontend/dist is readable") {
        let entry = entry.expect("frontend/dist is readable");
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        let path = entry.path();
        if path.is_dir() {
            list(&path, &format!("{name}/"), files);
        } else {
            files.push((name, path.to_string_lossy().into_owned()));
        }
    }
}
