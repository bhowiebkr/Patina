//! The commands of copy, paste and the node libraries (6.2, 15.11).

use std::fs;
use std::path::{Path, PathBuf};

use patina_engine::git::{self, Check};
use patina_engine::project::{self, ProjectFile};
use patina_engine::{NodeInfo, Pasted, Project, Toolchain};
use serde::Serialize;
use tauri::State;

use super::{Editor, View, folder_name, full_path};
use crate::data::{Data, Library};

/// The view after a paste, with the IDs of the new nodes and the warnings (15.11).
#[derive(Serialize)]
pub struct PastedView {
    view: View,
    #[serde(flatten)]
    pasted: Pasted,
}

/// A node or a subgraph of a library, for the node palette (6.2).
#[derive(Serialize)]
pub struct LibraryNode {
    library: String,
    path: PathBuf,
    #[serde(flatten)]
    info: NodeInfo,
}

/// The clip text of the selected nodes, for the clipboard (15.11).
#[tauri::command(async)]
pub fn copy_clip(editor: State<'_, Editor>, ids: Vec<String>) -> Result<String, String> {
    editor.with(|session| session.project.copy(&ids))
}

/// Pastes a clip at the graph position `at` (15.11).
#[tauri::command(async)]
pub fn paste_clip(editor: State<'_, Editor>, text: String, at: [f64; 2]) -> Result<PastedView, String> {
    paste(&editor, &text, at)
}

/// Adds the node or the subgraph `name` of a library, as a paste of one node (6.2).
#[tauri::command(async)]
pub fn paste_library(editor: State<'_, Editor>, path: PathBuf, name: String, subgraph: bool, at: [f64; 2]) -> Result<PastedView, String> {
    if !editor.data.libraries().iter().any(|p| crate::data::same_path(p, &path)) || !editor.data.is_trusted(&path) {
        return Err(format!("Add {} to the libraries in Settings first.", path.display()));
    }
    let text = Project::open(&path).map_err(|e| e.to_string())?.library_clip(&name, subgraph)?;
    paste(&editor, &text, at)
}

fn paste(editor: &Editor, text: &str, at: [f64; 2]) -> Result<PastedView, String> {
    let toolchain = Toolchain::find(&editor.data.toolchain).ok();
    let mut pasted = None;
    let view = editor.change(|project| {
        pasted = Some(project.paste(text, at, toolchain.as_ref(), |id| trusted_by_id(editor, id))?);
        Ok(())
    })?;
    let pasted = pasted.ok_or_else(|| "The paste did not run.".to_owned())?;
    Ok(PastedView { view, pasted })
}

/// The trusted project folder with the project ID `id`, or None (6.2).
pub(super) fn trusted_by_id(editor: &Editor, id: &str) -> Option<PathBuf> {
    let id_of = |dir: &PathBuf| project::read(&dir.join("patina.toml"), |f: &ProjectFile| f.format).ok().map(|f: ProjectFile| f.id);
    editor.data.trusted().into_iter().find(|dir| id_of(dir).as_deref() == Some(id))
}

/// The library folders, with the pins of the copies from Git (6.2).
#[tauri::command(async)]
pub fn libraries(editor: State<'_, Editor>) -> Vec<Library> {
    editor.data.library_pins()
}

/// Adds a Patina project to the libraries. Patina trusts it, because a paste builds its code (6.2, 11).
#[tauri::command(async)]
pub fn add_library(editor: State<'_, Editor>, path: PathBuf) -> Result<Vec<Library>, String> {
    full_path(&path)?;
    if !path.join("patina.toml").is_file() {
        return Err(format!("{} is not a Patina project.", path.display()));
    }
    editor.data.trust(&path)?;
    editor.data.add_library(&path)?;
    Ok(editor.data.library_pins())
}

/// Takes a folder off the libraries. A local folder stays trusted. A copy from Git is deleted (6.2).
#[tauri::command(async)]
pub fn remove_library(editor: State<'_, Editor>, path: PathBuf) -> Result<Vec<Library>, String> {
    let data = &editor.data;
    if data.pin(&path).is_some() {
        delete_copy(&editor, &path)?;
    }
    data.remove_library(&path)?;
    Ok(data.library_pins())
}

/// Downloads a tag of a Git repository into the data folder, and adds the copy to the libraries (6.2).
#[tauri::command(async)]
pub fn add_git_library(editor: State<'_, Editor>, url: String, tag: String) -> Result<Vec<Library>, String> {
    let data = &editor.data;
    let (folder, pin) = git::download(&url, &tag, &data.git_dir()).map_err(|e| e.to_string())?;
    data.set_pin(&folder, Some(pin))?;
    data.trust(&folder)?;
    data.add_library(&folder)?;
    Ok(data.library_pins())
}

/// The result of **Check for updates** for a library from Git (6.2).
#[derive(Serialize)]
pub struct Checked {
    path: PathBuf,
    #[serde(flatten)]
    check: Option<Check>,
    error: Option<String>,
}

/// Reads the tags of each library from Git again: a newer tag, or a tag that moved (6.2).
#[tauri::command(async)]
pub fn check_libraries(editor: State<'_, Editor>) -> Vec<Checked> {
    let work = editor.data.git_dir().join(".check");
    let mut list = Vec::new();
    for Library { path, pin } in editor.data.library_pins() {
        let Some(pin) = pin else { continue };
        let (check, error) = match git::check(&pin, &work) {
            Ok(check) => (Some(check), None),
            Err(e) => (None, Some(e.to_string())),
        };
        list.push(Checked { path, check, error });
    }
    let _ = fs::remove_dir_all(&work);
    list
}

/// Downloads the index of the shared libraries (6.2).
#[tauri::command(async)]
pub fn library_index(editor: State<'_, Editor>) -> Result<Vec<git::Entry>, String> {
    let work = editor.data.git_dir().join(".index");
    let list = git::index(&work).map_err(|e| e.to_string());
    let _ = fs::remove_dir_all(&work);
    list
}

/// Replaces a copy from Git with another tag of its repository, at the same place in the list (6.2).
#[tauri::command(async)]
pub fn update_library(editor: State<'_, Editor>, path: PathBuf, tag: String) -> Result<Vec<Library>, String> {
    let data = &editor.data;
    let old = data.pin(&path).ok_or_else(|| format!("{} is not a library from Git.", path.display()))?;
    let (folder, pin) = git::download(&old.url, &tag, &data.git_dir()).map_err(|e| e.to_string())?;
    data.set_pin(&folder, Some(pin))?;
    data.trust(&folder)?;
    data.replace_library(&path, &folder)?;
    delete_copy(&editor, &path)?;
    Ok(data.library_pins())
}

/// Deletes a copy from Git with its pin and its trust. The open project is never deleted.
fn delete_copy(editor: &Editor, path: &Path) -> Result<(), String> {
    if editor.lock().as_ref().is_some_and(|s| crate::data::same_path(s.project.root(), path)) {
        return Err(format!("{} is open. Close it first.", path.display()));
    }
    match fs::remove_dir_all(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("{}: {e}", path.display())),
        _ => {}
    }
    editor.data.set_pin(path, None)?;
    editor.data.untrust(path)
}

/// The nodes and the subgraphs of the trusted libraries, except the open project (6.2).
#[tauri::command(async)]
pub fn library_nodes(editor: State<'_, Editor>) -> Vec<LibraryNode> {
    let open = editor.lock().as_ref().map(|s| s.project.root().to_owned());
    library_list(&editor.data, open.as_deref())
}

fn library_list(data: &Data, open: Option<&Path>) -> Vec<LibraryNode> {
    let mut list = Vec::new();
    for path in data.libraries() {
        if !data.is_trusted(&path) || open.is_some_and(|root| crate::data::same_path(root, &path)) {
            continue;
        }
        let Ok(library) = Project::open(&path) else { continue };
        let name = folder_name(&path);
        let infos = library.nodes().unwrap_or_default().into_iter().chain(library.subgraphs());
        list.extend(infos.map(|info| LibraryNode { library: name.clone(), path: path.clone(), info }));
    }
    list
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn the_palette_lists_only_trusted_libraries_that_are_not_open() {
        let dir = std::env::temp_dir().join(format!("patina-libraries-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let data = Data::find(Some(dir.join("user"))).unwrap();
        let (lib, other) = (dir.join("lib"), dir.join("other"));
        for root in [&lib, &other] {
            Project::create(root, "1.94.0").unwrap();
            data.add_library(root).unwrap();
        }
        fs::write(lib.join("graphs/half.toml"), "format = 1
").unwrap();
        data.trust(&lib).unwrap();
        let list = library_list(&data, None);
        let open = library_list(&data, Some(&lib));
        fs::remove_dir_all(&dir).unwrap();
        assert!(!list.is_empty() && list.iter().all(|n| n.path == lib && n.library == "lib"), "the other library is not trusted");
        assert!(list.iter().any(|n| n.info.subgraph && n.info.name == "half"));
        assert!(list.iter().any(|n| !n.info.subgraph));
        assert!(open.is_empty(), "the open project is not a library of itself");
    }
}
