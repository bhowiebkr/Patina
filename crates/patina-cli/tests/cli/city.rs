//! The city workbench: terrain, streets, lots and buildings as scenes, on the model kernel (task E).

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::common::{copy, run};

#[test]
fn the_city_workbench_builds_a_district_and_exports_each_building() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workbenches/city"), "city");
    let main = project.join("graphs/main.toml");
    let graph = std::fs::read_to_string(&main).unwrap();
    let (output, stdout, stderr) = run(&project, &["--frames", "1"]);
    assert!(output.status.success(), "{stderr}");
    // Round trees and turned roofs can give another hash on another platform, so the check stops
    // before the hash. `main` is the organic city on the terrain (E9 to E12).
    let line = "573 items, 41357 places, 1277498 triangles, bounds -300 -225 -6.5 to 300 225 164.087, scene hash ";
    assert!(stdout.starts_with(line), "{stdout}");
    // `planned` puts the same buildings on the street grid on flat land.
    let (output, stdout, stderr) = run(&project, &["--graph", "planned", "--frames", "1"]);
    assert!(output.status.success(), "{stderr}");
    assert!(stdout.starts_with("1314 items, 159832 places, 4833586 triangles, bounds -300 -225 1 to 300 225 151.393, scene hash "), "{stdout}");
    let folder = project.join("buildings");
    assert!(!folder.exists(), "a run with no fire writes no file");

    // E8: a district of 1 km x 1 km runs in less than 2 s in a bake build, after the build.
    let wide = graph.replace("width = \"600\"\ndepth = \"450\"", "width = \"1000\"\ndepth = \"1000\"");
    assert_ne!(wide, graph);
    std::fs::write(&main, wide).unwrap();
    let start = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_patina-cli")).arg("run").arg(&project).args(["--frames", "1"]).output().unwrap();
    let total = start.elapsed().as_secs_f64();
    let (stdout, stderr) = (String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(output.status.success(), "{stderr}");
    assert!(stdout.starts_with("1791 items, 93244 places, 2886571 triangles, bounds -500 -500 -6.5 to 500 500 153.036, scene hash "), "{stdout}");
    let built: f64 = stderr.lines().find_map(|l| l.strip_prefix("Built in ")?.strip_suffix(" s")?.parse().ok()).unwrap();
    println!("1 km x 1 km district: {:.2} s after a build of {built:.2} s", total - built);
    assert!(total - built < 2.0, "the district took {:.2} s", total - built);

    // A trigger with an order wire fires `export-buildings` one time. It writes a file for each
    // building (E7).
    let trigger = "\n[[node]]\nid = \"f1e0000000000001\"\nbuiltin = \"trigger\"\n";
    std::fs::write(&main, format!("{graph}{trigger}\n[[order]]\nbefore = \"f1e0000000000001\"\nafter = \"c170000000000019\"\n")).unwrap();
    let (output, stdout, stderr) = run(&project, &["--events", "1", "--frames", "1"]);
    assert!(output.status.success(), "{stderr}");
    let count: usize = stdout.lines().find_map(|l| l.strip_prefix("Wrote ")?.split(' ').next()?.parse().ok()).expect(&stdout);
    let files: Vec<_> = std::fs::read_dir(&folder).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "glb")).collect();
    assert!(count > 100 && files.len() == count, "{count} buildings, {} files", files.len());
    assert_eq!(&std::fs::read(&files[0]).unwrap()[..4], b"glTF");
}
