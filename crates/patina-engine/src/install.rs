//! The **Install toolchain** button (spec 13.2). `curl.exe` makes the downloads and `tar.exe`
//! unpacks Zig. Windows has both in System32, and a firewall that allows only known programs
//! allows them. Rustup installs from a local copy of the server, so it needs no network.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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
/// and rust-analyzer for the code editor. The manifest renames `rust-analyzer` to this package.
const PACKAGES: &[&str] = &["rustc", "cargo", "rust-std", "rust-mingw", "rust-analyzer-preview"];
/// The Zig target for the C code of the Rust target `x86_64-pc-windows-gnu`.
const ZIG_TARGET: &str = "x86_64-windows-gnu";
/// The copies of the editor or the CLI that start Zig (see [`run_wrapper`]).
const WRAPPERS: [&str; 3] = ["zig-cc.exe", "zig-ar.exe", "zig-dlltool.exe"];

/// Installs the toolchain into `folder` and returns its Rust version. The steps work in the
/// folder `<folder>-install`, and the last step renames it, so Patina never uses a part install.
/// A retry continues with the files of the last try. `wrapper` is the program that becomes the
/// Zig wrappers (see [`run_wrapper`]). `step` gets the name of each step.
pub fn install(folder: &Path, wrapper: &Path, step: &mut dyn FnMut(&str)) -> Result<String, Error> {
    steps(folder, wrapper, step).map_err(|e| match e {
        Error::Build(text) => Error::Install(text),
        e => e,
    })
}

fn steps(folder: &Path, wrapper: &Path, step: &mut dyn FnMut(&str)) -> Result<String, Error> {
    if !cfg!(windows) {
        return Err(Error::Build("Install toolchain works only on Windows in this version".into()));
    }
    if Toolchain::installed(folder) {
        add_wrappers(folder, wrapper)?;
        return Ok(Toolchain::find(folder)?.release);
    }
    let mut name = folder.as_os_str().to_owned();
    name.push("-install");
    let work = PathBuf::from(name);
    let downloads = work.join("downloads");
    make_dir(&downloads)?;
    if !Toolchain::installed(&work) {
        rust(&work, &downloads, step)?;
    }
    let zig = work.join("zig");
    if !zig.join("zig.exe").is_file() {
        unpack_zig(&work, &downloads, step)?;
    }
    step("Copy the Zig wrappers");
    for name in WRAPPERS {
        fs::copy(wrapper, zig.join(name)).map_err(|e| Error::Io(zig.join(name), e))?;
    }
    step("Check the toolchain");
    let release = Toolchain::find(&work)?.release;
    let analyzer = work.join("cargo").join("bin").join("rust-analyzer.exe");
    run_text(tool(&work, &analyzer)?.arg("--version"))?;
    let source = downloads.join("check.c");
    fs::write(&source, "int patina_check(void) { return 1; }\n").map_err(|e| Error::Io(source.clone(), e))?;
    run_text(tool(&work, &zig.join("zig-cc.exe"))?.arg("-c").arg(&source).arg("-o").arg(downloads.join("check.o")))?;
    step("Move the toolchain into place");
    fs::remove_dir_all(&downloads).map_err(|e| Error::Io(downloads.clone(), e))?;
    if folder.is_dir() {
        // Only an empty folder goes. A folder with files gives an error, and the user decides.
        fs::remove_dir(folder).map_err(|e| Error::Io(folder.to_owned(), e))?;
    }
    rename(&work, folder)?;
    Ok(release)
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
        "zig-ar" => "ar",
        "zig-dlltool" => "dlltool",
        _ => return None,
    };
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let args = if tool == "cc" { cc_args(args) } else { args };
    let dir = exe.parent().unwrap_or(Path::new("."));
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

/// Installs Rust with rustup: the minimal profile of the stable channel and rust-analyzer, for the
/// gnu host.
fn rust(work: &Path, downloads: &Path, step: &mut dyn FnMut(&str)) -> Result<(), Error> {
    step("Download rustup-init");
    let init = downloads.join("rustup-init.exe");
    download_checked(work, &format!("{DIST}/rustup/dist/{HOST}/rustup-init.exe"), &init)?;
    step("Download the Rust channel manifest");
    let mirror = downloads.join("mirror");
    let manifest = mirror.join("dist").join("channel-rust-stable.toml");
    make_dir(&mirror.join("dist"))?;
    download_checked(work, &format!("{DIST}/dist/channel-rust-stable.toml"), &manifest)?;
    let text = fs::read_to_string(&manifest).map_err(|e| Error::Io(manifest.clone(), e))?;
    for (name, url, hash) in packages(&text, HOST)? {
        step(&format!("Download {name}"));
        let path = url.strip_prefix(DIST).ok_or_else(|| Error::Build(format!("{url} is not on {DIST}")))?;
        let file = mirror.join(path.trim_start_matches('/'));
        make_dir(file.parent().unwrap_or(&mirror))?;
        if verify(&file, &hash).is_err() {
            download(work, &url, &file)?;
            verify(&file, &hash)?;
        }
    }
    step("Install Rust");
    let server = format!("file:///{}", mirror.display().to_string().replace('\\', "/"));
    run_text(tool(work, &init)?.env("RUSTUP_DIST_SERVER", &server).args([
        "-y",
        "--no-modify-path",
        "--profile",
        "minimal",
        "--component",
        "rust-analyzer",
        "--default-host",
        HOST,
        "--default-toolchain",
        "stable",
    ]))?;
    Ok(())
}

/// Returns the name, the xz URL and the hash of each package of [`PACKAGES`].
fn packages(manifest: &str, target: &str) -> Result<Vec<(&'static str, String, String)>, Error> {
    PACKAGES
        .iter()
        .map(|&name| {
            let header = format!("[pkg.{name}.target.{target}]");
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
fn unpack_zig(work: &Path, downloads: &Path, step: &mut dyn FnMut(&str)) -> Result<(), Error> {
    step("Download the Zig index");
    let index_file = downloads.join("zig-index.json");
    download(work, "https://ziglang.org/download/index.json", &index_file)?;
    let bytes = fs::read(&index_file).map_err(|e| Error::Io(index_file.clone(), e))?;
    let index: Value = serde_json::from_slice(&bytes).map_err(|e| Error::Build(format!("the Zig index: {e}")))?;
    let (version, url, sha) = zig_file(&index)?;
    step(&format!("Download Zig {version}"));
    let zip = downloads.join("zig.zip");
    if verify(&zip, &sha).is_err() {
        download(work, &url, &zip)?;
        verify(&zip, &sha)?;
    }
    step(&format!("Unpack Zig {version}"));
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

/// Downloads a file over HTTPS only. A stalled connection fails after 60 s below 1 KB/s, so
/// the retry of curl starts.
fn download(work: &Path, url: &str, file: &Path) -> Result<(), Error> {
    run_text(
        tool(work, &system32().join("curl.exe"))?
            .args(["--fail", "--silent", "--show-error", "--location", "--retry", "3"])
            .args(["--proto", "=https", "--proto-redir", "=https"])
            .args(["--connect-timeout", "30", "--speed-limit", "1024", "--speed-time", "60", "--output"])
            .arg(file)
            .arg(url),
    )?;
    Ok(())
}

/// Downloads a file and its `.sha256` file, and checks the file.
fn download_checked(work: &Path, url: &str, file: &Path) -> Result<(), Error> {
    let mut sums = file.as_os_str().to_owned();
    sums.push(".sha256");
    let sums = PathBuf::from(sums);
    download(work, url, file)?;
    download(work, &format!("{url}.sha256"), &sums)?;
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
        assert_eq!([text("zig-cc.exe"), text("zig-ar.exe"), text("zig-dlltool.exe")], ["old", "new", "new"]);
        fs::remove_dir_all(&folder).unwrap();
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
            manifest += &format!("[pkg.{name}.target.x-gnullvm]\nxz_url = \"wrong\"\nxz_hash = \"wrong\"\n\n");
            manifest += &format!("[pkg.{name}.target.x-gnu]\navailable = true\nxz_url = \"u-{name}\"\nxz_hash = \"h\"\n\n");
        }
        let found = packages(&manifest, "x-gnu").unwrap();
        assert_eq!(found[1], ("cargo", "u-cargo".to_owned(), "h".to_owned()));
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
