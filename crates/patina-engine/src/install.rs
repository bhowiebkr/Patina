//! The **Install toolchain** button (spec 13.2). `curl.exe` makes the downloads and `tar.exe`
//! unpacks Zig. Windows has both in System32, and a firewall that allows only known programs
//! allows them. Rustup installs from a local copy of the server, so it needs no network.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::Error;
use crate::cargo::{Toolchain, command, own_env, run_text};

/// The host and the default target of the toolchain.
const HOST: &str = "x86_64-pc-windows-gnu";
/// The download server of Rust.
const DIST: &str = "https://static.rust-lang.org";
/// The packages of the minimal rustup profile (the `[profiles]` table of the channel manifest),
/// and rust-analyzer with the source of `std` for the code editor. The manifest renames
/// `rust-analyzer` to `rust-analyzer-preview`.
const PACKAGES: &[&str] = &["rustc", "cargo", "rust-std", "rust-mingw", "rust-analyzer-preview", "rust-src"];
/// The Zig target for the C code of the Rust target `x86_64-pc-windows-gnu`.
const ZIG_TARGET: &str = "x86_64-windows-gnu";
/// The copies of the editor or the CLI that start Zig (see [`run_wrapper`]).
const WRAPPERS: [&str; 4] = ["zig-cc.exe", "zig-cxx.exe", "zig-ar.exe", "zig-dlltool.exe"];

/// Installs the toolchain into `folder` and returns its Rust version. The steps work in the
/// folder `<folder>-install`, and the last step renames it, so Patina never uses a part install.
/// A retry continues with the files of the last try. `wrapper` is the program that becomes the
/// Zig wrappers (see [`run_wrapper`]). `report` gets the name of each step and the part of all
/// the work that is done, from 0 to 1.
pub fn install(folder: &Path, wrapper: &Path, report: &mut dyn FnMut(&str, f32)) -> Result<String, Error> {
    steps(folder, wrapper, report).map_err(install_error)
}

/// Updates the Rust of the toolchain in `folder` to the newest stable release and returns its
/// version (13.2). Rustup replaces the toolchain in one transaction, so a failed update keeps the
/// old Rust. Zig and the crate cache stay. A retry keeps the downloads with a correct hash.
pub fn update(folder: &Path, report: &mut dyn FnMut(&str, f32)) -> Result<String, Error> {
    update_steps(folder, report).map_err(install_error)
}

fn install_error(e: Error) -> Error {
    match e {
        Error::Build(text) => Error::Install(text),
        e => e,
    }
}

fn update_steps(folder: &Path, report: &mut dyn FnMut(&str, f32)) -> Result<String, Error> {
    if !cfg!(windows) {
        return Err(Error::Build("Update toolchain works only on Windows in this version".into()));
    }
    if !Toolchain::installed(folder) {
        return Err(Error::Build(format!("{} has no toolchain to update. Install the toolchain first", folder.display())));
    }
    let downloads = beside(folder, "-update");
    make_dir(&downloads)?;
    let step = &mut Steps::new(PACKAGES.len() + 3, report);
    let server = mirror(folder, &downloads, step)?;
    step.next("Update Rust");
    let bin = folder.join("cargo").join("bin");
    let rustup = bin.join("rustup.exe");
    run_text(tool(folder, &rustup)?.env("RUSTUP_DIST_SERVER", &server).args(["update", "stable", "--no-self-update"]))?;
    // A toolchain of an older Patina has no source of std.
    run_text(tool(folder, &rustup)?.env("RUSTUP_DIST_SERVER", &server).args(["component", "add", "rust-src"]))?;
    step.next("Check the toolchain");
    let toolchain = Toolchain::find(folder)?;
    check_analyzer(folder, &toolchain)?;
    let release = toolchain.release;
    fs::remove_dir_all(&downloads).map_err(|e| Error::Io(downloads.clone(), e))?;
    Ok(release)
}

/// The folder `<folder><suffix>` next to `folder`.
fn beside(folder: &Path, suffix: &str) -> PathBuf {
    let mut name = folder.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn steps(folder: &Path, wrapper: &Path, report: &mut dyn FnMut(&str, f32)) -> Result<String, Error> {
    if !cfg!(windows) {
        return Err(Error::Build("Install toolchain works only on Windows in this version".into()));
    }
    if Toolchain::installed(folder) {
        add_wrappers(folder, wrapper)?;
        return Ok(Toolchain::find(folder)?.release);
    }
    let work = beside(folder, "-install");
    let downloads = work.join("downloads");
    make_dir(&downloads)?;
    let zig = work.join("zig");
    let (has_rust, has_zig) = (Toolchain::installed(&work), zig.join("zig.exe").is_file());
    let total = if has_rust { 0 } else { PACKAGES.len() + 3 } + if has_zig { 0 } else { 3 } + 3;
    let step = &mut Steps::new(total, report);
    if !has_rust {
        rust(&work, &downloads, step)?;
    }
    if !has_zig {
        unpack_zig(&work, &downloads, step)?;
    }
    step.next("Copy the Zig wrappers");
    for name in WRAPPERS {
        fs::copy(wrapper, zig.join(name)).map_err(|e| Error::Io(zig.join(name), e))?;
    }
    step.next("Check the toolchain");
    let toolchain = Toolchain::find(&work)?;
    check_analyzer(&work, &toolchain)?;
    let release = toolchain.release;
    let source = downloads.join("check.c");
    fs::write(&source, "int patina_check(void) { return 1; }\n").map_err(|e| Error::Io(source.clone(), e))?;
    run_text(tool(&work, &zig.join("zig-cc.exe"))?.arg("-c").arg(&source).arg("-o").arg(downloads.join("check.o")))?;
    step.next("Move the toolchain into place");
    fs::remove_dir_all(&downloads).map_err(|e| Error::Io(downloads.clone(), e))?;
    if folder.is_dir() {
        // Only an empty folder goes. A folder with files gives an error, and the user decides.
        fs::remove_dir(folder).map_err(|e| Error::Io(folder.to_owned(), e))?;
    }
    rename(&work, folder)?;
    Ok(release)
}

/// Checks that rust-analyzer starts and that the source of `std` is there, for the code editor.
fn check_analyzer(folder: &Path, toolchain: &Toolchain) -> Result<(), Error> {
    run_text(tool(folder, &folder.join("cargo").join("bin").join("rust-analyzer.exe"))?.arg("--version"))?;
    let library = std_source(toolchain);
    match library.join("std").join("src").join("lib.rs").is_file() {
        true => Ok(()),
        false => Err(Error::Build(format!("the toolchain has no source of std in {}", library.display()))),
    }
}

/// The folder of the source of `std` that the `rust-src` component installs.
fn std_source(toolchain: &Toolchain) -> PathBuf {
    let rustlib = toolchain.libdir.parent().and_then(Path::parent).unwrap_or(&toolchain.libdir);
    rustlib.join("src").join("rust").join("library")
}

/// Copies `wrapper` to each Zig wrapper that the toolchain in `folder` does not have. A toolchain
/// from an older Patina has no `zig-dlltool.exe`, and each build fails without it.
pub fn add_wrappers(folder: &Path, wrapper: &Path) -> Result<(), Error> {
    let zig = folder.join("zig");
    if !zig.join("zig.exe").is_file() {
        return Ok(());
    }
    for to in WRAPPERS.map(|name| zig.join(name)).into_iter().filter(|to| !to.is_file()) {
        fs::copy(wrapper, &to).map_err(|e| Error::Io(to.clone(), e))?;
    }
    Ok(())
}

/// When this program is a Zig wrapper, runs Zig and returns its exit code. cc-rs takes `CC` as
/// one path, and a value such as `zig cc` splits on spaces. So each wrapper is a copy of the
/// editor or the CLI, and it starts Zig with the subcommand of its file name (13.2).
pub fn run_wrapper() -> Option<i32> {
    let exe = std::env::current_exe().ok()?;
    let tool = match exe.file_stem()?.to_str()? {
        "zig-cc" => "cc",
        "zig-cxx" => "c++",
        "zig-ar" => "ar",
        "zig-dlltool" => "dlltool",
        _ => return None,
    };
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let args = if tool == "cc" || tool == "c++" { cc_args(args) } else { args };
    let dir = exe.parent().unwrap_or(Path::new("."));
    if tool == "c++"
        && let Err(e) = cxx_runtime(dir)
    {
        eprintln!("zig-c++: cannot make the C++ runtime: {e}");
        return Some(1);
    }
    let cache = dir.join("cache");
    let status = command(&dir.join("zig.exe"))
        .arg(tool)
        .args(&args)
        // Else Zig keeps its cache in the folders of the user (13.3).
        .env("ZIG_GLOBAL_CACHE_DIR", &cache)
        .env("ZIG_LOCAL_CACHE_DIR", &cache)
        .status();
    Some(match status {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("zig-{tool}: cannot start {}: {e}", dir.join("zig.exe").display());
            1
        }
    })
}

/// Removes the target arguments of cc-rs, which Zig does not accept, and sets the Zig target.
/// Removes the C sanitizers that Zig adds to debug builds: their runtime is not in the files of
/// the Rust linker.
fn cc_args(args: Vec<OsString>) -> Vec<OsString> {
    let mut out: Vec<OsString> = vec!["-target".into(), ZIG_TARGET.into(), "-fno-sanitize=all".into()];
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some(s) if s.starts_with("--target=") => {}
            Some("-target") => {
                args.next();
            }
            _ => out.push(arg),
        }
    }
    out
}

/// A program that makes Zig build libc++ and libc++abi.
const CXX_PROBE: &str = "#include <string>\nint main() { return (int)std::string(\"a\").size() - 1; }\n";

/// libc++ of Zig imports these functions from the UCRT. The Rust gnu target links msvcrt, so each
/// import points at the msvcrt function. `_strftime_l` ignores the locale.
const CXX_SHIM: &str = r#"extern "C" {
#define FWD(name) extern char name##_crt __asm__(#name); void *__imp_##name = &name##_crt;
FWD(mbrtowc) FWD(mbrlen) FWD(mbsrtowcs) FWD(wcrtomb) FWD(wctob) FWD(btowc)
struct tm;
unsigned long long strftime_crt(char *, unsigned long long, const char *, const tm *) __asm__("strftime");
static unsigned long long strftime_l(char *s, unsigned long long n, const char *f, const tm *t, void *) { return strftime_crt(s, n, f, t); }
void *__imp__strftime_l = (void *)&strftime_l;
extern int *__imp__sys_nerr;
static int *sys_nerr() { return __imp__sys_nerr; }
void *__imp___sys_nerr = (void *)&sys_nerr;
}
"#;

/// Makes `cxx/libc++.a` in the Zig folder `zig` one time: libc++, libc++abi and [`CXX_SHIM`].
/// cc-rs links it with `CXXSTDLIB=c++`, and the rustc flags have the folder (13.2).
fn cxx_runtime(zig: &Path) -> Result<(), Error> {
    let out = zig.join("cxx");
    let lib = out.join("libc++.a");
    if lib.is_file() {
        return Ok(());
    }
    make_dir(&out)?;
    let lock_path = out.join("lock");
    let lock = fs::File::create(&lock_path).map_err(|e| Error::Io(lock_path.clone(), e))?;
    // cc-rs compiles the C++ files of a crate at the same time.
    lock.lock().map_err(|e| Error::Io(lock_path, e))?;
    if lib.is_file() {
        return Ok(());
    }
    let work = out.join("work");
    if work.exists() {
        fs::remove_dir_all(&work).map_err(|e| Error::Io(work.clone(), e))?;
    }
    make_dir(&work)?;
    for (name, text) in [("probe.cpp", CXX_PROBE), ("shim.cpp", CXX_SHIM)] {
        fs::write(work.join(name), text).map_err(|e| Error::Io(work.join(name), e))?;
    }
    let cache = work.join("cache");
    let zig_run = |args: &[&OsStr]| {
        let mut command = command(&zig.join("zig.exe"));
        command.current_dir(&work).args(args).env("ZIG_GLOBAL_CACHE_DIR", &cache).env("ZIG_LOCAL_CACHE_DIR", &cache);
        run_text(&mut command)
    };
    // Zig builds libc++ with the optimize mode of the link.
    let cxx = |rest: &[&'static str]| [&["c++", "-target", ZIG_TARGET, "-O2"][..], rest].concat();
    zig_run(&cxx(&["probe.cpp", "-o", "probe.exe"]).iter().map(OsStr::new).collect::<Vec<_>>())?;
    zig_run(&cxx(&["-c", "shim.cpp", "-o", "shim.o"]).iter().map(OsStr::new).collect::<Vec<_>>())?;
    let built = |name: &str| -> Result<PathBuf, Error> {
        let o = cache.join("o");
        let entries = fs::read_dir(&o).map_err(|e| Error::Io(o.clone(), e))?;
        let found = entries.filter_map(Result::ok).map(|entry| entry.path().join(name)).find(|path| path.is_file());
        found.ok_or_else(|| Error::Build(format!("Zig made no {name} in {}", o.display())))
    };
    let (libcxx, libcxxabi) = (built("c++.lib")?, built("c++abi.lib")?);
    zig_run(&["ar".as_ref(), "qcL".as_ref(), "libc++.a".as_ref(), libcxx.as_os_str(), libcxxabi.as_os_str(), "shim.o".as_ref()])?;
    rename(&work.join("libc++.a"), &lib)?;
    fs::remove_dir_all(&work).map_err(|e| Error::Io(work, e))
}

/// Installs Rust with rustup: the minimal profile of the stable channel, rust-analyzer and the
/// source of `std`, for the gnu host.
fn rust(work: &Path, downloads: &Path, step: &mut Steps) -> Result<(), Error> {
    step.next("Download rustup-init");
    let init = downloads.join("rustup-init.exe");
    download_checked(work, &format!("{DIST}/rustup/dist/{HOST}/rustup-init.exe"), &init, &mut |f| step.part(f))?;
    let server = mirror(work, downloads, step)?;
    step.next("Install Rust");
    run_text(tool(work, &init)?.env("RUSTUP_DIST_SERVER", &server).args([
        "-y",
        "--no-modify-path",
        "--profile",
        "minimal",
        "--component",
        "rust-analyzer,rust-src",
        "--default-host",
        HOST,
        "--default-toolchain",
        "stable",
    ]))?;
    Ok(())
}

/// Downloads the newest stable channel manifest and its packages of [`PACKAGES`] into a copy of
/// the server in `downloads`, and returns the `file:` URL of the copy for rustup.
fn mirror(work: &Path, downloads: &Path, step: &mut Steps) -> Result<String, Error> {
    step.next("Download the Rust channel manifest");
    let mirror = downloads.join("mirror");
    let manifest = mirror.join("dist").join("channel-rust-stable.toml");
    make_dir(&mirror.join("dist"))?;
    download_checked(work, &format!("{DIST}/dist/channel-rust-stable.toml"), &manifest, &mut |f| step.part(f))?;
    let text = fs::read_to_string(&manifest).map_err(|e| Error::Io(manifest.clone(), e))?;
    for (name, url, hash) in packages(&text, HOST)? {
        step.next(&format!("Download {name}"));
        let path = url.strip_prefix(DIST).ok_or_else(|| Error::Build(format!("{url} is not on {DIST}")))?;
        let file = mirror.join(path.trim_start_matches('/'));
        make_dir(file.parent().unwrap_or(&mirror))?;
        if verify(&file, &hash).is_err() {
            download(work, &url, &file, &mut |f| step.part(f))?;
            verify(&file, &hash)?;
        }
    }
    Ok(format!("file:///{}", mirror.display().to_string().replace('\\', "/")))
}

/// Returns the name, the xz URL and the hash of each package of [`PACKAGES`].
fn packages(manifest: &str, target: &str) -> Result<Vec<(&'static str, String, String)>, Error> {
    PACKAGES
        .iter()
        .map(|&name| {
            // The source of std is the same for all targets.
            let header = match name {
                "rust-src" => format!("[pkg.{name}.target.\"*\"]"),
                _ => format!("[pkg.{name}.target.{target}]"),
            };
            let table = manifest.split(&header).nth(1).unwrap_or_default();
            let field = |key: &str| {
                table.lines().skip(1).take_while(|line| !line.trim().is_empty()).find_map(|line| {
                    let value = line.strip_prefix(key)?.trim_start().strip_prefix('=')?.trim();
                    Some(value.strip_prefix('"')?.strip_suffix('"')?.to_owned())
                })
            };
            match (field("xz_url"), field("xz_hash")) {
                (Some(url), Some(hash)) => Ok((name, url, hash)),
                _ => Err(Error::Build(format!("the channel manifest has no xz file for {name} on {target}"))),
            }
        })
        .collect()
}

/// Installs the newest Zig release of the index of ziglang.org.
fn unpack_zig(work: &Path, downloads: &Path, step: &mut Steps) -> Result<(), Error> {
    step.next("Download the Zig index");
    let index_file = downloads.join("zig-index.json");
    download(work, "https://ziglang.org/download/index.json", &index_file, &mut |f| step.part(f))?;
    let bytes = fs::read(&index_file).map_err(|e| Error::Io(index_file.clone(), e))?;
    let index: Value = serde_json::from_slice(&bytes).map_err(|e| Error::Build(format!("the Zig index: {e}")))?;
    let (version, url, sha) = zig_file(&index)?;
    step.next(&format!("Download Zig {version}"));
    let zip = downloads.join("zig.zip");
    if verify(&zip, &sha).is_err() {
        download(work, &url, &zip, &mut |f| step.part(f))?;
        verify(&zip, &sha)?;
    }
    step.next(&format!("Unpack Zig {version}"));
    let unpack = downloads.join("zig");
    if unpack.exists() {
        fs::remove_dir_all(&unpack).map_err(|e| Error::Io(unpack.clone(), e))?;
    }
    make_dir(&unpack)?;
    run_text(tool(work, &system32().join("tar.exe"))?.arg("-xf").arg(&zip).arg("-C").arg(&unpack))?;
    let inner = fs::read_dir(&unpack)
        .and_then(|mut entries| entries.next().transpose())
        .map_err(|e| Error::Io(unpack.clone(), e))?
        .ok_or_else(|| Error::Build("the Zig zip is empty".into()))?
        .path();
    rename(&inner, &work.join("zig"))
}

/// Returns the version, the URL and the SHA-256 hash of the Windows file of the newest Zig
/// release. It skips `master`, and it accepts only an HTTPS URL of ziglang.org.
fn zig_file(index: &Value) -> Result<(String, String, String), Error> {
    let parse = |key: &str| -> Option<Vec<u32>> {
        let parts: Option<Vec<u32>> = key.split('.').map(|p| p.parse().ok()).collect();
        parts.filter(|p| p.len() == 3)
    };
    let (_, version, release) = index
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(key, value)| Some((parse(key)?, key, value)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .ok_or_else(|| Error::Build("the Zig index has no release".into()))?;
    let file = &release["x86_64-windows"];
    match (file["tarball"].as_str(), file["shasum"].as_str()) {
        (Some(url), Some(sha)) if url.starts_with("https://ziglang.org/") && sha.len() == 64 => {
            Ok((version.clone(), url.to_owned(), sha.to_owned()))
        }
        _ => Err(Error::Build(format!("Zig {version} has no Windows file on ziglang.org"))),
    }
}

/// A command with the clean environment of the toolchain in `work` (13.2).
fn tool(work: &Path, program: &Path) -> Result<Command, Error> {
    let mut command = command(program);
    command.env_clear().envs(own_env(work, |name| std::env::var_os(name))?);
    Ok(command)
}

pub(crate) fn system32() -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    Path::new(&root).join("System32")
}

/// Downloads a file over HTTPS only, and gives the part that is done to `part`. A stalled
/// connection fails after 60 s below 1 KB/s, so the retry of curl starts.
fn download(work: &Path, url: &str, file: &Path, part: &mut dyn FnMut(f32)) -> Result<(), Error> {
    let mut child = tool(work, &system32().join("curl.exe"))?
        .args(["--fail", "--progress-bar", "--show-error", "--location", "--retry", "3"])
        .args(["--proto", "=https", "--proto-redir", "=https"])
        .args(["--connect-timeout", "30", "--speed-limit", "1024", "--speed-time", "60", "--output"])
        .arg(file)
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Build(format!("cannot start curl.exe: {e}")))?;
    // The progress bar of curl writes each update after a carriage return. The other lines are errors.
    let mut errors = String::new();
    let mut end_line = |line: &[u8]| {
        let text = String::from_utf8_lossy(line);
        match percent(&text) {
            Some(p) => part(p),
            None if !text.trim().chars().all(|c| "#=Oo- ".contains(c)) => {
                errors.push_str(text.trim());
                errors.push('\n');
            }
            None => {}
        }
    };
    if let Some(mut stderr) = child.stderr.take() {
        let (mut line, mut buffer) = (Vec::new(), [0u8; 512]);
        loop {
            let n = stderr.read(&mut buffer).map_err(|e| Error::Build(format!("curl.exe: {e}")))?;
            if n == 0 {
                break;
            }
            for &byte in &buffer[..n] {
                if byte == b'\r' || byte == b'\n' {
                    end_line(&line);
                    line.clear();
                } else {
                    line.push(byte);
                }
            }
        }
        end_line(&line);
    }
    let status = child.wait().map_err(|e| Error::Build(format!("curl.exe: {e}")))?;
    if !status.success() {
        return Err(Error::Build(format!("curl.exe failed for {url}: {}", errors.trim())));
    }
    Ok(())
}

/// The part that is done in a line of the progress bar of curl, for example `###   12.3%`.
fn percent(line: &str) -> Option<f32> {
    let number = line.trim_end().strip_suffix('%')?.rsplit([' ', '#']).next()?;
    number.parse::<f32>().ok().map(|p| (p / 100.0).clamp(0.0, 1.0))
}

/// The steps of an install or an update. Each report has the name of the step and the part of
/// all the work that is done, from 0 to 1.
struct Steps<'a> {
    name: String,
    done: usize,
    total: usize,
    report: &'a mut dyn FnMut(&str, f32),
}

impl<'a> Steps<'a> {
    fn new(total: usize, report: &'a mut dyn FnMut(&str, f32)) -> Self {
        Self { name: String::new(), done: 0, total, report }
    }

    fn next(&mut self, name: &str) {
        if !self.name.is_empty() {
            self.done += 1;
        }
        self.name = name.to_owned();
        self.part(0.0);
    }

    /// The progress inside the step, from 0 to 1.
    fn part(&mut self, fraction: f32) {
        let all = (self.done as f32 + fraction) / self.total.max(1) as f32;
        (self.report)(&self.name, all.min(1.0));
    }
}

/// Downloads a file and its `.sha256` file, and checks the file.
fn download_checked(work: &Path, url: &str, file: &Path, part: &mut dyn FnMut(f32)) -> Result<(), Error> {
    let mut sums = file.as_os_str().to_owned();
    sums.push(".sha256");
    let sums = PathBuf::from(sums);
    download(work, url, file, part)?;
    download(work, &format!("{url}.sha256"), &sums, &mut |_| {})?;
    let text = fs::read_to_string(&sums).map_err(|e| Error::Io(sums.clone(), e))?;
    verify(file, text.split_whitespace().next().unwrap_or_default())
}

fn verify(file: &Path, expected: &str) -> Result<(), Error> {
    let mut hasher = Sha256::new();
    let mut reader = fs::File::open(file).map_err(|e| Error::Io(file.to_owned(), e))?;
    std::io::copy(&mut reader, &mut hasher).map_err(|e| Error::Io(file.to_owned(), e))?;
    let actual: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(Error::Build(format!("{} has SHA-256 {actual}, not {expected}", file.display())));
    }
    Ok(())
}

fn make_dir(dir: &Path) -> Result<(), Error> {
    fs::create_dir_all(dir).map_err(|e| Error::Io(dir.to_owned(), e))
}

/// Renames a folder. A virus scan or the search index can hold a file for a short time, so it
/// tries 5 times (STATUS.md, toolchain prototype).
fn rename(from: &Path, to: &Path) -> Result<(), Error> {
    let mut tries = 0;
    loop {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(_) if tries < 4 => {
                tries += 1;
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(e) => return Err(Error::Io(to.to_owned(), e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn add_wrappers_copies_only_the_missing_wrappers() {
        let folder = std::env::temp_dir().join(format!("patina-wrappers-{}", std::process::id()));
        let (zig, wrapper) = (folder.join("zig"), folder.join("patina.exe"));
        fs::create_dir_all(&zig).unwrap();
        fs::write(&wrapper, "new").unwrap();
        add_wrappers(&folder, &wrapper).unwrap();
        assert!(!zig.join("zig-cc.exe").exists(), "a folder with no Zig gets no wrapper");
        fs::write(zig.join("zig.exe"), "zig").unwrap();
        fs::write(zig.join("zig-cc.exe"), "old").unwrap();
        add_wrappers(&folder, &wrapper).unwrap();
        let text = |name: &str| fs::read_to_string(zig.join(name)).unwrap();
        let all = [text("zig-cc.exe"), text("zig-cxx.exe"), text("zig-ar.exe"), text("zig-dlltool.exe")];
        assert_eq!(all, ["old", "new", "new", "new"]);
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn an_update_needs_an_installed_toolchain() {
        let folder = std::env::temp_dir().join(format!("patina-update-{}", std::process::id()));
        let error = update(&folder, &mut |_, _| panic!("no step")).unwrap_err().to_string();
        assert!(error.contains(if cfg!(windows) { "Install the toolchain first" } else { "only on Windows" }), "{error}");
        assert!(!beside(&folder, "-update").exists());
        assert_eq!(beside(Path::new("a/toolchain"), "-update"), Path::new("a/toolchain-update"));
    }

    #[test]
    fn steps_report_the_part_of_all_the_work() {
        let mut seen = Vec::new();
        let mut report = |name: &str, all: f32| seen.push((name.to_owned(), all));
        let mut steps = Steps::new(4, &mut report);
        steps.next("a");
        steps.part(0.5);
        steps.next("b");
        assert_eq!(seen, [("a".to_owned(), 0.0), ("a".to_owned(), 0.125), ("b".to_owned(), 0.25)]);
        assert_eq!(percent("######                   12.5%"), Some(0.125));
        assert_eq!(percent("100.0%"), Some(1.0));
        assert_eq!(percent("#=#=#"), None);
        assert_eq!(percent("curl: (22) The requested URL returned error: 404"), None);
    }

    #[test]
    fn the_zig_file_is_the_newest_release_on_ziglang_org() {
        let file = |url: &str| json!({ "x86_64-windows": { "tarball": url, "shasum": "a".repeat(64) } });
        let good = "https://ziglang.org/download/0.15.10/zig.zip";
        let index = json!({ "master": file("x"), "0.9.1": file("y"), "0.15.2": file("z"), "0.15.10": file(good) });
        assert_eq!(zig_file(&index).unwrap().1, good);
        assert!(zig_file(&json!({ "0.15.10": file("https://example.com/zig.zip") })).is_err());
    }

    #[test]
    fn packages_read_only_the_target_table() {
        let mut manifest = String::new();
        for name in PACKAGES {
            let target = if *name == "rust-src" { "\"*\"" } else { "x-gnu" };
            manifest += &format!("[pkg.{name}.target.x-gnullvm]\nxz_url = \"wrong\"\nxz_hash = \"wrong\"\n\n");
            manifest += &format!("[pkg.{name}.target.{target}]\navailable = true\nxz_url = \"u-{name}\"\nxz_hash = \"h\"\n\n");
        }
        let found = packages(&manifest, "x-gnu").unwrap();
        assert_eq!(found[1], ("cargo", "u-cargo".to_owned(), "h".to_owned()));
        assert_eq!(found.last().unwrap(), &("rust-src", "u-rust-src".to_owned(), "h".to_owned()));
        let libdir = Path::new("s").join("lib").join("rustlib").join("x").join("lib");
        let toolchain = Toolchain { libdir, ..Toolchain::detect().unwrap() };
        assert_eq!(std_source(&toolchain), Path::new("s/lib/rustlib/src/rust/library"));
        assert!(packages("[pkg.rustc.target.x-gnu]\navailable = false\n", "x-gnu").is_err());
    }

    #[test]
    fn cc_args_replace_the_target_and_verify_reads_the_hash() {
        let args = ["--target=x86_64-pc-windows-gnu", "-O0", "-target", "x", "a b.c"];
        let out = cc_args(args.iter().map(OsString::from).collect());
        assert_eq!(out, ["-target", ZIG_TARGET, "-fno-sanitize=all", "-O0", "a b.c"].map(OsString::from));
        let file = std::env::temp_dir().join(format!("patina-verify-{}", std::process::id()));
        fs::write(&file, "abc").unwrap();
        assert!(verify(&file, "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD").is_ok());
        assert!(verify(&file, "00").is_err());
        fs::remove_file(file).unwrap();
    }
}
