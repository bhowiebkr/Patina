//! Packages (spec 14): a folder with the runner program, the built clusters, the project source
//! and the license notices. The runner runs the graph with no toolchain.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Checked, Cluster, Error, Mode, Project, Toolchain, cargo, checked, generate, import, node, project};

/// The header file of a package.
pub const HEADER: &str = "package.toml";

/// The project files and folders that a package holds (6.2). `build/` stays out.
const SOURCE: [&str; 9] = ["patina.toml", "Cargo.toml", "Cargo.lock", "graphs", "nodes", "views", "widgets", "connections", "types"];

/// The C header of the library of a package (14.2).
const C_HEADER: (&str, &str) = ("patina_capi.h", include_str!("../../patina-capi/patina_capi.h"));

const PATINA_LICENSES: [(&str, &str); 2] =
    [("LICENSE-MIT", include_str!("../../../LICENSE-MIT")), ("LICENSE-APACHE", include_str!("../../../LICENSE-APACHE"))];

/// The header of a package (14).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    format: u32,
    /// The version of Patina that made the package.
    patina: String,
    /// The Rust version of the build.
    rust: String,
    /// The SHA-256 of the generated code of the clusters. The runner checks the source with it.
    wiring: String,
}

/// Builds the project at `root` in `mode` and writes a package to the new folder `out`. `runner`
/// is the program that runs the package, and its copy gets the name of `out`. Gives the warnings.
pub fn make(root: &Path, toolchain: &Toolchain, mode: Mode, runner: &Path, out: &Path) -> Result<Vec<String>, Error> {
    if fs::read_dir(out).is_ok_and(|mut entries| entries.next().is_some()) {
        return Err(Error::File(out.to_owned(), "the folder is not empty".to_owned()));
    }
    let name = out.file_name().map(|n| n.to_string_lossy().into_owned()).filter(|n| !n.is_empty() && n != "..");
    let name = name.ok_or_else(|| Error::File(out.to_owned(), "the package folder has no name".to_owned()))?;
    let checked = Project::open(root)?.check()?;
    checked.check_rust(toolchain)?;
    let built = cargo::build(&checked.root, toolchain, &checked.plan, mode, &|| {})?;
    // The package goes to a temporary folder first, so a failure leaves no part of a package.
    let temp = out.with_file_name(format!("{name}.part"));
    if temp.exists() {
        fs::remove_dir_all(&temp).map_err(|e| Error::Io(temp.clone(), e))?;
    }
    let written = write(&temp, &checked, toolchain, &built.libraries, runner, &name);
    let renamed = written.and_then(|warnings| {
        if out.exists() {
            fs::remove_dir(out).map_err(|e| Error::Io(out.to_owned(), e))?;
        }
        fs::rename(&temp, out).map_err(|e| Error::Io(out.to_owned(), e))?;
        Ok(warnings)
    });
    if renamed.is_err() {
        let _ = fs::remove_dir_all(&temp);
    }
    renamed
}

fn write(dir: &Path, checked: &Checked, toolchain: &Toolchain, libraries: &[(node::Domain, PathBuf)], runner: &Path, name: &str) -> Result<Vec<String>, Error> {
    let lib = dir.join("lib");
    fs::create_dir_all(&lib).map_err(|e| Error::Io(lib.clone(), e))?;
    for (domain, path) in libraries {
        let to = lib.join(library_name(*domain));
        fs::copy(path, &to).map_err(|e| Error::Io(to, e))?;
    }
    checked::copy_runtime(&lib, &toolchain.libdir)?;
    for entry in SOURCE {
        let (from, to) = (checked.root.join(entry), dir.join("project").join(entry));
        if from.is_dir() {
            import::copy(&from, &to).map_err(Error::Build)?;
        } else if from.is_file() {
            fs::create_dir_all(dir.join("project")).map_err(|e| Error::Io(dir.join("project"), e))?;
            fs::copy(&from, &to).map_err(|e| Error::Io(to, e))?;
        }
    }
    let program = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    fs::copy(runner, &program).map_err(|e| Error::Io(program, e))?;
    // The C library is next to the runner in the portable zip (14.2).
    let mut warnings = Vec::new();
    let library = format!("{}patina_capi{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX);
    match runner.with_file_name(&library) {
        from if from.is_file() => drop(fs::copy(from, dir.join(&library)).map_err(|e| Error::Io(dir.join(&library), e))?),
        from => warnings.push(format!("{} is not there, so the package has no C library", from.display())),
    }
    fs::write(dir.join(C_HEADER.0), C_HEADER.1).map_err(|e| Error::Io(dir.join(C_HEADER.0), e))?;
    let header = Header { format: 1, patina: env!("CARGO_PKG_VERSION").to_owned(), rust: toolchain.release.clone(), wiring: wiring(checked) };
    let text = toml::to_string(&header).map_err(|e| Error::Build(e.to_string()))?;
    fs::write(dir.join(HEADER), text).map_err(|e| Error::Io(dir.join(HEADER), e))?;
    warnings.extend(notices(&checked.root, toolchain, runner, &dir.join("licenses"))?);
    Ok(warnings)
}

/// Opens the package in `dir` and loads its clusters. The project source must give the same
/// clusters as the build of the package.
pub fn open(dir: &Path) -> Result<(Checked, Cluster), Error> {
    let header: Header = project::read(&dir.join(HEADER), |h: &Header| h.format)?;
    let version = env!("CARGO_PKG_VERSION");
    if header.patina != version {
        return Err(Error::Load(format!("Patina {} made the package, but this runner is Patina {version}", header.patina)));
    }
    let checked = Project::open(&dir.join("project"))?.check()?;
    if wiring(&checked) != header.wiring {
        return Err(Error::Load("the project source of the package does not agree with its clusters".to_owned()));
    }
    let lib = dir.join("lib");
    let libraries: Vec<_> = generate::domains(&checked.plan).into_iter().map(|d| (d, lib.join(library_name(d)))).collect();
    let mut cluster = Cluster::load(&libraries, None, &checked.root, &checked.plan, vec![None; checked.plan.steps.len()])?;
    cluster.mode = Mode::Bake;
    Ok((checked, cluster))
}

/// The parameters of the graph, as `<node ID>.<input key>`. The ID of a node in a subgraph use is
/// `<use ID>/<node ID>` (6.4).
pub fn params(checked: &Checked) -> Vec<String> {
    let plan = &checked.plan;
    plan.slots.iter().map(|s| format!("{}.{}", plan.steps[s.step].id, s.key)).collect()
}

/// Sets the parameters of the arguments `<node ID>.<input key>=<value>` in the graph (14).
pub fn set_args(checked: &mut Checked, args: &[String]) -> Result<(), String> {
    let known = params(checked);
    for arg in args {
        let (name, value) = arg.split_once('=').ok_or_else(|| format!("{arg:?} is not <node ID>.<input key>=<value>"))?;
        if !known.iter().any(|k| k == name) {
            return Err(format!("the graph has no parameter {name}. The parameters are: {}", known.join(", ")));
        }
        let (id, key) = name.rsplit_once('.').expect("a parameter name has a dot");
        let node = checked.graph.node.iter_mut().find(|n| n.id == id).expect("a parameter belongs to a node of the graph");
        node.params.insert(key.to_owned(), value.to_owned());
    }
    Ok(())
}

fn library_name(domain: node::Domain) -> String {
    format!("cluster-{}.{}", domain.name(), std::env::consts::DLL_EXTENSION)
}

fn wiring(checked: &Checked) -> String {
    Sha256::digest(checked.wiring().as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Writes the license notices to `dest` (14): the license files of Patina, the notices of the
/// crates of the runner, and the notices of each crate from outside the project. Gives a warning
/// for each crate with no license and no license file.
fn notices(root: &Path, toolchain: &Toolchain, runner: &Path, dest: &Path) -> Result<Vec<String>, Error> {
    let mut warnings = Vec::new();
    fs::create_dir_all(dest).map_err(|e| Error::Io(dest.to_owned(), e))?;
    for (name, text) in PATINA_LICENSES {
        fs::write(dest.join(name), text).map_err(|e| Error::Io(dest.join(name), e))?;
    }
    // The portable zip has the notices of the crates of Patina next to its programs (13.1).
    match runner.parent().map(|d| d.join("licenses").join("crates")).filter(|d| d.is_dir()) {
        Some(crates) => import::copy(&crates, &dest.join("patina")).map_err(Error::Build)?,
        None => warnings.push(format!("{} has no licenses folder, so the package has no notices of the crates of the runner", runner.display())),
    }
    let mut command = toolchain.command(&toolchain.cargo)?;
    command.current_dir(root).args(["metadata", "--format-version", "1", "--offline", "--filter-platform", &toolchain.host]);
    let meta: serde_json::Value = serde_json::from_str(&cargo::run_text(&mut command)?).map_err(|e| Error::Build(format!("cargo metadata: {e}")))?;
    let mut readme = String::from("The license notices of the crates in this package. The crates of the runner are in `patina`.\n\n");
    for package in meta["packages"].as_array().into_iter().flatten() {
        let text = |key: &str| package[key].as_str().unwrap_or_default();
        let manifest = Path::new(text("manifest_path"));
        if manifest.starts_with(root) {
            continue;
        }
        let (name, version, license) = (text("name"), text("version"), package["license"].as_str());
        let dir = manifest.parent().unwrap_or(manifest);
        let count = copy_notices(dir, package["license_file"].as_str(), &dest.join("crates").join(format!("{name}-{version}")))?;
        if count == 0 && license.is_none() {
            warnings.push(format!("the crate {name} {version} has no license and no license file"));
        }
        let _ = writeln!(readme, "{name} {version}: {}", license.unwrap_or("no license field"));
    }
    fs::write(dest.join("README.txt"), readme).map_err(|e| Error::Io(dest.join("README.txt"), e))?;
    Ok(warnings)
}

/// Copies the notice files of the crate folder `dir` and its `license-file`. Gives the number of
/// files.
fn copy_notices(dir: &Path, extra: Option<&str>, dest: &Path) -> Result<usize, Error> {
    let entries = fs::read_dir(dir).map_err(|e| Error::Io(dir.to_owned(), e))?;
    let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_file() && is_notice(p)).collect();
    files.extend(extra.map(|file| dir.join(file)).filter(|p| !files.contains(p)));
    for file in &files {
        fs::create_dir_all(dest).map_err(|e| Error::Io(dest.to_owned(), e))?;
        let to = dest.join(file.file_name().unwrap_or_default());
        fs::copy(file, &to).map_err(|e| Error::Io(to, e))?;
    }
    Ok(files.len())
}

/// True for a license or notice file, for example `LICENSE-MIT` or `NOTICE.md`.
fn is_notice(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_ascii_uppercase();
    ["LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE", "UNLICENSE"].iter().any(|n| name.starts_with(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_set_parameters_and_a_changed_source_does_not_open() {
        let dir = std::env::temp_dir().join(format!("patina-package-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let root = dir.join("project");
        Project::create(&root, "1.94.0").unwrap();
        let graph = "format = 1\n\n[[node]]\nid = \"00000000000000a1\"\nfolder = \"nodes/number\"\npos = [0.0, 0.0]\n\n[node.params]\nvalue = \"2\"\n";
        fs::write(root.join("graphs/main.toml"), graph).unwrap();
        let mut checked = Project::open(&root).unwrap().check().unwrap();
        assert_eq!(params(&checked), ["00000000000000a1.value"]);
        set_args(&mut checked, &["00000000000000a1.value=3".to_owned()]).unwrap();
        assert_eq!(checked.graph.node[0].params["value"], "3");
        for bad in ["00000000000000a1.value", "b.value=1", "a.other=1"] {
            assert!(set_args(&mut checked, &[bad.to_owned()]).is_err(), "{bad}");
        }
        assert!(notices_of("LICENSE-MIT") && notices_of("notice.md") && !notices_of("README.md"));

        let header = |patina: &str, wiring: &str| format!("format = 1\npatina = \"{patina}\"\nrust = \"1.94.0\"\nwiring = \"{wiring}\"\n");
        fs::write(dir.join(HEADER), header("0.0.0-old", &wiring(&checked))).unwrap();
        let old = open(&dir).err().map(|e| e.to_string()).unwrap_or_default();
        fs::write(dir.join(HEADER), header(env!("CARGO_PKG_VERSION"), "0")).unwrap();
        let changed = open(&dir).err().map(|e| e.to_string()).unwrap_or_default();
        fs::remove_dir_all(&dir).unwrap();
        assert!(old.contains("this runner is Patina"), "{old}");
        assert!(changed.contains("does not agree"), "{changed}");
    }

    fn notices_of(name: &str) -> bool {
        is_notice(Path::new(name))
    }
}
