//! The gun workbench: seeded guns from parts, on the model kernel (task F).

use std::path::Path;

use crate::common::{copy, run};

#[test]
fn the_gun_workbench_makes_seeded_guns_and_exports_one() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workbenches/gun"), "gun");
    let main = project.join("graphs/main.toml");
    let graph = std::fs::read_to_string(&main).unwrap();
    let (output, stdout, stderr) = run(&project, &["--frames", "1"]);
    assert!(output.status.success(), "{stderr}");
    // The `gun` node with seed 7, and the parts of seed 3 in `assemble`.
    let seven = "Vesper common\n";
    let parts = "parts: receiver vesper, barrel vesper, grip vesper, magazine vesper, sight vesper\n";
    assert!(stdout.contains(seven) && stdout.contains(parts), "{stdout}");
    let built = "parts: receiver blockwerk, barrel vesper, grip blockwerk, stock scrapjaw, magazine halo, sight vesper\n";
    assert!(stdout.contains("Blockwerk epic\n") && stdout.contains(built), "{stdout}");
    // The `report` nodes after the `look` nodes. Round parts can give another hash on another platform.
    for scene in ["10 items, 10 places, 828 triangles, bounds ", "12 items, 12 places, 1628 triangles, bounds "] {
        assert!(stdout.contains(scene), "{stdout}");
    }
    let folder = project.join("guns");
    assert!(!folder.exists(), "a run with no fire writes no file");

    // A trigger with an order wire fires `export-gun` one time (F7).
    let trigger = "\n[[node]]\nid = \"f1e0000000000001\"\nbuiltin = \"trigger\"\n";
    std::fs::write(&main, format!("{graph}{trigger}\n[[order]]\nbefore = \"f1e0000000000001\"\nafter = \"9a70000000000004\"\n")).unwrap();
    let (output, stdout, stderr) = run(&project, &["--events", "1", "--frames", "1"]);
    assert!(output.status.success(), "{stderr}");
    assert!(stdout.contains("Wrote gun.glb and gun.txt to guns, "), "{stdout}");
    assert_eq!(&std::fs::read(folder.join("gun.glb")).unwrap()[..4], b"glTF");
    let stats = std::fs::read_to_string(folder.join("gun.txt")).unwrap();
    assert!(stats.starts_with(seven) && stats.ends_with(parts), "{stats}");
}
