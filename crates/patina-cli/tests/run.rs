//! Builds and runs projects with the CLI, as a person does (spec 3, milestone 1).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
    let release = patina_engine::Toolchain::detect().unwrap().release;
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

fn run(project: &Path) -> (Output, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_patina-cli")).arg("run").arg(project).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output, stdout, stderr)
}

#[test]
fn hello_adds_two_numbers() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello"), "hello");
    let (output, stdout, stderr) = run(&project);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(stdout.trim(), "6.5");
}

#[test]
fn failed_nodes_are_reported_and_stop_the_nodes_after_them() {
    let project = copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/projects/fail"), "fail");
    let (output, stdout, stderr) = run(&project);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(stdout, "", "a node after a failed node must not run");
    assert!(stderr.contains("Node fea12c51067a34d8 (fail): error: \"cannot use 1.5\""), "{stderr}");
    assert!(stderr.contains("Node dc1095edd35b9589 (boom): panic: the boom node panicked"), "{stderr}");

    // A parameter change builds nothing again, and a bad parameter text is reported.
    let graph = project.join("graphs/main.toml");
    let text = fs::read_to_string(&graph).unwrap().replace("value = \"1.5\"", "value = \"abc\"");
    fs::write(graph, text).unwrap();
    let (output, stdout, stderr) = run(&project);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(stdout, "");
    let expected = "Node fea12c51067a34d8 (fail): bad parameter: input `value`: \"abc\" is not a number";
    assert!(stderr.contains(expected), "{stderr}");
}
