//! The commands of the start screen and of the settings screen (15.10).

use std::path::PathBuf;

use patina_engine::{Project, Toolchain, package};
use serde::Serialize;
use tauri::State;

use super::{Editor, View, folder_name, full_path};
use crate::data::same_path;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartInfo {
    recent: Vec<Recent>,
    projects_dir: PathBuf,
    check: Option<PathBuf>,
    shot: Option<PathBuf>,
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

#[tauri::command(async)]
pub fn start_info(editor: State<'_, Editor>) -> StartInfo {
    let recent = editor.data.recent().into_iter();
    StartInfo {
        recent: recent.map(|path| Recent { name: folder_name(&path), exists: path.join("patina.toml").is_file(), path }).collect(),
        projects_dir: editor.data.projects.clone(),
        check: editor.check.clone(),
        shot: editor.shot.clone(),
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
