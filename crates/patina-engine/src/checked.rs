//! The build and the load of a checked graph (spec 7 and 8).

use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Checked, Cluster, Error, Mode, Toolchain, cargo, generate, node, plan, project};

/// A checked graph as data, for the node process (16.1). [`Parts::check`] checks it again there.
#[derive(Serialize, Deserialize)]
pub struct Parts {
    root: PathBuf,
    rust: String,
    graph: project::GraphFile,
    aliases: Vec<(String, String)>,
    test: bool,
}

impl Parts {
    /// Plans the graph again, with the node folders as they are now.
    pub fn check(self) -> Result<Checked, Error> {
        let mut plan = plan::check(&self.root, &self.graph).map_err(|problems| Error::Graph(crate::graph_path(&self.root, crate::MAIN), problems))?;
        if self.test {
            plan.probe_all(&self.aliases);
        }
        Ok(Checked { root: self.root, rust: self.rust, plan, graph: self.graph, aliases: self.aliases, test: self.test })
    }
}

impl Checked {
    /// The data of the checked graph, to send to the node process (16.1).
    pub fn into_parts(self) -> Parts {
        Parts { root: self.root, rust: self.rust, graph: self.graph, aliases: self.aliases, test: self.test }
    }

    /// The checked graph, with each use of a subgraph replaced by its nodes (6.4).
    pub fn graph(&self) -> &project::GraphFile {
        &self.graph
    }

    /// Each output pin `<use ID>.<pin key>` of a subgraph use of the main graph, with the inner
    /// output `<use ID>/<node ID>.<key>` that gives its value (6.4).
    pub fn aliases(&self) -> &[(String, String)] {
        &self.aliases
    }

    /// The clock domain of each node use, by the ID of `graph` (9.1).
    pub fn domains(&self) -> impl Iterator<Item = (&str, &'static str)> {
        self.plan.steps.iter().map(|s| (s.id.as_str(), s.domain.name()))
    }

    /// Puts the node folder `name` in the slot (7.1): its uses call the slot library, and a build
    /// of the slot alone builds only this node. `None` empties the slot.
    pub fn set_slot(&mut self, name: Option<&str>) -> Result<(), Error> {
        let Some(name) = name else {
            self.plan.slot = None;
            return Ok(());
        };
        let plan = &self.plan;
        let node = (plan.nodes.iter().position(|(n, _)| n == name))
            .filter(|&i| plan.steps.iter().any(|s| s.node == i))
            .ok_or_else(|| Error::Build(format!("the graph has no use of the node {name}")))?;
        if plan.is_builtin_node(node) {
            return Err(Error::Build(format!("the {name} node is built in, so it does not go into the slot")));
        }
        if plan.steps.iter().any(|s| s.node == node && s.domain == crate::node::Domain::Event) {
            // shortcut: an event node does not go into the slot; the slot call has no async wait (7.1). Add it when an event node needs a fast edit.
            return Err(Error::Build(format!("the node {name} is in the event domain, and an event node does not go into the slot yet")));
        }
        if !plan.nodes[node].1.generics.is_empty() {
            // shortcut: a generic node does not go into the slot; its slot needs a copy for each set of types (7.1).
            return Err(Error::Build(format!("the node {name} is generic, and a generic node does not go into the slot yet")));
        }
        self.plan.slot = Some(node);
        Ok(())
    }

    /// Builds the clusters of the graph in `mode` and loads them (8.3). A bake build with no slot
    /// is a fuse (7.2): `Cluster::adopt` then moves the states from the old clusters.
    pub fn build(&self, toolchain: &Toolchain, mode: Mode) -> Result<Cluster, Error> {
        self.build_with(toolchain, mode, &|| {})
    }

    /// Does `build`. `compiling` runs when cargo starts to compile a crate, so not when all crates
    /// are fresh.
    pub fn build_with(&self, toolchain: &Toolchain, mode: Mode, compiling: &dyn Fn()) -> Result<Cluster, Error> {
        self.check_rust(toolchain)?;
        let built = cargo::build(&self.root, toolchain, &self.plan, mode, compiling)?;
        let paths: Vec<PathBuf> = built.libraries.iter().map(|(_, path)| path.clone()).collect();
        let copies = copy_to_loaded(&self.root, "cluster-", &paths)?;
        let libraries: Vec<(node::Domain, PathBuf)> = built.libraries.iter().map(|(d, _)| *d).zip(copies).collect();
        let slot = copy_to_loaded(&self.root, "slot-", built.slot.as_slice())?;
        copy_runtime(&self.root.join("build").join("loaded"), &toolchain.libdir)?;
        let mut cluster = Cluster::load(&libraries, slot.first().map(PathBuf::as_path), &self.root, &self.plan, built.keys)?;
        cluster.common = built.common;
        cluster.base = self.base_hash();
        cluster.mode = mode;
        Ok(cluster)
    }

    /// Builds only the slot library and puts it in `cluster` (7.1). The nodes in the slot get new
    /// states. Gives false, with no build, when the graph has no slot or the clusters must change:
    /// then build all again with `build`.
    pub fn build_slot(&self, toolchain: &Toolchain, cluster: &mut Cluster) -> Result<bool, Error> {
        self.check_rust(toolchain)?;
        if self.plan.slot.is_none() || cluster.base != self.base_hash() {
            return Ok(false);
        }
        let (library, keys) = cargo::build_slot(&self.root, toolchain, &self.plan, cluster.common)?;
        let copies = copy_to_loaded(&self.root, "slot-", &[library])?;
        cluster.swap_slot(&copies[0], &self.root, keys)?;
        Ok(true)
    }

    pub(crate) fn check_rust(&self, toolchain: &Toolchain) -> Result<(), Error> {
        if toolchain.release == self.rust {
            return Ok(());
        }
        Err(Error::Build(format!("the project needs Rust {}, but the toolchain is Rust {}", self.rust, toolchain.release)))
    }

    /// A hash of the code of the clusters and of the signature of the node in the slot.
    /// shortcut: a type alias in the node folder does not change the signature text, so a change of the alias needs a full build; read the types from the build when users hit it.
    fn base_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        hasher.write(self.wiring().as_bytes());
        if let Some(slot) = self.plan.slot {
            hasher.write(format!("{:?}", self.plan.nodes[slot].1).as_bytes());
        }
        hasher.finish()
    }

    /// The generated code of the cluster. Two checks with the same wiring give the same text, so
    /// a cluster of one stays correct for the other.
    pub fn wiring(&self) -> String {
        let domains = generate::domains(&self.plan);
        domains.into_iter().map(|d| generate::manifest(&self.plan, d) + &generate::source(&self.plan, d)).collect()
    }
}

/// Copies the built libraries to `build/loaded/`, with a hash of the bytes in each file name (8.5).
/// Windows locks a loaded library, so cargo cannot write the library of the next build over it.
/// The same bytes give the same name, so a loaded library stays in use. The copies of `prefix`
/// that are not in `built` are removed.
pub(crate) fn copy_to_loaded(root: &Path, prefix: &str, built: &[PathBuf]) -> Result<Vec<PathBuf>, Error> {
    let dir = root.join("build").join("loaded");
    let mut copies = Vec::new();
    for library in built {
        let bytes = fs::read(library).map_err(|e| Error::Io(library.clone(), e))?;
        let mut hasher = DefaultHasher::new();
        hasher.write(&bytes);
        let path = dir.join(format!("{prefix}{:016x}.{}", hasher.finish(), std::env::consts::DLL_EXTENSION));
        copies.push((path, bytes));
    }
    // An old copy that a process has loaded does not delete on Windows. It stays until later.
    for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
        let old = entry.file_name().to_string_lossy().starts_with(prefix);
        if old && !copies.iter().any(|(path, _)| *path == entry.path()) {
            let _ = fs::remove_file(entry.path());
        }
    }
    fs::create_dir_all(&dir).map_err(|e| Error::Io(dir.clone(), e))?;
    for (path, bytes) in &copies {
        if !path.exists() {
            write_new(path, bytes)?;
        }
    }
    Ok(copies.into_iter().map(|(path, _)| path).collect())
}

/// Copies the shared runtime of the toolchain to `dir`, the folder of the clusters: the `std`
/// library and the libraries of the `bin` folder of the target. A cluster links them, and the
/// loader finds them only there (8.5). The name of `std` holds the hash of its build, so the same name has the same
/// bytes.
pub(crate) fn copy_runtime(dir: &Path, libdir: &Path) -> Result<(), Error> {
    let shared = |path: &Path| path.extension().is_some_and(|e| e == std::env::consts::DLL_EXTENSION);
    let entries = fs::read_dir(libdir).map_err(|e| Error::Io(libdir.to_owned(), e))?;
    let std = entries.flatten().map(|e| e.path()).find(|path| {
        let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
        (name.starts_with("std-") || name.starts_with("libstd-")) && shared(path)
    });
    let std = std.ok_or_else(|| Error::Build(format!("{} has no shared std library", libdir.display())))?;
    // On the gnu target of Windows, `std` needs libgcc_s and winpthreads from here. Other targets have no library here.
    let bin = libdir.with_file_name("bin");
    let more = fs::read_dir(&bin).into_iter().flatten().flatten().map(|e| e.path()).filter(|path| shared(path));
    for library in std::iter::once(std).chain(more) {
        let path = dir.join(library.file_name().unwrap_or_default());
        if !path.exists() {
            write_new(&path, &fs::read(&library).map_err(|e| Error::Io(library.clone(), e))?)?;
        }
    }
    Ok(())
}

/// Writes a new file through a temporary file, so a stopped write leaves no part of a library.
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let temp = path.with_extension("tmp");
    fs::write(&temp, bytes).map_err(|e| Error::Io(temp.clone(), e))?;
    fs::rename(&temp, path).map_err(|e| Error::Io(path.to_owned(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_build_removes_only_the_old_copies_of_its_prefix() {
        let root = std::env::temp_dir().join(format!("patina-loaded-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let built = [root.join("cluster.dll")];
        fs::write(&built[0], "one").unwrap();
        let first = copy_to_loaded(&root, "cluster-", &built).unwrap();
        let slot = copy_to_loaded(&root, "slot-", &built).unwrap();
        let again = copy_to_loaded(&root, "cluster-", &built).unwrap();
        fs::write(&built[0], "two").unwrap();
        let second = copy_to_loaded(&root, "cluster-", &built).unwrap();
        let state = (first[0].exists(), second[0].exists(), slot[0].exists());
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(first, again, "the same bytes keep the name");
        assert_ne!(first, second);
        assert_eq!(state, (false, true, true), "only the old copy of the prefix goes");
    }

    #[test]
    fn the_shared_runtime_is_copied_one_time() {
        let root = std::env::temp_dir().join(format!("patina-std-{}", std::process::id()));
        let (libdir, loaded) = (root.join("lib"), root.join("build").join("loaded"));
        fs::create_dir_all(&libdir).unwrap();
        fs::create_dir_all(&loaded).unwrap();
        fs::write(libdir.join("libstd-1.rlib"), "static").unwrap();
        assert!(copy_runtime(&loaded, &libdir).is_err(), "a static std is not used");
        let name = format!("std-1.{}", std::env::consts::DLL_EXTENSION);
        fs::write(libdir.join(&name), "shared").unwrap();
        copy_runtime(&loaded, &libdir).unwrap();
        fs::write(libdir.join(&name), "changed").unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        let gcc = format!("libgcc-1.{}", std::env::consts::DLL_EXTENSION);
        fs::write(root.join("bin").join(&gcc), "gcc").unwrap();
        fs::write(root.join("bin").join("tool.exe"), "tool").unwrap();
        copy_runtime(&loaded, &libdir).unwrap();
        let copied = fs::read_to_string(loaded.join(&name)).unwrap();
        let more = (loaded.join(&gcc).is_file(), loaded.join("tool.exe").exists());
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(copied, "shared", "a loaded library is not written again");
        assert_eq!(more, (true, false), "only the libraries of bin are copied");
    }
}
