//! The Patina engine: reads a project, edits and saves its graph, builds the graph into a cluster
//! and runs it (spec 3, 7, 8).

mod cargo;
pub mod clock;
mod edit;
mod generate;
pub mod install;
mod library;
mod load;
mod node;
mod plan;
mod pool;
pub mod project;
mod subgraph;
mod types;

use std::fmt;
use std::fs;
use std::hash::{BuildHasher, DefaultHasher, Hasher, RandomState};
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub use cargo::{Diagnostic, Mode, Toolchain, add_crate};
pub use edit::Edit;
pub use load::{Cluster, Lanes, NodeLog, NodeTime, Outcome, Report, ReportKind, Value};
pub use patina_sdk::glue::Control;
use project::GraphFile;

#[derive(Debug)]
pub enum Error {
    Io(PathBuf, io::Error),
    /// A project file does not read.
    File(PathBuf, String),
    /// The graph file has these problems.
    Graph(PathBuf, Vec<String>),
    Build(String),
    /// The code does not compile.
    Compile(Vec<Diagnostic>),
    Load(String),
    /// An install step of the toolchain failed.
    Install(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(path, e) => write!(f, "{}: {e}", path.display()),
            Error::File(path, text) => write!(f, "{}: {text}", path.display()),
            Error::Graph(path, problems) => {
                write!(f, "{} has problems:", path.display())?;
                problems.iter().try_for_each(|p| write!(f, "\n  {p}"))
            }
            Error::Build(text) => write!(f, "the build failed: {text}"),
            Error::Compile(errors) => {
                write!(f, "the code does not compile:")?;
                errors.iter().try_for_each(|e| write!(f, "\n{}", e.rendered.trim_end()))
            }
            Error::Load(text) => write!(f, "the cluster did not load: {text}"),
            Error::Install(text) => f.write_str(text),
        }
    }
}

impl std::error::Error for Error {}

/// A project with its main graph and the undo history of the graph and the code.
pub struct Project {
    root: PathBuf,
    /// The stable ID of `patina.toml` (5.3). The text is not checked.
    id: String,
    rust: String,
    history: edit::History,
    /// The number of code writes of edits, undo and redo.
    code_revision: u64,
    /// The path of the last code write.
    last_code: Option<String>,
}

/// An input or an output of a node (spec 4.1).
#[derive(Serialize, Debug, PartialEq)]
pub struct Pin {
    pub key: String,
    /// The Rust type as the code writes it. An output has the type of its value.
    pub ty: String,
}

/// A node folder of the project or a built-in node, for the node list of the editor (15.2).
#[derive(Serialize, Debug)]
pub struct NodeInfo {
    /// The folder name in `nodes/`, or the name of the built-in node.
    pub name: String,
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
    /// The problem of the folder when its node does not read.
    pub error: Option<String>,
    /// True for a built-in node, for example the delay (9.2).
    pub builtin: bool,
    /// The type parameters (4.5). A pin of such a type joins a wire of any type.
    pub generics: Vec<String>,
    /// True when the node GUI has a full size (4.3).
    pub full: bool,
}

impl NodeInfo {
    fn new(name: String, node: node::Node, builtin: bool) -> NodeInfo {
        let keys = node.output_keys().into_iter();
        let outputs = keys.zip(node.output_types).map(|(key, ty)| Pin { key, ty }).collect();
        let inputs = node.inputs.into_iter().map(|i| Pin { key: i.key, ty: i.ty }).collect();
        NodeInfo { name, inputs, outputs, error: None, builtin, generics: node.generics, full: node.full }
    }
}

/// The built-in nodes, for the node list of the editor (9.2).
pub fn builtins() -> Vec<NodeInfo> {
    vec![NodeInfo::new(plan::DELAY.to_owned(), node::delay(), true)]
}

/// A graph that passed all checks, ready to build.
pub struct Checked {
    root: PathBuf,
    rust: String,
    plan: plan::Plan,
    graph: project::GraphFile,
}

/// The files of a new project (15.10), from the hello example.
const NEW_FILES: [(&str, &str); 5] = [
    ("graphs/main.toml", "format = 1\n"),
    ("nodes/number/Cargo.toml", include_str!("../../../examples/hello/nodes/number/Cargo.toml")),
    ("nodes/number/src/lib.rs", include_str!("../../../examples/hello/nodes/number/src/lib.rs")),
    ("nodes/show/Cargo.toml", include_str!("../../../examples/hello/nodes/show/Cargo.toml")),
    ("nodes/show/src/lib.rs", include_str!("../../../examples/hello/nodes/show/src/lib.rs")),
];

impl Project {
    /// Reads `patina.toml` and `graphs/main.toml` of the project folder. A graph with problems
    /// opens, so that the user can repair it. `check` finds the problems.
    pub fn open(path: &Path) -> Result<Project, Error> {
        // The path must be absolute, because cargo reads the target folder from a variable.
        let root = std::path::absolute(path).map_err(|e| Error::Io(path.to_owned(), e))?;
        let file = project::read(&root.join("patina.toml"), |f: &project::ProjectFile| f.format)?;
        let graph = project::read(&graph_path(&root), |g: &GraphFile| g.format)?;
        Ok(Project { root, id: file.id, rust: file.rust, history: edit::History::new(graph), code_revision: 0, last_code: None })
    }

    /// Makes a new project in a new or empty folder, with the `number` and `show` nodes and an
    /// empty main graph (15.10).
    pub fn create(path: &Path, rust: &str) -> Result<Project, Error> {
        let root = std::path::absolute(path).map_err(|e| Error::Io(path.to_owned(), e))?;
        if fs::read_dir(&root).is_ok_and(|mut entries| entries.next().is_some()) {
            return Err(Error::File(root, "the folder is not empty".to_owned()));
        }
        let id = RandomState::new().hash_one(std::time::SystemTime::now());
        let project = format!("format = 1\nid = \"{id:016x}\"\nrust = \"{rust}\"\n");
        for (relative, text) in [("patina.toml", project.as_str())].into_iter().chain(NEW_FILES) {
            write_file(&root.join(relative), text)?;
        }
        Project::open(&root)
    }

    /// Makes the node folder `nodes/<name>` with a start node (15.2).
    pub fn new_node(&self, name: &str) -> Result<(), String> {
        let code = format!(
            "use patina_sdk::node;\n\n/// Gives the value of the input. Write the work of the node here.\n\
             #[node]\npub fn {}(value: f64) -> f64 {{\n    value\n}}\n",
            plan::name_key(name)
        );
        self.new_folder("nodes", name, &code)
    }

    /// Makes the widget folder `widgets/<name>` with a start widget (4.3, 6.2).
    pub fn new_widget(&self, name: &str) -> Result<(), String> {
        plan::check_name(name)?;
        let ty: String = name.split(['-', '_']).filter(|p| !p.is_empty()).map(|p| p[..1].to_ascii_uppercase() + &p[1..]).collect();
        self.new_folder("widgets", name, &WIDGET_CODE.replace("NAME", &ty))
    }

    /// Makes the crate folder `<group>/<name>` with `code` in `src/lib.rs`. No other folder of the
    /// project can have the same crate name (6.2).
    fn new_folder(&self, group: &str, name: &str, code: &str) -> Result<(), String> {
        plan::check_name(name)?;
        let key = plan::name_key(name);
        for group in ["nodes", "views", "widgets", "connections", "types"] {
            for entry in fs::read_dir(self.root.join(group)).into_iter().flatten().flatten() {
                let other = entry.file_name().to_string_lossy().into_owned();
                if plan::name_key(&other) == key {
                    return Err(format!("the folder {group}/{other} has this name"));
                }
            }
        }
        let parent = self.root.join(group);
        fs::create_dir_all(&parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        // create_dir fails when the folder exists, so the files of another folder stay.
        let dir = parent.join(name);
        fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let manifest = format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\npatina-sdk = {{ workspace = true }}\n"
        );
        let written = write_file(&dir.join("Cargo.toml"), &manifest).and_then(|()| write_file(&dir.join("src/lib.rs"), code));
        if let Err(e) = written {
            let _ = fs::remove_dir_all(&dir);
            return Err(e.to_string());
        }
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// The Rust version that the project builds with (6.4).
    pub fn rust(&self) -> &str {
        &self.rust
    }

    pub fn graph(&self) -> &GraphFile {
        &self.history.graph
    }

    /// Applies an edit of the user (7.4). A code edit writes its file at once, because cargo
    /// builds from the files.
    pub fn apply(&mut self, edit: &Edit) -> Result<(), String> {
        let Edit::SetCode { path, text } = edit else { return self.history.apply(edit) };
        let old = swap_code(&self.root, path, text)?;
        if old == *text {
            return Err("the edit changes nothing".to_owned());
        }
        self.history.push_code(path.clone(), old);
        self.code_revision += 1;
        self.last_code = Some(path.clone());
        Ok(())
    }

    /// Replaces the main graph as one step of undo, for example with the graph of a recovery file
    /// (15.10). The graph is not saved.
    pub fn restore_graph(&mut self, graph: GraphFile) {
        self.history.replace(graph);
    }

    /// Removes the last edit. Returns false when there is no edit to remove.
    pub fn undo(&mut self) -> Result<bool, String> {
        self.step(true)
    }

    /// Applies the last removed edit again. Returns false when there is no such edit.
    pub fn redo(&mut self) -> Result<bool, String> {
        self.step(false)
    }

    fn step(&mut self, undo: bool) -> Result<bool, String> {
        let root = &self.root;
        let mut wrote = None;
        let write = |path: &str, text: &str| {
            wrote = Some(path.to_owned());
            swap_code(root, path, text)
        };
        let done = if undo { self.history.undo(write) } else { self.history.redo(write) };
        if let (Some(path), true) = (wrote, done.is_ok()) {
            self.code_revision += 1;
            self.last_code = Some(path);
        }
        done
    }

    /// Changes when an edit, undo or redo writes a code file. Each step writes one file.
    pub fn code_revision(&self) -> u64 {
        self.code_revision
    }

    /// The path of the last code file that an edit, undo or redo wrote.
    pub fn last_code(&self) -> Option<&str> {
        self.last_code.as_deref()
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// True when the graph has changes that are not saved.
    pub fn dirty(&self) -> bool {
        self.history.dirty()
    }

    /// Writes the main graph to `graphs/main.toml`. The undo history stays (15.10).
    pub fn save(&mut self) -> Result<(), Error> {
        project::write(&graph_path(&self.root), &self.history.graph)?;
        self.history.mark_saved();
        Ok(())
    }

    /// The node folders in `nodes/`, sorted by name.
    pub fn nodes(&self) -> Result<Vec<NodeInfo>, Error> {
        let dir = self.root.join("nodes");
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::Io(dir, e)),
        };
        let mut nodes = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| Error::Io(dir.clone(), e))?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
            if !entry.path().is_dir() {
                continue;
            }
            nodes.push(match plan::read_node(&self.root, &name) {
                Ok(node) => NodeInfo::new(name, node, false),
                Err(text) => NodeInfo { name, inputs: Vec::new(), outputs: Vec::new(), error: Some(text), builtin: false, generics: Vec::new(), full: false },
            });
        }
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(nodes)
    }

    /// The widget folders in `widgets/` (6.2), sorted by name.
    pub fn widgets(&self) -> Vec<String> {
        let entries = fs::read_dir(self.root.join("widgets")).into_iter().flatten().flatten();
        let dirs = entries.filter(|e| e.path().is_dir()).filter_map(|e| e.file_name().into_string().ok());
        let mut names: Vec<String> = dirs.filter(|name| plan::check_name(name).is_ok()).collect();
        names.sort();
        names
    }

    /// The files of a node or widget folder, for example `nodes/number` (15.4), as sorted paths
    /// from the project root. Hidden files and `target` folders are not in the list.
    pub fn folder_files(&self, folder: &str) -> Result<Vec<String>, Error> {
        let dir = self.root.join(folder);
        code_folder(folder).map_err(|text| Error::File(dir.clone(), text))?;
        let mut files = Vec::new();
        list_files(&dir, folder, &mut files)?;
        files.sort();
        Ok(files)
    }

    /// The text of a file in a node or widget folder, for example `nodes/number/src/lib.rs`.
    pub fn read_code(&self, path: &str) -> Result<String, String> {
        let file = code_file(&self.root, path)?;
        fs::read_to_string(&file).map_err(|e| format!("{path}: {e}"))
    }

    /// Checks the main graph and puts its nodes in run order.
    pub fn check(&self) -> Result<Checked, Error> {
        let fail = |problems| Error::Graph(graph_path(&self.root), problems);
        let graph = subgraph::flatten(&self.root, &self.history.graph).map_err(fail)?;
        let plan = plan::check(&self.root, &graph).map_err(fail)?;
        Ok(Checked { root: self.root.clone(), rust: self.rust.clone(), plan, graph })
    }
}

impl Checked {
    /// The checked graph, with each use of a subgraph replaced by its nodes (6.4).
    pub fn graph(&self) -> &project::GraphFile {
        &self.graph
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
        if plan.delay == Some(node) {
            return Err(Error::Build("the delay node is built in, so it does not go into the slot".to_owned()));
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
        self.check_rust(toolchain)?;
        let built = cargo::build(&self.root, toolchain, &self.plan, mode)?;
        let paths: Vec<PathBuf> = built.libraries.iter().map(|(_, path)| path.clone()).collect();
        let copies = copy_to_loaded(&self.root, "cluster-", &paths)?;
        let libraries: Vec<(node::Domain, PathBuf)> = built.libraries.iter().map(|(d, _)| *d).zip(copies).collect();
        let slot = copy_to_loaded(&self.root, "slot-", built.slot.as_slice())?;
        copy_runtime(&self.root, &toolchain.libdir)?;
        let mut cluster = Cluster::load(&libraries, slot.first().map(PathBuf::as_path), &self.plan, built.keys)?;
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
        cluster.swap_slot(&copies[0], keys)?;
        Ok(true)
    }

    fn check_rust(&self, toolchain: &Toolchain) -> Result<(), Error> {
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

fn write_file(path: &Path, text: &str) -> Result<(), Error> {
    let parent = path.parent().expect("a project file is in a folder");
    fs::create_dir_all(parent).map_err(|e| Error::Io(parent.to_owned(), e))?;
    fs::write(path, text).map_err(|e| Error::Io(path.to_owned(), e))
}

fn graph_path(root: &Path) -> PathBuf {
    root.join("graphs").join("main.toml")
}

/// The start code of a widget folder. `NAME` is the type name.
const WIDGET_CODE: &str = "/// A bar that the user drags to set a value from 0 to 1. Write the drawing and the input of the
/// widget here. A node keeps the widget in its state and shows it with `ui.add(&mut self.bar)`.
pub struct NAME {
    pub value: f32,
}

impl patina_sdk::Widget for NAME {
    fn show(&mut self, ui: &mut patina_sdk::Ui) -> patina_sdk::Response {
        const SIZE: [f32; 2] = [120.0, 16.0];
        let mut canvas = ui.canvas(\"\", SIZE);
        let mut changed = false;
        for pointer in canvas.pointer() {
            if let patina_sdk::Pointer::Down(at) | patina_sdk::Pointer::Move(at) = pointer {
                self.value = (at[0] / SIZE[0]).clamp(0.0, 1.0);
                changed = true;
            }
        }
        canvas.rect([0.0, 0.0], [SIZE[0] * self.value, SIZE[1]], [200, 140, 60], true);
        patina_sdk::Response::new(false, changed)
    }
}
";

/// Checks a node or widget folder as `nodes/<name>` or `widgets/<name>` (6.2). The page sends it.
pub(crate) fn code_folder(folder: &str) -> Result<(), String> {
    let (group, name) = folder.split_once('/').ok_or_else(|| format!("{folder:?} is not a node or widget folder"))?;
    if group != "nodes" && group != "widgets" {
        return Err(format!("{folder:?} is not a node or widget folder"));
    }
    plan::check_name(name)
}

/// The full path of an existing file in a node or widget folder, from a path like
/// `nodes/number/src/lib.rs` (6.2). The page sends the path, so it must stay in the folder.
fn code_file(root: &Path, path: &str) -> Result<PathBuf, String> {
    let bad = || format!("{path:?} is not a file in a node or widget folder");
    let mut parts = path.split('/');
    let folder = [parts.next().ok_or_else(bad)?, parts.next().ok_or_else(bad)?].join("/");
    code_folder(&folder).map_err(|_| bad())?;
    let mut file = root.join(&folder);
    let mut depth = 0;
    for part in parts {
        if part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']) {
            return Err(bad());
        }
        file.push(part);
        depth += 1;
    }
    if depth == 0 || !file.is_file() {
        return Err(format!("the project has no file {path}"));
    }
    Ok(file)
}

/// Writes the text to a code file and returns the old text. A temporary file and a rename keep
/// the old text when the write fails.
fn swap_code(root: &Path, path: &str, text: &str) -> Result<String, String> {
    let file = code_file(root, path)?;
    let old = fs::read_to_string(&file).map_err(|e| format!("{path}: {e}"))?;
    if old != text {
        let temp = file.with_extension("patina-tmp");
        fs::write(&temp, text).map_err(|e| format!("{}: {e}", temp.display()))?;
        fs::rename(&temp, &file).map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(old)
}

pub(crate) fn list_files(dir: &Path, relative: &str, files: &mut Vec<String>) -> Result<(), Error> {
    for entry in fs::read_dir(dir).map_err(|e| Error::Io(dir.to_owned(), e))? {
        let entry = entry.map_err(|e| Error::Io(dir.to_owned(), e))?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
        // The file type does not follow links, so a link loop cannot make the walk endless.
        let Ok(kind) = entry.file_type() else { continue };
        if name.starts_with('.') || (kind.is_dir() && name == "target") {
            continue;
        }
        let path = format!("{relative}/{name}");
        if kind.is_dir() {
            list_files(&entry.path(), &path, files)?;
        } else if kind.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

/// Copies the built libraries to `build/loaded/`, with a hash of the bytes in each file name (8.5).
/// Windows locks a loaded library, so cargo cannot write the library of the next build over it.
/// The same bytes give the same name, so a loaded library stays in use. The copies of `prefix`
/// that are not in `built` are removed.
fn copy_to_loaded(root: &Path, prefix: &str, built: &[PathBuf]) -> Result<Vec<PathBuf>, Error> {
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

/// Copies the shared runtime of the toolchain to `build/loaded/`: the `std` library and the
/// libraries of the `bin` folder of the target. A cluster links them, and the loader finds them
/// only there (8.5). The name of `std` holds the hash of its build, so the same name has the same
/// bytes.
fn copy_runtime(root: &Path, libdir: &Path) -> Result<(), Error> {
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
        let path = root.join("build").join("loaded").join(library.file_name().unwrap_or_default());
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
    fn a_new_project_lists_its_nodes_and_saves_edits() {
        let root = std::env::temp_dir().join(format!("patina-new-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut project = Project::create(&root, "1.94.0").unwrap();
        assert!(Project::create(&root, "1.94.0").is_err(), "a folder with files is not used");
        let nodes = project.nodes().unwrap();
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["number", "show"]);
        let pin = |key: &str, ty: &str| Pin { key: key.to_owned(), ty: ty.to_owned() };
        assert_eq!((&nodes[0].inputs[..], &nodes[0].outputs[..]), (&[pin("value", "f64")][..], &[pin("out", "f64")][..]));
        assert_eq!(nodes[1].inputs, [pin("value", "&f64")]);
        assert_eq!(project.folder_files("nodes/show").unwrap(), ["nodes/show/Cargo.toml", "nodes/show/src/lib.rs"]);
        assert!(project.read_code("nodes/show/src/lib.rs").unwrap().contains("println!"));
        for bad in ["patina.toml", "nodes/show", "nodes/show/../../patina.toml", "nodes/show/src\\..\\x", "nodes/show/c:x"] {
            assert!(project.read_code(bad).is_err(), "{bad}");
        }
        for bad in ["nodes/..", "nodes", "graphs/main", "widgets/a/b"] {
            assert!(project.folder_files(bad).is_err(), "{bad}");
        }

        project.new_node("my-node").unwrap();
        let made = project.nodes().unwrap().into_iter().find(|n| n.name == "my-node").unwrap();
        assert_eq!((made.error, &made.inputs[..]), (None, &[pin("value", "f64")][..]));
        for bad in ["My_Node", "number", "type", "Con", "1a", "a/b", "..", ""] {
            assert!(project.new_node(bad).is_err(), "{bad}");
        }
        project.new_widget("gradient-bar").unwrap();
        assert!(project.new_widget("Number").is_err(), "a widget and a node have other names");
        assert_eq!(project.widgets(), ["gradient-bar"]);
        let code = project.read_code("widgets/gradient-bar/src/lib.rs").unwrap();
        assert!(code.contains("pub struct GradientBar {"), "{code}");

        let path = "nodes/show/src/lib.rs".to_owned();
        let code = Edit::SetCode { path: path.clone(), text: "// new\n".to_owned() };
        project.apply(&code).unwrap();
        assert!(project.apply(&code).is_err(), "an edit that changes nothing is not kept");
        assert_eq!((project.read_code(&path).unwrap().as_str(), project.code_revision()), ("// new\n", 1));
        assert!(project.undo().unwrap());
        assert!(project.read_code(&path).unwrap().contains("println!"), "undo writes the old text");
        assert!(project.redo().unwrap());
        assert_eq!((project.read_code(&path).unwrap().as_str(), project.code_revision()), ("// new\n", 3));
        assert_eq!(project.last_code(), Some(path.as_str()));
        let missing = Edit::SetCode { path: "nodes/show/src/new.rs".to_owned(), text: String::new() };
        assert!(project.apply(&missing).is_err(), "a code edit does not make a file");

        let edit = Edit::AddNode { id: "000000000000000a".to_owned(), folder: "nodes/number".to_owned(), builtin: false, pos: [1.0, 2.0] };
        project.apply(&edit).unwrap();
        assert!(project.dirty());
        project.save().unwrap();
        assert!(!project.dirty());
        let reopened = Project::open(&root).unwrap();
        let checked = reopened.check();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(reopened.graph(), project.graph());
        assert!(checked.is_ok());
    }

    #[test]
    fn the_shared_runtime_is_copied_one_time() {
        let root = std::env::temp_dir().join(format!("patina-std-{}", std::process::id()));
        let (libdir, loaded) = (root.join("lib"), root.join("build").join("loaded"));
        fs::create_dir_all(&libdir).unwrap();
        fs::create_dir_all(&loaded).unwrap();
        fs::write(libdir.join("libstd-1.rlib"), "static").unwrap();
        assert!(copy_runtime(&root, &libdir).is_err(), "a static std is not used");
        let name = format!("std-1.{}", std::env::consts::DLL_EXTENSION);
        fs::write(libdir.join(&name), "shared").unwrap();
        copy_runtime(&root, &libdir).unwrap();
        fs::write(libdir.join(&name), "changed").unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        let gcc = format!("libgcc-1.{}", std::env::consts::DLL_EXTENSION);
        fs::write(root.join("bin").join(&gcc), "gcc").unwrap();
        fs::write(root.join("bin").join("tool.exe"), "tool").unwrap();
        copy_runtime(&root, &libdir).unwrap();
        let copied = fs::read_to_string(loaded.join(&name)).unwrap();
        let more = (loaded.join(&gcc).is_file(), loaded.join("tool.exe").exists());
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(copied, "shared", "a loaded library is not written again");
        assert_eq!(more, (true, false), "only the libraries of bin are copied");
    }
}
