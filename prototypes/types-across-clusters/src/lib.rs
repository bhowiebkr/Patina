//! The harness of the milestone 0 prototype "Rust types across clusters" (spec 19).
//!
//! The harness does the work of the engine and the build system for this prototype only:
//!
//! - It copies the user project in `project/` to a work folder.
//! - It generates the shared types library and builds the clusters with `-C prefer-dynamic`.
//! - It installs the libraries into one `build/` folder with new file names (spec 8.5).
//! - It loads the clusters, compares the type keys and passes erased values (spec 5.2).

pub mod abi;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use abi::{ErasedValue, INPUT, InputArg, KeyHasher, OUTPUT, PinDesc, ProbeStats, SinkSummary};
use abi::TypeKey;

/// The folder of the user project that the harness copies.
pub const TEMPLATE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/project");

/// The environment variables that the harness removes before it starts cargo. They come from
/// the cargo process that runs the tests, and they would change the build of the user project.
const CLEARED_ENV: &[&str] = &[
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

fn error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn run_text(command: &mut Command) -> io::Result<String> {
    let output = command.output()?;
    if !output.status.success() {
        return Err(error(format!(
            "{command:?} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    String::from_utf8(output.stdout).map_err(|e| error(e.to_string()))
}

/// The Rust toolchain that runs the tests. The harness builds the user project with it.
pub struct Toolchain {
    pub cargo: PathBuf,
    pub rustc: PathBuf,
    /// The output of `rustc -vV`.
    pub version_text: String,
    pub release: String,
    pub host: String,
    pub sysroot: PathBuf,
}

impl Toolchain {
    pub fn detect() -> io::Result<Self> {
        let cargo = std::env::var_os("CARGO").map_or_else(|| PathBuf::from("cargo"), PathBuf::from);
        let rustc_name = format!("rustc{}", std::env::consts::EXE_SUFFIX);
        let rustc = cargo
            .parent()
            .map(|dir| dir.join(&rustc_name))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from(&rustc_name));
        let version_text = run_text(Command::new(&rustc).arg("-vV"))?;
        let field = |key: &str| {
            version_text
                .lines()
                .find_map(|line| line.strip_prefix(key))
                .map(|value| value.trim().to_owned())
                .ok_or_else(|| error(format!("no {key} in rustc -vV")))
        };
        let release = field("release:")?;
        let host = field("host:")?;
        let sysroot = run_text(Command::new(&rustc).args(["--print", "sysroot"]))?;
        Ok(Toolchain {
            cargo,
            rustc,
            release,
            host,
            sysroot: PathBuf::from(sysroot.trim()),
            version_text,
        })
    }
}

/// The compiler settings of a build. They are part of each build ID (spec 5.1).
pub struct Settings {
    pub target: String,
    pub profile: String,
    pub rustflags: String,
}

impl Settings {
    pub fn text(&self) -> String {
        format!(
            "target={}\nprofile={}\nrustflags={}\n",
            self.target, self.profile, self.rustflags
        )
    }
}

/// One file set that cargo made for one crate.
#[derive(Debug, Clone)]
pub struct Artifact {
    /// The name of the crate target, for example `cluster_a`.
    pub target: String,
    /// The folder name of the package, for example `cluster-a`.
    pub package: String,
    pub kinds: Vec<String>,
    pub files: Vec<PathBuf>,
}

pub struct BuildOutput {
    pub artifacts: Vec<Artifact>,
    pub seconds: f64,
}

impl BuildOutput {
    /// Returns the `.dll` file of a crate target.
    pub fn dll(&self, target: &str) -> io::Result<&Path> {
        self.artifacts
            .iter()
            .filter(|a| a.target == target)
            .flat_map(|a| &a.files)
            .find(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("dll")))
            .map(PathBuf::as_path)
            .ok_or_else(|| error(format!("no dll for {target}")))
    }
}

/// The libraries of one build, in the `build/` folder.
pub struct Installed {
    pub types: PathBuf,
    pub cluster_a: PathBuf,
    pub cluster_b: PathBuf,
}

/// Where the loader finds a library that a file imports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    /// The library is in the `build/` folder.
    Build,
    /// An API set of Windows, for example `api-ms-win-crt-heap-l1-1-0.dll`.
    ApiSet,
    /// The Microsoft Visual C++ runtime. A clean Windows does not have it.
    MsvcRuntime,
    /// A library in the system folder of Windows.
    System,
    /// The loader cannot find the library.
    Missing,
}

/// A copy of the user project and the folders of its builds.
pub struct Project {
    pub toolchain: Toolchain,
    pub settings: Settings,
    pub root: PathBuf,
    pub target_dir: PathBuf,
    pub build_dir: PathBuf,
    builds: u32,
}

impl Project {
    /// Copies the user project to `work/project`. Removes the old `work` folder first.
    pub fn from_template(work: &Path, profile: &str) -> io::Result<Self> {
        let work = std::path::absolute(work)?;
        if work.exists() {
            fs::remove_dir_all(&work)?;
        }
        let root = work.join("project");
        copy_dir(Path::new(TEMPLATE), &root)?;
        let build_dir = work.join("build");
        fs::create_dir_all(&build_dir)?;
        let toolchain = Toolchain::detect()?;
        let settings = Settings {
            target: toolchain.host.clone(),
            profile: profile.to_owned(),
            rustflags: "-C prefer-dynamic".to_owned(),
        };
        Ok(Project {
            toolchain,
            settings,
            root,
            target_dir: work.join("cargo-target"),
            build_dir,
            builds: 0,
        })
    }

    /// Replaces one exact text in a file of the project.
    pub fn replace_in_file(&self, relative: &str, from: &str, to: &str) -> io::Result<()> {
        let path = self.root.join(relative);
        let text = fs::read_to_string(&path)?;
        if !text.contains(from) {
            return Err(error(format!("{relative} does not contain {from:?}")));
        }
        fs::write(&path, text.replace(from, to))
    }

    /// Returns the build ID of a type crate (spec 5.1): a hash of each file of the crate, the
    /// compiler settings and the toolchain. The type crates of this project have no
    /// dependencies, so the hash has no resolved dependencies.
    pub fn crate_build_id(&self, dir: &Path) -> io::Result<String> {
        let mut files = Vec::new();
        list_files(dir, dir, &mut files)?;
        files.sort();
        let mut hasher = KeyHasher::new();
        for relative in &files {
            hasher.part(relative.as_bytes()).part(&fs::read(dir.join(relative))?);
        }
        hasher
            .part(self.settings.text().as_bytes())
            .part(self.toolchain.version_text.as_bytes());
        Ok(hasher.key().to_string())
    }

    /// Writes the generated files of `patina-types` and returns the name of its library.
    /// The name is a hash of the content, so a change to a type gives a new file name.
    pub fn generate_shared(&self) -> io::Result<String> {
        let crate_info = |konst: &str, dir: &str| -> io::Result<String> {
            let dir = self.root.join(dir);
            let manifest = fs::read_to_string(dir.join("Cargo.toml"))?;
            let name = manifest_field(&manifest, "name")?;
            let version = manifest_field(&manifest, "version")?;
            let id = self.crate_build_id(&dir)?;
            Ok(crate_const(konst, &name, &version, &id))
        };
        let std_id = KeyHasher::new()
            .part(b"std")
            .part(self.settings.text().as_bytes())
            .part(self.toolchain.version_text.as_bytes())
            .key()
            .to_string();
        let keys = format!(
            "// Generated by the prototype harness. Do not edit.\n\n\
             use crate::CrateInfo;\n\n\
             pub const TOOLCHAIN: &str = {:?};\n\n{}\n{}\n{}\n{}",
            self.toolchain.version_text,
            crate_const("STD", "std", &self.toolchain.release, &std_id),
            crate_info("GEOMETRY", "types/geometry")?,
            crate_info("UNITS_1", "types/units-1")?,
            crate_info("UNITS_2", "types/units-2")?,
        );
        let abi = include_str!("abi.rs");
        let dir = self.root.join("patina-types");
        let lib = fs::read_to_string(dir.join("src/lib.rs"))?;
        let hash = KeyHasher::new()
            .part(keys.as_bytes())
            .part(lib.as_bytes())
            .part(abi.as_bytes())
            .key();
        let lib_name = format!("patina_types_{:016x}", hash.hi);
        let manifest = format!(
            "# Generated by the prototype harness. Do not edit.\n\
             [package]\n\
             name = \"patina-types\"\n\
             version = \"0.1.0\"\n\
             edition = \"2024\"\n\
             publish = false\n\n\
             [lib]\n\
             name = \"{lib_name}\"\n\
             crate-type = [\"dylib\"]\n\n\
             [dependencies]\n\
             geometry = {{ path = \"../types/geometry\" }}\n\
             units1 = {{ package = \"units\", path = \"../types/units-1\" }}\n\
             units2 = {{ package = \"units\", path = \"../types/units-2\" }}\n"
        );
        fs::write(dir.join("Cargo.toml"), manifest)?;
        fs::write(dir.join("src/abi.rs"), abi)?;
        fs::write(dir.join("src/keys.rs"), keys)?;
        Ok(lib_name)
    }

    /// Builds the two clusters with cargo.
    pub fn build(&self) -> io::Result<BuildOutput> {
        let mut command = Command::new(&self.toolchain.cargo);
        command.current_dir(&self.root).args([
            "build",
            "-p",
            "cluster-a",
            "-p",
            "cluster-b",
            "--target",
            &self.settings.target,
            "--profile",
            &self.settings.profile,
            "--offline",
            "--message-format=json-render-diagnostics",
        ]);
        for var in CLEARED_ENV {
            command.env_remove(var);
        }
        command
            .env("RUSTFLAGS", &self.settings.rustflags)
            .env("CARGO_TARGET_DIR", &self.target_dir)
            .env("RUSTC", &self.toolchain.rustc);
        let start = Instant::now();
        let output = command.output()?;
        let seconds = start.elapsed().as_secs_f64();
        if !output.status.success() {
            return Err(error(format!(
                "cargo build failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let mut artifacts = Vec::new();
        for line in output.stdout.split(|b| *b == b'\n') {
            let Ok(message) = serde_json::from_slice::<serde_json::Value>(line) else {
                continue;
            };
            if message["reason"] != "compiler-artifact" {
                continue;
            }
            let strings = |value: &serde_json::Value| -> Vec<String> {
                value
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            };
            let package = message["manifest_path"]
                .as_str()
                .and_then(|p| Path::new(p).parent()?.file_name()?.to_str().map(str::to_owned))
                .unwrap_or_default();
            artifacts.push(Artifact {
                target: message["target"]["name"].as_str().unwrap_or("?").to_owned(),
                package,
                kinds: strings(&message["target"]["kind"]),
                files: strings(&message["filenames"]).into_iter().map(PathBuf::from).collect(),
            });
        }
        Ok(BuildOutput { artifacts, seconds })
    }

    /// Copies the libraries of a build into `build/`. Each cluster gets a new file name. The
    /// types library keeps its name, because the name changes when the content changes.
    pub fn install(&mut self, output: &BuildOutput, types_lib: &str) -> io::Result<Installed> {
        self.builds += 1;
        let types_src = output.dll(types_lib)?;
        let types = self.build_dir.join(types_src.file_name().unwrap_or_default());
        if !types.exists() {
            fs::copy(types_src, &types)?;
        }
        let cluster = |target: &str| -> io::Result<PathBuf> {
            let name = format!("{target}-{}-{}.dll", self.settings.profile, self.builds);
            let path = self.build_dir.join(name);
            fs::copy(output.dll(target)?, &path)?;
            Ok(path)
        };
        let cluster_a = cluster("cluster_a")?;
        let cluster_b = cluster("cluster_b")?;
        for path in [&types, &cluster_a, &cluster_b] {
            for lib in pe_info(path)?.imports.keys() {
                if lib.starts_with("std-") && !self.build_dir.join(lib).exists() {
                    fs::copy(self.find_std(lib)?, self.build_dir.join(lib))?;
                }
            }
        }
        Ok(Installed {
            types,
            cluster_a,
            cluster_b,
        })
    }

    /// Finds a `std` library of the toolchain in the sysroot.
    pub fn find_std(&self, name: &str) -> io::Result<PathBuf> {
        let rustlib = self.toolchain.sysroot.join("lib/rustlib").join(&self.settings.target);
        [self.toolchain.sysroot.join("bin"), rustlib.join("lib")]
            .into_iter()
            .map(|dir| dir.join(name))
            .find(|path| path.is_file())
            .ok_or_else(|| error(format!("{name} is not in the sysroot")))
    }

    /// Tells where the loader finds a library that a file in `build/` imports.
    pub fn classify(&self, lib: &str) -> ImportKind {
        let lower = lib.to_ascii_lowercase();
        if self.build_dir.join(lib).is_file() {
            ImportKind::Build
        } else if lower.starts_with("api-ms-") || lower.starts_with("ext-ms-") {
            ImportKind::ApiSet
        } else if ["vcruntime", "msvcp", "concrt"].iter().any(|p| lower.starts_with(p)) {
            ImportKind::MsvcRuntime
        } else if std::env::var_os("SystemRoot")
            .is_some_and(|root| Path::new(&root).join("System32").join(lib).is_file())
        {
            ImportKind::System
        } else {
            ImportKind::Missing
        }
    }
}

fn crate_const(konst: &str, name: &str, version: &str, build_id: &str) -> String {
    format!(
        "pub const {konst}: CrateInfo = CrateInfo {{\n    name: {name:?},\n    \
         version: {version:?},\n    build_id: {build_id:?},\n}};\n"
    )
}

/// Reads `key = "value"` from a simple manifest. The type crates have only a `[package]` table.
fn manifest_field(manifest: &str, key: &str) -> io::Result<String> {
    manifest
        .lines()
        .find_map(|line| {
            let value = line.trim().strip_prefix(key)?.trim_start().strip_prefix('=')?;
            Some(value.trim().trim_matches('"').to_owned())
        })
        .ok_or_else(|| error(format!("no {key} in manifest")))
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "target" || name == "Cargo.lock" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &to.join(&name))?;
        } else {
            fs::copy(entry.path(), to.join(&name))?;
        }
    }
    Ok(())
}

fn list_files(base: &Path, dir: &Path, out: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if entry.file_name() != "target" {
                list_files(base, &path, out)?;
            }
        } else if let Ok(relative) = path.strip_prefix(base) {
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// The imports and the export count of a PE or ELF file.
#[derive(Debug, Default)]
pub struct PeInfo {
    /// The imported symbols of each library.
    pub imports: BTreeMap<String, Vec<String>>,
    pub export_count: usize,
}

pub fn pe_info(path: &Path) -> io::Result<PeInfo> {
    use object::Object;
    let data = fs::read(path)?;
    let file = object::File::parse(&*data).map_err(|e| error(e.to_string()))?;
    let mut info = PeInfo::default();
    for import in file.imports().map_err(|e| error(e.to_string()))? {
        let lib = String::from_utf8_lossy(import.library()).into_owned();
        let name = String::from_utf8_lossy(import.name()).into_owned();
        info.imports.entry(lib).or_default().push(name);
    }
    info.export_count = file.exports().map_err(|e| error(e.to_string()))?.len();
    Ok(info)
}

#[cfg(windows)]
mod sys {
    use std::ffi::{CString, c_void};
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::System::LibraryLoader::{
        GetProcAddress, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
        LoadLibraryExW,
    };

    pub type RawSymbol = unsafe extern "system" fn() -> isize;

    /// A loaded library. The engine never unloads a library (spec 8.5), so there is no `Drop`.
    pub struct Library(*mut c_void);

    impl Library {
        /// Loads a library by its full path. The loader finds the libraries that it imports
        /// in its own folder and in the system folders (spec 8.5).
        pub fn load(path: &Path) -> io::Result<Self> {
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            // SAFETY: `wide` is a path that ends with a NUL.
            let handle = unsafe {
                LoadLibraryExW(
                    wide.as_ptr(),
                    std::ptr::null_mut(),
                    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                )
            };
            if handle.is_null() {
                Err(io::Error::last_os_error())
            } else {
                Ok(Library(handle))
            }
        }

        pub fn symbol(&self, name: &str) -> Option<RawSymbol> {
            let name = CString::new(name).ok()?;
            // SAFETY: the handle is a loaded library, and `name` ends with a NUL.
            unsafe { GetProcAddress(self.0, name.as_ptr().cast()) }
        }
    }
}

#[cfg(not(windows))]
mod sys {
    use std::io;
    use std::path::Path;

    pub type RawSymbol = unsafe extern "C" fn() -> isize;

    /// The loader of this prototype supports only Windows.
    pub struct Library;

    impl Library {
        pub fn load(_path: &Path) -> io::Result<Self> {
            Err(io::Error::from(io::ErrorKind::Unsupported))
        }

        pub fn symbol(&self, _name: &str) -> Option<RawSymbol> {
            None
        }
    }
}

pub use sys::Library;

type AbiVersionFn = unsafe extern "C" fn() -> u32;
type PinsFn = unsafe extern "C" fn(*mut PinDesc, usize) -> usize;
type ProbeStatsFn = unsafe extern "C" fn() -> ProbeStats;
type TypeIdFn = unsafe extern "C" fn() -> u64;
type SourceRunFn = unsafe extern "C" fn(*mut ErasedValue, usize) -> usize;
type SinkRunFn = unsafe extern "C" fn(*const InputArg, usize, *mut SinkSummary) -> i32;

/// A loaded cluster.
pub struct Cluster {
    pub path: PathBuf,
    library: Library,
    pins: Vec<PinDesc>,
}

impl Cluster {
    /// Loads a cluster, checks its ABI version and reads its pins.
    pub fn load(path: &Path) -> io::Result<Self> {
        let library = Library::load(path)?;
        let mut cluster = Cluster {
            path: path.to_owned(),
            library,
            pins: Vec::new(),
        };
        // SAFETY: the symbols have the types of the cluster ABI.
        unsafe {
            let version = cluster.symbol::<AbiVersionFn>("patina_abi_version")?();
            if version != abi::ABI_VERSION {
                return Err(error(format!("ABI version {version}, not {}", abi::ABI_VERSION)));
            }
            let pins_fn = cluster.symbol::<PinsFn>("patina_pins")?;
            let count = pins_fn(std::ptr::null_mut(), 0);
            let mut pins = vec![PinDesc::EMPTY; count];
            let written = pins_fn(pins.as_mut_ptr(), pins.len());
            pins.truncate(written.min(count));
            cluster.pins = pins;
        }
        Ok(cluster)
    }

    /// # Safety
    ///
    /// `T` must be the function pointer type of the export.
    unsafe fn symbol<T: Copy>(&self, name: &str) -> io::Result<T> {
        let raw = self
            .library
            .symbol(name)
            .ok_or_else(|| error(format!("{} has no export {name}", self.path.display())))?;
        assert_eq!(size_of::<T>(), size_of::<sys::RawSymbol>());
        // SAFETY: the caller gives the right function pointer type.
        Ok(unsafe { std::mem::transmute_copy(&raw) })
    }

    pub fn pins(&self) -> &[PinDesc] {
        &self.pins
    }

    pub fn pin(&self, name: &str) -> &PinDesc {
        self.pins
            .iter()
            .find(|p| p.name() == name)
            .unwrap_or_else(|| panic!("{} has no pin {name}", self.path.display()))
    }

    /// The counters of `Probe` in the copy of `geometry` that this cluster uses.
    pub fn probe_stats(&self) -> ProbeStats {
        // SAFETY: the export has this type.
        unsafe { self.symbol::<ProbeStatsFn>("patina_probe_stats").map(|f| f()) }
            .expect("patina_probe_stats")
    }

    /// A hash of the Rust `TypeId` of `Point`, made in this cluster.
    pub fn point_type_id(&self) -> u64 {
        // SAFETY: the export has this type.
        unsafe { self.symbol::<TypeIdFn>("patina_point_type_id").map(|f| f()) }
            .expect("patina_point_type_id")
    }

    /// Runs the source node and returns its outputs in pin order.
    pub fn source_run(&self) -> Vec<Value> {
        const CAP: usize = 8;
        let mut out: Vec<ErasedValue> = Vec::with_capacity(CAP);
        // SAFETY: the export has this type, and `out` has room for `CAP` values.
        let count = unsafe {
            let run = self.symbol::<SourceRunFn>("patina_source_run").expect("source run");
            run(out.as_mut_ptr(), CAP)
        };
        assert!(count <= CAP);
        // SAFETY: the cluster wrote `count` values.
        unsafe { out.set_len(count) };
        out.into_iter().map(Value).collect()
    }

    /// Runs the sink node on four inputs in pin order.
    ///
    /// # Safety
    ///
    /// Each input must hold a value with the type key of its pin, and the engine must give
    /// away the values of the `TAKE` pins.
    pub unsafe fn sink_run(&self, args: &[InputArg]) -> SinkSummary {
        let mut summary = SinkSummary::default();
        // SAFETY: the export has this type, and the caller gives correct inputs.
        let status = unsafe {
            let run = self.symbol::<SinkRunFn>("patina_sink_run").expect("sink run");
            run(args.as_ptr(), args.len(), &mut summary)
        };
        assert_eq!(status, 0, "patina_sink_run failed");
        summary
    }
}

/// A value that the engine holds (spec 5.2). The engine drops it with the drop function of
/// the library that made it.
pub struct Value(ErasedValue);

impl Value {
    pub fn key(&self) -> TypeKey {
        self.0.key
    }

    /// Lends the value to a `READ` input. The engine keeps the value.
    pub fn read_arg(&self) -> InputArg {
        InputArg {
            ptr: self.0.ptr,
            key: self.0.key,
        }
    }

    /// The address of the drop function. It is in the library that made the value.
    pub fn drop_fn_addr(&self) -> usize {
        self.0.drop as usize
    }

    /// Gives the value to a `TAKE` input. The node drops it.
    pub fn into_arg(self) -> InputArg {
        let arg = self.read_arg();
        std::mem::forget(self);
        arg
    }
}

impl Drop for Value {
    fn drop(&mut self) {
        // SAFETY: the library that made the value stays loaded, and the value has one owner.
        unsafe { (self.0.drop)(self.0.ptr) }
    }
}

/// Why the engine refuses a connection or a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mismatch {
    Direction,
    Key { output: TypeKey, input: TypeKey },
}

/// Checks a connection from an output pin to an input pin.
pub fn connect(output: &PinDesc, input: &PinDesc) -> Result<(), Mismatch> {
    if output.dir != OUTPUT || input.dir != INPUT {
        return Err(Mismatch::Direction);
    }
    accept(input, output.key)
}

/// Checks a value with type key `key` before the engine gives it to an input pin.
pub fn accept(input: &PinDesc, key: TypeKey) -> Result<(), Mismatch> {
    if input.key == key {
        Ok(())
    } else {
        Err(Mismatch::Key {
            output: key,
            input: input.key,
        })
    }
}
