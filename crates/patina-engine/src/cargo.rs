//! Finds the Rust toolchain and builds the cluster crate with cargo (spec 8).

use std::path::{Path, PathBuf};
use std::process::Command;

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
const SDK_FILES: [(&str, &str); 5] = [
    (
        "build/sdk/patina-sdk/Cargo.toml",
        "[package]\nname = \"patina-sdk\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [dependencies]\npatina-macros = { path = \"../patina-macros\" }\n",
    ),
    ("build/sdk/patina-sdk/src/lib.rs", include_str!("../../patina-sdk/src/lib.rs")),
    ("build/sdk/patina-sdk/src/glue.rs", include_str!("../../patina-sdk/src/glue.rs")),
    (
        "build/sdk/patina-macros/Cargo.toml",
        "[package]\nname = \"patina-macros\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n\
         [lib]\nproc-macro = true\n",
    ),
    ("build/sdk/patina-macros/src/lib.rs", include_str!("../../patina-macros/src/lib.rs")),
];

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
        let version = run_text(Command::new(&rustc).arg("-vV"))?;
        let release = version
            .lines()
            .find_map(|line| line.strip_prefix("release:"))
            .map(|value| value.trim().to_owned())
            .ok_or_else(|| Error::Build(format!("{} -vV gave no release", rustc.display())))?;
        // The programs in the sysroot are not rustup proxies, so a `rust-toolchain.toml` in the
        // project cannot select another toolchain.
        let bin = PathBuf::from(run_text(Command::new(&rustc).args(["--print", "sysroot"]))?.trim()).join("bin");
        let pick = |name: String, found: PathBuf| Some(bin.join(name)).filter(|p| p.is_file()).unwrap_or(found);
        Ok(Toolchain { cargo: pick(format!("cargo{exe}"), cargo), rustc: pick(rustc_name, rustc), release })
    }
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

    let mut command = Command::new(&toolchain.cargo);
    command
        .current_dir(root)
        .args(["build", "-p", generate::PACKAGE, "--message-format=json-render-diagnostics"]);
    for var in CLEARED_ENV {
        command.env_remove(var);
    }
    command.env("CARGO_TARGET_DIR", root.join("build/target")).env("RUSTC", &toolchain.rustc);
    let output = command
        .output()
        .map_err(|e| Error::Build(format!("cannot start {}: {e}", toolchain.cargo.display())))?;
    if !output.status.success() {
        return Err(Error::Build(String::from_utf8_lossy(&output.stderr).into_owned()));
    }
    for line in output.stdout.split(|b| *b == b'\n') {
        let Ok(message) = serde_json::from_slice::<serde_json::Value>(line) else { continue };
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
