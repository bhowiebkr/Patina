//! The Patina engine: reads a project, edits and saves its graph, builds the graph into a cluster
//! and runs it (spec 3, 7, 8).

mod cargo;
mod checked;
pub mod clock;
pub mod device;
mod edit;
mod events;
mod folders;
mod generate;
pub mod import;
pub mod install;
mod library;
mod load;
mod node;
pub mod package;
mod plan;
mod pool;
mod run;
pub mod project;
mod subgraph;
mod types;
pub mod update;
pub mod view;

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::hash::{BuildHasher, RandomState};
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use edit::Disk;
use folders::{code_file, code_folder, swap_code, write_file};
pub(crate) use folders::list_files;

pub use cargo::{Diagnostic, Mode, Toolchain, add_crate};
pub use checked::Parts;
pub use edit::Edit;
pub use events::{EventRun, Events, Inbox, Remote, remote};
pub use run::{Audio, Frames};
pub use library::{in_unnamed_call, thread_node};
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

/// A project with the open graph and the undo history of the graph and the code.
pub struct Project {
    root: PathBuf,
    /// The stable ID of `patina.toml` (5.3). The text is not checked.
    id: String,
    rust: String,
    /// The name of the open graph: `main` or the name of a subgraph (6.1).
    name: String,
    history: edit::History,
    /// The saved graphs that the user opened before, with their undo history. `main` is here
    /// while a subgraph is open.
    parked: HashMap<String, edit::History>,
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
    /// The name of a public pin of a subgraph. Its key is random (6.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
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
    /// The header color of `#[node(color = "#rrggbb")]` (15.3).
    pub color: Option<String>,
    /// True for a subgraph in `graphs/` (6.1). Its name is the file name.
    pub subgraph: bool,
    /// The kind for the node icon (15.3): `function`, `stateful`, `generic`, `async`, `trigger` or `subgraph`.
    pub kind: &'static str,
}

/// A connection kind (6.3), for the lanes, the bus sockets and the colors of the editor (15.9).
#[derive(Serialize, Debug)]
pub struct KindInfo {
    /// The Rust type that the kind shows.
    pub ty: String,
    /// The lane names of `lanes = ...`, lane 0 first. With none, the lanes come with the live values.
    pub lanes: Vec<String>,
    pub label: Option<String>,
    pub color: Option<String>,
}

impl NodeInfo {
    fn new(name: String, node: node::Node, builtin: bool) -> NodeInfo {
        let keys = node.output_keys().into_iter();
        let outputs = keys.zip(node.output_types).map(|(key, ty)| Pin { key, ty, name: None }).collect();
        let inputs = node.inputs.into_iter().map(|i| Pin { key: i.key, ty: i.ty, name: None }).collect();
        let kind = if node.is_async {
            "async"
        } else if node.state.is_some() {
            "stateful"
        } else if !node.generics.is_empty() {
            "generic"
        } else {
            "function"
        };
        NodeInfo { name, inputs, outputs, error: None, builtin, generics: node.generics, full: node.full, color: node.color, subgraph: false, kind }
    }
}

/// The built-in nodes, for the node list of the editor (9.2).
pub fn builtins() -> Vec<NodeInfo> {
    // The delay keeps the last value, so it shows as stateful.
    let info = |b: node::Builtin| NodeInfo { kind: if b.fires() { "trigger" } else { "stateful" }, ..NodeInfo::new(b.name().to_owned(), b.node(), true) };
    node::Builtin::ALL.into_iter().map(info).collect()
}

/// A graph that passed all checks, ready to build.
pub struct Checked {
    root: PathBuf,
    rust: String,
    plan: plan::Plan,
    graph: project::GraphFile,
    aliases: Vec<(String, String)>,
    /// A use of the open subgraph alone (15.9): its public outputs send their values.
    test: bool,
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
        let graph = project::read(&graph_path(&root, MAIN), |g: &GraphFile| g.format)?;
        let history = edit::History::new(graph);
        Ok(Project { root, id: file.id, rust: file.rust, name: MAIN.to_owned(), history, parked: HashMap::new(), code_revision: 0, last_code: None })
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
    pub fn new_node(&mut self, name: &str) -> Result<(), String> {
        self.new_folder("nodes", name)
    }

    /// Makes the widget folder `widgets/<name>` with a start widget (4.3, 6.2).
    pub fn new_widget(&mut self, name: &str) -> Result<(), String> {
        self.new_folder("widgets", name)
    }

    /// Makes the view folder `views/<name>` with a start view (15.8).
    pub fn new_view(&mut self, name: &str) -> Result<(), String> {
        self.new_folder("views", name)
    }

    /// Makes the connection folder `connections/<name>` with a start connection kind (6.3).
    pub fn new_connection(&mut self, name: &str) -> Result<(), String> {
        self.new_folder("connections", name)
    }

    /// Makes the type folder `types/<name>` with a start type (5.1).
    pub fn new_type(&mut self, name: &str) -> Result<(), String> {
        self.new_folder("types", name)
    }

    /// Makes a crate folder as one step of undo. Undo removes the folder while it has only its
    /// start files (15.2).
    fn new_folder(&mut self, group: &str, name: &str) -> Result<(), String> {
        folders::new_folder(&self.root, group, name)?;
        self.history.push_disk(Disk::Folder { path: format!("{group}/{name}"), made: true });
        Ok(())
    }

    /// The folder that the next undo removes, for example `nodes/number`.
    pub fn undo_folder(&self) -> Option<&str> {
        self.history.undo_folder()
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

    /// The open graph.
    pub fn graph(&self) -> &GraphFile {
        &self.history.graph
    }

    /// The name of the open graph: `main` or the name of a subgraph.
    pub fn graph_name(&self) -> &str {
        &self.name
    }

    /// Opens the main graph or a subgraph for edits (6.1). Only the open graph can have unsaved
    /// changes, so the user saves before another graph opens.
    pub fn open_graph(&mut self, name: &str) -> Result<(), String> {
        if name == self.name {
            return Ok(());
        }
        if self.history.dirty() {
            return Err("save the graph before you open another graph".to_owned());
        }
        if name != MAIN {
            plan::check_name(name).map_err(|e| format!("the subgraph name {name:?}: {e}"))?;
        }
        let history = match self.parked.remove(name) {
            Some(history) => history,
            None => edit::History::new(project::read(&graph_path(&self.root, name), |g: &GraphFile| g.format).map_err(|e| e.to_string())?),
        };
        let old = std::mem::replace(&mut self.history, history);
        self.parked.insert(std::mem::replace(&mut self.name, name.to_owned()), old);
        Ok(())
    }

    /// The file of the subgraph `name`, with its unsaved edits, to show its nodes in place in the
    /// graph that uses it (15.3).
    pub fn subgraph_file(&self, name: &str) -> Result<GraphFile, String> {
        if name == MAIN {
            return Err("the main graph is not a subgraph".to_owned());
        }
        plan::check_name(name).map_err(|e| format!("the subgraph name {name:?}: {e}"))?;
        if name == self.name {
            return Ok(self.history.graph.clone());
        }
        match self.parked.get(name) {
            Some(history) => Ok(history.graph.clone()),
            None => project::read(&graph_path(&self.root, name), |g: &GraphFile| g.format).map_err(|e| e.to_string()),
        }
    }

    /// Moves the nodes `ids` of the open graph into the new subgraph file `graphs/<name>.toml`, with
    /// the use `use_id` in their place, as one step of undo (6.1). An undo keeps the file.
    pub fn collapse(&mut self, name: &str, ids: &[String], use_id: &str) -> Result<(), String> {
        plan::check_name(name).map_err(|e| format!("the subgraph name {name:?}: {e}"))?;
        let path = graph_path(&self.root, name);
        if name == MAIN || path.exists() {
            return Err(format!("the graph {name} exists"));
        }
        let state = RandomState::new();
        let mut count = 0u32;
        let key = || {
            count += 1;
            format!("{:08x}", state.hash_one(count) as u32)
        };
        let (outer, inner) = subgraph::collapse(&self.history.graph, ids, use_id, name, key)?;
        project::write(&path, &inner).map_err(|e| e.to_string())?;
        self.history.replace(outer);
        Ok(())
    }

    /// Puts the nodes of each subgraph use `ids` of the open graph in place of the use, as one step
    /// of undo (6.1). The subgraph files stay.
    pub fn expand(&mut self, ids: &[String]) -> Result<(), String> {
        let state = RandomState::new();
        let mut count = 0u32;
        let mut new_id = || {
            count += 1;
            format!("{:016x}", state.hash_one(count))
        };
        let mut graph = self.history.graph.clone();
        for id in ids {
            let name = graph.node.iter().find(|n| &n.id == id).and_then(|n| n.subgraph.clone()).ok_or(format!("the node {id} is not a use of a subgraph"))?;
            let inner = project::read(&graph_path(&self.root, &name), |g: &GraphFile| g.format).map_err(|e| e.to_string())?;
            graph = subgraph::expand_use(&graph, id, &inner, &mut new_id)?;
        }
        self.history.replace(graph);
        Ok(())
    }

    /// Applies an edit of the user (7.4). A code edit writes its file at once, because cargo
    /// builds from the files.
    pub fn apply(&mut self, edit: &Edit) -> Result<(), String> {
        let Edit::SetCode { path, text } = edit else { return self.history.apply(edit) };
        let old = swap_code(&self.root, path, text)?;
        if old == *text {
            return Err("the edit changes nothing".to_owned());
        }
        self.history.push_disk(Disk::Code { path: path.clone(), text: old });
        self.code_revision += 1;
        self.last_code = Some(path.clone());
        Ok(())
    }

    /// Replaces the open graph as one step of undo, for example with the graph of a recovery file
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
        let write = |disk: &Disk| {
            // A new or removed folder changes the crates, as a change of its Cargo.toml does.
            wrote = Some(match disk {
                Disk::Code { path, .. } => path.clone(),
                Disk::Folder { path, .. } => format!("{path}/Cargo.toml"),
            });
            folders::write_disk(root, disk)
        };
        let done = if undo { self.history.undo(write) } else { self.history.redo(write) };
        if let (Some(path), true) = (wrote, done.is_ok()) {
            self.code_revision += 1;
            self.last_code = Some(path);
        }
        done
    }

    /// Changes when an edit, undo or redo writes a code file, or when undo or redo removes or makes
    /// a folder. Each step writes one file or one folder.
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

    /// Writes the open graph to its file in `graphs/`. The undo history stays (15.10).
    pub fn save(&mut self) -> Result<(), Error> {
        project::write(&graph_path(&self.root, &self.name), &self.history.graph)?;
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
                Err(text) => NodeInfo { name, inputs: Vec::new(), outputs: Vec::new(), error: Some(text), builtin: false, generics: Vec::new(), full: false, color: None, subgraph: false, kind: "function" },
            });
        }
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(nodes)
    }

    /// The connection kinds in `connections/` that read. A kind with a problem stops the build, so
    /// the build shows its problem.
    pub fn kinds(&self) -> Vec<KindInfo> {
        let kinds = plan::read_kinds(&self.root, &mut Vec::new());
        kinds.into_iter().map(|(_, k)| KindInfo { ty: k.ty, lanes: k.lanes, label: k.label, color: k.color }).collect()
    }

    /// The subgraphs in `graphs/` (6.1), sorted by name, with their public pins as the pins. The
    /// open subgraph gives the pins of its edits.
    pub fn subgraphs(&self) -> Vec<NodeInfo> {
        subgraph::infos(&self.root, Some((&self.name, &self.history.graph)))
    }

    /// The widget folders in `widgets/` (6.2), sorted by name.
    pub fn widgets(&self) -> Vec<String> {
        self.crate_folders("widgets")
    }

    /// The view folders in `views/` (15.8), sorted by name.
    pub fn views(&self) -> Vec<String> {
        self.crate_folders("views")
    }

    /// The connection folders in `connections/` (6.3), sorted by name.
    pub fn connections(&self) -> Vec<String> {
        self.crate_folders("connections")
    }

    /// The type folders in `types/` (5.1), sorted by name.
    pub fn types(&self) -> Vec<String> {
        self.crate_folders("types")
    }

    /// Builds the view folder `views/<name>` in edit mode and gives its library for `View::load`
    /// (15.8). The library is a copy in `build/loaded/`, as the clusters are (8.5).
    pub fn build_view(&self, toolchain: &Toolchain, name: &str) -> Result<PathBuf, Error> {
        let library = cargo::build_view(&self.root, toolchain, name, Mode::Edit)?;
        // A dot is not in a name, so the prefix of one view is not the prefix of another view.
        let copy = checked::copy_to_loaded(&self.root, &format!("view.{name}."), &[library])?;
        checked::copy_runtime(&self.root.join("build").join("loaded"), &toolchain.libdir)?;
        Ok(copy.into_iter().next().unwrap_or_default())
    }

    fn crate_folders(&self, group: &str) -> Vec<String> {
        let entries = fs::read_dir(self.root.join(group)).into_iter().flatten().flatten();
        let dirs = entries.filter(|e| e.path().is_dir()).filter_map(|e| e.file_name().into_string().ok());
        let mut names: Vec<String> = dirs.filter(|name| plan::check_name(name).is_ok()).collect();
        names.sort();
        names
    }

    /// The files of a code folder, for example `nodes/number` (15.4), as sorted paths
    /// from the project root. Hidden files and `target` folders are not in the list.
    pub fn folder_files(&self, folder: &str) -> Result<Vec<String>, Error> {
        let dir = self.root.join(folder);
        code_folder(folder).map_err(|text| Error::File(dir.clone(), text))?;
        let mut files = Vec::new();
        list_files(&dir, folder, &mut files)?;
        files.sort();
        Ok(files)
    }

    /// The files of the project with one of `extensions`, for an asset picker (4.3), sorted and
    /// relative to the root with `/`. The walk skips `build/`, the `target` folders and hidden files.
    pub fn asset_files(&self, extensions: &[String]) -> Result<Vec<String>, Error> {
        const MAX_ASSETS: usize = 1000;
        let mut files = Vec::new();
        list_files(&self.root, ".", &mut files)?;
        let ext = |f: &Path| f.extension().is_some_and(|e| extensions.iter().any(|x| e.eq_ignore_ascii_case(x)));
        files.retain(|f| !f.starts_with("./build/") && ext(Path::new(f)));
        files.iter_mut().for_each(|f| drop(f.drain(.."./".len())));
        files.sort();
        files.truncate(MAX_ASSETS);
        Ok(files)
    }

    /// The text of a file in a node or widget folder, for example `nodes/number/src/lib.rs`.
    pub fn read_code(&self, path: &str) -> Result<String, String> {
        let file = code_file(&self.root, path)?;
        fs::read_to_string(&file).map_err(|e| format!("{path}: {e}"))
    }

    /// Checks the main graph and puts its nodes in run order. An open subgraph adds its unsaved
    /// changes.
    pub fn check(&self) -> Result<Checked, Error> {
        let fail = |problems| Error::Graph(graph_path(&self.root, MAIN), problems);
        let main = self.parked.get(MAIN).map_or(&self.history.graph, |h| &h.graph);
        let open = (self.name != MAIN).then_some((self.name.as_str(), &self.history.graph));
        let (graph, aliases) = subgraph::flatten(&self.root, main, open).map_err(fail)?;
        let plan = plan::check(&self.root, &graph).map_err(fail)?;
        Ok(Checked { root: self.root.clone(), rust: self.rust.clone(), plan, graph, aliases, test: false })
    }

    /// Checks one use of the open subgraph alone, with `params` for its public inputs (15.9). The
    /// use has the ID [`TEST`]. With the main graph open, it checks the main graph.
    pub fn check_test(&self, params: &BTreeMap<String, String>) -> Result<Checked, Error> {
        if self.name == MAIN {
            return self.check();
        }
        let fail = |problems| Error::Graph(graph_path(&self.root, &self.name), problems);
        let test = project::NodeEntry { id: TEST.to_owned(), folder: None, subgraph: Some(self.name.clone()), builtin: None, pos: None, domain: None, settings: None, params: params.clone() };
        let root = GraphFile { format: self.history.graph.format, node: vec![test], wire: Vec::new(), order: Vec::new(), collar: Vec::new(), cable: Vec::new(), input: Vec::new(), output: Vec::new() };
        let (graph, aliases) = subgraph::flatten(&self.root, &root, Some((&self.name, &self.history.graph))).map_err(fail)?;
        let mut plan = plan::check(&self.root, &graph).map_err(fail)?;
        plan.probe_all(&aliases);
        Ok(Checked { root: self.root.clone(), rust: self.rust.clone(), plan, graph, aliases, test: true })
    }
}

/// The name of the main graph file.
const MAIN: &str = "main";

/// The ID of the use that runs the open subgraph alone (15.9).
pub const TEST: &str = "7e57000000000000";

fn graph_path(root: &Path, name: &str) -> PathBuf {
    root.join("graphs").join(format!("{name}.toml"))
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
        let pin = |key: &str, ty: &str| Pin { key: key.to_owned(), ty: ty.to_owned(), name: None };
        assert_eq!((&nodes[0].inputs[..], &nodes[0].outputs[..]), (&[pin("value", "f64")][..], &[pin("out", "f64")][..]));
        assert_eq!(nodes[1].inputs, [pin("value", "&f64")]);
        assert_eq!(project.folder_files("nodes/show").unwrap(), ["nodes/show/Cargo.toml", "nodes/show/src/lib.rs"]);
        fs::create_dir_all(root.join("build")).unwrap();
        fs::write(root.join("build/x.toml"), "").unwrap();
        assert_eq!(project.asset_files(&["TOML".to_owned()]).unwrap(), ["graphs/main.toml", "nodes/number/Cargo.toml", "nodes/show/Cargo.toml", "patina.toml"]);
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
        project.new_type("seg-digit").unwrap();
        project.new_connection("word-bus").unwrap();
        assert_eq!((project.types(), project.connections()), (vec!["seg-digit".to_owned()], vec!["word-bus".to_owned()]));
        assert!(project.read_code("types/seg-digit/src/lib.rs").unwrap().contains("pub struct SegDigit("));
        assert!(!project.read_code("types/seg-digit/Cargo.toml").unwrap().contains("patina-sdk"));
        let kind = &project.kinds()[0];
        assert_eq!((kind.ty.as_str(), kind.lanes.len(), kind.label.as_deref()), ("u16", 16, Some("word_bus")));

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
        assert_eq!(reopened.graph(), project.graph());
        // The editor shows the kind and the clock domain of each node (15.3).
        let checked = checked.unwrap();
        assert_eq!(checked.domains().collect::<Vec<_>>(), [("000000000000000a", "batch")]);
        assert_eq!((nodes[0].kind, made.kind), ("function", "function"));
        let kinds: Vec<_> = builtins().into_iter().map(|b| (b.name, b.kind)).collect();
        assert_eq!(kinds[0], ("delay".to_owned(), "stateful"));
        assert!(kinds[1..].iter().all(|k| k.1 == "trigger"), "{kinds:?}");
        let kind = |state: bool, is_async: bool| {
            let node = node::Node { state: state.then(|| "State".to_owned()), is_async, ..node::Builtin::Delay.node() };
            NodeInfo::new(String::new(), node, false).kind
        };
        assert_eq!([kind(false, false), kind(true, false), kind(true, true)], ["generic", "stateful", "async"]);

        // A subgraph opens, runs with its unsaved changes and saves to its own file (6.1).
        fs::write(root.join("graphs/half.toml"), "format = 1\n").unwrap();
        assert_eq!(project.subgraphs()[0].kind, "subgraph");
        project.apply(&Edit::AddNode { id: "00000000000000b1".to_owned(), folder: "graphs/half".to_owned(), builtin: false, pos: [0.0, 0.0] }).unwrap();
        assert!(project.open_graph("half").is_err(), "the main graph has unsaved changes");
        project.save().unwrap();
        for bad in ["missing", "../patina", "half.toml", ""] {
            assert!(project.open_graph(bad).is_err(), "{bad}");
        }
        project.open_graph("half").unwrap();
        assert_eq!((project.graph_name(), project.graph().node.len()), ("half", 0));
        project.apply(&edit).unwrap();
        assert!(project.check().unwrap().graph().node.iter().any(|n| n.id == "00000000000000b1/000000000000000a"));
        project.save().unwrap();
        project.open_graph("main").unwrap();
        assert_eq!((project.graph_name(), project.graph().node.len()), ("main", 2));
        // The graph view shows the nodes of a subgraph use in place (15.3).
        assert_eq!(project.subgraph_file("half").unwrap().node.len(), 1);
        for bad in ["main", "missing", "../patina"] {
            assert!(project.subgraph_file(bad).is_err(), "{bad}");
        }
        assert!(project.can_undo(), "the undo history of the main graph stays");
        let half = fs::read_to_string(root.join("graphs/half.toml")).unwrap();
        assert!(half.contains("000000000000000a"), "{half}");

        // A collapse writes the new subgraph file. An undo keeps the file.
        let ids = ["000000000000000a".to_owned()];
        assert!(project.collapse("half", &ids, "00000000000000c1").is_err(), "the graph exists");
        project.collapse("part", &ids, "00000000000000c1").unwrap();
        assert!(project.check().unwrap().graph().node.iter().any(|n| n.id == "00000000000000c1/000000000000000a"));
        project.expand(&["00000000000000c1".to_owned()]).unwrap();
        assert_eq!(project.graph().node[1].id, "000000000000000a", "the expand gives the node again");
        assert!(project.expand(&ids).is_err(), "the node is not a use");
        project.undo().unwrap();
        project.undo().unwrap();
        let kept = root.join("graphs/part.toml").is_file();
        let count = project.graph().node.len();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!((kept, count), (true, 2));
    }

    #[test]
    fn a_test_runs_one_use_of_the_open_subgraph_with_the_input_params() {
        let root = std::env::temp_dir().join(format!("patina-test-use-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut project = Project::create(&root, "1.94.0").unwrap();
        let half = "format = 1
[[node]]
id = \"00000000000000a1\"
folder = \"nodes/number\"
[[input]]
key = \"0000000a\"
name = \"x\"
pos = [0.0, 0.0]
to = [\"00000000000000a1.value\"]
[[output]]
key = \"0000000b\"
name = \"y\"
pos = [0.0, 0.0]
from = \"00000000000000a1.out\"
";
        fs::write(root.join("graphs/half.toml"), half).unwrap();
        let params = BTreeMap::from([("0000000a".to_owned(), "7".to_owned())]);
        assert!(project.check_test(&params).unwrap().graph.node.is_empty(), "the main graph has no use");
        project.open_graph("half").unwrap();
        let checked = project.check_test(&params).unwrap();
        assert_eq!((checked.graph.node[0].id.as_str(), checked.graph.node[0].params["value"].as_str()), ("7e57000000000000/00000000000000a1", "7"));
        assert_eq!(checked.aliases, [("7e57000000000000.0000000b".to_owned(), "7e57000000000000/00000000000000a1.out".to_owned())]);
        assert_eq!(checked.plan.steps[0].consumers, [1], "the public output sends its value");
        let json = serde_json::to_string(&checked.into_parts()).unwrap();
        let again = serde_json::from_str::<Parts>(&json).unwrap().check().unwrap();
        assert_eq!((again.plan.steps[0].consumers.as_slice(), again.aliases.len()), (&[1][..], 1), "the node process checks the same plan");
        fs::remove_dir_all(&root).unwrap();
    }
}
