//! The commands of **Import**, **Update** and the package (6.2, 14.1).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use patina_engine::{Toolchain, import};
use serde::Serialize;
use tauri::State;

use super::{Editor, View, folder_name};
use crate::engine::Job;
use crate::node_process::ToNode;

/// The folders of another project that **Import** can copy (6.2).
#[tauri::command(async)]
pub fn import_folders(editor: State<'_, Editor>, other: PathBuf) -> Result<Vec<String>, String> {
    import_source(&editor, &other)?;
    Ok(import::folders(&other))
}

/// The folders that an import of `folder` copies, with the conflicts (6.2).
#[tauri::command(async)]
pub fn import_plan(editor: State<'_, Editor>, other: PathBuf, folder: String) -> Result<Vec<import::Item>, String> {
    let root = import_source(&editor, &other)?;
    import::plan(&root, &other, &folder)
}

/// Imports a folder with the choices of the user (6.2). The project stays locked during the
/// build check, so no edit comes between the copy and an undo. Undo does not remove the copy.
#[tauri::command(async)]
pub fn import_folder(editor: State<'_, Editor>, other: PathBuf, folder: String, choices: BTreeMap<String, import::Choice>) -> Result<View, String> {
    import_source(&editor, &other)?;
    let toolchain = Toolchain::find(&editor.data.toolchain).map_err(|e| e.to_string())?;
    editor.change(|project, _| project.import(&toolchain, &other, &folder, &choices))
}

/// The trusted project that the imported `folder` came from, or None when Patina does not know
/// its folder (6.2).
#[tauri::command(async)]
pub fn update_source(editor: State<'_, Editor>, folder: String) -> Result<Option<PathBuf>, String> {
    let id = editor.with(|session| import::source_id(session.project.root(), &folder))?;
    Ok(super::clip::trusted_by_id(&editor, &id))
}

/// The differences between the imported `folder` and its folder in the project at `other` (6.2).
#[tauri::command(async)]
pub fn update_changes(editor: State<'_, Editor>, other: PathBuf, folder: String) -> Result<import::Changes, String> {
    let root = import_source(&editor, &other)?;
    import::changes(&root, &other, &folder)
}

/// Updates the imported `folder` with the picks of the user, and builds it first (6.2).
#[tauri::command(async)]
pub fn update_folder(editor: State<'_, Editor>, other: PathBuf, folder: String, picks: BTreeMap<String, import::Pick>) -> Result<View, String> {
    import_source(&editor, &other)?;
    let toolchain = Toolchain::find(&editor.data.toolchain).map_err(|e| e.to_string())?;
    editor.change(|project, _| project.update(&toolchain, &other, &folder, &picks))
}

/// The package folder and the warnings of a package build (14.1).
#[derive(Serialize)]
pub struct Packaged {
    out: PathBuf,
    warnings: Vec<String>,
}

/// Makes a package of the top graph of the saved project in `<parent>/<project name>` (14.1). The
/// page saves first.
#[tauri::command(async)]
pub fn make_package(editor: State<'_, Editor>, parent: PathBuf) -> Result<Packaged, String> {
    let root = editor.trusted_root("make a package")?;
    // A package builds from the files, and only Save writes the files (15.10).
    if editor.dirty() {
        return Err("Save the changes before you make a package.".to_owned());
    }
    let out = parent.join(folder_name(&root));
    let graph = editor.with(|session| Ok(session.project.top().to_owned()))?;
    let warnings = editor.engine.ask(true, |ask| ToNode::Job(Job::Package { root, graph, out: out.clone(), ask }))?;
    Ok(Packaged { out, warnings })
}

/// The root of the open project. Patina imports only from a trusted project into a trusted
/// project (11), because the build check compiles the copy.
fn import_source(editor: &Editor, other: &Path) -> Result<PathBuf, String> {
    if !editor.data.is_trusted(other) {
        return Err("Open the other project and trust it before you import from it.".to_owned());
    }
    editor.trusted_root("change it")
}
