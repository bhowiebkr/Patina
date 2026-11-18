//! The Patina engine: reads a project, edits and saves its graph, builds the graph into a cluster
//! and runs it (spec 3, 7, 8).

mod cargo;
mod checked;
mod clip;
pub mod clock;
pub mod device;
mod edit;
mod events;
mod folders;
mod generate;
pub mod git;
mod graphs;
pub mod import;
pub mod install;
mod library;
mod load;
mod node;
mod outcome;
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
use folders::{code_folder, write_file};
pub(crate) use folders::list_files;

pub use cargo::{Diagnostic, Mode, Toolchain, add_crate, write_workspace};
pub use checked::{Compiled, Parts};
pub use clip::{Link, Need, NeedState, Pasted};
pub use edit::Edit;
pub use folders::Stamps;
pub use events::{EventRun, Events, Inbox, Remote, remote};
pub use run::{Audio, Frames};
pub use library::{in_unnamed_call, thread_node};
pub use load::Cluster;
pub use outcome::{Lanes, NodeLog, NodeTime, Outcome, Report, ReportKind, Value};
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
    /// The frames each second of the frame domain (6.4, 9.1).
    frame_rate: u32,
    /// The frame rate in `patina.toml`. Save writes `frame_rate` when the two are not the same.
    saved_rate: u32,
    /// The name of the open graph: a top graph or a subgraph (6.1).
    name: String,
    /// The top graph that runs: `main` or a graph with `top = true` that the user opened (6.1).
    top: String,
    history: edit::History,
    /// The graphs that the user opened before, with their undo history. The top graph is here
    /// while a subgraph is open.
    parked: HashMap<String, edit::History>,
    /// The number of code writes of edits, undo and redo.
    code_revision: u64,
    /// The path of the last code write.
    last_code: Option<String>,
}

/// The text that an undo or a redo of a code change gives to the **Code** tab. The file changes
/// only when the user saves (15.10).
#[derive(Serialize, Debug, PartialEq)]
pub struct CodeText {
    pub path: String,
    pub text: String,
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
    /// True when the node GUI is the whole node: no header, its own shape and pin places (4.3).
    pub free: bool,
    /// The header color of `#[node(color = "#rrggbb")]` (15.3).
    pub color: Option<String>,
    /// The circuit symbol in place of the box (4.3).
    pub symbol: Option<project::Symbol>,
    /// True for a subgraph in `graphs/` (6.1). Its name is the file name.
    pub subgraph: bool,
    /// The kind for the node icon (15.3): `function`, `stateful`, `generic`, `async`, `trigger` or `subgraph`.
    pub kind: &'static str,
    /// The doc comment of the node code (15.3).
    pub doc: Option<String>,
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
        NodeInfo { name, inputs, outputs, error: None, builtin, generics: node.generics, full: node.full, free: node.free, color: node.color, symbol: node.symbol.map(|shape| project::Symbol { shape, label: None }), subgraph: false, kind, doc: node.doc }
    }
}

/// The built-in nodes, for the node list of the editor (9.2).
pub fn builtins() -> Vec<NodeInfo> {
    // The delay keeps the last value, so it shows as stateful.
    let info = |b: node::Builtin| NodeInfo { kind: if b.fires() { "trigger" } else { "stateful" }, ..NodeInfo::new(b.name().to_owned(), b.node(), true) };
    node::Builtin::ALL.into_iter().map(info).collect()
}

/// A graph that passed all checks, ready to build.
#[derive(Clone)]
pub struct Checked {
    root: PathBuf,
    /// The graph that the check started from: the top graph, or the open subgraph of a test.
    name: String,
    rust: String,
    frame_rate: u32,
    plan: plan::Plan,
    graph: project::GraphFile,
    aliases: Vec<(String, String)>,
    /// A use of the open subgraph alone (15.9): its public outputs send their values.
    test: bool,
}

/// The node folders of the hello example, for the tests.
#[cfg(test)]
const HELLO_NODES: [(&str, &str); 6] = [
    ("nodes/number/Cargo.toml", include_str!("../../../examples/hello/nodes/number/Cargo.toml")),
    ("nodes/number/src/lib.rs", include_str!("../../../examples/hello/nodes/number/src/lib.rs")),
    ("nodes/show/Cargo.toml", include_str!("../../../examples/hello/nodes/show/Cargo.toml")),
    ("nodes/show/src/lib.rs", include_str!("../../../examples/hello/nodes/show/src/lib.rs")),
    ("widgets/calc/Cargo.toml", include_str!("../../../examples/hello/widgets/calc/Cargo.toml")),
    ("widgets/calc/src/lib.rs", include_str!("../../../examples/hello/widgets/calc/src/lib.rs")),
];

impl Project {
    /// Reads `patina.toml` and `graphs/main.toml` of the project folder. A graph with problems
    /// opens, so that the user can repair it. `check` finds the problems.
    pub fn open(path: &Path) -> Result<Project, Error> {
        // The path must be absolute, because cargo reads the target folder from a variable.
        let root = std::path::absolute(path).map_err(|e| Error::Io(path.to_owned(), e))?;
        let file = project::read(&root.join("patina.toml"), |f: &project::ProjectFile| f.format)?;
        let frame_rate = check_rate(file.frame_rate.unwrap_or(clock::RATE)).map_err(|e| Error::File(root.join("patina.toml"), e))?;
        let graph = project::read(&graph_path(&root, MAIN), |g: &GraphFile| g.format)?;
        let history = edit::History::new(graph);
        Ok(Project { root, id: file.id, rust: file.rust, frame_rate, saved_rate: frame_rate, name: MAIN.to_owned(), top: MAIN.to_owned(), history, parked: HashMap::new(), code_revision: 0, last_code: None })
    }

    /// Opens the project at `path` with the top graph `name` as the graph that runs (6.1).
    pub fn open_top(path: &Path, name: &str) -> Result<Project, Error> {
        let mut project = Project::open(path)?;
        let file = graph_path(&project.root, name);
        if !file.is_file() {
            return Err(Error::File(file, format!("the project has no graph {name}")));
        }
        project.open_graph(name).map_err(|e| Error::File(file.clone(), e))?;
        if project.top != name {
            return Err(Error::File(file, format!("the graph {name} is a subgraph, not a top graph")));
        }
        Ok(project)
    }

    /// Makes a new project in a new or empty folder, with an empty main graph and no nodes (15.10).
    pub fn create(path: &Path, rust: &str) -> Result<Project, Error> {
        let root = std::path::absolute(path).map_err(|e| Error::Io(path.to_owned(), e))?;
        if fs::read_dir(&root).is_ok_and(|mut entries| entries.next().is_some()) {
            return Err(Error::File(root, "the folder is not empty".to_owned()));
        }
        let id = RandomState::new().hash_one(std::time::SystemTime::now());
        let project = format!("format = 1\nid = \"{id:016x}\"\nrust = \"{rust}\"\n");
        for (relative, text) in [("patina.toml", project.as_str()), ("graphs/main.toml", "format = 1\n")] {
            write_file(&root.join(relative), text)?;
        }
        Project::open(&root)
    }

    /// Makes a new project with the `number` and `show` nodes of the hello example, for the tests.
    #[cfg(test)]
    pub(crate) fn hello(path: &Path) -> Project {
        let root = Project::create(path, "1.94.0").unwrap().root;
        for (relative, text) in HELLO_NODES {
            write_file(&root.join(relative), text).unwrap();
        }
        Project::open(&root).unwrap()
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

    /// The frames each second of the frame domain (6.4, 9.1).
    pub fn frame_rate(&self) -> u32 {
        self.frame_rate
    }

    /// Sets a new frame rate in memory. The next run uses it, and Save writes it into
    /// `patina.toml` (15.10).
    pub fn set_frame_rate(&mut self, rate: u32) -> Result<(), Error> {
        self.frame_rate = check_rate(rate).map_err(|e| Error::File(self.root.join("patina.toml"), e))?;
        Ok(())
    }

    /// The frame rate when it is not the rate in `patina.toml`.
    pub fn unsaved_rate(&self) -> Option<u32> {
        (self.frame_rate != self.saved_rate).then_some(self.frame_rate)
    }

    /// Writes an unsaved frame rate into `patina.toml`.
    fn save_rate(&mut self) -> Result<(), Error> {
        let Some(rate) = self.unsaved_rate() else { return Ok(()) };
        let path = self.root.join("patina.toml");
        let mut file = project::read(&path, |f: &project::ProjectFile| f.format)?;
        file.frame_rate = (rate != clock::RATE).then_some(rate);
        project::write(&path, &file)?;
        self.saved_rate = rate;
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
            nodes.push(match project::read_node(&self.root, &name) {
                Ok(node) => NodeInfo::new(name, node, false),
                Err(text) => NodeInfo { name, inputs: Vec::new(), outputs: Vec::new(), error: Some(text), builtin: false, generics: Vec::new(), full: false, free: false, color: None, symbol: None, subgraph: false, kind: "function", doc: None },
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

    /// The subgraphs in `graphs/` (6.1), sorted by name, with their public pins as the pins. A
    /// subgraph in memory gives the pins of its edits.
    pub fn subgraphs(&self) -> Vec<NodeInfo> {
        self.graphs().1
    }

    /// The top graphs (6.1), `main` first and then sorted, and the subgraphs as `subgraphs` gives them.
    pub fn graphs(&self) -> (Vec<String>, Vec<NodeInfo>) {
        subgraph::infos(&self.root, &self.edits())
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

    /// Builds the view folder `views/<name>` in `mode` and gives its library for `View::load`
    /// (15.8). The library is a copy in `build/loaded/`, as the clusters are (8.5).
    pub fn build_view(&self, toolchain: &Toolchain, name: &str, mode: Mode) -> Result<PathBuf, Error> {
        let library = cargo::build_view(&self.root, toolchain, name, mode)?;
        // A dot is not in a name, so the prefix of one view is not the prefix of another view.
        let copy = checked::copy_to_loaded(&self.root, &format!("view.{name}."), &[library])?;
        checked::copy_runtime(&self.root.join("build").join("loaded"), &toolchain.libdir)?;
        Ok(copy.into_iter().next().unwrap_or_default())
    }

    fn crate_folders(&self, group: &str) -> Vec<String> {
        let entries = fs::read_dir(self.root.join(group)).into_iter().flatten().flatten();
        let dirs = entries.filter(|e| e.path().is_dir()).filter_map(|e| e.file_name().into_string().ok());
        let mut names: Vec<String> = dirs.filter(|name| project::check_name(name).is_ok()).collect();
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
        folders::read_code(&self.root, path)
    }

    /// Checks the main graph and puts its nodes in run order. Each graph adds its unsaved changes.
    pub fn check(&self) -> Result<Checked, Error> {
        let fail = |problems| Error::Graph(graph_path(&self.root, &self.top), problems);
        let top = self.parked.get(&self.top).map_or(&self.history.graph, |h| &h.graph);
        let (graph, aliases) = subgraph::flatten(&self.root, top, &self.edits()).map_err(fail)?;
        let plan = plan::check(&self.root, &graph).map_err(fail)?;
        Ok(Checked { root: self.root.clone(), name: self.top.clone(), rust: self.rust.clone(), frame_rate: self.frame_rate, plan, graph, aliases, test: false })
    }

    /// Checks one use of the open subgraph alone, with `params` for its public inputs (15.9). The
    /// use has the ID [`TEST`]. With the main graph open, it checks the main graph.
    pub fn check_test(&self, params: &BTreeMap<String, String>) -> Result<Checked, Error> {
        if self.name == self.top {
            return self.check();
        }
        let fail = |problems| Error::Graph(graph_path(&self.root, &self.name), problems);
        let test = project::NodeEntry { id: TEST.to_owned(), folder: None, subgraph: Some(self.name.clone()), builtin: None, pos: None, domain: None, settings: None, params: params.clone() };
        let root = GraphFile { format: self.history.graph.format, node: vec![test], ..GraphFile::default() };
        let (graph, aliases) = subgraph::flatten(&self.root, &root, &self.edits()).map_err(fail)?;
        let mut plan = plan::check(&self.root, &graph).map_err(fail)?;
        plan.probe_all(&aliases);
        Ok(Checked { root: self.root.clone(), name: self.name.clone(), rust: self.rust.clone(), frame_rate: self.frame_rate, plan, graph, aliases, test: true })
    }
}

/// The name of the main graph file.
const MAIN: &str = "main";

/// True for the main graph and the other top graphs (6.1).
fn is_top(name: &str, graph: &GraphFile) -> bool {
    name == MAIN || graph.top
}

/// The ID of the use that runs the open subgraph alone (15.9).
pub const TEST: &str = "7e57000000000000";

fn graph_path(root: &Path, name: &str) -> PathBuf {
    root.join("graphs").join(format!("{name}.toml"))
}

/// A frame rate of `patina.toml` must be in `clock::RATES` (6.4).
fn check_rate(rate: u32) -> Result<u32, String> {
    match clock::RATES.contains(&rate) {
        true => Ok(rate),
        false => Err(format!("the frame rate {rate} is not {} to {}", clock::RATES.start(), clock::RATES.end())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn a_new_project_lists_its_nodes_and_saves_edits() {
        let root = std::env::temp_dir().join(format!("patina-new-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let empty = Project::create(&root, "1.94.0").unwrap();
        assert!(empty.nodes().unwrap().is_empty() && empty.check().is_ok(), "a new project has no nodes and checks");
        assert!(Project::create(&root, "1.94.0").is_err(), "a folder with files is not used");
        fs::remove_dir_all(&root).unwrap();
        let mut project = Project::hello(&root);
        assert_eq!(project.frame_rate(), 60);
        assert!(project.set_frame_rate(0).is_err() && project.set_frame_rate(100_001).is_err());
        // A new rate stays in memory until Save writes it (15.10).
        project.set_frame_rate(600).unwrap();
        assert!(project.dirty() && !fs::read_to_string(root.join("patina.toml")).unwrap().contains("frame_rate"));
        assert_eq!((project.unsaved_rate(), project.check().unwrap().frame_rate()), (Some(600), 600));
        project.save().unwrap();
        assert!(!project.dirty() && fs::read_to_string(root.join("patina.toml")).unwrap().contains("frame_rate = 600"));
        assert_eq!(Project::open(&root).unwrap().frame_rate(), 600);
        project.set_frame_rate(60).unwrap();
        project.save().unwrap();
        assert!(!fs::read_to_string(root.join("patina.toml")).unwrap().contains("frame_rate"));
        let nodes = project.nodes().unwrap();
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["number", "show"]);
        let pin = |key: &str, ty: &str| Pin { key: key.to_owned(), ty: ty.to_owned(), name: None };
        assert_eq!((&nodes[0].inputs[..], &nodes[0].outputs[..]), (&[pin("value", "f64")][..], &[pin("out", "f64")][..]));
        assert_eq!(nodes[1].inputs, [pin("value", "&f64")]);
        assert_eq!(project.folder_files("nodes/show").unwrap(), ["nodes/show/Cargo.toml", "nodes/show/src/lib.rs"]);
        fs::create_dir_all(root.join("build")).unwrap();
        fs::write(root.join("build/x.toml"), "").unwrap();
        assert_eq!(project.asset_files(&["TOML".to_owned()]).unwrap(), ["graphs/main.toml", "nodes/number/Cargo.toml", "nodes/show/Cargo.toml", "patina.toml", "widgets/calc/Cargo.toml"]);
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
        assert_eq!(project.widgets(), ["calc", "gradient-bar"]);
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
        // An undo of a code change gives the old text to the Code tab and writes no file (15.10).
        let mut unsaved = BTreeMap::new();
        let old = project.undo(&mut unsaved).unwrap().unwrap();
        assert!(old.path == path && old.text.contains("println!") && unsaved[&path] == old.text);
        assert_eq!((project.read_code(&path).unwrap().as_str(), project.code_revision()), ("// new\n", 1), "undo writes no file");
        unsaved.insert(path.clone(), "// draft\n".to_owned());
        assert_eq!(project.redo(&mut unsaved).unwrap().unwrap().text, "// new\n");
        assert!(unsaved.is_empty(), "the text of the file is not an unsaved change");
        assert_eq!(project.undo(&mut unsaved).unwrap().unwrap().text, "// draft\n", "undo gives back the unsaved text");
        assert_eq!((project.read_code(&path).unwrap().as_str(), project.code_revision()), ("// new\n", 1));
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
        // The number node of hello keeps its value for its GUI, so it is a stateful node.
        assert_eq!((nodes[0].kind, made.kind), ("stateful", "function"));
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
        let main = fs::read_to_string(root.join("graphs/main.toml")).unwrap();
        // A change of graph writes no file. The unsaved changes of each graph stay (15.10).
        project.open_graph("half").unwrap();
        project.open_graph("main").unwrap();
        assert_eq!(fs::read_to_string(root.join("graphs/main.toml")).unwrap(), main);
        assert_eq!(project.unsaved().map(|g| g.0).collect::<Vec<_>>(), ["main"]);
        project.save().unwrap();
        for bad in ["missing", "../patina", "half.toml", ""] {
            assert!(project.open_graph(bad).is_err(), "{bad}");
        }
        project.open_graph("half").unwrap();
        assert_eq!((project.graph_name(), project.graph().node.len()), ("half", 0));
        project.apply(&edit).unwrap();
        project.open_graph("main").unwrap();
        // A run uses the unsaved changes of a graph that is not open. A save writes them.
        assert!(project.check().unwrap().graph().node.iter().any(|n| n.id == "00000000000000b1/000000000000000a"));
        assert_eq!(project.unsaved().map(|g| g.0).collect::<Vec<_>>(), ["half"]);
        project.save().unwrap();
        assert!(!project.dirty());
        assert_eq!((project.graph_name(), project.graph().node.len()), ("main", 2));
        // The graph view shows the nodes of a subgraph use in place (15.3).
        assert_eq!(project.subgraph_file("half").unwrap().node.len(), 1);
        for bad in ["main", "missing", "../patina"] {
            assert!(project.subgraph_file(bad).is_err(), "{bad}");
        }
        assert!(project.can_undo(), "the undo history of the main graph stays");
        let half = fs::read_to_string(root.join("graphs/half.toml")).unwrap();
        assert!(half.contains("000000000000000a"), "{half}");

        // A collapse keeps the new subgraph in memory until a save.
        let ids = ["000000000000000a".to_owned()];
        assert!(project.collapse("half", &ids, "00000000000000c1").is_err(), "the graph exists");
        assert!(project.collapse("HALF", &ids, "00000000000000c1").is_err(), "the case is not important");
        project.collapse("part", &ids, "00000000000000c1").unwrap();
        assert!(!root.join("graphs/part.toml").exists(), "only a save writes the file");
        assert!(project.subgraphs().iter().any(|s| s.name == "part" && s.error.is_none()));
        assert!(project.check().unwrap().graph().node.iter().any(|n| n.id == "00000000000000c1/000000000000000a"));
        project.expand(&["00000000000000c1".to_owned()]).unwrap();
        assert_eq!(project.graph().node[1].id, "000000000000000a", "the expand gives the node again");
        assert!(project.expand(&ids).is_err(), "the node is not a use");
        project.undo(&mut BTreeMap::new()).unwrap();
        project.undo(&mut BTreeMap::new()).unwrap();
        // An undone collapse keeps the subgraph for a redo. Save writes it only while a graph uses it.
        assert!(!project.dirty(), "nothing uses the new subgraph");
        project.save().unwrap();
        let unused = root.join("graphs/part.toml").is_file();
        project.redo(&mut BTreeMap::new()).unwrap();
        assert!(project.dirty());
        project.save().unwrap();
        let kept = root.join("graphs/part.toml").is_file();
        let count = project.graph().node.len();
        // A new subgraph is empty, and a save writes it only after an edit or a use.
        assert!(project.new_graph("Part").is_err(), "the graph exists");
        assert!(project.new_graph("a/b").is_err(), "a bad name");
        project.new_graph("blank").unwrap();
        assert!(project.subgraphs().iter().any(|s| s.name == "blank"));
        assert!(!project.dirty(), "nothing changed");
        fs::remove_dir_all(&root).unwrap();
        assert_eq!((unused, kept, count), (false, true, 2));
    }

    #[test]
    fn a_top_graph_runs_in_place_of_main_and_no_use_names_it() {
        let root = std::env::temp_dir().join(format!("patina-top-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut project = Project::hello(&root);
        project.new_top("bench").unwrap();
        assert!(project.new_top("Bench").is_err() && project.new_top("main").is_err(), "each graph name is used one time");
        assert_eq!((project.top(), project.graph_name()), ("main", "main"), "a new graph does not open");
        assert_eq!(project.graphs().0, ["main", "bench"]);
        assert!(project.subgraphs().is_empty() && project.subgraph_file("bench").is_err());
        project.open_graph("bench").unwrap();
        assert_eq!(project.top(), "bench");
        assert!(project.check().unwrap().graph.node.is_empty() && project.use_paths().is_empty());
        assert!(project.dirty() && !root.join("graphs/bench.toml").exists(), "only Save writes the new graph");
        project.save().unwrap();
        assert!(fs::read_to_string(root.join("graphs/bench.toml")).unwrap().lines().any(|l| l == "top = true"));
        fs::write(root.join("graphs/main.toml"), "format = 1\n[[node]]\nid = \"00000000000000a1\"\nsubgraph = \"bench\"\n").unwrap();
        let problem = Project::open(&root).unwrap().check().err().unwrap().to_string();
        assert!(problem.contains("the graph bench is a top graph"), "{problem}");
        let bench = Project::open_top(&root, "bench").unwrap();
        assert!(bench.top() == "bench" && bench.check().is_ok());
        fs::write(root.join("graphs/half.toml"), "format = 1\n").unwrap();
        assert!(Project::open_top(&root, "half").is_err());
        let missing = Project::open_top(&root, "missing").err().unwrap().to_string();
        assert!(missing.ends_with("missing.toml: the project has no graph missing"), "{missing}");
        // A recovery file brings back a graph with no file as a new graph (15.10).
        project.restore("fresh", GraphFile { format: 1, top: true, ..GraphFile::default() }).unwrap();
        assert_eq!((project.graph_name(), project.top()), ("fresh", "fresh"));
        project.open_graph("main").unwrap();
        assert!(!project.drop_graph("main") && project.drop_graph("fresh") && !project.has_graph("fresh"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_test_runs_one_use_of_the_open_subgraph_with_the_input_params() {
        let root = std::env::temp_dir().join(format!("patina-test-use-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut project = Project::hello(&root);
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
