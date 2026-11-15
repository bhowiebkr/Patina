//! The Patina engine: reads a project, edits and saves its graph, builds the graph into a cluster
//! and runs it (spec 3, 7, 8).

mod cargo;
mod edit;
mod generate;
mod load;
mod node;
mod plan;
pub mod project;

use std::fmt;
use std::fs;
use std::hash::{BuildHasher, DefaultHasher, Hasher, RandomState};
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub use cargo::{Diagnostic, Toolchain};
pub use edit::Edit;
pub use load::{Cluster, Outcome, Report, ReportKind, Value};
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
}

/// An input or an output of a node (spec 4.1).
#[derive(Serialize, Debug, PartialEq)]
pub struct Pin {
    pub key: String,
    /// The Rust type as the code writes it. An output has the type of its value.
    pub ty: String,
}

/// A node folder of the project, for the node list of the editor (15.2).
#[derive(Serialize, Debug)]
pub struct NodeInfo {
    /// The folder name in `nodes/`.
    pub name: String,
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
    /// The problem of the folder when its node does not read.
    pub error: Option<String>,
}

/// A graph that passed all checks, ready to build.
pub struct Checked {
    root: PathBuf,
    rust: String,
    plan: plan::Plan,
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
        Ok(Project { root, id: file.id, rust: file.rust, history: edit::History::new(graph), code_revision: 0 })
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
            let path = root.join(relative);
            let parent = path.parent().expect("a project file is in a folder");
            fs::create_dir_all(parent).map_err(|e| Error::Io(parent.to_owned(), e))?;
            fs::write(&path, text).map_err(|e| Error::Io(path, e))?;
        }
        Project::open(&root)
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
        let mut wrote = false;
        let write = |path: &str, text: &str| {
            wrote = true;
            swap_code(root, path, text)
        };
        let done = if undo { self.history.undo(write) } else { self.history.redo(write) };
        self.code_revision += u64::from(wrote && done.is_ok());
        done
    }

    /// Changes when an edit, undo or redo writes a code file.
    pub fn code_revision(&self) -> u64 {
        self.code_revision
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
                Ok(node) => {
                    let keys = node.output_keys().into_iter();
                    let outputs = keys.zip(node.output_types).map(|(key, ty)| Pin { key, ty }).collect();
                    let inputs = node.inputs.into_iter().map(|i| Pin { key: i.key, ty: i.ty }).collect();
                    NodeInfo { name, inputs, outputs, error: None }
                }
                Err(text) => NodeInfo { name, inputs: Vec::new(), outputs: Vec::new(), error: Some(text) },
            });
        }
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(nodes)
    }

    /// The files of the node folder `nodes/<name>` (15.4), as sorted paths from the project root.
    /// Hidden files and `target` folders are not in the list.
    pub fn node_files(&self, name: &str) -> Result<Vec<String>, Error> {
        let dir = self.root.join("nodes").join(name);
        plan::check_name(name).map_err(|text| Error::File(dir.clone(), text))?;
        let mut files = Vec::new();
        list_files(&dir, &format!("nodes/{name}"), &mut files)?;
        files.sort();
        Ok(files)
    }

    /// The text of a file in a node folder, for example `nodes/number/src/lib.rs`.
    pub fn read_code(&self, path: &str) -> Result<String, String> {
        let file = code_file(&self.root, path)?;
        fs::read_to_string(&file).map_err(|e| format!("{path}: {e}"))
    }

    /// Checks the main graph and puts its nodes in run order.
    pub fn check(&self) -> Result<Checked, Error> {
        let plan = plan::check(&self.root, &self.history.graph)
            .map_err(|problems| Error::Graph(graph_path(&self.root), problems))?;
        Ok(Checked { root: self.root.clone(), rust: self.rust.clone(), plan })
    }
}

impl Checked {
    /// Builds the cluster of the graph and loads it.
    pub fn build(&self, toolchain: &Toolchain) -> Result<Cluster, Error> {
        if toolchain.release != self.rust {
            return Err(Error::Build(format!(
                "the project needs Rust {}, but the toolchain is Rust {}",
                self.rust, toolchain.release
            )));
        }
        let built = cargo::build(&self.root, toolchain, &self.plan)?;
        let library = copy_to_loaded(&self.root, &built)?;
        let plan = &self.plan;
        let names = plan.steps.iter().map(|s| (s.id.clone(), plan.nodes[s.node].0.clone())).collect();
        let slots = plan.slots.iter().map(|s| (plan.steps[s.step].id.clone(), s.key.clone())).collect();
        let probes = plan
            .held()
            .map(|(n, k)| format!("{}.{}", plan.steps[n].id, plan.nodes[plan.steps[n].node].1.output_keys()[k]))
            .collect();
        Cluster::load(&library, names, slots, probes)
    }

    /// The generated code of the cluster. Two checks with the same wiring give the same text, so
    /// a cluster of one stays correct for the other.
    pub fn wiring(&self) -> String {
        generate::manifest(&self.plan) + &generate::source(&self.plan)
    }
}

fn graph_path(root: &Path) -> PathBuf {
    root.join("graphs").join("main.toml")
}

/// The full path of an existing file in a node folder, from a path like `nodes/number/src/lib.rs`
/// (6.2). The page sends the path, so it must stay in the node folder.
fn code_file(root: &Path, path: &str) -> Result<PathBuf, String> {
    let bad = || format!("{path:?} is not a file in a node folder");
    let mut parts = path.split('/');
    if parts.next() != Some("nodes") {
        return Err(bad());
    }
    let name = parts.next().ok_or_else(bad)?;
    plan::check_name(name)?;
    let mut file = root.join("nodes").join(name);
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

fn list_files(dir: &Path, relative: &str, files: &mut Vec<String>) -> Result<(), Error> {
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

/// Copies the built library to `build/loaded/`, with a hash of its bytes in the file name (8.5).
/// Windows locks a loaded library, so cargo cannot write the library of the next build over it.
/// The same bytes give the same name, so the loaded library stays in use.
fn copy_to_loaded(root: &Path, built: &Path) -> Result<PathBuf, Error> {
    let bytes = fs::read(built).map_err(|e| Error::Io(built.to_owned(), e))?;
    let mut hasher = DefaultHasher::new();
    hasher.write(&bytes);
    let dir = root.join("build").join("loaded");
    let path = dir.join(format!("cluster-{:016x}.{}", hasher.finish(), std::env::consts::DLL_EXTENSION));
    if path.exists() {
        return Ok(path);
    }
    // An old copy that a process has loaded does not delete on Windows. It stays until later.
    for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
        let _ = fs::remove_file(entry.path());
    }
    fs::create_dir_all(&dir).map_err(|e| Error::Io(dir.clone(), e))?;
    let temp = path.with_extension("tmp");
    fs::write(&temp, &bytes).map_err(|e| Error::Io(temp.clone(), e))?;
    fs::rename(&temp, &path).map_err(|e| Error::Io(path.clone(), e))?;
    Ok(path)
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
        assert_eq!(project.node_files("show").unwrap(), ["nodes/show/Cargo.toml", "nodes/show/src/lib.rs"]);
        assert!(project.read_code("nodes/show/src/lib.rs").unwrap().contains("println!"));
        for bad in ["patina.toml", "nodes/show", "nodes/show/../../patina.toml", "nodes/show/src\\..\\x", "nodes/show/c:x"] {
            assert!(project.read_code(bad).is_err(), "{bad}");
        }
        assert!(project.node_files("..").is_err());

        let path = "nodes/show/src/lib.rs".to_owned();
        let code = Edit::SetCode { path: path.clone(), text: "// new\n".to_owned() };
        project.apply(&code).unwrap();
        assert!(project.apply(&code).is_err(), "an edit that changes nothing is not kept");
        assert_eq!((project.read_code(&path).unwrap().as_str(), project.code_revision()), ("// new\n", 1));
        assert!(project.undo().unwrap());
        assert!(project.read_code(&path).unwrap().contains("println!"), "undo writes the old text");
        assert!(project.redo().unwrap());
        assert_eq!((project.read_code(&path).unwrap().as_str(), project.code_revision()), ("// new\n", 3));
        let missing = Edit::SetCode { path: "nodes/show/src/new.rs".to_owned(), text: String::new() };
        assert!(project.apply(&missing).is_err(), "a code edit does not make a file");

        let edit = Edit::AddNode { id: "000000000000000a".to_owned(), folder: "nodes/number".to_owned(), pos: [1.0, 2.0] };
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
}
