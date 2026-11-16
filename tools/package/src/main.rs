//! Makes the portable zip of Patina (spec 13.1 and 13.3). Run `cargo run -p package`.
//!
//! The tool builds the page and the release programs, collects the license notices of each crate
//! and npm package in the build, copies the example projects to `projects/`, and writes `target/package/patina-<version>-<os>-<arch>.zip`.
//! Then it updates the programs in `target/package/Patina`. That folder keeps its data, its
//! toolchain and its projects, so a person can run Patina from it.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

type Result<T> = std::result::Result<T, String>;

/// The workspace packages in the zip, and their programs. `patina-capi` gives the C library of
/// the packages (spec 14.2).
const PACKAGES: [&str; 3] = ["patina-editor", "patina-cli", "patina-capi"];
const PROGRAMS: [&str; 2] = ["patina", "patina-cli"];

/// The license texts in `notices/` for the crates that have no license file. Each text comes
/// from the repository of the crate.
const NOTICES: [(&str, &str); 4] = [
    ("selectors", "MPL-2.0.txt"),
    ("webview2-com", "webview2-rs-MIT.txt"),
    ("webview2-com-macros", "webview2-rs-MIT.txt"),
    ("webview2-com-sys", "webview2-rs-MIT.txt"),
];

const PORTABLE: &str = "This file marks the portable edition of Patina. Patina keeps all its data in this folder.\n";

fn main() {
    if let Err(e) = run() {
        eprintln!("package: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).ok_or("no repository folder")?;
    let frontend = root.join("crates/patina-editor/frontend");
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    tool(Command::new(npm).args(["run", "build"]).current_dir(&frontend))?;
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut build = Command::new(&cargo);
    build.args(["build", "--release", "--locked"]).current_dir(root);
    for name in PACKAGES {
        build.args(["-p", name]);
    }
    tool(&mut build)?;

    let out = root.join("target").join("package");
    let stage = out.join("stage");
    let folder = stage.join("Patina");
    if folder.exists() {
        fs::remove_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    }
    let licenses = folder.join("licenses");
    make_dir(&licenses)?;
    for name in PROGRAMS {
        let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
        copy(&root.join("target").join("release").join(&file), &folder.join(&file))?;
    }
    let library = format!("{}patina_capi{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX);
    copy(&root.join("target").join("release").join(&library), &folder.join(&library))?;
    write(&folder.join("portable"), PORTABLE)?;
    copy_tree(&root.join("examples"), &folder.join("projects"))?;
    for name in ["LICENSE-MIT", "LICENSE-APACHE"] {
        copy(&root.join(name), &licenses.join(name))?;
    }

    let meta = metadata(&cargo, root)?;
    let mut notes = String::from("The license notices of the crates and npm packages in this build of Patina.\n\n");
    let members: BTreeSet<&str> = list(&meta, "workspace_members").iter().filter_map(Value::as_str).collect();
    let mut version = String::new();
    for package in build_packages(&meta)? {
        let (name, ver) = (text(package, "name"), text(package, "version"));
        // The workspace crates have the license files of Patina.
        if members.contains(text(package, "id")) {
            if name == PACKAGES[0] {
                version = ver.to_owned();
            }
            continue;
        }
        let dir = Path::new(text(package, "manifest_path")).parent().ok_or("a crate has no folder")?;
        let extra = package["license_file"].as_str();
        let dest = licenses.join("crates").join(format!("{name}-{ver}"));
        let mut count = copy_notices(dir, extra, &dest)?;
        if count == 0
            && let Some((_, file)) = NOTICES.iter().find(|(crate_name, _)| *crate_name == name)
        {
            make_dir(&dest)?;
            copy(&root.join("tools/package/notices").join(file), &dest.join(file))?;
            count = 1;
        }
        line(&mut notes, "crate", name, ver, package["license"].as_str(), count);
    }
    for (key, entry) in npm_packages(&frontend)? {
        let name = key.rsplit_once("node_modules/").map_or(key.as_str(), |(_, name)| name);
        let ver = entry["version"].as_str().unwrap_or("0");
        let dir = frontend.join(&key);
        if !dir.is_dir() {
            return Err(format!("{} is missing. Run `npm ci` in {}.", dir.display(), frontend.display()));
        }
        let dest = licenses.join("npm").join(format!("{}-{ver}", name.replace('/', "+")));
        let count = copy_notices(&dir, None, &dest)?;
        line(&mut notes, "npm", name, ver, entry["license"].as_str(), count);
    }
    write(&licenses.join("README.txt"), &notes)?;

    let zip = out.join(format!("patina-{version}-{}-{}.zip", std::env::consts::OS, std::env::consts::ARCH));
    if zip.exists() {
        fs::remove_file(&zip).map_err(|e| format!("{}: {e}", zip.display()))?;
    }
    if cfg!(windows) {
        let system = std::env::var_os("SystemRoot").map_or(PathBuf::from(r"C:\Windows"), PathBuf::from);
        let mut tar = Command::new(system.join(r"System32\tar.exe"));
        tool(tar.arg("-a").arg("-cf").arg(&zip).arg("-C").arg(&stage).arg("Patina"))?;
    } else {
        tool(Command::new("zip").arg("-qr").arg(&zip).arg("Patina").current_dir(&stage))?;
    }
    println!("{}", zip.display());
    update(&folder, &out.join("Patina")).map_err(|e| format!("{e}. Close Patina in target/package/Patina, then run the tool again."))
}

/// Puts the new programs and licenses in the folder that a person runs. The data and the toolchain
/// stay, and an example project is copied only when the folder does not have it (13.3).
fn update(from: &Path, to: &Path) -> Result<()> {
    make_dir(to)?;
    for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", from.display()))?;
        let (path, name) = (entry.path(), entry.file_name());
        let dest = to.join(&name);
        if name == "projects" {
            make_dir(&dest)?;
            for project in fs::read_dir(&path).map_err(|e| format!("{}: {e}", path.display()))? {
                let project = project.map_err(|e| format!("{}: {e}", path.display()))?;
                let target = dest.join(project.file_name());
                if !target.exists() {
                    copy_tree(&project.path(), &target)?;
                }
            }
        } else if path.is_dir() {
            if dest.exists() {
                fs::remove_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
            }
            copy_tree(&path, &dest)?;
        } else {
            copy(&path, &dest)?;
        }
    }
    Ok(())
}

fn metadata(cargo: &std::ffi::OsStr, root: &Path) -> Result<Value> {
    let host = output(Command::new("rustc").arg("-vV"))?;
    let host = host.lines().find_map(|l| l.strip_prefix("host: ")).ok_or("rustc names no host")?;
    let json = output(
        Command::new(cargo)
            .args(["metadata", "--format-version", "1", "--locked", "--filter-platform", host])
            .current_dir(root),
    )?;
    serde_json::from_str(&json).map_err(|e| format!("cargo metadata: {e}"))
}

/// The packages that the build of the programs uses, sorted by name. Dev-dependencies are not
/// in the build.
fn build_packages(meta: &Value) -> Result<Vec<&Value>> {
    let packages: BTreeMap<&str, &Value> = list(meta, "packages").iter().map(|p| (text(p, "id"), p)).collect();
    let nodes: BTreeMap<&str, &Value> =
        list(&meta["resolve"], "nodes").iter().map(|n| (text(n, "id"), n)).collect();
    let mut todo: Vec<&str> = packages.values().filter(|p| PACKAGES.contains(&text(p, "name"))).map(|p| text(p, "id")).collect();
    let mut seen = BTreeSet::new();
    while let Some(id) = todo.pop() {
        if !seen.insert(id) {
            continue;
        }
        let node = nodes.get(id).ok_or_else(|| format!("{id} is not in the resolve graph"))?;
        for dep in list(node, "deps") {
            if list(dep, "dep_kinds").iter().any(|k| k["kind"].as_str() != Some("dev")) {
                todo.push(text(dep, "pkg"));
            }
        }
    }
    let mut found: Vec<&Value> = seen.iter().filter_map(|id| packages.get(id).copied()).collect();
    found.sort_by_key(|p| (text(p, "name"), text(p, "version")));
    Ok(found)
}

/// The npm packages of the page, from `package-lock.json`, without the dev-dependencies.
fn npm_packages(frontend: &Path) -> Result<BTreeMap<String, Value>> {
    let lock = frontend.join("package-lock.json");
    let json = fs::read_to_string(&lock).map_err(|e| format!("{}: {e}", lock.display()))?;
    let lock: Value = serde_json::from_str(&json).map_err(|e| format!("package-lock.json: {e}"))?;
    let packages = lock["packages"].as_object().ok_or("package-lock.json has no packages")?;
    Ok(packages
        .iter()
        .filter(|(key, entry)| key.starts_with("node_modules/") && entry["dev"] != true)
        .map(|(key, entry)| (key.clone(), entry.clone()))
        .collect())
}

/// True for the name of a license or notice file, for example `LICENSE-MIT` or `NOTICE.md`.
fn is_notice(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    ["LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE", "UNLICENSE"].iter().any(|n| name.starts_with(n))
}

/// Copies the notice files of a package folder, and the `license-file` of the crate. Gives the
/// number of files.
fn copy_notices(dir: &Path, extra: Option<&str>, dest: &Path) -> Result<usize> {
    let mut files: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if path.is_file() && path.file_name().is_some_and(|n| is_notice(&n.to_string_lossy())) {
            files.insert(path);
        }
    }
    if let Some(extra) = extra {
        files.insert(dir.join(extra));
    }
    if !files.is_empty() {
        make_dir(dest)?;
    }
    for file in &files {
        copy(file, &dest.join(file.file_name().ok_or("a notice file has no name")?))?;
    }
    Ok(files.len())
}

fn line(notes: &mut String, kind: &str, name: &str, version: &str, license: Option<&str>, count: usize) {
    let note = if count == 0 { " (the package has no license file)" } else { "" };
    let _ = writeln!(notes, "{kind} {name} {version}: {}{note}", license.unwrap_or("no license field"));
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key].as_array().map_or(&[], Vec::as_slice)
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}

fn tool(command: &mut Command) -> Result<()> {
    let name = command.get_program().to_string_lossy().into_owned();
    let status = command.status().map_err(|e| format!("cannot start {name}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{name} failed: {status}")) }
}

fn output(command: &mut Command) -> Result<String> {
    let name = command.get_program().to_string_lossy().into_owned();
    let out = command.output().map_err(|e| format!("cannot start {name}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{name} failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("{name}: {e}"))
}

/// Copies a folder without its build output.
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    make_dir(to)?;
    for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", from.display()))?;
        let (path, name) = (entry.path(), entry.file_name());
        if path.is_dir() {
            if name != "build" && name != "target" {
                copy_tree(&path, &to.join(&name))?;
            }
        } else {
            copy(&path, &to.join(&name))?;
        }
    }
    Ok(())
}

fn make_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    fs::copy(from, to).map(drop).map_err(|e| format!("{}: {e}", from.display()))
}

fn write(path: &Path, text: &str) -> Result<()> {
    fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_names() {
        for name in ["LICENSE", "license-mit", "LICENSE.txt", "COPYING", "NOTICE.md", "UNLICENSE", "Licence"] {
            assert!(is_notice(name), "{name}");
        }
        for name in ["README.md", "Cargo.toml", "src", "MY-LICENSE"] {
            assert!(!is_notice(name), "{name}");
        }
    }

    #[test]
    fn build_packages_skip_dev_dependencies() {
        let meta = serde_json::json!({
            "packages": [
                {"id": "e", "name": "patina-editor", "version": "0.1.0"},
                {"id": "a", "name": "a", "version": "1.0.0"},
                {"id": "b", "name": "b", "version": "2.0.0"},
                {"id": "t", "name": "test-only", "version": "1.0.0"},
            ],
            "resolve": {"nodes": [
                {"id": "e", "deps": [
                    {"pkg": "a", "dep_kinds": [{"kind": null}]},
                    {"pkg": "t", "dep_kinds": [{"kind": "dev"}]},
                ]},
                {"id": "a", "deps": [{"pkg": "b", "dep_kinds": [{"kind": "dev"}, {"kind": "build"}]}]},
                {"id": "b", "deps": []},
                {"id": "t", "deps": []},
            ]},
        });
        let names: Vec<&str> = build_packages(&meta).unwrap().iter().map(|p| text(p, "name")).collect();
        assert_eq!(names, ["a", "b", "patina-editor"]);
    }
}
