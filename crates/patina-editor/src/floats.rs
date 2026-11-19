//! The float windows of the view panels (15.8). Each float window shows one view slot. The page of
//! the editor window says which slots float, and this module opens and closes their windows.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, Window, WindowEvent};

use crate::session::Editor;

const PREFIX: &str = "float-";
/// The size of a new float window, in logical pixels.
const SIZE: [f64; 2] = [480.0, 640.0];

/// The place and the size of a float window on the screen, in logical pixels.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// A view slot that floats.
#[derive(Deserialize)]
pub struct Float {
    slot: String,
    /// The view folder, for example `views/scene`.
    name: String,
    title: String,
    /// The last place of the window, or none for a new window.
    rect: Option<Rect>,
}

#[derive(Clone, Serialize)]
struct Moved<'a> {
    slot: &'a str,
    rect: Rect,
}

/// Opens a window for each float, and closes each other float window.
#[tauri::command(async)]
pub fn set_floats(app: AppHandle, editor: State<'_, Editor>, floats: Vec<Float>) -> Result<(), String> {
    if let Some(bad) = floats.iter().find(|f| !matches!(f.slot.as_str(), "view" | "view2" | "view3" | "view4" | "view5" | "view6" | "view7" | "view8" | "view9" | "view10" | "view11" | "view12")) {
        return Err(format!("{} is not a view slot.", bad.slot));
    }
    for (label, window) in app.webview_windows() {
        let Some(slot) = label.strip_prefix(PREFIX) else { continue };
        match floats.iter().find(|f| f.slot == slot) {
            Some(float) => window.set_title(&float.title).map_err(|e| e.to_string())?,
            None => close(&editor, &window, slot)?,
        }
    }
    for float in &floats {
        if app.get_webview_window(&format!("{PREFIX}{}", float.slot)).is_none() {
            open(&app, &editor, float).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn close(editor: &Editor, window: &WebviewWindow, slot: &str) -> Result<(), String> {
    editor.drop_window_view(window.label(), slot)?;
    window.destroy().map_err(|e| e.to_string())
}

fn open(app: &AppHandle, editor: &Editor, float: &Float) -> tauri::Result<()> {
    let mut url = crate::page_url();
    url.query_pairs_mut().append_pair("float", &float.slot).append_pair("name", &float.name);
    let check = editor.check.is_some();
    let builder = crate::window(app, &format!("{PREFIX}{}", float.slot), url, editor.data.webview.clone(), check).title(&float.title);
    if check {
        builder.build()?;
        return Ok(());
    }
    let rect = float.rect.filter(|r| on_screen(app, r));
    let builder = builder.inner_size(rect.map_or(SIZE[0], |r| r.w), rect.map_or(SIZE[1], |r| r.h));
    match rect {
        Some(r) => builder.position(r.x, r.y).build()?,
        None => builder.center().build()?,
    };
    Ok(())
}

/// True when the top of the window is on a screen, so the user can drag it.
fn on_screen(app: &AppHandle, r: &Rect) -> bool {
    let monitors = app.available_monitors().unwrap_or_default();
    monitors.iter().any(|m| {
        let (p, s) = (m.position(), m.size());
        inside(r, [p.x as f64, p.y as f64, s.width as f64, s.height as f64], m.scale_factor())
    })
}

/// True when a point near the top left of `r` is in the screen area `screen` (physical pixels).
fn inside(r: &Rect, [x, y, w, h]: [f64; 4], scale: f64) -> bool {
    let (px, py) = ((r.x + 40.0) * scale, (r.y + 10.0) * scale);
    px >= x && px < x + w && py >= y && py < y + h
}

/// The window events of the float windows. A close asks the editor window, which drops the view
/// first. A move or a size change goes into the layout.
pub fn window_event(window: &Window, event: &WindowEvent) {
    let Some(slot) = window.label().strip_prefix(PREFIX) else { return };
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.emit("float-closed", slot);
        }
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            let rect = || -> tauri::Result<Rect> {
                let scale = window.scale_factor()?;
                let (p, s) = (window.outer_position()?.to_logical::<f64>(scale), window.inner_size()?.to_logical::<f64>(scale));
                Ok(Rect { x: p.x, y: p.y, w: s.width, h: s.height })
            };
            // A minimized window has no useful place.
            if let Some(rect) = rect().ok().filter(|r| r.w > 0.0 && r.h > 0.0) {
                let _ = window.emit("float-moved", Moved { slot, rect });
            }
        }
        WindowEvent::Destroyed => window.state::<Editor>().window_gone(window.label()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_shows_only_when_its_top_is_on_a_screen() {
        let r = |x, y| Rect { x, y, w: 400.0, h: 300.0 };
        let screen = [0.0, 0.0, 1920.0, 1080.0];
        assert!(inside(&r(100.0, 100.0), screen, 1.0));
        assert!(!inside(&r(-20000.0, 0.0), screen, 1.0));
        assert!(!inside(&r(100.0, 1075.0), screen, 1.0));
        // At 150 %, logical 1000 is physical 1500.
        assert!(inside(&r(1000.0, 100.0), [1440.0, 0.0, 1920.0, 1080.0], 1.5));
    }
}
