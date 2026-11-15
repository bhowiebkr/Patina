//! The open project and the commands of the page (spec 15.10).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard};

use patina_engine::project::{GraphFile, NodeEntry, PublicInput, PublicOutput, Wire};
use patina_engine::{Edit, Error, NodeInfo, Project, Toolchain};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State, WebviewWindow};

use crate::data::Data;
use crate::engine::{Job, Run};

pub struct Editor {
    pub data: Data,
    pub engine: Sender<Job>,
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
    /// True when the Code tab has changes that are not saved (15.10).
    unsaved_code: bool,
}

/// All that the page shows of the open project.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    serial: u64,
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartInfo {
    recent: Vec<Recent>,
    projects_dir: PathBuf,
    check: Option<PathBuf>,
}

#[derive(Serialize)]
pub struct Recent {
    path: PathBuf,
    name: String,
    exists: bool,
}

impl Editor {
    pub fn new(data: Data, engine: Sender<Job>, check: Option<PathBuf>) -> Editor {
        Editor { data, engine, session: Mutex::new(None), check, serial: AtomicU64::new(0) }
    }

    /// True when the open project or the Code tab has changes that are not saved.
    pub fn dirty(&self) -> bool {
        self.lock().as_ref().is_some_and(|s| s.unsaved_code || s.project.dirty())
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
        let _ = self.engine.send(Job::Unload);
        let mut session = self.lock();
        // A project opens paused (15.10).
        Ok(session.insert(Session { project, serial, trusted, live: false, unsaved_code: false }).view(&self.engine, None))
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
            let built = (session.project.code_revision() != code).then_some(true);
            let build = old.and_then(|old| built.or(affects_run(&old, session.project.graph())));
            Ok(session.view(&self.engine, build))
        })
    }
}

impl Session {
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
        let (nodes, problems) = match project.nodes() {
            Ok(nodes) => (nodes, problems),
            Err(e) => (Vec::new(), [e.to_string()].into_iter().chain(problems).collect()),
        };
        View {
            serial: self.serial,
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
    *editor.lock() = None;
    let _ = editor.engine.send(Job::Unload);
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
        Ok(session.view(&editor.engine, None))
    })
}

/// The files of a node folder for the Code tab (15.4).
#[tauri::command(async)]
pub fn node_files(editor: State<'_, Editor>, name: String) -> Result<Vec<String>, String> {
    editor.with(|session| session.project.node_files(&name).map_err(|e| e.to_string()))
}

/// The page tells when the Code tab has unsaved changes, so a close of the window asks first.
#[tauri::command(async)]
pub fn set_unsaved_code(editor: State<'_, Editor>, unsaved: bool) -> Result<(), String> {
    editor.with(|session| {
        session.unsaved_code = unsaved;
        Ok(())
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
        session.live = true;
        Ok(session.view(&editor.engine, Some(true)))
    })
}

/// Stops the live runs. For a batch graph, **Stop** does the same (Q10).
#[tauri::command(async)]
pub fn pause(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        session.live = false;
        Ok(session.view(&editor.engine, None))
    })
}

/// The page calls this after it asked the user about the unsaved changes.
#[tauri::command(async)]
pub fn quit(app: AppHandle) {
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
