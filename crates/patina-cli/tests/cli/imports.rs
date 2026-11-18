//! An import and an update of a node from another project, with the real toolchain (6.2).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use patina_engine::{Control, Mode, Project, Toolchain};

use crate::common::copy;

/// The code of the imported node, which multiplies its input by `factor`.
fn scale(factor: &str) -> String {
    format!("#[patina_sdk::node]\npub fn scale(value: f64) -> f64 {{\n    value * {factor}\n}}\n")
}

#[test]
fn an_import_builds_and_a_bad_update_goes_back() {
    let hello = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello");
    let (this, other) = (copy(&hello, "import-this"), copy(&hello, "import-other"));
    let id = fs::read_to_string(other.join("patina.toml")).unwrap().replace("f7ab5a61a3754081", "0123456789abcdef");
    fs::write(other.join("patina.toml"), id).unwrap();
    let lib = |root: &Path| root.join("nodes/scale/src/lib.rs");
    fs::create_dir_all(lib(&other).parent().unwrap()).unwrap();
    let add = fs::read_to_string(other.join("nodes/add/Cargo.toml")).unwrap();
    fs::write(other.join("nodes/scale/Cargo.toml"), add.replace("\"add\"", "\"scale\"")).unwrap();
    fs::write(lib(&other), scale("2.0")).unwrap();

    let toolchain = Toolchain::detect().unwrap();
    let mut project = Project::open(&this).unwrap();
    project.import(&toolchain, &other, "nodes/scale", &BTreeMap::new()).unwrap();
    assert_eq!(project.imports(), ["nodes/scale"]);
    assert_eq!(fs::read_to_string(lib(&this)).unwrap(), scale("2.0"));

    // An update that does not compile puts the node back as it was.
    fs::write(lib(&other), scale("2.0 +")).unwrap();
    let picks = BTreeMap::new();
    let e = project.update(&toolchain, &other, "nodes/scale", &picks).unwrap_err();
    assert!(e.contains("expected expression"), "{e}");
    assert_eq!(fs::read_to_string(lib(&this)).unwrap(), scale("2.0"));

    // A good update builds, and the graph runs the new code.
    fs::write(lib(&other), scale("3.0")).unwrap();
    project.update(&toolchain, &other, "nodes/scale", &picks).unwrap();
    assert_eq!(fs::read_to_string(lib(&this)).unwrap(), scale("3.0"));
    let graph = this.join("graphs/main.toml");
    let more = "\n[[node]]\nid = \"00000000000000f0\"\nfolder = \"nodes/scale\"\n\n[[wire]]\nfrom = \"f89c6e329e8ea336.out\"\nto = \"00000000000000f0.value\"\n
[[node]]\nid = \"00000000000000f1\"\nfolder = \"nodes/show\"\n\n[[wire]]\nfrom = \"00000000000000f0.out\"\nto = \"00000000000000f1.value\"\n";
    fs::write(&graph, fs::read_to_string(&graph).unwrap() + more).unwrap();
    let opened = Project::open(&this).unwrap();
    let mut cluster = opened.check().unwrap().build(&toolchain, Mode::Edit).unwrap();
    cluster.set_params(opened.graph());
    let out = cluster.run(&Control::default());
    assert!(out.ok, "{:?}", out.reports);
    assert_eq!(out.values.into_iter().find(|v| v.from == "00000000000000f0.out").unwrap().text, "19.5");

    // A deleted folder is not in the list, and a restore shows it again.
    let (folder, gone) = (this.join("nodes/scale"), this.join("scale-gone"));
    fs::rename(&folder, &gone).unwrap();
    assert!(opened.imports().is_empty());
    fs::rename(&gone, &folder).unwrap();
    assert_eq!(opened.imports(), ["nodes/scale"]);
}
