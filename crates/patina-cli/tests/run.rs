//! Builds and runs projects with the CLI, as a person does (spec 3, milestone 1).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patina_engine::{Control, Lanes, Project, Toolchain};

/// Copies a project into the temporary folder of the tests and sets the Rust version of the
/// toolchain. The `build/` folder stays, so a second test run builds only the changes.
fn copy(from: &Path, name: &str) -> PathBuf {
    let to = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    for entry in fs::read_dir(&to).into_iter().flatten().map(Result::unwrap) {
        if entry.file_name() != "build" {
            let path = entry.path();
            if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) }.unwrap();
        }
    }
    copy_dir(from, &to);
    let release = Toolchain::detect().unwrap().release;
    let file = to.join("patina.toml");
    let text = fs::read_to_string(&file).unwrap().replace("rust = \"1.94.0\"", &format!("rust = \"{release}\""));
    fs::write(file, text).unwrap();
    to
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().map(Result::unwrap) {
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        if entry.file_name() == "build" {
            continue;
        } else if source.is_dir() {
            copy_dir(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap();
        }
    }
}

fn run(project: &Path, more: &[&str]) -> (Output, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_patina-cli")).arg("run").arg(project).args(more).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output, stdout, stderr)
}

/// A connection kind of the test, for the f64 wires of the hello example (6.3).
const KIND: &str = "#[patina_sdk::connection]
pub fn whole(value: &f64) -> patina_sdk::Lanes {
    patina_sdk::Lanes::bits(8, *value as u64).label(\"whole\")
}
";

#[test]
fn hello_adds_two_numbers_and_a_kind_shows_lanes() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello"), "hello");
    let kind = project.join("connections/whole");
    fs::create_dir_all(kind.join("src")).unwrap();
    let manifest = "[package]\nname = \"whole\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\npatina-sdk = { workspace = true }\n";
    fs::write(kind.join("Cargo.toml"), manifest).unwrap();
    fs::write(kind.join("src/lib.rs"), KIND).unwrap();
    let (output, stdout, stderr) = run(&project, &[]);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(stdout.trim(), "6.5");

    // The cluster sends the lanes of each held value, the most significant lane first.
    let opened = Project::open(&project).unwrap();
    let mut cluster = opened.check().unwrap().build(&Toolchain::detect().unwrap()).unwrap();
    cluster.set_params(opened.graph());
    let values = cluster.run(&Control::default()).values;
    let sum = values.iter().find(|v| v.from == "f89c6e329e8ea336.out").unwrap();
    assert_eq!(sum.lanes, Some(Lanes { bits: "00000110".to_owned(), label: "whole".to_owned() }));

    // A frame node and the show node after it run at each frame, after the batch run (9.1).
    let tick = project.join("nodes/tick");
    fs::create_dir_all(tick.join("src")).unwrap();
    fs::write(tick.join("Cargo.toml"), manifest.replace("whole", "tick")).unwrap();
    fs::write(tick.join("src/lib.rs"), "#[patina_sdk::node(domain = frame)]\npub fn tick() -> f64 {\n    1.0\n}\n").unwrap();
    let graph = project.join("graphs/main.toml");
    let more = "\n[[node]]\nid = \"00000000000000f1\"\nfolder = \"nodes/tick\"\n\n[[node]]\nid = \"00000000000000f2\"\n\
                folder = \"nodes/show\"\n\n[[wire]]\nfrom = \"00000000000000f1.out\"\nto = \"00000000000000f2.value\"\n";
    fs::write(&graph, fs::read_to_string(&graph).unwrap() + more).unwrap();
    let (output, stdout, stderr) = run(&project, &["--frames", "3"]);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(stdout.lines().collect::<Vec<_>>(), ["6.5", "1", "1", "1"]);
}

#[test]
fn failed_nodes_are_reported_and_stop_the_nodes_after_them() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/projects/fail"), "fail");
    let (output, stdout, stderr) = run(&project, &[]);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(stdout, "", "a node after a failed node must not run");
    assert!(stderr.contains("Node fea12c51067a34d8 (fail): error: \"cannot use 1.5\""), "{stderr}");
    assert!(stderr.contains("Node dc1095edd35b9589 (boom): panic: the boom node panicked"), "{stderr}");
    assert!(stderr.contains("Node df93472b07eaca16: skipped: an input has no value"), "{stderr}");
    assert!(stderr.contains("Node 7363866a8cd1c455: skipped"), "{stderr}");

    // A parameter change builds nothing again, and a bad parameter text is reported.
    let graph = project.join("graphs/main.toml");
    let text = fs::read_to_string(&graph).unwrap().replace("value = \"1.5\"", "value = \"abc\"");
    fs::write(graph, text).unwrap();
    let (output, stdout, stderr) = run(&project, &[]);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(stdout, "");
    let expected = "Node fea12c51067a34d8 (fail): bad parameter: input `value`: \"abc\" is not a number";
    assert!(stderr.contains(expected), "{stderr}");
}
