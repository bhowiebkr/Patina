//! Find an installed ATL library for static DXC without changing the user's environment.
use std::path::{Path, PathBuf};

pub fn main() {
    println!("cargo:rerun-if-env-changed=LIB");
    #[cfg(windows)]
    find_atl();
}

#[cfg(windows)]
fn find_atl() {
    use std::os::windows::process::CommandExt;
    if std::env::var("TARGET").as_deref() != Ok("x86_64-pc-windows-msvc") {
        return;
    }
    let Some(programs) = std::env::var_os("ProgramFiles(x86)") else {
        return;
    };
    let tool = PathBuf::from(programs).join("Microsoft Visual Studio/Installer/vswhere.exe");
    let Ok(output) = std::process::Command::new(tool)
        .args(["-latest", "-products", "*", "-property", "installationPath"])
        .creation_flags(0x08000000)
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let path = String::from_utf8_lossy(&output.stdout);
    if let Some(dir) = atl_dir(&Path::new(path.trim()).join("VC/Tools/MSVC")) {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn atl_dir(versions: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<_> = std::fs::read_dir(versions)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs.into_iter()
        .rev()
        .map(|p| p.join("atlmfc/lib/x64"))
        .find(|p| p.join("atls.lib").is_file())
}
