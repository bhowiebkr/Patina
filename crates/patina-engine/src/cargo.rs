//! Finds the Rust toolchain and builds the cluster crate with cargo (spec 8).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use serde_json::Value;

use crate::Error;
use crate::generate;
use crate::plan::Plan;

/// The variables that the engine removes before it starts cargo. They come from the process
/// that started the engine, for example `cargo test`, and they would change the build.
const CLEARED_ENV: &[&str] = &[
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_BUILD_RUSTFLAGS",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
    "CARGO_BUILD_TARGET",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_MAKEFLAGS",
    "MAKEFLAGS",
    "MFLAGS",
    "RUSTDOCFLAGS",
];

const WORKSPACE: &str = "# Made by Patina. Patina writes this file again at each build.\n\
[workspace]\nresolver = \"3\"\nmembers = [\"nodes/*\", \"build/clusters/*\"]\n\n\
[workspace.dependencies]\npatina-sdk = { path = \"build/sdk/patina-sdk\" }\n";

/// The SDK crates that a project builds with. The engine carries their source.
const SDK_FILES: [(&str, &str); 6] = [
    (
        "build/sdk/patina-sdk/Cargo.toml",
        "[package]\nname = \"patina-sdk\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [dependencies]\npatina-macros = { path = \"../patina-macros\" }\n",
    ),
    ("build/sdk/patina-sdk/src/lib.rs", include_str!("../../patina-sdk/src/lib.rs")),
    ("build/sdk/patina-sdk/src/glue.rs", include_str!("../../patina-sdk/src/glue.rs")),
    ("build/sdk/patina-sdk/src/view.rs", include_str!("../../patina-sdk/src/view.rs")),
    (
        "build/sdk/patina-macros/Cargo.toml",
        "[package]\nname = \"patina-macros\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [lib]\nproc-macro = true\n",
    ),
    ("build/sdk/patina-macros/src/lib.rs", include_str!("../../patina-macros/src/lib.rs")),
];

/// A compile error (15.5).
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub message: String,
    /// The note at the code of the error, for example "expected `f32`, found `&str`".
    pub label: Option<String>,
    /// The text as rustc shows it, with the lines of code.
    pub rendered: String,
    /// The file from the project root, for example `nodes/number/src/lib.rs`, when the error is in
    /// a file of the project.
    pub file: Option<String>,
    /// The first line and column of the error, from 1, and the line and column after its end.
    pub start: [u32; 2],
    pub end: [u32; 2],
}

/// The Rust toolchain that builds a project.
pub struct Toolchain {
    pub cargo: PathBuf,
    pub rustc: PathBuf,
    /// The Rust version, for example `1.94.0`.
    pub release: String,
}

impl Toolchain {
    /// Finds the toolchain of the `CARGO` variable, or else of `PATH`.
    // shortcut: no own toolchain yet, the toolchain button (13.1) comes at milestone 3.
    pub fn detect() -> Result<Toolchain, Error> {
        let exe = std::env::consts::EXE_SUFFIX;
        let cargo = std::env::var_os("CARGO").map_or_else(|| PathBuf::from("cargo"), PathBuf::from);
        let rustc_name = format!("rustc{exe}");
        let rustc = cargo
            .parent()
            .map(|dir| dir.join(&rustc_name))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from(&rustc_name));
        let version = run_text(command(&rustc).arg("-vV"))?;
        let release = version
            .lines()
            .find_map(|line| line.strip_prefix("release:"))
            .map(|value| value.trim().to_owned())
            .ok_or_else(|| Error::Build(format!("{} -vV gave no release", rustc.display())))?;
        // The programs in the sysroot are not rustup proxies, so a `rust-toolchain.toml` in the
        // project cannot select another toolchain.
        let bin = PathBuf::from(run_text(command(&rustc).args(["--print", "sysroot"]))?.trim()).join("bin");
        let pick = |name: String, found: PathBuf| Some(bin.join(name)).filter(|p| p.is_file()).unwrap_or(found);
        Ok(Toolchain { cargo: pick(format!("cargo{exe}"), cargo), rustc: pick(rustc_name, rustc), release })
    }
}

/// A command that opens no console window, also when a GUI program starts it on Windows.
fn command(program: &Path) -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

fn run_text(command: &mut Command) -> Result<String, Error> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command.output().map_err(|e| Error::Build(format!("cannot start {program}: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Build(format!("{program} failed: {stderr}")));
    }
    String::from_utf8(output.stdout).map_err(|e| Error::Build(format!("{program}: {e}")))
}

/// Writes the files that Patina makes, builds the cluster and returns the path of its library.
// shortcut: one cluster with a static std in the dev profile. Clusters, the shared types
// library and bake mode come at milestone 6.
pub fn build(root: &Path, toolchain: &Toolchain, plan: &Plan) -> Result<PathBuf, Error> {
    write(root, "Cargo.toml", WORKSPACE)?;
    for (path, text) in SDK_FILES {
        write(root, path, text)?;
    }
    write(root, "build/clusters/main/Cargo.toml", &generate::manifest(plan))?;
    write(root, "build/clusters/main/lib.rs", &generate::source(plan))?;

    let mut command = command(&toolchain.cargo);
    command
        .current_dir(root)
        .args(["build", "-p", generate::PACKAGE, "--message-format=json"]);
    for var in CLEARED_ENV {
        command.env_remove(var);
    }
    command.env("CARGO_TARGET_DIR", root.join("build/target")).env("RUSTC", &toolchain.rustc);
    let output = command
        .output()
        .map_err(|e| Error::Build(format!("cannot start {}: {e}", toolchain.cargo.display())))?;
    if !output.status.success() {
        let errors = compile_errors(root, &output.stdout);
        if errors.is_empty() {
            return Err(Error::Build(String::from_utf8_lossy(&output.stderr).into_owned()));
        }
        return Err(Error::Compile(errors));
    }
    for line in output.stdout.split(|b| *b == b'\n') {
        let Ok(message) = serde_json::from_slice::<Value>(line) else { continue };
        if message["reason"] != "compiler-artifact" || message["target"]["name"] != generate::TARGET {
            continue;
        }
        let files = message["filenames"].as_array().into_iter().flatten().filter_map(|f| f.as_str());
        let mut libraries = files.map(PathBuf::from).filter(|f| {
            f.extension().is_some_and(|e| e == std::env::consts::DLL_EXTENSION)
        });
        if let Some(library) = libraries.next() {
            return Ok(library);
        }
    }
    Err(Error::Build("cargo gave no library for the cluster".to_owned()))
}

/// The errors in the JSON lines of cargo. The summary line "aborting due to ..." is not one.
fn compile_errors(root: &Path, stdout: &[u8]) -> Vec<Diagnostic> {
    let lines = stdout.split(|b| *b == b'\n').filter_map(|line| serde_json::from_slice::<Value>(line).ok());
    let messages = lines.filter(|line| line["reason"] == "compiler-message").map(|line| line["message"].clone());
    let errors = messages.filter(|m| m["level"].as_str().is_some_and(|level| level.starts_with("error")));
    errors.filter_map(|m| diagnostic(root, &m)).collect()
}

fn diagnostic(root: &Path, message: &Value) -> Option<Diagnostic> {
    let text = message["message"].as_str().unwrap_or_default();
    let spans = message["spans"].as_array().map_or(&[][..], Vec::as_slice);
    if spans.is_empty() && text.starts_with("aborting due to") {
        return None;
    }
    let span = spans.iter().find(|s| s["is_primary"] == true);
    let number = |key: &str| span.and_then(|s| s[key].as_u64()).and_then(|n| u32::try_from(n).ok()).unwrap_or(0);
    let file = span.and_then(|s| s["file_name"].as_str()).and_then(|name| project_file(root, name));
    Some(Diagnostic {
        message: text.to_owned(),
        label: span.and_then(|s| s["label"].as_str()).map(str::to_owned),
        rendered: message["rendered"].as_str().unwrap_or(text).to_owned(),
        file,
        start: [number("line_start"), number("column_start")],
        end: [number("line_end"), number("column_end")],
    })
}

/// A path of rustc as a path from the project root with `/`, or none for a file outside it.
fn project_file(root: &Path, name: &str) -> Option<String> {
    let path = Path::new(name);
    let relative = if path.is_absolute() { path.strip_prefix(root).ok()? } else { path };
    let parts: Option<Vec<&str>> = relative.components().map(|c| c.as_os_str().to_str()).collect();
    Some(parts?.join("/"))
}

/// Writes a file only when its content changes, so that cargo does not build it again.
fn write(root: &Path, relative: &str, text: &str) -> Result<(), Error> {
    let path = root.join(relative);
    if std::fs::read_to_string(&path).is_ok_and(|old| old == text) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::Io(dir.to_owned(), e))?;
    }
    std::fs::write(&path, text).map_err(|e| Error::Io(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn message(level: &str, text: &str, file: &Path) -> Value {
        let span = json!({ "is_primary": true, "file_name": file, "line_start": 3, "column_start": 5, "line_end": 3, "column_end": 9, "label": "here" });
        json!({ "reason": "compiler-message", "message": { "level": level, "message": text, "rendered": text, "spans": [span] } })
    }

    #[test]
    fn compile_errors_point_to_project_files() {
        let root = std::env::temp_dir().join("patina-errors");
        let relative = Path::new("nodes").join("a").join("src").join("lib.rs");
        let lines = [
            message("error", "bad", &root.join(&relative)),
            message("error", "bad too", &relative),
            message("error", "outside", &std::env::temp_dir().join("crate").join("lib.rs")),
            message("warning", "unused", &relative),
            json!({ "reason": "compiler-message", "message": { "level": "error", "message": "aborting due to 2 previous errors", "spans": [] } }),
        ];
        let stdout = lines.iter().map(|l| format!("{l}\nnot json\n")).collect::<String>();
        let errors = compile_errors(&root, stdout.as_bytes());
        let files: Vec<Option<&str>> = errors.iter().map(|e| e.file.as_deref()).collect();
        assert_eq!(files, [Some("nodes/a/src/lib.rs"), Some("nodes/a/src/lib.rs"), None]);
        assert_eq!((errors[0].start, errors[0].end, errors[0].label.as_deref()), ([3, 5], [3, 9], Some("here")));
    }
}
