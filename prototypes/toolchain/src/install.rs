//! The install steps of the **Install toolchain** button (spec 13.2), in the folder layout of
//! spec 13.3. All downloads use `curl.exe` and the zip uses `tar.exe`. Windows 10 and 11 have
//! both in System32. Rustup installs from a local mirror, so it needs no network access.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{error, run_text};

/// The host and the default target of the toolchain.
pub const HOST: &str = "x86_64-pc-windows-gnu";
/// The download server of Rust.
const DIST: &str = "https://static.rust-lang.org";
/// The packages of the minimal rustup profile (the `[profiles]` table of the channel manifest).
const MINIMAL: &[&str] = &["rustc", "cargo", "rust-std", "rust-mingw"];

/// The environment variables that a command keeps. The prototype removes all the others, so no
/// tool finds Visual Studio, a C compiler or a Rust install through them.
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

/// The `toolchain/` folder of a Patina folder.
pub struct Toolchain {
    pub root: PathBuf,
}

impl Toolchain {
    pub fn at(patina: &Path) -> Self {
        Toolchain { root: patina.join("toolchain") }
    }

    pub fn cargo_home(&self) -> PathBuf {
        self.root.join("cargo")
    }

    pub fn zig_dir(&self) -> PathBuf {
        self.root.join("zig")
    }

    pub fn cargo(&self) -> PathBuf {
        self.cargo_home().join("bin").join("cargo.exe")
    }

    fn downloads(&self) -> PathBuf {
        self.root.join("downloads")
    }

    /// A command with the environment of a clean computer: `PATH` has only the Rust tools and
    /// System32, and the C compiler of the Rust target is the Zig wrapper.
    pub fn command(&self, program: impl AsRef<OsStr>) -> io::Result<Command> {
        let mut command = Command::new(program);
        command.env_clear();
        for var in KEPT_ENV {
            if let Some(value) = env::var_os(var) {
                command.env(var, value);
            }
        }
        let path = env::join_paths([self.cargo_home().join("bin"), system32()])
            .map_err(|e| error(format!("the folder path cannot go in PATH: {e}")))?;
        let zig = self.zig_dir();
        command
            .env("PATH", path)
            .env("RUSTUP_HOME", self.root.join("rustup"))
            .env("CARGO_HOME", self.cargo_home())
            .env("CC_x86_64_pc_windows_gnu", zig.join("zig-cc.exe"))
            .env("AR_x86_64_pc_windows_gnu", zig.join("zig-ar.exe"))
            // GNU ld reads ANSI arguments, so it cannot open a path with CJK letters. rust-lld can.
            // Cargo also uses this linker for build scripts, because the target is the host.
            .env("CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER", "rust-lld")
            .env("CARGO_TERM_COLOR", "never");
        Ok(command)
    }
}

pub fn system32() -> PathBuf {
    let root = env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    Path::new(&root).join("System32")
}

fn download(toolchain: &Toolchain, url: &str, file: &Path) -> io::Result<()> {
    fs::create_dir_all(toolchain.downloads())?;
    run_text(
        toolchain
            .command(system32().join("curl.exe"))?
            .args(["--fail", "--silent", "--show-error", "--location", "--retry", "3"])
            // A stalled connection fails after 60 s below 1 KB/s, so the retry starts.
            .args(["--connect-timeout", "30", "--speed-limit", "1024", "--speed-time", "60", "--output"])
            .arg(file)
            .arg(url),
    )?;
    Ok(())
}

fn verify(file: &Path, expected: &str) -> io::Result<()> {
    let actual: String = Sha256::digest(fs::read(file)?).iter().map(|b| format!("{b:02x}")).collect();
    if actual != expected.to_ascii_lowercase() {
        return Err(error(format!("{} has SHA-256 {actual}, not {expected}", file.display())));
    }
    Ok(())
}

/// Downloads a file and its `.sha256` file, and checks the file. Returns the hash.
fn download_checked(toolchain: &Toolchain, url: &str, file: &Path) -> io::Result<String> {
    let mut sums = file.as_os_str().to_owned();
    sums.push(".sha256");
    let sums = PathBuf::from(sums);
    download(toolchain, url, file)?;
    download(toolchain, &format!("{url}.sha256"), &sums)?;
    let text = fs::read_to_string(&sums)?;
    let expected = text.split_whitespace().next().unwrap_or_default().to_owned();
    verify(file, &expected)?;
    Ok(expected)
}

/// Installs Rust with rustup: the minimal profile of the stable channel, for the gnu host.
pub fn rust(toolchain: &Toolchain) -> io::Result<Value> {
    if toolchain.cargo().is_file() {
        return Ok(json!("reused"));
    }
    let init = toolchain.downloads().join("rustup-init.exe");
    let url = format!("{DIST}/rustup/dist/{HOST}/rustup-init.exe");
    let expected = download_checked(toolchain, &url, &init)?;
    let mirror = mirror(toolchain)?;
    let server = format!("file:///{}", mirror.display().to_string().replace('\\', "/"));
    run_text(toolchain.command(&init)?.env("RUSTUP_DIST_SERVER", &server).args([
        "-y",
        "--no-modify-path",
        "--profile",
        "minimal",
        "--default-host",
        HOST,
        "--default-toolchain",
        "stable",
    ]))?;
    Ok(json!({ "rustup_init_sha256": expected, "dist_server": server }))
}

/// Downloads the stable channel manifest and the packages of the minimal profile into a local
/// mirror of static.rust-lang.org.
/// shortcut: an allowlist firewall can block a new rustup-init.exe, so curl.exe downloads all
/// the files. Keep it until the engine has its own downloader (spec 13.2).
fn mirror(toolchain: &Toolchain) -> io::Result<PathBuf> {
    let mirror = toolchain.downloads().join("mirror");
    let manifest = mirror.join("dist").join("channel-rust-stable.toml");
    fs::create_dir_all(mirror.join("dist"))?;
    download_checked(toolchain, &format!("{DIST}/dist/channel-rust-stable.toml"), &manifest)?;
    for (url, hash) in packages(&fs::read_to_string(&manifest)?, HOST)? {
        let path = url.strip_prefix(DIST).ok_or_else(|| error(format!("{url} is not on {DIST}")))?;
        let file = mirror.join(path.trim_start_matches('/'));
        fs::create_dir_all(file.parent().unwrap_or(&mirror))?;
        download(toolchain, &url, &file)?;
        verify(&file, &hash)?;
    }
    Ok(mirror)
}

/// Returns the xz URL and the hash of each package of the minimal profile for a target.
fn packages(manifest: &str, target: &str) -> io::Result<Vec<(String, String)>> {
    MINIMAL
        .iter()
        .map(|name| {
            let header = format!("[pkg.{name}.target.{target}]");
            let table = manifest.split(&header).nth(1).unwrap_or_default();
            let field = |key: &str| {
                table.lines().skip(1).take_while(|line| !line.trim().is_empty()).find_map(|line| {
                    let value = line.strip_prefix(key)?.trim_start().strip_prefix('=')?.trim();
                    Some(value.strip_prefix('"')?.strip_suffix('"')?.to_owned())
                })
            };
            match (field("xz_url"), field("xz_hash")) {
                (Some(url), Some(hash)) => Ok((url, hash)),
                _ => Err(error(format!("the channel manifest has no xz file for {name} on {target}"))),
            }
        })
        .collect()
}

/// Installs the newest Zig release from the index of ziglang.org.
pub fn zig(toolchain: &Toolchain) -> io::Result<Value> {
    let exe = toolchain.zig_dir().join("zig.exe");
    if exe.is_file() {
        return Ok(json!("reused"));
    }
    let index_file = toolchain.downloads().join("zig-index.json");
    download(toolchain, "https://ziglang.org/download/index.json", &index_file)?;
    let index: Value = serde_json::from_slice(&fs::read(&index_file)?)?;
    let (version, release) = latest_release(&index).ok_or_else(|| error("no Zig release in the index"))?;
    let file = &release["x86_64-windows"];
    let (Some(url), Some(sha)) = (file["tarball"].as_str(), file["shasum"].as_str()) else {
        return Err(error(format!("Zig {version} has no x86_64-windows file")));
    };
    let zip = toolchain.downloads().join("zig.zip");
    download(toolchain, url, &zip)?;
    verify(&zip, sha)?;
    let unpack = toolchain.root.join("zig-unpack");
    fs::create_dir_all(&unpack)?;
    run_text(toolchain.command(system32().join("tar.exe"))?.arg("-xf").arg(&zip).arg("-C").arg(&unpack))?;
    let inner = fs::read_dir(&unpack)?
        .next()
        .ok_or_else(|| error("the Zig zip is empty"))??
        .path();
    fs::rename(inner, toolchain.zig_dir())?;
    fs::remove_dir(&unpack)?;
    fs::remove_dir_all(toolchain.downloads())?;
    Ok(json!({ "version": version, "url": url, "sha256": sha }))
}

/// Copies this executable to the Zig wrappers (see the wrapper module).
pub fn wrappers(toolchain: &Toolchain) -> io::Result<Value> {
    let me = env::current_exe()?;
    for name in ["zig-cc.exe", "zig-ar.exe"] {
        fs::copy(&me, toolchain.zig_dir().join(name))?;
    }
    Ok(json!({ "copied_from": me }))
}

/// Returns the highest version `x.y.z` of the index. It skips `master`.
fn latest_release(index: &Value) -> Option<(String, &Value)> {
    let parse = |key: &str| -> Option<Vec<u32>> {
        let parts: Option<Vec<u32>> = key.split('.').map(|p| p.parse().ok()).collect();
        parts.filter(|p| p.len() == 3)
    };
    index
        .as_object()?
        .iter()
        .filter_map(|(key, value)| Some((parse(key)?, key, value)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, key, value)| (key.clone(), value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_release_skips_master() {
        let index = json!({ "master": {}, "0.9.1": {}, "0.15.2": { "a": 1 }, "0.15.10": { "a": 2 } });
        let (version, release) = latest_release(&index).unwrap();
        assert_eq!((version.as_str(), &release["a"]), ("0.15.10", &json!(2)));
    }

    #[test]
    fn packages_read_only_the_target_table() {
        let mut manifest = String::new();
        for name in MINIMAL {
            manifest += &format!("[pkg.{name}.target.x-gnullvm]\nxz_url = \"wrong\"\nxz_hash = \"wrong\"\n\n");
            manifest += &format!("[pkg.{name}.target.x-gnu]\navailable = true\nxz_url = \"u-{name}\"\nxz_hash = \"h\"\n\n");
        }
        let found = packages(&manifest, "x-gnu").unwrap();
        assert_eq!(found[1], ("u-cargo".to_owned(), "h".to_owned()));
        assert!(packages("[pkg.rustc.target.x-gnu]\navailable = false\n", "x-gnu").is_err());
    }
}
