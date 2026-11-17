//! The **Install toolchain** and **Update toolchain** buttons (13.2). The install runs on its own
//! thread and sends each step to the page.

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::session::Editor;

/// True while an install or an update runs. Only one runs at a time.
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
    let folder = editor.data.toolchain.clone();
    start(app, move |step| {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        patina_engine::install::install(&folder, &exe, step).map_err(|e| e.to_string())
    })
}

/// Starts the update to the newest stable Rust, with the same events as the install.
#[tauri::command]
pub fn update_toolchain(app: AppHandle, editor: State<'_, Editor>) -> Result<(), String> {
    let folder = editor.data.toolchain.clone();
    start(app, move |step| patina_engine::install::update(&folder, step).map_err(|e| e.to_string()))
}

fn start(app: AppHandle, work: impl FnOnce(&mut dyn FnMut(&str)) -> Result<String, String> + Send + 'static) -> Result<(), String> {
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err("The toolchain install runs.".to_owned());
    }
    std::thread::spawn(move || {
        let mut last = String::new();
        let result = work(&mut |name: &str| {
            last = name.to_owned();
            let _ = app.emit("toolchain", Progress { step: last.clone(), done: false, error: None });
        });
        INSTALLING.store(false, Ordering::SeqCst);
        let _ = app.emit("toolchain", Progress { step: last, done: true, error: result.err() });
    });
    Ok(())
}
