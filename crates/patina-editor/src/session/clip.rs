//! The commands of copy, paste and the node libraries (6.2, 15.11).

use std::path::PathBuf;

use patina_engine::project::{self, ProjectFile};
use patina_engine::{NodeInfo, Pasted, Project, Toolchain};
use serde::Serialize;
use tauri::State;

use super::{Editor, View, folder_name, full_path};

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

/// The library folders (6.2).
#[tauri::command(async)]
pub fn libraries(editor: State<'_, Editor>) -> Vec<PathBuf> {
    editor.data.libraries()
}

/// Adds a Patina project to the libraries. Patina trusts it, because a paste builds its code (6.2, 11).
#[tauri::command(async)]
pub fn add_library(editor: State<'_, Editor>, path: PathBuf) -> Result<Vec<PathBuf>, String> {
    full_path(&path)?;
    if !path.join("patina.toml").is_file() {
        return Err(format!("{} is not a Patina project.", path.display()));
    }
    editor.data.trust(&path)?;
    editor.data.add_library(&path)?;
    Ok(editor.data.libraries())
}

/// Takes a folder off the libraries. The folder stays trusted (6.2).
#[tauri::command(async)]
pub fn remove_library(editor: State<'_, Editor>, path: PathBuf) -> Result<Vec<PathBuf>, String> {
    editor.data.remove_library(&path)?;
    Ok(editor.data.libraries())
}

/// The nodes and the subgraphs of the trusted libraries, except the open project (6.2).
#[tauri::command(async)]
pub fn library_nodes(editor: State<'_, Editor>) -> Vec<LibraryNode> {
    let open = editor.lock().as_ref().map(|s| s.project.root().to_owned());
    let mut list = Vec::new();
    for path in editor.data.libraries() {
        if !editor.data.is_trusted(&path) || open.as_deref().is_some_and(|root| crate::data::same_path(root, &path)) {
            continue;
        }
        let Ok(library) = Project::open(&path) else { continue };
        let name = folder_name(&path);
        let infos = library.nodes().unwrap_or_default().into_iter().chain(library.subgraphs());
        list.extend(infos.map(|info| LibraryNode { library: name.clone(), path: path.clone(), info }));
    }
    list
}
