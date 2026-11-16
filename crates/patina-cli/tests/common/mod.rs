//! The helpers of the CLI tests: a copy of a project, and a run of the CLI.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use patina_engine::Toolchain;

/// Copies a project into the temporary folder of the tests and sets the Rust version of the
/// toolchain. The `build/` folder stays, so a second test run builds only the changes.
pub fn copy(from: &Path, name: &str) -> PathBuf {
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
    let text = fs::read_to_string(&file).unwrap();
    let text: Vec<String> =
        text.lines().map(|l| if l.starts_with("rust = ") { format!("rust = \"{release}\"") } else { l.to_owned() }).collect();
    fs::write(file, text.join("
")).unwrap();
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

pub fn run(project: &Path, more: &[&str]) -> (Output, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_patina-cli")).arg("run").arg(project).arg("--edit").args(more).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output, stdout, stderr)
}
