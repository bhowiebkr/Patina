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
    // The export node writes only after a click or a fire, not at a run (task D6).
    let export = "\n[[node]]\nid = \"e8a0000000000001\"\nfolder = \"nodes/export\"\n\n[node.params]\npath = \"tower.glb\"\nrequest = \"7\"\nsockets = \"top 0 0 3\"\n";
    write!(graph, "{export}\n[[wire]]\nfrom = \"c07000000000000c.out\"\nto = \"e8a0000000000001.solid\"\n").unwrap();
    drop(graph);
    let (output, stdout, stderr) = run(&project, &["--frames", "30"]);
    assert!(output.status.success(), "{stderr}");
    // The MSVC build and the Patina toolchain gave this hash at each run on 2026-11-19 (task D3).
    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        ["144 triangles, bounds -1.2 -1.2 0 to 1.2 1.2 2.933, mesh hash 8049817992ebb0d7"]
    );
    let file = project.join("tower.glb");
    assert!(!file.exists());
    // An order wire from a trigger puts the export node in the event domain. The CLI fires the
    // trigger one time, and the node writes the file with the socket in the glTF extras.
    let mut graph = std::fs::OpenOptions::new().append(true).open(project.join("graphs/main.toml")).unwrap();
    let trigger = "\n[[node]]\nid = \"f1e0000000000001\"\nbuiltin = \"trigger\"\n";
    write!(graph, "{trigger}\n[[order]]\nbefore = \"f1e0000000000001\"\nafter = \"e8a0000000000001\"\n").unwrap();
    drop(graph);
    let (output, stdout, stderr) = run(&project, &["--events", "1", "--frames", "1"]);
    assert!(output.status.success(), "{stderr}");
    assert!(stdout.contains("Wrote tower.glb, 144 triangles"), "{stdout}");
    let glb = std::fs::read(&file).unwrap();
    assert_eq!(&glb[..4], b"glTF");
    assert!(String::from_utf8_lossy(&glb).contains(r#""sockets":{"top":{"origin":[0,3,0]"#));
}
