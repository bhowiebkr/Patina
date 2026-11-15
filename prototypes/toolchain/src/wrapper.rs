//! The Zig wrappers. cc-rs takes `CC` as one path when the file exists, also with spaces in the
//! path. A value with arguments, such as `zig cc`, splits on spaces. Thus the prototype copies
//! its own executable to `zig-cc.exe` and `zig-ar.exe` next to `zig.exe`, and each copy starts
//! Zig with the subcommand of its file name.

use std::env;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;

/// The Zig target for C code of the Rust target `x86_64-pc-windows-gnu`.
const ZIG_TARGET: &str = "x86_64-windows-gnu";

/// Runs Zig and exits when this executable is a wrapper. Returns when it is not.
pub fn dispatch() {
    let Ok(exe) = env::current_exe() else { return };
    let tool = match exe.file_stem().and_then(|s| s.to_str()) {
        Some("zig-cc") => "cc",
        Some("zig-ar") => "ar",
        _ => return,
    };
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let args = if tool == "cc" { cc_args(args) } else { args };
    let zig_dir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let cache = zig_dir.join("cache");
    if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(zig_dir.join("calls.log")) {
        let text: Vec<_> = args.iter().map(|a| a.to_string_lossy()).collect();
        let _ = writeln!(log, "{tool} {}", text.join(" "));
    }
    let status = Command::new(zig_dir.join("zig.exe"))
        .arg(tool)
        .args(&args)
        // Else Zig keeps its cache in the folders of the user (spec 13.1, portable edition).
        .env("ZIG_GLOBAL_CACHE_DIR", &cache)
        .env("ZIG_LOCAL_CACHE_DIR", &cache)
        .status();
    match status {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("zig-{tool}: cannot start {}: {e}", zig_dir.join("zig.exe").display());
            std::process::exit(1);
        }
    }
}

/// Removes the target arguments of cc-rs, which Zig does not accept, and sets the Zig target.
/// Removes the C sanitizers that Zig adds to debug builds: their runtime is not in the files
/// of the Rust linker.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cc_args_replace_the_target() {
        let args = ["--target=x86_64-pc-windows-gnu", "-O0", "-target", "x", "a b.c"];
        let out = cc_args(args.iter().map(OsString::from).collect());
        let expected = ["-target", ZIG_TARGET, "-fno-sanitize=all", "-O0", "a b.c"];
        assert_eq!(out, expected.map(OsString::from));
    }
}
