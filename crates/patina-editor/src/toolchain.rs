//! The **Install toolchain** button (13.2). The install runs on its own thread and sends each
//! step to the page.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::session::Editor;

/// True while an install runs. Only one install runs at a time.
static INSTALLING: AtomicBool = AtomicBool::new(false);

pub fn installing() -> bool {
    INSTALLING.load(Ordering::SeqCst)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    step: String,
    done: bool,
    error: Option<String>,
}

/// Starts the install. The page gets a `toolchain` event for each step, and a last event with
/// `done`. The editor program becomes the Zig wrappers (`patina_engine::install::run_wrapper`).
#[tauri::command]
pub fn install_toolchain(app: AppHandle, editor: State<'_, Editor>) -> Result<(), String> {
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err("The toolchain install runs.".to_owned());
    }
    let folder = editor.data.toolchain.clone();
    std::thread::spawn(move || {
        let mut last = String::new();
        let result = std::env::current_exe().map_err(|e| e.to_string()).and_then(|exe| {
            let mut step = |name: &str| {
                last = name.to_owned();
                let _ = app.emit("toolchain", Progress { step: last.clone(), done: false, error: None });
            };
            patina_engine::install::install(&folder, &exe, &mut step).map_err(|e| e.to_string())
        });
        INSTALLING.store(false, Ordering::SeqCst);
        let _ = app.emit("toolchain", Progress { step: last, done: true, error: result.err() });
    });
    Ok(())
}
