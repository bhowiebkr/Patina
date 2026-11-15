//! The milestone 0 prototype "Toolchain" (spec 13.2 and 19). Windows only.
//!
//! `toolchain run <work>` does the steps of the **Install toolchain** button in
//! `<work>/patina/toolchain`. It builds the fixture project, which has C code, with a clean
//! environment, and calls its node library from this executable through the C ABI. Then it moves
//! the folder to a path with spaces and non-ASCII letters, and builds and calls again. It writes
//! `<work>/report.json`.
//!
//! `toolchain call <dll>` loads a node library, calls it and prints the results as JSON.

mod install;
mod wrapper;

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use serde_json::{Map, Value, json};

use install::{HOST, Toolchain};

/// The files of the user project. The executable holds them, so it runs alone on a clean computer.
const FIXTURE: &[(&str, &str)] = &[
    ("Cargo.toml", include_str!("../fixture/Cargo.toml")),
    ("Cargo.lock", include_str!("../fixture/Cargo.lock")),
    ("cmix/Cargo.toml", include_str!("../fixture/cmix/Cargo.toml")),
    ("cmix/build.rs", include_str!("../fixture/cmix/build.rs")),
    ("cmix/csrc/mix.c", include_str!("../fixture/cmix/csrc/mix.c")),
    ("cmix/src/lib.rs", include_str!("../fixture/cmix/src/lib.rs")),
    ("node/Cargo.toml", include_str!("../fixture/node/Cargo.toml")),
    ("node/src/lib.rs", include_str!("../fixture/node/src/lib.rs")),
];
/// The moved folder: spaces, a Latin-1 letter, and a CJK letter that the ANSI code page of a
/// Western Windows (1252) cannot show.
const MOVED: &str = "Patina T\u{e9}st \u{6587}";
/// The results that the fixture node must return.
const EXPECTED: &str = r#"{"mix":2065,"sqlite":42,"threads":10}"#;

pub fn error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

/// Runs a command and returns its standard output. The error has the end of standard error.
pub fn run_text(command: &mut Command) -> io::Result<String> {
    let output = command.output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let lines: Vec<&str> = stderr.lines().collect();
        let tail = lines[lines.len().saturating_sub(60)..].join("\n");
        return Err(error(format!("{:?} failed ({}):\n{tail}", command.get_program(), output.status)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn main() -> ExitCode {
    wrapper::dispatch();
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let result = match (args.first().and_then(|a| a.to_str()), args.get(1)) {
        (Some("run"), Some(work)) => run(Path::new(work)),
        (Some("call"), Some(dll)) => call(Path::new(dll)),
        _ => Err(error("usage: toolchain run <work folder> | toolchain call <node library>")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The report of a run. It is written again after each step.
struct Report {
    file: PathBuf,
    facts: Map<String, Value>,
    steps: Vec<Value>,
}

impl Report {
    fn step(&mut self, name: &str, f: impl FnOnce() -> io::Result<Value>) -> io::Result<Value> {
        println!("== {name}");
        let start = Instant::now();
        let result = f();
        let seconds = (start.elapsed().as_secs_f64() * 10.0).round() / 10.0;
        let entry = match &result {
            Ok(value) => json!({ "step": name, "ok": true, "seconds": seconds, "result": value }),
            Err(e) => json!({ "step": name, "ok": false, "seconds": seconds, "error": e.to_string() }),
        };
        println!("{entry:#}");
        self.steps.push(entry);
        let text = json!({ "facts": self.facts, "steps": self.steps });
        fs::write(&self.file, format!("{text:#}\n"))?;
        result
    }
}

fn run(work: &Path) -> io::Result<()> {
    if !cfg!(windows) {
        return Err(error("this prototype runs on Windows only (spec 13.2)"));
    }
    let work = std::path::absolute(work)?;
    let first = work.join("patina");
    let moved = work.join(MOVED);
    // A run that stopped after the move leaves the moved folder. Use it again.
    if moved.exists() && !first.exists() {
        fs::rename(&moved, &first)?;
    }
    fs::create_dir_all(&first)?;
    let mut facts = Map::new();
    let vswhere = Path::new(r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe");
    facts.insert("visual_studio_installer_on_computer".into(), vswhere.is_file().into());
    facts.insert("host_exe".into(), json!(env::current_exe()?));
    facts.insert("host_imports".into(), json!(imports(&fs::read(env::current_exe()?)?)?));
    let mut report = Report { file: work.join("report.json"), facts, steps: Vec::new() };

    let toolchain = Toolchain::at(&first);
    report.step("install rust", || install::rust(&toolchain))?;
    report.step("install zig", || install::zig(&toolchain))?;
    report.step("make the zig wrappers", || install::wrappers(&toolchain))?;
    report.step("versions", || {
        let rustc = run_text(toolchain.command(toolchain.cargo_home().join("bin/rustc.exe"))?.arg("-vV"))?;
        let zig = run_text(Command::new(toolchain.zig_dir().join("zig.exe")).arg("version"))?;
        Ok(json!({ "rustc": rustc.trim(), "zig": zig.trim() }))
    })?;
    let project = first.join("projects").join("fixture");
    report.step("copy the fixture", || {
        if project.exists() {
            fs::remove_dir_all(&project)?;
        }
        for (name, text) in FIXTURE {
            let file = project.join(name);
            fs::create_dir_all(file.parent().unwrap_or(&project))?;
            fs::write(file, text)?;
        }
        Ok(json!(project))
    })?;
    report.step("fetch the crates", || fetch(&toolchain, &project))?;
    report.step("build", || build(&toolchain, &project))?;
    report.step("call", || call_child(&toolchain, &project))?;

    report.step("move the folder", || {
        fs::rename(&first, &moved)?;
        Ok(json!(moved))
    })?;
    let toolchain = Toolchain::at(&moved);
    let project = moved.join("projects").join("fixture");
    report.step("build after the move", || {
        fs::remove_dir_all(project.join("target"))?;
        build(&toolchain, &project)
    })?;
    report.step("call after the move", || call_child(&toolchain, &project))?;
    println!("== all steps passed, report: {}", report.file.display());
    Ok(())
}

/// Downloads the crates of the fixture into the toolchain folder.
/// shortcut: `PATINA_FETCH_CARGO` names a cargo.exe that an allowlist firewall lets through,
/// because the firewall blocks the new cargo.exe. Remove it when the engine downloads crates.
fn fetch(toolchain: &Toolchain, project: &Path) -> io::Result<Value> {
    let cargo = env::var_os("PATINA_FETCH_CARGO").map_or_else(|| toolchain.cargo(), PathBuf::from);
    let mut command = toolchain.command(&cargo)?;
    run_text(command.current_dir(project).args(["fetch", "--locked"]))?;
    Ok(json!({ "cargo": cargo }))
}

/// Builds the node library of the fixture with the toolchain alone, offline.
fn build(toolchain: &Toolchain, project: &Path) -> io::Result<Value> {
    let calls = toolchain.zig_dir().join("calls.log");
    if calls.exists() {
        fs::remove_file(&calls)?;
    }
    let mut command = toolchain.command(toolchain.cargo())?;
    command.current_dir(project).args(["build", "-p", "node", "--target", HOST, "--locked", "--offline"]);
    run_text(&mut command)?;
    let dll = node_dll(project);
    let data = fs::read(&dll)?;
    let calls = fs::read_to_string(&calls).unwrap_or_default();
    let count = |tool: &str| calls.lines().filter(|line| line.starts_with(tool)).count();
    Ok(json!({
        "dll": dll,
        "imports": imports(&data)?,
        // The MSVC linker writes a Rich header. GNU ld does not.
        "rich_header": has_rich_header(&data),
        "zig_cc_calls": count("cc "),
        "zig_ar_calls": count("ar "),
    }))
}

fn node_dll(project: &Path) -> PathBuf {
    project.join("target").join(HOST).join("debug").join("node.dll")
}

/// Calls the node in a new process with the clean environment, so a missing DLL shows.
fn call_child(toolchain: &Toolchain, project: &Path) -> io::Result<Value> {
    let out = run_text(toolchain.command(env::current_exe()?)?.arg("call").arg(node_dll(project)))?;
    let value: Value = serde_json::from_str(out.trim())?;
    let expected: Value = serde_json::from_str(EXPECTED)?;
    if value != expected {
        return Err(error(format!("the node returned {value}, not {expected}")));
    }
    Ok(value)
}

type MixFn = extern "C" fn(i32, i32) -> i32;
type NumberFn = extern "C" fn() -> i32;

fn call(dll: &Path) -> io::Result<()> {
    let lib_error = |e: libloading::Error| error(e.to_string());
    // SAFETY: the prototype built this library from the fixture. Its load runs no user code.
    let lib = unsafe { libloading::Library::new(dll) }.map_err(lib_error)?;
    // SAFETY: the fixture node exports these functions with these C signatures.
    let (mix, sqlite, threads) = unsafe {
        (
            *lib.get::<MixFn>(b"patina_fixture_mix").map_err(lib_error)?,
            *lib.get::<NumberFn>(b"patina_fixture_sqlite").map_err(lib_error)?,
            *lib.get::<NumberFn>(b"patina_fixture_threads").map_err(lib_error)?,
        )
    };
    let result = json!({ "mix": mix(2, 3), "sqlite": sqlite(), "threads": threads() });
    println!("{result}");
    drop(lib);
    Ok(())
}

fn imports(data: &[u8]) -> io::Result<Vec<String>> {
    use object::Object;
    let file = object::File::parse(data).map_err(|e| error(e.to_string()))?;
    let mut libs: Vec<String> = file
        .imports()
        .map_err(|e| error(e.to_string()))?
        .iter()
        .map(|import| String::from_utf8_lossy(import.library()).into_owned())
        .collect();
    libs.sort();
    libs.dedup();
    Ok(libs)
}

fn contains(data: &[u8], needle: &[u8]) -> bool {
    data.windows(needle.len()).any(|w| w == needle)
}

/// Tells if the DOS stub of a PE file has the Rich header of the Microsoft linker.
fn has_rich_header(data: &[u8]) -> bool {
    let offset = data.get(0x3c..0x40).and_then(|b| <[u8; 4]>::try_from(b).ok());
    let end = offset.map_or(0, |b| u32::from_le_bytes(b) as usize);
    data.get(0x40..end).is_some_and(|stub| contains(stub, b"Rich"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_has_each_file_of_the_folder() {
        fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
            for entry in fs::read_dir(dir).unwrap().map(Result::unwrap) {
                let path = entry.path();
                if path.is_dir() {
                    if entry.file_name() != "target" {
                        walk(&path, base, out);
                    }
                } else {
                    out.push(path.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
                }
            }
        }
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture");
        let mut found = Vec::new();
        walk(&base, &base, &mut found);
        found.sort();
        let mut listed: Vec<String> = FIXTURE.iter().map(|(name, _)| name.to_string()).collect();
        listed.sort();
        assert_eq!(found, listed);
    }

    #[test]
    fn rich_header_only_in_the_dos_stub() {
        let mut data = vec![0u8; 0x100];
        data[0x3c] = 0x80;
        data[0x90..0x94].copy_from_slice(b"Rich");
        assert!(!has_rich_header(&data));
        data[0x50..0x54].copy_from_slice(b"Rich");
        assert!(has_rich_header(&data));
    }
}
