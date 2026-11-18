//! The commands of the self-check (15.10).

use std::path::{Component, Path};

use serde_json::Value;
use tauri::{AppHandle, State, WebviewWindow};

use super::Editor;

/// The result of the self-check goes to `<check folder>/check.json`. Then the editor stops.
#[tauri::command(async)]
pub fn check_done(app: AppHandle, editor: State<'_, Editor>, result: Value) {
    let Some(dir) = &editor.check else { return };
    let ok = result["ok"] == true;
    write_check(dir, &result);
    app.exit(if ok { 0 } else { 1 });
}

/// Writes or, with no text, removes a file or folder of the open project, as a change from
/// outside the editor (15.10). Only the self-check can do it.
#[tauri::command(async)]
pub fn check_write(editor: State<'_, Editor>, path: String, text: Option<String>) -> Result<(), String> {
    if editor.check.is_none() || !Path::new(&path).components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(format!("The self-check can not write {path}."));
    }
    let file = editor.project_root()?.join(&path);
    let done = match text {
        Some(text) => file.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&file, text)),
        None if file.is_dir() => std::fs::remove_dir_all(&file),
        None => std::fs::remove_file(&file),
    };
    done.map_err(|e| format!("{path}: {e}"))
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
