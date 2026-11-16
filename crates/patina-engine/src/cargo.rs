//! Finds the Rust toolchain and builds the cluster crates with cargo (spec 8).

use std::collections::HashMap;
use std::ffi::OsString;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use serde_json::Value;

use crate::Error;
use crate::generate::{self, slot};
use crate::node::Domain;
use crate::plan::Plan;

mod sdk;
use sdk::SDK_FILES;

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
/// `nodes/*`, `views/*`, `widgets/*`, `connections/*` and `build/clusters/*` are members only with a folder.
fn workspace(root: &Path) -> String {
    let used = |group: &&str| std::fs::read_dir(root.join(group)).into_iter().flatten().flatten().any(|e| e.path().is_dir());
    let members: Vec<String> =
        ["nodes", "views", "widgets", "connections", "build/clusters"].into_iter().filter(used).map(|g| format!("\"{g}/*\"")).collect();
    // The debug info of a large cluster function overflows the stack of LLVM on macOS.
    let mut clusters: Vec<String> = std::fs::read_dir(root.join("build/clusters"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| format!("\n[profile.edit.package.cluster-{}]\ndebug = false\n", e.file_name().to_string_lossy()))
        .collect();
    clusters.sort();
    format!(
        "# Made by Patina. Patina writes this file again at each build.\n[workspace]\nresolver = \"3\"\n\
         members = [{}]\n\n[workspace.dependencies]\npatina-sdk = {{ path = \"build/sdk/patina-sdk\" }}\n\
         patina-gpu = {{ path = \"build/sdk/patina-gpu\" }}\npatina-cuda = {{ path = \"build/sdk/patina-cuda\" }}\n\
         patina-event = {{ path = \"build/sdk/patina-event\" }}\n{PROFILES}{}",
        members.join(", "),
        clusters.concat()
    )
}

/// The build modes (8.3). Both have no debug assertions, so `cfg(debug_assertions)` and the
/// layout of each type are the same in the two modes, and a state moves between them (7.2).
const PROFILES: &str = "\n[profile.edit]\ninherits = \"dev\"\ndebug-assertions = false\n\n\
                        [profile.bake]\ninherits = \"release\"\ncodegen-units = 1\n";

/// The build mode of the clusters (8.3). The slot always builds in edit mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// A fast build with low optimization.
    Edit,
    /// A slow build with full optimization, for fast execution.
    Bake,
}

impl Mode {
    fn profile(self) -> &'static str {
        match self {
            Mode::Edit => "edit",
            Mode::Bake => "bake",
        }
    }
}

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
    /// The host target, for example `x86_64-pc-windows-msvc`.
    pub host: String,
    /// The folder of the libraries of the host target, with the shared `std` library.
    pub libdir: PathBuf,
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
        let own = Some(folder.to_owned());
        Toolchain { cargo, rustc, release: String::new(), host: String::new(), libdir: PathBuf::new(), own }.with_release()
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
        Toolchain { cargo, rustc, release: String::new(), host: String::new(), libdir: PathBuf::new(), own: None }
            .with_release()
    }

    /// Reads the Rust version and uses the programs of the sysroot. They are not rustup proxies,
    /// so a `rust-toolchain.toml` in the project cannot select another toolchain.
    fn with_release(mut self) -> Result<Toolchain, Error> {
        let version = run_text(self.command(&self.rustc)?.arg("-vV"))?;
        let field = |name: &str| {
            let value = version.lines().find_map(|line| line.strip_prefix(name)).map(|value| value.trim().to_owned());
            value.ok_or_else(|| Error::Build(format!("{} -vV gave no {name}", self.rustc.display())))
        };
        self.release = field("release:")?;
        self.host = field("host:")?;
        let sysroot = PathBuf::from(run_text(self.command(&self.rustc)?.args(["--print", "sysroot"]))?.trim());
        self.libdir = sysroot.join("lib").join("rustlib").join(&self.host).join("lib");
        let bin = sysroot.join("bin");
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

    /// The `rustc` flags of a build. Each library links `std` as a shared library, so the clusters
    /// and the slot use one copy of `std` (5.2). The link gives the same bytes for the same code
    /// (8.5). On Linux and macOS, a library finds `std` in its own folder.
    fn rustflags(&self) -> String {
        let mut flags = vec!["-Cprefer-dynamic".to_owned()];
        if self.host.ends_with("windows-msvc") {
            flags.push("-Clink-arg=/Brepro".to_owned());
        } else if let Some(folder) = self.own.as_ref().filter(|_| self.host.ends_with("windows-gnu")) {
            // The own toolchain links with rust-lld (13.2). The flag of another linker is not known.
            flags.push("-Clink-arg=--no-insert-timestamp".to_owned());
            // rustc makes the import libraries of `std` with dlltool. The binutils dlltool reads ANSI arguments.
            flags.push(format!("-Cdlltool={}", folder.join("zig").join("zig-dlltool.exe").display()));
        } else if self.host.contains("apple") {
            flags.push("-Clink-arg=-Wl,-rpath,@loader_path".to_owned());
        } else if !self.host.contains("windows") {
            flags.push("-Clink-arg=-Wl,-rpath,$ORIGIN".to_owned());
        }
        flags.join("\x1f")
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

/// A built graph: the library of the cluster of each domain (8.2), and the state key of each step
/// (7.2).
pub struct Built {
    pub libraries: Vec<(Domain, PathBuf)>,
    /// The slot library (7.1), when the plan has a node in the slot.
    pub slot: Option<PathBuf>,
    /// A hash of all that can change the layout of the state type of each stateful step. A state
    /// moves to the next cluster only when its key did not change.
    pub keys: Vec<Option<u64>>,
    /// The part of the keys that all steps share. A build of the slot alone uses it again.
    pub common: u64,
}

/// Writes the files that Patina makes, builds the clusters in one cargo run and returns their
/// libraries. The clusters build in `mode`, and the slot library in edit mode.
pub fn build(root: &Path, toolchain: &Toolchain, plan: &Plan, mode: Mode) -> Result<Built, Error> {
    let domains = generate::domains(plan);
    remove_old_clusters(root, &domains, plan.slot.is_some())?;
    for &domain in &domains {
        let folder = format!("build/clusters/{}", domain.name());
        write(root, &format!("{folder}/Cargo.toml"), &generate::manifest(plan, domain))?;
        write(root, &format!("{folder}/lib.rs"), &generate::source(plan, domain))?;
    }
    if let Some(slot) = plan.slot {
        write_slot(root, plan, slot)?;
    }
    write_workspace(root)?;
    let packages: Vec<String> = domains.iter().map(|&d| generate::package(d)).collect();
    let mut messages = run_cargo(root, toolchain, &packages, mode)?;
    if plan.slot.is_some() {
        messages.extend(run_cargo(root, toolchain, &[slot::PACKAGE.to_owned()], Mode::Edit)?);
    }
    let mut libraries = Vec::new();
    let mut clusters = Vec::new();
    for domain in domains {
        let (library, id) = artifact(&messages, &generate::target(domain))?;
        libraries.push((domain, library));
        clusters.push(id);
    }
    let slot = match plan.slot {
        Some(_) => {
            let (library, id) = artifact(&messages, slot::TARGET)?;
            clusters.push(id);
            Some(library)
        }
        None => None,
    };
    let (common, keys) = keys(toolchain, plan, &messages, &clusters, None)?;
    Ok(Built { libraries, slot, keys, common })
}

/// Builds only the slot library (7.2). Gives its library and the state keys of the uses of the
/// node in the slot. `common` is the shared part of the keys of the last full build.
pub fn build_slot(root: &Path, toolchain: &Toolchain, plan: &Plan, common: u64) -> Result<(PathBuf, Vec<Option<u64>>), Error> {
    let slot = plan.slot.ok_or_else(|| Error::Build("no node is in the slot".to_owned()))?;
    write_slot(root, plan, slot)?;
    let messages = run_cargo(root, toolchain, &[slot::PACKAGE.to_owned()], Mode::Edit)?;
    let (library, id) = artifact(&messages, slot::TARGET)?;
    let (_, keys) = keys(toolchain, plan, &messages, &[id], Some(common))?;
    Ok((library, keys))
}

/// Builds the view folder `views/<name>` and gives its library (15.8).
pub fn build_view(root: &Path, toolchain: &Toolchain, name: &str, mode: Mode) -> Result<PathBuf, Error> {
    crate::plan::check_name(name).map_err(Error::Build)?;
    write_workspace(root)?;
    let messages = run_cargo(root, toolchain, &[name.to_owned()], mode)?;
    artifact(&messages, &name.replace('-', "_")).map(|(library, _)| library)
}

fn write_slot(root: &Path, plan: &Plan, slot: usize) -> Result<(), Error> {
    write(root, "build/clusters/slot/Cargo.toml", &slot::manifest(plan, slot))?;
    write(root, "build/clusters/slot/lib.rs", &slot::source(plan, slot))
}

/// Runs one cargo build of `packages` in `mode`, and gives its JSON messages.
fn run_cargo(root: &Path, toolchain: &Toolchain, packages: &[String], mode: Mode) -> Result<Vec<Value>, Error> {
    let mut command = toolchain.command(&toolchain.cargo)?;
    command.current_dir(root).arg("build").arg("--profile").arg(mode.profile());
    for package in packages {
        command.arg("-p").arg(package);
    }
    command.arg("--message-format=json");
    command.env("CARGO_TARGET_DIR", root.join("build/target")).env("RUSTC", &toolchain.rustc);
    command.env("CARGO_ENCODED_RUSTFLAGS", toolchain.rustflags());
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
    Ok(output.stdout.split(|b| *b == b'\n').filter_map(|line| serde_json::from_slice(line).ok()).collect())
}

/// The library of the library target `target`, and the ID of its package.
fn artifact<'a>(messages: &'a [Value], target: &str) -> Result<(PathBuf, &'a str), Error> {
    let cluster = messages
        .iter()
        .find(|m| m["reason"] == "compiler-artifact" && m["target"]["name"] == target)
        .ok_or_else(|| Error::Build(format!("cargo gave no artifact for {target}")))?;
    let files = cluster["filenames"].as_array().into_iter().flatten().filter_map(|f| f.as_str());
    let library = files
        .map(PathBuf::from)
        .find(|f| f.extension().is_some_and(|e| e == std::env::consts::DLL_EXTENSION))
        .ok_or_else(|| Error::Build(format!("cargo gave no library for {target}")))?;
    Ok((library, cluster["package_id"].as_str().unwrap_or_default()))
}

/// Removes each cluster folder of an older build that the plan does not use, because the member
/// glob of the workspace reads each folder in `build/clusters`.
fn remove_old_clusters(root: &Path, domains: &[Domain], slot: bool) -> Result<(), Error> {
    let folder = root.join("build/clusters");
    let Ok(entries) = std::fs::read_dir(&folder) else { return Ok(()) };
    for entry in entries.flatten() {
        let path = entry.path();
        let used = domains.iter().any(|d| entry.file_name() == d.name()) || (slot && entry.file_name() == "slot");
        if path.is_dir() && !used {
            std::fs::remove_dir_all(&path).map_err(|e| Error::Io(path, e))?;
        }
    }
    Ok(())
}

/// The state key of each step (7.2), from the units of the build. A unit of a node folder goes
/// only into the keys of that node. All other units go into all keys. The cluster crates change
/// with the wiring, so they are not in the keys. `common` replaces the shared part, for a build
/// of the slot alone. Gives the shared part and the keys.
fn keys(
    toolchain: &Toolchain,
    plan: &Plan,
    messages: &[Value],
    clusters: &[&str],
    common: Option<u64>,
) -> Result<(u64, Vec<Option<u64>>), Error> {
    let folders: HashMap<&str, &Path> = (messages.iter())
        .filter(|m| m["reason"] == "compiler-artifact")
        .filter_map(|m| Some((m["package_id"].as_str()?, Path::new(m["manifest_path"].as_str()?).parent()?)))
        .filter(|(id, _)| !clusters.contains(id))
        .collect();
    // A path package keeps its version when its files change, so its files go into the hash.
    let node_of = |id: &str| {
        let folder = folders.get(id).filter(|_| id.contains("path+file:"))?;
        plan.nodes.iter().position(|(name, _)| folder.ends_with(format!("nodes/{name}")))
    };
    let mut shared = Vec::new();
    let mut own = vec![Vec::new(); plan.nodes.len()];
    let mut add = |id: &str, hash: u64| match node_of(id) {
        Some(node) => own[node].push(hash),
        None => shared.push(hash),
    };
    for m in messages {
        let id = m["package_id"].as_str().unwrap_or_default();
        if clusters.contains(&id) || (m["reason"] != "compiler-artifact" && m["reason"] != "build-script-executed") {
            continue;
        }
        let mut hasher = DefaultHasher::new();
        for key in ["package_id", "target", "features", "cfgs", "env"] {
            m[key].to_string().hash(&mut hasher);
        }
        // The optimization does not change the layout of a type, so a state moves from an edit
        // build to a bake build. The debug assertions can change it through `cfg`.
        m["profile"]["debug_assertions"].to_string().hash(&mut hasher);
        add(id, hasher.finish());
    }
    for (id, folder) in folders.iter().filter(|(id, _)| id.contains("path+file:")) {
        add(id, folder_hash(folder)?);
    }
    // Cargo gives the units in an order that can change, so the hashes are sorted. A unit of two
    // cargo runs, for example of the clusters and of the slot, counts one time.
    shared.sort_unstable();
    shared.dedup();
    own.iter_mut().for_each(|units| {
        units.sort_unstable();
        units.dedup();
    });
    let common = common.unwrap_or_else(|| {
        let mut hasher = DefaultHasher::new();
        (&toolchain.rustc, &toolchain.release, &toolchain.host, &toolchain.libdir, toolchain.rustflags()).hash(&mut hasher);
        shared.hash(&mut hasher);
        hasher.finish()
    });
    let mut keys = vec![None; plan.steps.len()];
    for n in plan.stateful() {
        let step = &plan.steps[n];
        let mut hasher = DefaultHasher::new();
        (common, &own[step.node], &plan.nodes[step.node].0, &step.types).hash(&mut hasher);
        // The wire types set the parameters of a generic node, so they can change its state type.
        for source in &step.inputs {
            if let crate::plan::Source::Wire { step: from, output } = *source {
                plan.steps[from].types[output].hash(&mut hasher);
            }
        }
        keys[n] = Some(hasher.finish());
    }
    Ok((common, keys))
}

/// A hash of the paths and the bytes of the files of a crate folder.
fn folder_hash(folder: &Path) -> Result<u64, Error> {
    let mut files = Vec::new();
    crate::list_files(folder, ".", &mut files)?;
    files.sort();
    let mut hasher = DefaultHasher::new();
    for file in &files {
        let path = folder.join(file);
        (file, std::fs::read(&path).map_err(|e| Error::Io(path.clone(), e))?).hash(&mut hasher);
    }
    Ok(hasher.finish())
}

/// Builds the crates `packages` of the workspace, for example the folders of an import (6.2).
pub(crate) fn build_packages(root: &Path, toolchain: &Toolchain, packages: &[String]) -> Result<(), Error> {
    if packages.is_empty() {
        return Ok(());
    }
    write_workspace(root)?;
    run_cargo(root, toolchain, packages, Mode::Edit).map(drop)
}

/// Writes the manifest of the project workspace and the SDK crates.
fn write_workspace(root: &Path) -> Result<(), Error> {
    write(root, "Cargo.toml", &workspace(root))?;
    for (path, text) in SDK_FILES {
        write(root, path, text)?;
    }
    Ok(())
}

/// Runs `cargo add` for a code folder, for example `nodes/number`, and returns the new text of its
/// `Cargo.toml` (15.4). The file keeps its old text, so the caller applies the new text as one
/// edit with undo. A widget or type folder of the project goes in by its path (6.2).
pub fn add_crate(root: &Path, toolchain: &Toolchain, folder: &str, spec: &str) -> Result<String, String> {
    crate::code_folder(folder)?;
    check_crate(spec)?;
    let relative = format!("{folder}/Cargo.toml");
    let manifest = root.join(&relative);
    let old = std::fs::read_to_string(&manifest).map_err(|e| format!("{relative}: {e}"))?;
    write_workspace(root).map_err(|e| e.to_string())?;
    let mut command = toolchain.command(&toolchain.cargo).map_err(|e| e.to_string())?;
    command.current_dir(root).arg("add").arg("--manifest-path").arg(&manifest);
    let local = ["widgets", "types"].map(|group| root.join(group).join(spec));
    if let Some(local) = local.into_iter().find(|dir| dir.is_dir()) {
        command.arg("--path").arg(local);
    } else {
        command.arg("--").arg(spec);
    }
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
        return Err(format!("the Cargo.toml of {folder} has {spec} already"));
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
    fn the_sdk_manifests_have_the_dependencies_of_the_crates() {
        let gpu = include_str!("../../patina-gpu/Cargo.toml");
        let cuda = include_str!("../../patina-cuda/Cargo.toml");
        let event = include_str!("../../patina-event/Cargo.toml");
        for (name, own) in [("patina-gpu", gpu), ("patina-cuda", cuda), ("patina-event", event)] {
            let sdk = SDK_FILES.iter().find(|(path, _)| path.ends_with(&format!("{name}/Cargo.toml"))).unwrap().1;
            for line in own.lines().filter(|l| ["wgpu", "wesl", "cudarc", "tokio", "rust-version"].iter().any(|k| l.starts_with(k))) {
                assert!(sdk.contains(line), "{line}");
            }
        }
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
        let text = add_crate(&root, &toolchain, "nodes/num", "itoa@1").unwrap();
        assert!(text.contains("itoa = \"1"), "{text}");
        assert_eq!(std::fs::read_to_string(root.join("nodes/num/Cargo.toml")).unwrap(), manifest);
        let error = add_crate(&root, &toolchain, "nodes/num", "patina-no-such-crate-x").unwrap_err();
        assert!(error.contains("patina-no-such-crate-x"), "{error}");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_type_folder_goes_in_by_its_path() {
        let root = std::env::temp_dir().join(format!("patina-add-type-{}", std::process::id()));
        for name in ["nodes/num", "types/digit"] {
            let manifest = format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n", &name[6..]);
            write(&root, &format!("{name}/Cargo.toml"), &manifest).unwrap();
            write(&root, &format!("{name}/src/lib.rs"), "").unwrap();
        }
        let text = add_crate(&root, &Toolchain::detect().unwrap(), "nodes/num", "digit").unwrap();
        assert!(text.contains("path = \"../../types/digit\""), "{text}");
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
    fn each_host_gets_its_link_flags() {
        let flags = |host: &str, own: Option<&str>| {
            let (cargo, rustc, release, libdir) = (PathBuf::new(), PathBuf::new(), String::new(), PathBuf::new());
            Toolchain { cargo, rustc, release, host: host.to_owned(), libdir, own: own.map(PathBuf::from) }.rustflags()
        };
        assert_eq!(flags("x86_64-pc-windows-msvc", None), "-Cprefer-dynamic\x1f-Clink-arg=/Brepro");
        assert_eq!(flags("x86_64-pc-windows-gnu", None), "-Cprefer-dynamic");
        let own = flags("x86_64-pc-windows-gnu", Some("t"));
        assert!(own.contains("\x1f-Clink-arg=--no-insert-timestamp\x1f-Cdlltool=t") && own.ends_with("zig-dlltool.exe"), "{own}");
        assert!(flags("x86_64-unknown-linux-gnu", None).ends_with("\x1f-Clink-arg=-Wl,-rpath,$ORIGIN"));
        assert!(flags("aarch64-apple-darwin", None).ends_with("\x1f-Clink-arg=-Wl,-rpath,@loader_path"));
    }

    #[test]
    fn a_member_glob_needs_a_folder() {
        let root = std::env::temp_dir().join(format!("patina-members-{}", std::process::id()));
        std::fs::create_dir_all(root.join("connections")).unwrap();
        assert!(workspace(&root).contains("members = []"));
        std::fs::create_dir_all(root.join("build/clusters/main")).unwrap();
        assert!(workspace(&root).contains("members = [\"build/clusters/*\"]"));
        assert!(workspace(&root).ends_with("\n[profile.edit.package.cluster-main]\ndebug = false\n"));
        std::fs::create_dir_all(root.join("nodes/num")).unwrap();
        assert!(workspace(&root).contains("members = [\"nodes/*\", \"build/clusters/*\"]"), "an empty folder matches no member");
        std::fs::create_dir_all(root.join("connections/byte")).unwrap();
        assert!(workspace(&root).contains("members = [\"nodes/*\", \"connections/*\", \"build/clusters/*\"]"));
        std::fs::create_dir_all(root.join("widgets/bar")).unwrap();
        assert!(workspace(&root).contains("members = [\"nodes/*\", \"widgets/*\", \"connections/*\", \"build/clusters/*\"]"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn only_the_clusters_of_the_plan_stay() {
        let root = std::env::temp_dir().join(format!("patina-old-clusters-{}", std::process::id()));
        for folder in ["main", "batch", "frame", "slot"] {
            std::fs::create_dir_all(root.join("build/clusters").join(folder)).unwrap();
        }
        // The order of read_dir depends on the file system, so the names are sorted.
        let left = || -> Vec<_> {
            let mut names: Vec<_> = std::fs::read_dir(root.join("build/clusters")).unwrap().flatten().map(|e| e.file_name()).collect();
            names.sort();
            names
        };
        remove_old_clusters(&root, &[Domain::Batch], true).unwrap();
        assert_eq!(left(), ["batch", "slot"]);
        remove_old_clusters(&root, &[Domain::Batch], false).unwrap();
        assert_eq!(left(), ["batch"]);
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
