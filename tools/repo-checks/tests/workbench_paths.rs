//! Task B5 of `docs/workbenches-plan.md`: a workbench names no path outside its own folder, so it
//! can move out of the repository.

use std::fs;
use std::path::Path;

use repo_checks::{outside_paths, repo_root};

#[test]
fn no_workbench_names_a_path_outside_its_folder() {
    let mut found = Vec::new();
    let mut count = 0;
    for entry in fs::read_dir(repo_root().join("workbenches")).expect("read the workbenches") {
        let path = entry.unwrap().path();
        if path.join("patina.toml").is_file() {
            count += 1;
            found.extend(outside_paths(&path).unwrap().into_iter().map(|p| format!("  {}/{p}", path.file_name().unwrap().to_string_lossy())));
        }
    }
    assert!(count > 0, "the check found no workbench");
    assert!(found.is_empty(), "these paths go outside their workbench:\n{}\nUse a link to the repository page, or copy the file into the workbench.", found.join("\n"));
}

#[test]
fn the_check_finds_a_path_that_climbs_out() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("workbench-paths");
    if root.exists() {
        fs::remove_dir_all(&root).expect("remove the old test folder");
    }
    let write = |path: &str, text: &str| {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    write("patina.toml", "");
    write("nodes/a/Cargo.toml", "sdk = { path = \"../../build/sdk/patina-sdk\" }\nx = { path = \"../../../crates/x\" }\n");
    write("README.md", "See [the docs](../docs/a.md). Wait a.../b.\n");
    write("build/sdk/x.rs", "../../../../../x");
    write("images/a.png", "");
    fs::write(root.join("images/b.png"), [0xff, 0xfe, b'.', b'.', b'/', 0x80]).unwrap();
    assert_eq!(outside_paths(&root).unwrap(), ["README.md: ../docs/a.md", "nodes/a/Cargo.toml: ../../../crates/x"]);
}
