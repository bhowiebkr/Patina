//! The model workbench: solids of the Manifold kernel, C++ in a type crate.

use std::io::Write;
use std::path::Path;

use crate::common::{copy, run};

#[test]
fn the_model_workbench_builds_the_kernel_and_reports_a_tower() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workbenches/model"), "model");
    // A delay reads the tower in the frame domain. The model nodes stay in the batch domain, so
    // the tower builds one time, not at each frame (task D4).
    let mut graph = std::fs::OpenOptions::new().append(true).open(project.join("graphs/main.toml")).unwrap();
    let delay = "\n[[node]]\nid = \"de1a000000000001\"\nbuiltin = \"delay\"\npos = [1560.0, 500.0]\n";
    write!(graph, "{delay}\n[[wire]]\nfrom = \"c07000000000000c.out\"\nto = \"de1a000000000001.value\"\n").unwrap();
    drop(graph);
    let (output, stdout, stderr) = run(&project, &["--frames", "30"]);
    assert!(output.status.success(), "{stderr}");
    // The MSVC build and the Patina toolchain gave this hash at each run on 2026-11-19 (task D3).
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        ["144 triangles, bounds -1.2 -1.2 0 to 1.2 1.2 2.933, mesh hash 8049817992ebb0d7"]
    );
}
