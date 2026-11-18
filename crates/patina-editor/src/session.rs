//! The open project and the commands of the page (spec 15.10).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use patina_engine::project::{GraphFile, NodeEntry, PublicInput, PublicOutput, Wire};
use patina_engine::{CodeText, Edit, Error, KindInfo, NodeInfo, Project, Stamps, Toolchain};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tauri::{AppHandle, State};

use crate::data::{Data, Recovery};
use crate::engine::Job;
use crate::node_process::{Host, ToNode, wait};

mod check;
mod clip;
mod imports;
mod runs;
mod start;

pub use check::*;
pub use clip::*;
pub use imports::*;
pub use runs::*;
pub use start::*;

pub struct Editor {
    pub data: Data,
    /// The node process, which runs the engine (16.1).
    pub engine: Host,
    session: Mutex<Option<Session>>,
    /// The folder of the self-check (`--check`), or none in normal use.
    pub check: Option<PathBuf>,
    /// The project of `--shot`, which the page opens and runs. Its screenshots go to `check`.
    shot: Option<PathBuf>,
    serial: AtomicU64,
    /// The labels of the float windows that close. Their views get no more jobs (15.8).
    closing: Mutex<Vec<String>>,
    /// True while a layout change of the page waits for its save (15.2).
    pub layout_pending: AtomicBool,
    /// rust-analyzer of the Code tab (15.1).
    pub analyzer: crate::analyzer::Analyzer,
}

struct Session {
    project: Project,
    /// The number of this opening of a project. Results of the engine carry it.
    serial: u64,
    trusted: bool,
    /// True after **Run**. Each edit then runs the graph again, with a build when it needs one.
    live: bool,
    /// The unsaved text of the Code tab, by path (15.10).
    code: BTreeMap<String, String>,
    /// The values of the public inputs while the open subgraph runs alone (15.9).
    test: Option<BTreeMap<String, String>>,
    /// The recovery file that the user did not restore or discard yet. Until the answer, Patina
    /// does not change the file.
    recovery: Option<Recovery>,
    /// The error of the last read or write of the recovery file.
    recovery_error: Option<String>,
    /// The domains and the problems of the last check. An edit of only the layout keeps them.
    checked: Option<(BTreeMap<String, &'static str>, Vec<String>)>,
    /// The stamps of the code and graph files at the last check, to find a change from outside the editor.
    stamps: Stamps,
}

/// All that the page shows of the open project.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    serial: u64,
    /// The project ID, which names the saved layout (15.2).
    id: String,
    root: PathBuf,
    name: String,
    graph: GraphFile,
    /// The name of the open graph: `main` or a subgraph (6.1).
    graph_name: String,
    nodes: Vec<NodeInfo>,
    /// The connection kinds (6.3).
    kinds: Vec<KindInfo>,
    /// The widget folders (6.2).
    widgets: Vec<String>,
    /// The view folders (15.8).
    views: Vec<String>,
    /// The connection folders (6.3).
    connections: Vec<String>,
    /// The type folders (5.1).
    types: Vec<String>,
    /// The imported folders (6.4), for example `nodes/blur`.
    imports: Vec<String>,
    /// The problems of the graph check, for example `node <id>: <text>`.
    problems: Vec<String>,
    dirty: bool,
    trusted: bool,
    live: bool,
    can_undo: bool,
    can_redo: bool,
    /// Changes when an edit writes a code file or an undo changes a folder, so the page reads the
    /// files again.
    code_revision: u64,
    /// The text that an undo or a redo of a code change puts into the **Code** tab (15.10).
    #[serde(skip_serializing_if = "Option::is_none")]
    code_text: Option<CodeText>,
    /// True when the project has a recovery file and the user did not answer yet (15.10).
    recovery: bool,
    /// The values of the public inputs while the open subgraph runs alone, or none (15.9).
    test: Option<BTreeMap<String, String>>,
    /// The clock domain of each node use of the checked graph, empty when the check fails (9.1, 15.3).
    domains: BTreeMap<String, &'static str>,
    /// The frames each second of the project (9.1).
    frame_rate: u32,
    /// The use path of each use of the open subgraph, for its live values (6.4, 15.9).
    uses: Vec<String>,
}

/// The answer to a recovery file: the view and the unsaved code for the Code tab (15.10).
#[derive(Serialize)]
pub struct Recovered {
    view: View,
    code: BTreeMap<String, String>,
}

impl Editor {
    pub fn new(data: Data, engine: Host, check: Option<PathBuf>, shot: Option<PathBuf>) -> Editor {
        Editor { data, engine, session: Mutex::new(None), check, shot, serial: AtomicU64::new(0), closing: Mutex::default(), layout_pending: AtomicBool::new(false), analyzer: Default::default() }
    }

    /// True when the open project or the Code tab has changes that are not saved.
    pub fn dirty(&self) -> bool {
        self.lock().as_ref().is_some_and(|s| !s.code.is_empty() || s.project.dirty())
    }

    /// A panic in a command does not stop the editor, so the state stays in use after it.
    fn lock(&self) -> MutexGuard<'_, Option<Session>> {
        self.session.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn with<T>(&self, f: impl FnOnce(&mut Session) -> Result<T, String>) -> Result<T, String> {
        f(self.lock().as_mut().ok_or("No project is open.")?)
    }

    /// The root of the open project. A path pick needs no trust, because no project code runs.
    pub fn project_root(&self) -> Result<PathBuf, String> {
        self.with(|session| Ok(session.project.root().to_owned()))
    }

    /// The root of the open project, when the user trusts it (11). `to` says what needs the trust.
    pub(crate) fn trusted_root(&self, to: &str) -> Result<PathBuf, String> {
        self.with(|session| match session.trusted {
            true => Ok(session.project.root().to_owned()),
            false => Err(format!("Trust the project before you {to}.")),
        })
    }

    /// Sends a view job of window `label` and waits for its answer. A window that closes gets an
    /// error. The check and the send hold one lock, so no job of a window comes after its drop.
    fn window_ask<T: DeserializeOwned + Default>(&self, label: &str, job: impl FnOnce(u64) -> Job) -> Result<T, String> {
        let answer = {
            let closing = crate::engine::lock(&self.closing);
            if closing.iter().any(|l| l == label) {
                return Err("The window closes.".to_owned());
            }
            self.engine.send_ask(true, |ask| ToNode::Job(job(ask)))?
        };
        wait(answer)
    }

    /// Drops view instance `slot` of the float window `label` before the window closes, because
    /// the surface of a viewport must go before its window (15.8).
    pub fn drop_window_view(&self, label: &str, slot: &str) -> Result<(), String> {
        crate::engine::lock(&self.closing).push(label.to_owned());
        self.engine.ask(false, |ask| ToNode::Job(Job::DropView(slot.to_owned(), ask)))
    }

    /// Forgets a float window that closed, so a new window can use its label.
    pub fn window_gone(&self, label: &str) {
        crate::engine::lock(&self.closing).retain(|l| l != label);
    }

    fn open(&self, project: Project) -> Result<View, String> {
        self.data.add_recent(project.root())?;
        let trusted = self.data.is_trusted(project.root());
        let serial = self.serial.fetch_add(1, Ordering::Relaxed) + 1;
        let (recovery, recovery_error) = match self.data.recovery(project.root()) {
            Ok(recovery) => (recovery, None),
            Err(e) => (None, Some(e)),
        };
        self.engine.unload(serial);
        self.analyzer.stop();
        let mut session = self.lock();
        // A project opens paused (15.10).
        let new = Session { project, serial, trusted, live: false, code: BTreeMap::new(), test: None, recovery, recovery_error, checked: None, stamps: Stamps::new() };
        let session = session.insert(new);
        // The view goes first, so the window knows the serial of the load events.
        let view = session.view(&self.engine, None, None);
        session.load(&self.engine);
        Ok(view)
    }

    /// Closes the project. The user saved or discarded the changes, so the recovery file goes. A
    /// recovery file without an answer stays for the next open.
    fn close(&self) {
        self.analyzer.stop();
        if let Some(session) = self.lock().take()
            && session.recovery.is_none()
            && let Err(e) = self.data.remove_recovery(session.project.root())
        {
            eprintln!("{e}");
        }
        self.engine.end();
    }

    /// Applies a change to the graph or the code, then sends the graph to the engine when it is
    /// live. A code change needs a build (15.10).
    fn change(&self, f: impl FnOnce(&mut Project, &mut BTreeMap<String, String>) -> Result<(), String>) -> Result<View, String> {
        self.with(|session| {
            if !session.trusted {
                return Err("Trust the project before you change it.".to_owned());
            }
            let old = session.project.graph().clone();
            let code = session.project.code_revision();
            f(&mut session.project, &mut session.code)?;
            session.keep_recovery(&self.data);
            let coded = session.project.code_revision() != code;
            // A view is not part of the clusters, so its code drops only the loaded view (15.8).
            let view = coded.then(|| session.project.last_code().and_then(view_folder)).flatten();
            if let Some(name) = view.clone() {
                self.engine.send(Job::CloseView(name));
            }
            let built = (coded && view.is_none()).then_some(true);
            let run = built.or_else(|| affects_run(&old, session.project.graph()));
            // A move of a node changes no domain and no problem, so it needs no check of a large graph.
            if !coded && run.is_none() && session.checked.is_some() {
                return Ok(session.show());
            }
            let build = run.filter(|_| session.live);
            let slot = built.and(session.project.last_code()).and_then(slot_folder);
            Ok(session.view(&self.engine, build, slot))
        })
    }

    /// An undo or a redo. The project refuses to remove a new folder with unsaved changes (15.2).
    fn step(&self, undo: bool) -> Result<View, String> {
        let mut text = None;
        let mut view = self.change(|project, code| {
            text = if undo { project.undo(code) } else { project.redo(code) }?;
            Ok(())
        })?;
        view.code_text = text;
        Ok(view)
    }
}

impl Session {
    /// Writes the unsaved changes to the recovery file, or removes the file when there are none
    /// (15.10). An error shows with the problems of the project.
    fn keep_recovery(&mut self, data: &Data) {
        if self.recovery.is_none() {
            let graphs: Vec<_> = self.project.unsaved().collect();
            self.recovery_error = data.save_recovery(self.project.root(), self.project.graph_name(), &graphs, &self.code).err();
        }
    }

    /// Loads the clusters of the last build of a trusted project, so the node GUIs show while the
    /// graph is paused (15.10). A project with no build waits for **Run**.
    fn load(&self, engine: &Host) {
        if let (true, false, Ok(checked)) = (self.trusted, self.live, self.project.check())
            && checked.built_mode().is_some()
        {
            engine.run(self.serial, checked, true, None, false);
        }
    }

    /// Checks the graph. With `build`, a live graph that passes the check goes to the engine.
    /// `slot` is the node folder of a code change, for the slot (7.1).
    fn view(&mut self, engine: &Host, build: Option<bool>, slot: Option<String>) -> View {
        let checked = match &self.test {
            Some(inputs) => self.project.check_test(inputs),
            None => self.project.check(),
        };
        let (checked, problems) = match checked {
            Ok(checked) => (Some(checked), Vec::new()),
            Err(Error::Graph(_, problems)) => (None, problems),
            Err(e) => (None, vec![e.to_string()]),
        };
        let domains = checked.iter().flat_map(|c| c.domains()).map(|(id, d)| (id.to_owned(), d)).collect();
        if let (Some(checked), Some(build), true) = (checked, build, self.live) {
            engine.run(self.serial, checked, build, slot, true);
        }
        self.checked = Some((domains, problems));
        self.stamps = self.project.stamps();
        self.show()
    }

    /// The view with the domains and the problems of the last check.
    fn show(&self) -> View {
        let project = &self.project;
        let (domains, problems) = self.checked.clone().unwrap_or_default();
        let (mut nodes, mut problems) = match project.nodes() {
            Ok(nodes) => (nodes, problems),
            Err(e) => (Vec::new(), [e.to_string()].into_iter().chain(problems).collect()),
        };
        nodes.extend(patina_engine::builtins());
        nodes.extend(project.subgraphs());
        problems.extend(self.recovery_error.iter().map(|e| format!("recovery file: {e}")));
        View {
            serial: self.serial,
            id: project.id().to_owned(),
            root: project.root().to_owned(),
            name: folder_name(project.root()),
            graph: project.graph().clone(),
            graph_name: project.graph_name().to_owned(),
            nodes,
            kinds: project.kinds(),
            widgets: project.widgets(),
            views: project.views(),
            connections: project.connections(),
            types: project.types(),
            imports: project.imports(),
            problems,
            dirty: project.dirty(),
            trusted: self.trusted,
            live: self.live,
            can_undo: project.can_undo(),
            can_redo: project.can_redo(),
            code_revision: project.code_revision(),
            code_text: None,
            recovery: self.recovery.is_some(),
            test: self.test.clone(),
            domains,
            frame_rate: project.frame_rate(),
            uses: project.use_paths(),
        }
    }
}

/// The view of a code path in `views/<name>/`.
fn view_folder(path: &str) -> Option<String> {
    path.strip_prefix("views/")?.split_once('/').map(|(name, _)| name.to_owned())
}

/// The node folder of a code file, when a change of the file can build in the slot: a Rust file of
/// the node folder, not its `Cargo.toml`. A new crate can change the crates of the base (7.1).
fn slot_folder(path: &str) -> Option<String> {
    let (name, file) = path.strip_prefix("nodes/")?.split_once('/')?;
    file.ends_with(".rs").then(|| name.to_owned())
}

/// Some(true) when the change needs a new build, and Some(false) when only parameters changed.
/// None when nothing that runs changed, for example a position or the points of a wire.
fn affects_run(old: &GraphFile, new: &GraphFile) -> Option<bool> {
    type Node<'a> = (&'a str, [&'a Option<String>; 5]);
    fn node(n: &NodeEntry) -> Node<'_> {
        (&n.id, [&n.folder, &n.subgraph, &n.builtin, &n.domain, &n.settings])
    }
    fn wire(w: &Wire) -> (&str, &str) {
        (&w.from, &w.to)
    }
    fn input(i: &PublicInput) -> (&str, &str, &[String]) {
        (&i.key, &i.name, &i.to)
    }
    fn output(o: &PublicOutput) -> (&str, &str, &str) {
        (&o.key, &o.name, &o.from)
    }
    fn params(n: &NodeEntry) -> &BTreeMap<String, String> {
        &n.params
    }
    let build = !old.node.iter().map(node).eq(new.node.iter().map(node))
        || !old.wire.iter().map(wire).eq(new.wire.iter().map(wire))
        || old.order != new.order
        || !old.input.iter().map(input).eq(new.input.iter().map(input))
        || !old.output.iter().map(output).eq(new.output.iter().map(output));
    if build {
        return Some(true);
    }
    (!old.node.iter().map(params).eq(new.node.iter().map(params))).then_some(false)
}

fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

fn full_path(path: &Path) -> Result<(), String> {
    if path.is_absolute() { Ok(()) } else { Err(format!("Type the full path of the folder, not {}.", path.display())) }
}

#[tauri::command(async)]
pub fn edit(editor: State<'_, Editor>, edit: Edit) -> Result<View, String> {
    editor.change(|project, _| project.apply(&edit))
}

/// Checks the graph again when a code file or a graph file changed from outside the editor, and
/// builds a live graph (15.10). The page calls it at a set time. Gives None when no file changed.
#[tauri::command(async)]
pub fn refresh(editor: State<'_, Editor>) -> Result<Option<View>, String> {
    editor.with(|session| {
        let stamps = session.project.stamps();
        if stamps == session.stamps {
            return Ok(None);
        }
        let (views, other) = outside_changes(&session.stamps, &stamps);
        session.project.outside_change();
        for name in views {
            editor.engine.send(Job::CloseView(name));
        }
        let build = (other && session.live).then_some(true);
        Ok(Some(session.view(&editor.engine, build, None)))
    })
}

/// Moves a code folder or a subgraph of the Project panel to the Recycle Bin (15.4). The nodes
/// and the uses of the graphs stay, and show the error of a missing folder until the user deletes
/// them. Only the Recycle Bin gets the folder back.
#[tauri::command(async)]
pub fn delete_folder(editor: State<'_, Editor>, folder: String) -> Result<View, String> {
    editor.with(|session| {
        if !session.trusted {
            return Err("Trust the project before you delete a folder.".to_owned());
        }
        let path = session.project.root().join(deletable_folder(&folder)?);
        if !path.exists() {
            return Err(format!("The project has no {folder}."));
        }
        if folder.strip_prefix("graphs/") == Some(session.project.graph_name()) {
            return Err(format!("Open another graph before you delete {folder}."));
        }
        let inside = format!("{folder}/");
        if session.code.keys().any(|file| file.starts_with(&inside)) {
            return Err(format!("The Code tab has unsaved changes in {folder}. Save or undo them first."));
        }
        // rust-analyzer watches each folder, so Windows cannot move one while it runs. The Code tab starts it again.
        editor.analyzer.stop();
        start::recycle(&path)?;
        session.project.outside_change();
        if let Some(name) = folder.strip_prefix("views/") {
            editor.engine.send(Job::CloseView(name.to_owned()));
        }
        let build = session.live.then_some(true);
        Ok(session.view(&editor.engine, build, None))
    })
}

/// The path of a folder that the Project panel can delete, so the page cannot delete another file.
fn deletable_folder(folder: &str) -> Result<String, String> {
    let not = || format!("{folder} is not a folder of the Project panel.");
    let (group, name) = folder.split_once('/').ok_or_else(not)?;
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        return Err(not());
    }
    match group {
        "graphs" if name == "main" => Err("The main graph cannot go.".to_owned()),
        "graphs" => Ok(format!("graphs/{name}.toml")),
        "nodes" | "widgets" | "views" | "connections" | "types" => Ok(folder.to_owned()),
        _ => Err(not()),
    }
}

/// The views of the files that changed, came or went, and true when another file did.
fn outside_changes(old: &Stamps, new: &Stamps) -> (BTreeSet<String>, bool) {
    let came = new.iter().filter(|&(path, stamp)| old.get(path) != Some(stamp)).map(|(path, _)| path);
    let went = old.keys().filter(|path| !new.contains_key(*path));
    let (mut views, mut other) = (BTreeSet::new(), false);
    for path in came.chain(went) {
        match view_folder(path) {
            Some(name) => _ = views.insert(name),
            None => other = true,
        }
    }
    (views, other)
}

/// Undo and redo with nothing to do give the same view. An undo of a code change writes no file:
/// the view has the text for the Code tab (15.10).
#[tauri::command(async)]
pub fn undo(editor: State<'_, Editor>) -> Result<View, String> {
    editor.step(true)
}

#[tauri::command(async)]
pub fn redo(editor: State<'_, Editor>) -> Result<View, String> {
    editor.step(false)
}

#[tauri::command(async)]
pub fn save(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        session.project.save().map_err(|e| e.to_string())?;
        session.keep_recovery(&editor.data);
        Ok(session.view(&editor.engine, None, None))
    })
}

/// The files of a node folder for the Code tab (15.4).
#[tauri::command(async)]
pub fn folder_files(editor: State<'_, Editor>, folder: String) -> Result<Vec<String>, String> {
    editor.with(|session| session.project.folder_files(&folder).map_err(|e| e.to_string()))
}

/// The project files for an asset picker (4.3).
#[tauri::command(async)]
pub fn asset_files(editor: State<'_, Editor>, extensions: Vec<String>) -> Result<Vec<String>, String> {
    editor.with(|session| session.project.asset_files(&extensions).map_err(|e| e.to_string()))
}

/// Adds a crate of crates.io, a widget folder or a type folder to a code folder with `cargo add`
/// (15.4). The new `Cargo.toml` is one `SetCode` edit, so undo removes the crate.
#[tauri::command(async)]
pub fn add_crate(editor: State<'_, Editor>, folder: String, spec: String) -> Result<View, String> {
    let path = format!("{folder}/Cargo.toml");
    let root = editor.with(|session| {
        if !session.trusted {
            return Err("Trust the project before you change it.".to_owned());
        }
        if session.code.contains_key(&path) {
            return Err("Save or undo the changes of Cargo.toml first.".to_owned());
        }
        Ok(session.project.root().to_owned())
    })?;
    // Cargo reads the index of crates.io, so the session stays unlocked while it runs.
    let toolchain = Toolchain::find(&editor.data.toolchain).map_err(|e| e.to_string())?;
    let text = patina_engine::add_crate(&root, &toolchain, &folder, &spec)?;
    editor.change(|project, _| {
        if project.root() != root {
            return Err("Another project opened while cargo ran.".to_owned());
        }
        project.apply(&Edit::SetCode { path, text })
    })
}

/// Makes the node folder `nodes/<name>` (15.2). Undo removes the folder while it has only its start files.
#[tauri::command(async)]
pub fn new_node(editor: State<'_, Editor>, name: String) -> Result<View, String> {
    editor.change(|project, _| project.new_node(&name))
}

/// Makes the widget folder `widgets/<name>` (4.3). Undo removes the folder while it has only its start files.
#[tauri::command(async)]
pub fn new_widget(editor: State<'_, Editor>, name: String) -> Result<View, String> {
    editor.change(|project, _| project.new_widget(&name))
}

/// Makes the view folder `views/<name>` (15.8). Undo removes the folder while it has only its start files.
#[tauri::command(async)]
pub fn new_view(editor: State<'_, Editor>, name: String) -> Result<View, String> {
    editor.change(|project, _| project.new_view(&name))
}

/// Makes the connection folder `connections/<name>` (6.3). Undo removes the folder while it has only its start files.
#[tauri::command(async)]
pub fn new_connection(editor: State<'_, Editor>, name: String) -> Result<View, String> {
    editor.change(|project, _| project.new_connection(&name))
}

/// Makes the type folder `types/<name>` (5.1). Undo removes the folder while it has only its start files.
#[tauri::command(async)]
pub fn new_type(editor: State<'_, Editor>, name: String) -> Result<View, String> {
    editor.change(|project, _| project.new_type(&name))
}

/// The page sends the unsaved text of the Code tab, so a close of the window asks first and the
/// recovery file has the text (15.10). Text that comes after the close of the project is old.
#[tauri::command(async)]
pub fn set_unsaved_code(editor: State<'_, Editor>, code: BTreeMap<String, String>) -> Result<(), String> {
    let mut session = editor.lock();
    let Some(session) = session.as_mut() else { return Ok(()) };
    session.code = code;
    session.keep_recovery(&editor.data);
    session.recovery_error.clone().map_or(Ok(()), Err)
}

/// Restores or discards the changes of the recovery file (15.10). A restored graph is one step of
/// undo. The page puts the code into the Code tab.
#[tauri::command(async)]
pub fn recover(editor: State<'_, Editor>, restore: bool) -> Result<Recovered, String> {
    editor.with(|session| {
        if !session.trusted {
            return Err("Trust the project before you restore its changes.".to_owned());
        }
        let file = session.recovery.as_ref().ok_or("The project has no recovery file.")?;
        if restore {
            // Each graph opens and gets its changes as one step of undo. The open graph comes last.
            let open = file.graph_name.as_deref().filter(|_| file.graph.is_some()).unwrap_or("main");
            for (name, graph) in file.others.iter().map(|(name, g)| (name.as_str(), g)).chain(file.graph.as_ref().map(|g| (open, g))) {
                session.project.open_graph(name)?;
                session.project.restore_graph(graph.clone());
            }
            session.project.open_graph(open)?;
        }
        let file = session.recovery.take().ok_or("The project has no recovery file.")?;
        let code = if restore { file.code } else { BTreeMap::new() };
        session.code = code.clone();
        session.keep_recovery(&editor.data);
        Ok(Recovered { view: session.view(&editor.engine, None, None), code })
    })
}

/// Opens the main graph or a subgraph in the graph view (6.1).
#[tauri::command(async)]
pub fn open_graph(editor: State<'_, Editor>, name: String) -> Result<View, String> {
    editor.with(|session| {
        session.project.open_graph(&name)?;
        // The test of a subgraph ends, so the main graph runs again (15.9).
        let build = session.test.take().map(|_| true);
        Ok(session.view(&editor.engine, build, None))
    })
}

/// The file of a subgraph, to show its nodes in place in the graph that uses it (15.3).
#[tauri::command(async)]
pub fn subgraph_file(editor: State<'_, Editor>, name: String) -> Result<GraphFile, String> {
    editor.with(|session| session.project.subgraph_file(&name))
}

/// Moves the selected nodes into a new subgraph (6.1).
#[tauri::command(async)]
pub fn collapse(editor: State<'_, Editor>, name: String, ids: Vec<String>, use_id: String) -> Result<View, String> {
    editor.change(|project, _| project.collapse(&name, &ids, &use_id))
}

/// Makes the new empty subgraph `name` in memory (6.1). Only a save writes its file.
#[tauri::command(async)]
pub fn new_graph(editor: State<'_, Editor>, name: String) -> Result<View, String> {
    editor.change(|project, _| project.new_graph(&name))
}

/// Puts the nodes of the selected subgraph uses in their place (6.1).
#[tauri::command(async)]
pub fn expand(editor: State<'_, Editor>, ids: Vec<String>) -> Result<View, String> {
    editor.change(|project, _| project.expand(&ids))
}

#[tauri::command(async)]
pub fn read_code(editor: State<'_, Editor>, path: String) -> Result<String, String> {
    editor.with(|session| session.project.read_code(&path))
}

/// The saved layout of a project (15.2). The page checks the text.
#[tauri::command(async)]
pub fn layout(editor: State<'_, Editor>, id: String) -> Result<Option<String>, String> {
    editor.data.layout(&id)
}

#[tauri::command(async)]
pub fn save_layout(editor: State<'_, Editor>, id: String, text: String) -> Result<(), String> {
    editor.data.save_layout(&id, &text)
}

/// The page calls this when a layout change waits for its save, and after the save. A close of
/// the window then waits for the save (15.2).
#[tauri::command]
pub fn layout_pending(editor: State<'_, Editor>, pending: bool) {
    editor.layout_pending.store(pending, Ordering::Relaxed);
}

/// The page calls this after it saved the layout and asked the user about the unsaved changes.
#[tauri::command(async)]
pub fn quit(app: AppHandle, editor: State<'_, Editor>) {
    editor.close();
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_rust_file_of_a_node_goes_into_the_slot() {
        assert_eq!(slot_folder("nodes/acc/src/lib.rs").as_deref(), Some("acc"));
        assert_eq!(slot_folder("nodes/acc/Cargo.toml"), None);
        assert_eq!(slot_folder("widgets/bar/src/lib.rs"), None);
        assert_eq!(view_folder("views/map/src/lib.rs").as_deref(), Some("map"));
        assert_eq!(view_folder("nodes/map/src/lib.rs"), None);
    }

    #[test]
    fn only_a_folder_of_the_project_panel_can_go() {
        assert_eq!(deletable_folder("nodes/switch").as_deref(), Ok("nodes/switch"));
        assert_eq!(deletable_folder("types/seg-2").as_deref(), Ok("types/seg-2"));
        assert_eq!(deletable_folder("graphs/filter").as_deref(), Ok("graphs/filter.toml"));
        for bad in ["graphs/main", "nodes/..", "nodes/a/b", "nodes/", "src/lib", "nodes", "target/debug", "nodes/a\\b"] {
            assert!(deletable_folder(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn an_outside_change_of_a_view_closes_only_the_view() {
        let stamp = |len: u64| (None, len);
        let old: Stamps = [("views/map/src/lib.rs".to_owned(), stamp(1)), ("nodes/n/src/lib.rs".to_owned(), stamp(1))].into();
        let mut new = old.clone();
        new.insert("views/map/src/lib.rs".to_owned(), stamp(2));
        assert_eq!(outside_changes(&old, &new), (["map".to_owned()].into(), false));
        new.remove("nodes/n/src/lib.rs");
        assert!(outside_changes(&old, &new).1, "a removed file is a change");
        assert_eq!(outside_changes(&old, &old), (BTreeSet::new(), false));
    }

    #[test]
    fn only_a_change_that_runs_needs_the_engine() {
        let graph: GraphFile = toml::from_str(include_str!("../../../examples/hello/graphs/main.toml")).unwrap();
        let mut moved = graph.clone();
        moved.node[0].pos = Some([500.0, 500.0]);
        assert_eq!(affects_run(&graph, &moved), None);
        let mut routed = graph.clone();
        routed.wire[0].points.push([40.0, 40.0]);
        assert_eq!(affects_run(&graph, &routed), None, "a route point changes only the drawing");
        let mut param = moved.clone();
        param.node[0].params.insert("value".to_owned(), "7".to_owned());
        assert_eq!(affects_run(&moved, &param), Some(false));
        let mut unwired = param.clone();
        unwired.wire.clear();
        assert_eq!(affects_run(&param, &unwired), Some(true));
    }
}
