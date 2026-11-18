//! The helpers of the CLI tests: a copy of a project, and a run of the CLI.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};
use std::time::{Duration, Instant};

/// Copies a project into the temporary folder of the tests, with its Rust version, as the zip has
/// it (8.1). The `build/` folder stays, so a second test run builds only the changes.
pub fn copy(from: &Path, name: &str) -> PathBuf {
    let to = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    for entry in fs::read_dir(&to).into_iter().flatten().map(Result::unwrap) {
        if entry.file_name() != "build" {
            let path = entry.path();
            if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) }.unwrap();
        }
    }
    copy_dir(from, &to);
    to
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().map(Result::unwrap) {
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        // A cargo `target` folder is build output, not part of the project.
        if entry.file_name() == "build" || (source.is_dir() && entry.file_name() == "target") {
            continue;
        } else if source.is_dir() {
            copy_dir(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap();
        }
    }
}

fn cli(project: &Path, more: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_patina-cli"));
    command.arg("run").arg(project).arg("--edit").args(more);
    command
}

/// Runs the CLI as `run` does, and calls `poke` about each 50 ms until the CLI ends. Stops the CLI
/// after 300 s.
pub fn run_while(project: &Path, more: &[&str], mut poke: impl FnMut()) -> (ExitStatus, String, String) {
    let (out, err) = (project.with_extension("out"), project.with_extension("err"));
    let mut child = cli(project, more).stdout(fs::File::create(&out).unwrap()).stderr(fs::File::create(&err).unwrap()).spawn().unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(300) {
            let _ = child.kill().and_then(|()| child.wait());
            panic!("the CLI did not end in 300 s: {}", fs::read_to_string(&err).unwrap());
        }
        poke();
        std::thread::sleep(Duration::from_millis(50));
    };
    (status, fs::read_to_string(out).unwrap(), fs::read_to_string(err).unwrap())
}

pub fn run(project: &Path, more: &[&str]) -> (Output, String, String) {
    let output = cli(project, more).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output, stdout, stderr)
}
