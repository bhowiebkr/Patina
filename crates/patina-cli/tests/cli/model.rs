//! The model workbench: solids of the Manifold kernel, C++ in a type crate.

use std::path::Path;

use crate::common::{copy, run};

#[test]
fn the_model_workbench_builds_the_kernel_and_reports_a_box() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workbenches/model"), "model");
    let (output, stdout, stderr) = run(&project, &["--frames", "1"]);
    assert!(output.status.success(), "{stderr}");
    // The hash is the same on each platform and toolchain (task D2).
    assert_eq!(stdout.lines().collect::<Vec<_>>(), ["12 triangles, bounds -1 -2 -0.5 to 1 2 0.5, mesh hash 1463ee2a258be625"]);
}
