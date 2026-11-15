//! The open project and the commands of the page (spec 15.10).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};

use patina_engine::project::{GraphFile, NodeEntry, PublicInput, PublicOutput, Wire};
use patina_engine::{Edit, Error, NodeInfo, Project, Toolchain};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State, WebviewWindow};

use crate::data::{Data, Recovery, same_path};
use crate::engine::{self, Job, Run, Shared};

pub struct Editor {
    pub data: Data,
    pub engine: Sender<Job>,
    shared: Arc<Shared>,
    session: Mutex<Option<Session>>,
    /// The folder of the self-check (`--check`), or none in normal use.
    pub check: Option<PathBuf>,
    serial: AtomicU64,
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
    /// The recovery file that the user did not restore or discard yet. Until the answer, Patina
    /// does not change the file.
    recovery: Option<Recovery>,
    /// The error of the last read or write of the recovery file.
    recovery_error: Option<String>,
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
    nodes: Vec<NodeInfo>,
    /// The problems of the graph check, for example `node <id>: <text>`.
    problems: Vec<String>,
    dirty: bool,
    trusted: bool,
    live: bool,
    can_undo: bool,
    can_redo: bool,
    /// Changes when an edit, undo or redo writes a code file, so the page reads the file again.
    code_revision: u64,
    /// True when the project has a recovery file and the user did not answer yet (15.10).
    recovery: bool,
}

/// The answer to a recovery file: the view and the unsaved code for the Code tab (15.10).
#[derive(Serialize)]
pub struct Recovered {
    view: View,
    code: BTreeMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartInfo {
    recent: Vec<Recent>,
    projects_dir: PathBuf,
    check: Option<PathBuf>,
}

/// The settings screen (15.10): the toolchain, the folders and the trusted projects.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// The Rust version, or none when Patina finds no toolchain.
    rust: Option<String>,
    cargo: Option<PathBuf>,
    toolchain_problem: Option<String>,
    data_dir: PathBuf,
    projects_dir: PathBuf,
    portable: bool,
    trusted: Vec<PathBuf>,
}

#[derive(Serialize)]
pub struct Recent {
    path: PathBuf,
    name: String,
    exists: bool,
}

impl Editor {
    pub fn new(data: Data, (engine, shared): (Sender<Job>, Arc<Shared>), check: Option<PathBuf>) -> Editor {
        Editor { data, engine, shared, session: Mutex::new(None), check, serial: AtomicU64::new(0) }
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

    fn open(&self, project: Project) -> Result<View, String> {
        self.data.add_recent(project.root())?;
        let trusted = self.data.is_trusted(project.root());
        let serial = self.serial.fetch_add(1, Ordering::Relaxed) + 1;
        let (recovery, recovery_error) = match self.data.recovery(project.root()) {
            Ok(recovery) => (recovery, None),
            Err(e) => (None, Some(e)),
        };
        let _ = self.engine.send(Job::Unload);
        let mut session = self.lock();
        // A project opens paused (15.10).
        let new = Session { project, serial, trusted, live: false, code: BTreeMap::new(), recovery, recovery_error };
        Ok(session.insert(new).view(&self.engine, None))
    }

    /// Closes the project. The user saved or discarded the changes, so the recovery file goes. A
    /// recovery file without an answer stays for the next open.
    fn close(&self) {
        if let Some(session) = self.lock().take()
            && session.recovery.is_none()
            && let Err(e) = self.data.remove_recovery(session.project.root())
        {
            eprintln!("{e}");
        }
        let _ = self.engine.send(Job::Unload);
    }

    /// Applies a change to the graph or the code, then sends the graph to the engine when it is
    /// live. A code change needs a build (15.10).
    fn change(&self, f: impl FnOnce(&mut Project) -> Result<(), String>) -> Result<View, String> {
        self.with(|session| {
            if !session.trusted {
                return Err("Trust the project before you change it.".to_owned());
            }
            let old = session.live.then(|| session.project.graph().clone());
            let code = session.project.code_revision();
            f(&mut session.project)?;
            session.keep_recovery(&self.data);
            let built = (session.project.code_revision() != code).then_some(true);
            let build = old.and_then(|old| built.or(affects_run(&old, session.project.graph())));
            Ok(session.view(&self.engine, build))
        })
    }
}

impl Session {
    /// Writes the unsaved changes to the recovery file, or removes the file when there are none
    /// (15.10). An error shows with the problems of the project.
    fn keep_recovery(&mut self, data: &Data) {
        if self.recovery.is_none() {
            let graph = self.project.dirty().then(|| self.project.graph());
            self.recovery_error = data.save_recovery(self.project.root(), graph, &self.code).err();
        }
    }

    /// Checks the graph. With `build`, a live graph that passes the check goes to the engine.
    fn view(&self, engine: &Sender<Job>, build: Option<bool>) -> View {
        let project = &self.project;
        let (checked, problems) = match project.check() {
            Ok(checked) => (Some(checked), Vec::new()),
            Err(Error::Graph(_, problems)) => (None, problems),
            Err(e) => (None, vec![e.to_string()]),
        };
        if let (Some(checked), Some(build), true) = (checked, build, self.live) {
            let run = Run { serial: self.serial, checked, graph: project.graph().clone(), build };
            let _ = engine.send(Job::Run(Box::new(run)));
        }
        let (nodes, mut problems) = match project.nodes() {
            Ok(nodes) => (nodes, problems),
            Err(e) => (Vec::new(), [e.to_string()].into_iter().chain(problems).collect()),
        };
        problems.extend(self.recovery_error.iter().map(|e| format!("recovery file: {e}")));
        View {
            serial: self.serial,
            id: project.id().to_owned(),
            root: project.root().to_owned(),
            name: folder_name(project.root()),
            graph: project.graph().clone(),
            nodes,
            problems,
            dirty: project.dirty(),
            trusted: self.trusted,
            live: self.live,
            can_undo: project.can_undo(),
            can_redo: project.can_redo(),
            code_revision: project.code_revision(),
            recovery: self.recovery.is_some(),
        }
    }
}

/// Some(true) when the change needs a new build, and Some(false) when only parameters changed.
/// None when nothing that runs changed, for example a position or the points of a wire.
fn affects_run(old: &GraphFile, new: &GraphFile) -> Option<bool> {
    fn node(n: &NodeEntry) -> (&str, &Option<String>, &Option<String>, &Option<String>, &Option<String>) {
        (&n.id, &n.folder, &n.subgraph, &n.domain, &n.settings)
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

/// A project name is a folder name that works on each platform.
fn check_project_name(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && !name.ends_with(['.', ' '])
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.'));
    if ok {
        Ok(())
    } else {
        Err("Use letters, digits, spaces, '-', '_' and '.' in the name. Do not start or end it with '.'.".to_owned())
    }
}

fn full_path(path: &Path) -> Result<(), String> {
    if path.is_absolute() { Ok(()) } else { Err(format!("Type the full path of the folder, not {}.", path.display())) }
}

#[tauri::command(async)]
pub fn start_info(editor: State<'_, Editor>) -> StartInfo {
    let recent = editor.data.recent().into_iter();
    StartInfo {
        recent: recent.map(|path| Recent { name: folder_name(&path), exists: path.join("patina.toml").is_file(), path }).collect(),
        projects_dir: editor.data.projects.clone(),
        check: editor.check.clone(),
    }
}

#[tauri::command(async)]
pub fn settings(editor: State<'_, Editor>) -> Settings {
    let (rust, cargo, toolchain_problem) = match Toolchain::detect() {
        Ok(t) => (Some(t.release), Some(t.cargo), None),
        Err(e) => (None, None, Some(e.to_string())),
    };
    let data = &editor.data;
    Settings { rust, cargo, toolchain_problem, data_dir: data.dir.clone(), projects_dir: data.projects.clone(), portable: data.portable, trusted: data.trusted() }
}

/// Takes a folder off the list of trusted projects (11). Patina asks again when it opens next.
#[tauri::command(async)]
pub fn untrust_project(editor: State<'_, Editor>, path: PathBuf) -> Result<Vec<PathBuf>, String> {
    if editor.lock().as_ref().is_some_and(|s| same_path(s.project.root(), &path)) {
        return Err("Close the project first.".to_owned());
    }
    editor.data.untrust(&path)?;
    Ok(editor.data.trusted())
}

/// Makes a project with the `number` and `show` nodes in `<parent>/<name>` (15.10).
#[tauri::command(async)]
pub fn new_project(editor: State<'_, Editor>, parent: PathBuf, name: String) -> Result<View, String> {
    check_project_name(&name)?;
    full_path(&parent)?;
    let rust = Toolchain::detect().map_err(|e| e.to_string())?.release;
    let project = Project::create(&parent.join(&name), &rust).map_err(|e| e.to_string())?;
    // The user made the project, so its code is the code of the user (11).
    editor.data.trust(project.root())?;
    editor.open(project)
}

#[tauri::command(async)]
pub fn open_project(editor: State<'_, Editor>, path: PathBuf) -> Result<View, String> {
    full_path(&path)?;
    editor.open(Project::open(&path).map_err(|e| e.to_string())?)
}

#[tauri::command(async)]
pub fn close_project(editor: State<'_, Editor>) {
    editor.close();
}

#[tauri::command(async)]
pub fn trust_project(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        editor.data.trust(session.project.root())?;
        session.trusted = true;
        Ok(session.view(&editor.engine, None))
    })
}

#[tauri::command(async)]
pub fn edit(editor: State<'_, Editor>, edit: Edit) -> Result<View, String> {
    editor.change(|project| project.apply(&edit))
}

/// Undo and redo with nothing to do give the same view.
#[tauri::command(async)]
pub fn undo(editor: State<'_, Editor>) -> Result<View, String> {
    editor.change(|project| project.undo().map(drop))
}

#[tauri::command(async)]
pub fn redo(editor: State<'_, Editor>) -> Result<View, String> {
    editor.change(|project| project.redo().map(drop))
}

#[tauri::command(async)]
pub fn save(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        session.project.save().map_err(|e| e.to_string())?;
        session.keep_recovery(&editor.data);
        Ok(session.view(&editor.engine, None))
    })
}

/// The files of a node folder for the Code tab (15.4).
#[tauri::command(async)]
pub fn node_files(editor: State<'_, Editor>, name: String) -> Result<Vec<String>, String> {
    editor.with(|session| session.project.node_files(&name).map_err(|e| e.to_string()))
}

/// Adds a crate of crates.io to a node with `cargo add` (15.4). The new `Cargo.toml` is one
/// `SetCode` edit, so undo removes the crate.
#[tauri::command(async)]
pub fn add_crate(editor: State<'_, Editor>, node: String, spec: String) -> Result<View, String> {
    let path = format!("nodes/{node}/Cargo.toml");
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
    let toolchain = Toolchain::detect().map_err(|e| e.to_string())?;
    let text = patina_engine::add_crate(&root, &toolchain, &node, &spec)?;
    editor.change(|project| {
        if project.root() != root {
            return Err("Another project opened while cargo ran.".to_owned());
        }
        project.apply(&Edit::SetCode { path, text })
    })
}

/// Makes the node folder `nodes/<name>` (15.2). Undo does not remove the folder.
#[tauri::command(async)]
pub fn new_node(editor: State<'_, Editor>, name: String) -> Result<View, String> {
    editor.change(|project| project.new_node(&name))
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
        let file = session.recovery.take().ok_or("The project has no recovery file.")?;
        let mut code = BTreeMap::new();
        if restore {
            if let Some(graph) = file.graph {
                session.project.restore_graph(graph);
            }
            code = file.code;
        }
        session.code = code.clone();
        session.keep_recovery(&editor.data);
        Ok(Recovered { view: session.view(&editor.engine, None), code })
    })
}

#[tauri::command(async)]
pub fn read_code(editor: State<'_, Editor>, path: String) -> Result<String, String> {
    editor.with(|session| session.project.read_code(&path))
}

/// Builds and runs the graph, and keeps it live (15.10).
#[tauri::command(async)]
pub fn run(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        if !session.trusted {
            return Err("Trust the project before you run it (11).".to_owned());
        }
        session.project.check().map_err(|e| e.to_string())?;
        editor.shared.control.set_stop(false);
        session.live = true;
        Ok(session.view(&editor.engine, Some(true)))
    })
}

/// Stops the live runs. The active run finishes (15.2).
#[tauri::command(async)]
pub fn pause(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        session.live = false;
        Ok(session.view(&editor.engine, None))
    })
}

/// Stops the live runs, and asks the active run to stop (15.2). **Run** removes the request.
#[tauri::command(async)]
pub fn stop(editor: State<'_, Editor>) -> Result<View, String> {
    editor.shared.control.set_stop(true);
    pause(editor)
}

/// The ID of the node that runs, or that ran last in the active run. None between runs (15.2).
#[tauri::command(async)]
pub fn running(editor: State<'_, Editor>) -> Option<String> {
    editor.shared.running()
}

/// The full text of the live value of output `from` ("<node id>.<output key>") after the last run
/// (15.7). A read never changes the run (9.2).
#[tauri::command(async)]
pub fn value(editor: State<'_, Editor>, from: String) -> Option<String> {
    engine::lock(&editor.shared.values).iter().find(|v| v.from == from).map(|v| v.text.clone())
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

/// The page calls this after it asked the user about the unsaved changes.
#[tauri::command(async)]
pub fn quit(app: AppHandle, editor: State<'_, Editor>) {
    editor.close();
    app.exit(0);
}

/// The result of the self-check goes to `<check folder>/check.json`. Then the editor stops.
#[tauri::command(async)]
pub fn check_done(app: AppHandle, editor: State<'_, Editor>, result: Value) {
    let Some(dir) = &editor.check else { return };
    let ok = result["ok"] == true;
    write_check(dir, &result);
    app.exit(if ok { 0 } else { 1 });
}

#[tauri::command(async)]
pub fn page_error(app: AppHandle, editor: State<'_, Editor>, message: String) {
    eprintln!("page error: {message}");
    if let Some(dir) = &editor.check {
        write_check(dir, &serde_json::json!({ "ok": false, "error": message }));
        app.exit(1);
    }
}

pub fn write_check(dir: &Path, result: &Value) {
    let text = serde_json::to_vec_pretty(result).unwrap_or_default();
    if let Err(e) = std::fs::write(dir.join("check.json"), text) {
        eprintln!("cannot write check.json: {e}");
    }
}

/// Writes a screenshot of the page to `<check folder>/<name>.png`, only in the self-check.
#[tauri::command(async)]
pub fn capture(window: WebviewWindow, editor: State<'_, Editor>, name: String) -> Result<(), String> {
    let Some(dir) = &editor.check else { return Err("capture runs only in the self-check".to_owned()) };
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("bad screenshot name {name}"));
    }
    #[cfg(windows)]
    {
        let png = crate::shot::capture(&window)?;
        std::fs::write(dir.join(format!("{name}.png")), png).map_err(|e| e.to_string())?;
    }
    // shortcut: screenshots work on Windows only; add a path for Linux when the check runs there.
    #[cfg(not(windows))]
    let _ = (window, dir);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn project_names_are_safe_folder_names() {
        for name in ["hello", "My project 2", "a-b_c.d"] {
            assert!(check_project_name(name).is_ok(), "{name}");
        }
        for name in ["", ".hidden", "a/b", "a\\b", "..", "end.", "end ", "c:x", "\u{e9}t\u{e9}"] {
            assert!(check_project_name(name).is_err(), "{name}");
        }
    }
}
