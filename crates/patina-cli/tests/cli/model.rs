//! The model workbench: solids of the Manifold kernel, C++ in a type crate.

use std::path::Path;

use crate::common::{copy, run};

#[test]
fn the_model_workbench_builds_the_kernel_and_reports_a_tower() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workbenches/model"), "model");
    let (output, stdout, stderr) = run(&project, &["--frames", "1"]);
    assert!(output.status.success(), "{stderr}");
    // The MSVC build and the Patina toolchain gave this hash at each run on 2026-11-19 (task D3).
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        ["144 triangles, bounds -1.2 -1.2 0 to 1.2 1.2 2.933, mesh hash 8049817992ebb0d7"]
    );
}
