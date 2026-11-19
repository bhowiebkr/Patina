//! A shared hash table must keep its seed when another project library reads it.
use patina_engine::{Control, Mode, Project, Toolchain};
use std::path::Path;

fn put(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

#[test]
#[ignore = "Builds and loads two project libraries; run explicitly for shared GPU changes"]
fn shared_hash_survives_separate_libraries() {
    let root = std::env::temp_dir().join(format!("patina-shared-hash-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    put(
        &root,
        "patina.toml",
        "format = 1\nid = \"0123456789abcdef\"\nrust = \"1.100.0\"\n",
    );
    put(
        &root,
        "types/cache/Cargo.toml",
        "[package]\nname = \"cache\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\npatina-sdk = { workspace = true }\nfoldhash = \"0.2\"\n",
    );
    put(
        &root,
        "types/cache/src/lib.rs",
        r#"
use std::collections::HashMap;
use std::sync::Mutex;
pub struct Table(Mutex<HashMap<u64, u64, foldhash::fast::RandomState>>);
fn table() -> &'static Table {
    patina_sdk::shared(|| Table(Mutex::new(HashMap::with_hasher(Default::default()))))
}
pub fn write() {
    for i in 0..64 { table().0.lock().unwrap().insert(i, i * 3); }
}
pub fn read() -> Result<u64, String> {
    let _local_seed = foldhash::fast::RandomState::default();
    let table = table().0.lock().unwrap();
    for i in 0..64 {
        if table.get(&i) != Some(&(i * 3)) { return Err(format!("Lost shared key {i}")); }
    }
    Ok(64)
}
"#,
    );
    for (name, body) in [
        ("writer", "pub fn process() { cache::write(); }"),
        (
            "reader",
            "pub fn process() -> Result<u64, String> { cache::read() }",
        ),
    ] {
        put(
            &root,
            &format!("nodes/{name}/Cargo.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\npatina-sdk = {{ workspace = true }}\ncache = {{ path = \"../../types/cache\" }}\n"
            ),
        );
        put(
            &root,
            &format!("nodes/{name}/src/lib.rs"),
            &format!("#[patina_sdk::node]\n{body}\n"),
        );
        put(
            &root,
            &format!("graphs/{}.toml", if name == "writer" { "main" } else { name }),
            &format!(
                "format = 1\ntop = true\n[[node]]\nid = \"0123456789abcdef\"\nfolder = \"nodes/{name}\"\npos = [0.0, 0.0]\n"
            ),
        );
    }
    let tools = Toolchain::detect().unwrap();
    let control = Control::default();
    for name in ["main", "reader"] {
        let checked = Project::open_top(&root, name).unwrap().check().unwrap();
        let mut cluster = checked.build(&tools, Mode::Edit).unwrap();
        let result = cluster.run(&control);
        assert!(result.ok, "{name}: {:?}", result.reports);
    }
    // Windows keeps loaded DLLs until this process ends; the runner owns the temporary folder.
    println!("Checked temporary project: {}", root.display());
}
