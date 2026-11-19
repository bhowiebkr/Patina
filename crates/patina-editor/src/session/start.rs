//! The commands of the start screen and of the settings screen (15.10).

use std::path::{Path, PathBuf};

use patina_engine::{Project, Toolchain, package};
use serde::Serialize;
use tauri::State;

use super::{Editor, View, folder_name, full_path};
use crate::data::same_path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartInfo {
    recent: Vec<Recent>,
    /// The included workbenches, sorted by name.
    workbenches: Vec<Recent>,
    projects_dir: PathBuf,
    check: Option<PathBuf>,
    shot: Option<PathBuf>,
    /// `patina --check <dir> --bench` runs the canvas bench in place of the self-check.
    bench: bool,
}

/// The settings screen (15.10): the toolchain, the folders and the trusted projects.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// The Rust version, or none when Patina finds no toolchain.
    rust: Option<String>,
    cargo: Option<PathBuf>,
    toolchain_problem: Option<String>,
    /// The toolchain folder of Patina (13.3), and true when the toolchain comes from it.
    toolchain_dir: PathBuf,
    own_toolchain: bool,
    /// True while the **Install toolchain** button runs.
    installing: bool,
    data_dir: PathBuf,
    projects_dir: PathBuf,
    portable: bool,
    trusted: Vec<PathBuf>,
    libraries: Vec<crate::data::Library>,
}

#[derive(Serialize)]
pub struct Recent {
    path: PathBuf,
    name: String,
    exists: bool,
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

/// The project folders in `dir`, sorted by name. A missing folder gives an empty list.
fn workbenches(dir: &Path) -> Vec<Recent> {
    let folders = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|entry| entry.path());
    let mut list: Vec<_> = folders.filter(|path| path.join("patina.toml").is_file()).map(|path| Recent { name: folder_name(&path), exists: true, path }).collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

#[tauri::command(async)]
pub fn start_info(editor: State<'_, Editor>) -> StartInfo {
    let recent = editor.data.recent().into_iter();
    StartInfo {
        recent: recent.map(|path| Recent { name: folder_name(&path), exists: path.join("patina.toml").is_file(), path }).collect(),
        workbenches: workbenches(&editor.data.workbenches),
        projects_dir: editor.data.projects.clone(),
        check: editor.check.clone(),
        shot: editor.shot.clone(),
        bench: editor.check.is_some() && std::env::args_os().any(|arg| arg == "--bench"),
    }
}

#[tauri::command(async)]
pub fn settings(editor: State<'_, Editor>) -> Settings {
    let data = &editor.data;
    let (rust, cargo, own_toolchain, toolchain_problem) = match Toolchain::find(&data.toolchain) {
        Ok(t) => (Some(t.release), Some(t.cargo), t.own.is_some(), None),
        Err(e) => (None, None, false, Some(e.to_string())),
    };
    Settings { rust, cargo, toolchain_problem, toolchain_dir: data.toolchain.clone(), own_toolchain, installing: crate::toolchain::installing(), data_dir: data.dir.clone(), projects_dir: data.projects.clone(), portable: data.portable, trusted: data.trusted(), libraries: data.library_pins() }
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
    let rust = Toolchain::find(&editor.data.toolchain).map_err(|e| e.to_string())?.release;
    let project = Project::create(&parent.join(&name), &rust).map_err(|e| e.to_string())?;
    // The user made the project, so its code is the code of the user (11).
    editor.data.trust(project.root())?;
    editor.open(project)
}

#[tauri::command(async)]
pub fn open_project(editor: State<'_, Editor>, path: PathBuf) -> Result<View, String> {
    full_path(&path)?;
    // A package opens as its original project (14.1).
    let path = if path.join(package::HEADER).is_file() { path.join("project") } else { path };
    editor.open(Project::open(&path).map_err(|e| e.to_string())?)
}

#[tauri::command(async)]
pub fn close_project(editor: State<'_, Editor>) {
    editor.close();
}

/// Takes a project off the recent list. The folder stays (15.10).
#[tauri::command(async)]
pub fn forget_project(editor: State<'_, Editor>, path: PathBuf) -> Result<StartInfo, String> {
    editor.data.remove_recent(&path)?;
    Ok(start_info(editor))
}

/// Moves a recent project to the Recycle Bin, and takes it off the recent and trusted lists, so
/// that a new folder at the same path is not trusted (11, 15.10).
#[tauri::command(async)]
pub fn delete_project(editor: State<'_, Editor>, path: PathBuf) -> Result<StartInfo, String> {
    deletable(&editor.data.recent(), &path)?;
    if editor.lock().is_some() {
        return Err("Close the project first.".to_owned());
    }
    // No process of Patina may keep a file of the project open (16.1).
    editor.analyzer.stop();
    editor.engine.end();
    recycle(&path)?;
    editor.data.remove_recent(&path)?;
    editor.data.untrust(&path)?;
    Ok(start_info(editor))
}

/// Only a project of the recent list can go, so the page cannot delete another folder.
fn deletable(recent: &[PathBuf], path: &Path) -> Result<(), String> {
    if !recent.iter().any(|r| r == path) {
        return Err(format!("{} is not a recent project.", path.display()));
    }
    if !path.is_absolute() || !path.join("patina.toml").is_file() {
        return Err(format!("{} is not a Patina project.", path.display()));
    }
    Ok(())
}

#[cfg(windows)]
pub(super) use crate::recycle::recycle;

// shortcut: no trash on Linux and macOS, add the freedesktop trash when a user asks.
#[cfg(not(windows))]
pub(super) fn recycle(_: &Path) -> Result<(), String> {
    Err("Patina can move a folder to the trash only on Windows. Delete the folder in the file manager.".to_owned())
}

#[tauri::command(async)]
pub fn trust_project(editor: State<'_, Editor>) -> Result<View, String> {
    editor.with(|session| {
        editor.data.trust(session.project.root())?;
        session.trusted = true;
        let view = session.view(&editor.engine, None, None);
        session.load(&editor.engine);
        Ok(view)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn only_a_recent_project_can_go() {
        let dir = std::env::temp_dir().join(format!("patina-deletable-{}", std::process::id()));
        let other = dir.join("other");
        fs::create_dir_all(&other).unwrap();
        fs::write(dir.join("patina.toml"), "").unwrap();
        fs::write(other.join("patina.toml"), "").unwrap();
        let recent = [dir.clone(), dir.join("missing")];
        let results = [deletable(&recent, &dir), deletable(&recent, &other), deletable(&recent, &dir.join("missing"))];
        fs::remove_dir_all(&dir).unwrap();
        assert!(results[0].is_ok());
        assert!(results[1].as_ref().unwrap_err().contains("not a recent project"));
        assert!(results[2].as_ref().unwrap_err().contains("not a Patina project"));
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

    #[test]
    fn the_workbench_list_has_only_project_folders() {
        let dir = std::env::temp_dir().join(format!("patina-workbenches-{}", std::process::id()));
        for name in ["synth", "hello", "notes"] {
            fs::create_dir_all(dir.join(name)).unwrap();
        }
        fs::write(dir.join("synth/patina.toml"), "").unwrap();
        fs::write(dir.join("hello/patina.toml"), "").unwrap();
        let names: Vec<_> = workbenches(&dir).into_iter().map(|w| w.name).collect();
        assert_eq!(names, ["hello", "synth"]);
        assert!(workbenches(&dir.join("missing")).is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }
}
