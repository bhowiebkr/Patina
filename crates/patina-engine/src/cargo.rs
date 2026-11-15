//! Finds the Rust toolchain and builds the cluster crate with cargo (spec 8).

use std::ffi::OsString;
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

/// The variables that a command of the own toolchain keeps. The engine removes all the others, so
/// no tool finds Visual Studio, a C compiler or another Rust install through them (13.2).
#[cfg(windows)]
const KEPT_ENV: &[&str] = &[
    "SystemRoot",
    "SystemDrive",
    "windir",
    "TEMP",
    "TMP",
    "ComSpec",
    "PATHEXT",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "OS",
];
#[cfg(not(windows))]
const KEPT_ENV: &[&str] = &["HOME", "USER", "LANG", "TMPDIR"];

/// The manifest of the project workspace. Cargo fails on a member glob that matches no folder, so
/// `nodes/*`, `connections/*` (6.3) and `build/clusters/*` are members only when they have a folder.
fn workspace(root: &Path) -> String {
    let used = |group: &&str| std::fs::read_dir(root.join(group)).into_iter().flatten().flatten().any(|e| e.path().is_dir());
    let members: Vec<String> =
        ["nodes", "connections", "build/clusters"].into_iter().filter(used).map(|g| format!("\"{g}/*\"")).collect();
    format!(
        "# Made by Patina. Patina writes this file again at each build.\n[workspace]\nresolver = \"3\"\n\
         members = [{}]\n\n[workspace.dependencies]\npatina-sdk = {{ path = \"build/sdk/patina-sdk\" }}\n",
        members.join(", ")
    )
}

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
    /// The toolchain folder of Patina (13.3), or none for the Rust of the system.
    pub own: Option<PathBuf>,
}

impl Toolchain {
    /// Finds the toolchain in the toolchain folder of Patina (13.3). When the folder has no cargo,
    /// it finds the Rust of the system, so a checkout of Patina builds without the folder.
    pub fn find(folder: &Path) -> Result<Toolchain, Error> {
        if !Toolchain::installed(folder) {
            return Toolchain::detect();
        }
        let bin = folder.join("cargo").join("bin");
        let cargo = bin.join(format!("cargo{}", std::env::consts::EXE_SUFFIX));
        let rustc = bin.join(format!("rustc{}", std::env::consts::EXE_SUFFIX));
        Toolchain { cargo, rustc, release: String::new(), own: Some(folder.to_owned()) }.with_release()
    }

    /// True when the toolchain folder has cargo.
    pub fn installed(folder: &Path) -> bool {
        folder.join("cargo").join("bin").join(format!("cargo{}", std::env::consts::EXE_SUFFIX)).is_file()
    }

    /// Finds the Rust of the system: the toolchain of the `CARGO` variable, or else of `PATH`.
    pub fn detect() -> Result<Toolchain, Error> {
        let cargo = std::env::var_os("CARGO").map_or_else(|| PathBuf::from("cargo"), PathBuf::from);
        let rustc_name = format!("rustc{}", std::env::consts::EXE_SUFFIX);
        let rustc = cargo
            .parent()
            .map(|dir| dir.join(&rustc_name))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from(&rustc_name));
        Toolchain { cargo, rustc, release: String::new(), own: None }.with_release()
    }

    /// Reads the Rust version and uses the programs of the sysroot. They are not rustup proxies,
    /// so a `rust-toolchain.toml` in the project cannot select another toolchain.
    fn with_release(mut self) -> Result<Toolchain, Error> {
        let version = run_text(self.command(&self.rustc)?.arg("-vV"))?;
        self.release = version
            .lines()
            .find_map(|line| line.strip_prefix("release:"))
            .map(|value| value.trim().to_owned())
            .ok_or_else(|| Error::Build(format!("{} -vV gave no release", self.rustc.display())))?;
        let sysroot = run_text(self.command(&self.rustc)?.args(["--print", "sysroot"]))?;
        let bin = PathBuf::from(sysroot.trim()).join("bin");
        for (file, name) in [(&mut self.cargo, "cargo"), (&mut self.rustc, "rustc")] {
            let found = bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
            if found.is_file() {
                *file = found;
            }
        }
        Ok(self)
    }

    /// A command of this toolchain. The own toolchain gets the environment of a clean computer
    /// (13.2). The Rust of the system loses only the variables that change a build.
    pub fn command(&self, program: &Path) -> Result<Command, Error> {
        let mut command = command(program);
        match &self.own {
            None => {
                for var in CLEARED_ENV {
                    command.env_remove(var);
                }
            }
            Some(folder) => {
                command.env_clear().envs(own_env(folder, |name| std::env::var_os(name))?);
            }
        }
        Ok(command)
    }
}

/// The environment of the own toolchain: `PATH` has only the Rust tools and the system folder.
/// On Windows, the Zig wrappers compile the C code of crates and `rust-lld` links (13.2).
pub(crate) fn own_env(folder: &Path, var: impl Fn(&str) -> Option<OsString>) -> Result<Vec<(String, OsString)>, Error> {
    let mut env: Vec<(String, OsString)> = KEPT_ENV.iter().filter_map(|name| Some((name.to_string(), var(name)?))).collect();
    let cargo_home = folder.join("cargo");
    let system: Vec<PathBuf> = if cfg!(windows) {
        let root = var("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
        vec![Path::new(&root).join("System32")]
    } else {
        vec!["/usr/bin".into(), "/bin".into()]
    };
    let path = std::env::join_paths(std::iter::once(cargo_home.join("bin")).chain(system))
        .map_err(|e| Error::Build(format!("the toolchain folder {} cannot go in PATH: {e}", folder.display())))?;
    env.push(("PATH".to_owned(), path));
    env.push(("RUSTUP_HOME".to_owned(), folder.join("rustup").into()));
    env.push(("CARGO_HOME".to_owned(), cargo_home.into()));
    if cfg!(windows) {
        let zig = folder.join("zig");
        for (name, file) in [("CC", "zig-cc.exe"), ("AR", "zig-ar.exe")] {
            if zig.join(file).is_file() {
                env.push((format!("{name}_x86_64_pc_windows_gnu"), zig.join(file).into()));
            }
        }
        // GNU ld reads ANSI arguments, so it cannot open a path with CJK letters. rust-lld can.
        env.push(("CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER".to_owned(), "rust-lld".into()));
    }
    Ok(env)
}

/// A command that opens no console window, also when a GUI program starts it on Windows.
pub(crate) fn command(program: &Path) -> Command {
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

pub(crate) fn run_text(command: &mut Command) -> Result<String, Error> {
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
    write(root, "build/clusters/main/Cargo.toml", &generate::manifest(plan))?;
    write(root, "build/clusters/main/lib.rs", &generate::source(plan))?;
    write_workspace(root)?;

    let mut command = toolchain.command(&toolchain.cargo)?;
    command
        .current_dir(root)
        .args(["build", "-p", generate::PACKAGE, "--message-format=json"]);
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

/// Writes the manifest of the project workspace and the SDK crates.
fn write_workspace(root: &Path) -> Result<(), Error> {
    write(root, "Cargo.toml", &workspace(root))?;
    for (path, text) in SDK_FILES {
        write(root, path, text)?;
    }
    Ok(())
}

/// Runs `cargo add` for the node folder `nodes/<node>` and returns the new text of its `Cargo.toml`
/// (15.4). The file keeps its old text, so the caller applies the new text as one edit with undo.
pub fn add_crate(root: &Path, toolchain: &Toolchain, node: &str, spec: &str) -> Result<String, String> {
    crate::plan::check_name(node)?;
    check_crate(spec)?;
    let relative = format!("nodes/{node}/Cargo.toml");
    let manifest = root.join(&relative);
    let old = std::fs::read_to_string(&manifest).map_err(|e| format!("{relative}: {e}"))?;
    write_workspace(root).map_err(|e| e.to_string())?;
    let mut command = toolchain.command(&toolchain.cargo).map_err(|e| e.to_string())?;
    command.current_dir(root).arg("add").arg("--manifest-path").arg(&manifest).arg("--").arg(spec);
    let output = command.output().map_err(|e| format!("cannot start {}: {e}", toolchain.cargo.display()));
    let new = std::fs::read_to_string(&manifest).map_err(|e| format!("{relative}: {e}"));
    write(root, &relative, &old).map_err(|e| e.to_string())?;
    let output = output?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.lines().find_map(|line| line.strip_prefix("error: ")).unwrap_or(stderr.trim()).to_owned());
    }
    let new = new?;
    if new == old {
        return Err(format!("the Cargo.toml of {node} has {spec} already"));
    }
    Ok(new)
}

/// Checks a crate of crates.io as `name` or `name@version`. Cargo gets the text as an argument, so
/// it cannot be an option, a path or a second crate.
fn check_crate(spec: &str) -> Result<(), String> {
    let (name, version) = spec.split_once('@').unwrap_or((spec, "1"));
    let name_ok = name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let version_ok = (1..=32).contains(&version.len()) && version.chars().all(|c| c.is_ascii_alphanumeric() || ".+^~=<>*-".contains(c));
    if name_ok && version_ok {
        Ok(())
    } else {
        Err("type a crate of crates.io as name or name@version, for example rand or rand@0.9".to_owned())
    }
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
    fn a_crate_is_a_name_and_a_version() {
        for good in ["rand", "rand@0.9", "serde_json@=1.0.100", "a-b_c@^1.2-beta.1"] {
            assert!(check_crate(good).is_ok(), "{good}");
        }
        for bad in ["", "-x", "--path=..", "../x", "a b", "rand@", "rand@0.9 x", "1rand", "@1", "a@b@c"] {
            assert!(check_crate(bad).is_err(), "{bad}");
        }
    }

    /// Run it with `cargo test -p patina-engine -- --ignored`. It reads the index of crates.io.
    #[test]
    #[ignore = "needs the network"]
    fn cargo_adds_a_crate_to_a_node() {
        let root = std::env::temp_dir().join(format!("patina-add-crate-{}", std::process::id()));
        let manifest = "[package]\nname = \"num\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n";
        write(&root, "nodes/num/Cargo.toml", manifest).unwrap();
        write(&root, "nodes/num/src/lib.rs", "").unwrap();
        let toolchain = Toolchain::detect().unwrap();
        let text = add_crate(&root, &toolchain, "num", "itoa@1").unwrap();
        assert!(text.contains("itoa = \"1"), "{text}");
        assert_eq!(std::fs::read_to_string(root.join("nodes/num/Cargo.toml")).unwrap(), manifest);
        let error = add_crate(&root, &toolchain, "num", "patina-no-such-crate-x").unwrap_err();
        assert!(error.contains("patina-no-such-crate-x"), "{error}");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_own_toolchain_has_a_clean_environment() {
        let folder = std::env::temp_dir().join(format!("patina-toolchain-{}", std::process::id()));
        let names = |folder: &Path| -> Vec<String> {
            let env = own_env(folder, |name| Some(format!("old {name}").into())).unwrap();
            let path = &env.iter().find(|(name, _)| name == "PATH").unwrap().1;
            assert_eq!(std::env::split_paths(path).next(), Some(folder.join("cargo").join("bin")));
            env.into_iter().map(|(name, _)| name).collect()
        };
        let first = names(&folder);
        assert!(first.contains(&"CARGO_HOME".to_owned()) && !first.iter().any(|n| n.starts_with("CC_") || n == "RUSTFLAGS"));
        if cfg!(windows) {
            std::fs::create_dir_all(folder.join("zig")).unwrap();
            std::fs::write(folder.join("zig").join("zig-cc.exe"), "").unwrap();
            assert!(names(&folder).contains(&"CC_x86_64_pc_windows_gnu".to_owned()));
            std::fs::remove_dir_all(&folder).unwrap();
        }
        // A folder with no cargo gives the Rust of the system.
        assert_eq!(Toolchain::find(&folder).unwrap().own, None);
    }

    #[test]
    fn a_member_glob_needs_a_folder() {
        let root = std::env::temp_dir().join(format!("patina-members-{}", std::process::id()));
        std::fs::create_dir_all(root.join("connections")).unwrap();
        assert!(workspace(&root).contains("members = []"));
        std::fs::create_dir_all(root.join("build/clusters/main")).unwrap();
        assert!(workspace(&root).contains("members = [\"build/clusters/*\"]"));
        std::fs::create_dir_all(root.join("nodes/num")).unwrap();
        assert!(workspace(&root).contains("members = [\"nodes/*\", \"build/clusters/*\"]"), "an empty folder matches no member");
        std::fs::create_dir_all(root.join("connections/byte")).unwrap();
        assert!(workspace(&root).contains("members = [\"nodes/*\", \"connections/*\", \"build/clusters/*\"]"));
        std::fs::remove_dir_all(root).unwrap();
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
