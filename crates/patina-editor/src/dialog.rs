//! The dialogs of the platform: the folder dialog (15.10), and the messages before the window
//! opens (13.1).

use std::path::PathBuf;

use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
use tauri::{State, WebviewWindow};

use crate::data::relative_path;
use crate::session::Editor;

/// Opens the folder dialog of the platform at `start`. Gives `None` when the user cancels.
// Not async: Tauri runs the command on the main thread, as the dialogs of macOS and GTK need.
#[tauri::command]
pub fn pick_folder(window: WebviewWindow, start: String) -> Option<PathBuf> {
    FileDialog::new().set_parent(&window).set_directory(start).pick_folder()
}

/// Opens the dialog for the **Browse** button of a file path widget (4.3). `kind` is `open`, `save`
/// or `folder`. The dialog starts at the folder of `start`. A path in the project comes back
/// relative to the project root, with `/`.
#[tauri::command]
pub fn pick_path(window: WebviewWindow, editor: State<'_, Editor>, kind: String, start: String) -> Result<Option<String>, String> {
    let root = editor.project_root()?;
    let start = root.join(&start);
    let folder = start.ancestors().find(|p| p.is_dir()).unwrap_or(root.as_path());
    let dialog = FileDialog::new().set_parent(&window).set_directory(folder);
    let picked = match kind.as_str() {
        "open" => dialog.pick_file(),
        "save" => match start.file_name().filter(|_| !start.is_dir()) {
            Some(name) => dialog.set_file_name(name.to_string_lossy()).save_file(),
            None => dialog.save_file(),
        },
        "folder" => dialog.pick_folder(),
        _ => return Err(format!("The path kind {kind} is not known.")),
    };
    Ok(picked.map(|path| relative_path(&root, &path)))
}

/// Shows an error in a message box of the platform.
pub fn error(text: &str) {
    message(text, MessageButtons::Ok);
}

/// Offers to open the download page of the WebView2 Runtime. The Runtime installs for the current
/// user (13.1).
#[cfg(windows)]
pub fn offer_webview() {
    let text = "Patina needs the Microsoft Edge WebView2 Runtime, and it is not installed.\n\n\
        Open the download page of Microsoft? The Runtime installs for the current user. Then start Patina again.";
    if message(text, MessageButtons::YesNo) == MessageDialogResult::Yes {
        let windows = std::env::var_os("SystemRoot").map_or(PathBuf::from(r"C:\Windows"), PathBuf::from);
        let url = "https://developer.microsoft.com/microsoft-edge/webview2/";
        let _ = std::process::Command::new(windows.join("explorer.exe")).arg(url).spawn();
    }
}

fn message(text: &str, buttons: MessageButtons) -> MessageDialogResult {
    MessageDialog::new().set_level(MessageLevel::Error).set_title("Patina").set_description(text).set_buttons(buttons).show()
}
