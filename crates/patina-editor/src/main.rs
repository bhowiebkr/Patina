//! The Patina editor (spec 15). `patina` opens the start screen.
//!
//! `patina --portable <dir>` keeps the data in `<dir>/data` (13.3). `patina --check <dir>` runs
//! the self-check in a hidden window: it makes the project `<dir>/check`, edits, runs, saves and
//! opens it again. Then it writes `<dir>/check.json` and `<dir>/check.png`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod data;
mod engine;
mod output;
mod session;
#[cfg(windows)]
mod shot;

use std::borrow::Cow;
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use tauri::http::header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE};
use tauri::http::{Request, Response, StatusCode};
use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use data::Data;
use session::Editor;

#[cfg(windows)]
const ORIGIN: &str = "http://patina.localhost";
#[cfg(not(windows))]
const ORIGIN: &str = "patina://localhost";

/// The wry defaults, and the flags that keep an off-screen window rendering.
const CHECK_BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion \
    --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows";

/// The page loads only its own files and talks only to the Rust side.
const CSP: &str = "default-src 'self'; connect-src 'self' ipc: http://ipc.localhost; style-src 'self' 'unsafe-inline'; img-src 'self' data:";

/// The self-check stops after this time, also when the page does not answer.
const CHECK_LIMIT: Duration = Duration::from_secs(180);

#[derive(Serialize, Clone)]
struct OutputText {
    text: String,
    err: bool,
}

struct Args {
    portable: Option<PathBuf>,
    check: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args { portable: None, check: None };
    let mut list = std::env::args_os().skip(1);
    while let Some(arg) = list.next() {
        let name = arg.to_string_lossy().into_owned();
        let mut folder = || {
            let path = PathBuf::from(list.next().ok_or(format!("{name} needs a folder"))?);
            std::path::absolute(&path).map_err(|e| format!("{}: {e}", path.display()))
        };
        match name.as_str() {
            "--portable" => args.portable = Some(folder()?),
            "--check" => args.check = Some(folder()?),
            _ => return Err(format!("unknown argument {name}")),
        }
    }
    Ok(args)
}

// shortcut: a fatal error before the window shows only on a console; add a message box at milestone 3.
fn fatal(text: &str) -> ! {
    eprintln!("patina: {text}");
    std::process::exit(1);
}

// The files of `frontend/dist`, from build.rs. The list is empty when the page was not built.
include!(concat!(env!("OUT_DIR"), "/page.rs"));

fn page_file(path: &str) -> Option<&'static [u8]> {
    PAGE.iter().find(|(name, _)| *name == path).map(|(_, body)| *body)
}

/// Accepts only plain relative paths, the names of the page files.
fn safe_path(path: &str) -> Option<&str> {
    let path = path.strip_prefix('/').unwrap_or(path);
    let path = if path.is_empty() { "index.html" } else { path };
    let bad = |part: &str| part.is_empty() || part == "." || part == ".." || part.contains([':', '\\']);
    if path.split('/').any(bad) {
        return None;
    }
    Some(path)
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html",
        Some("js") => "text/javascript",
        Some("css") => "text/css",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
}

fn serve(request: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let reply = |status: StatusCode, kind: &str, body: &'static [u8]| {
        let reply = Response::builder().status(status).header(CONTENT_TYPE, kind).header(CONTENT_SECURITY_POLICY, CSP);
        reply.body(Cow::Borrowed(body)).unwrap()
    };
    let Some(path) = safe_path(request.uri().path()) else {
        return reply(StatusCode::FORBIDDEN, "text/plain", &[]);
    };
    match page_file(path) {
        Some(body) => reply(StatusCode::OK, content_type(path), body),
        None => reply(StatusCode::NOT_FOUND, "text/plain", &[]),
    }
}

fn build_window(app: &AppHandle, webview_dir: PathBuf, check: bool) -> tauri::Result<WebviewWindow> {
    let url = format!("{ORIGIN}/index.html").parse().expect("the page URL is valid");
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::CustomProtocol(url))
        .title("Patina")
        .data_directory(webview_dir)
        .inner_size(1440.0, 900.0)
        .background_color(Color(0x14, 0x14, 0x14, 0xff));
    let builder = if check {
        // Off-screen and not focused: the check must not take the desktop (AGENTS.md).
        builder
            .position(-20000.0, 0.0)
            .focused(false)
            .focusable(false)
            .skip_taskbar(true)
            .decorations(false)
            .additional_browser_args(CHECK_BROWSER_ARGS)
    } else {
        builder.maximized(true)
    };
    builder.build()
}

/// Writes a failed result and stops the editor when the self-check takes too long.
fn limit_check(dir: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(CHECK_LIMIT);
        let text = format!("the self-check took more than {} s", CHECK_LIMIT.as_secs());
        session::write_check(&dir, &serde_json::json!({ "ok": false, "error": text }));
        std::process::exit(1);
    });
}

fn main() {
    let args = parse_args().unwrap_or_else(|e| fatal(&e));
    if page_file("index.html").is_none() {
        fatal("the page is not built. Run `npm run build` in crates/patina-editor/frontend, then build the editor again.");
    }
    if let Some(dir) = &args.check {
        // A new project folder each time, so the check never writes over a project.
        if dir.join("check").exists() {
            fatal(&format!("{} exists. Give the check a new folder.", dir.join("check").display()));
        }
        std::fs::create_dir_all(dir).unwrap_or_else(|e| fatal(&format!("{}: {e}", dir.display())));
    }
    let data = Data::find(args.portable.or_else(|| args.check.clone())).unwrap_or_else(|e| fatal(&e));
    let check = args.check;
    tauri::Builder::default()
        .register_uri_scheme_protocol("patina", |_ctx, request| serve(&request))
        .invoke_handler(tauri::generate_handler![
            session::start_info,
            session::new_project,
            session::open_project,
            session::close_project,
            session::trust_project,
            session::settings,
            session::untrust_project,
            session::edit,
            session::undo,
            session::redo,
            session::save,
            session::node_files,
            session::new_node,
            session::add_crate,
            session::read_code,
            session::set_unsaved_code,
            session::recover,
            session::run,
            session::pause,
            session::stop,
            session::running,
            session::value,
            session::layout,
            session::save_layout,
            session::quit,
            session::check_done,
            session::page_error,
            session::capture,
        ])
        .on_window_event(|window, event| {
            // The page asks **Save**, **Discard** or **Cancel**, then calls `quit` (15.10).
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.state::<Editor>().dirty()
            {
                api.prevent_close();
                let _ = window.emit("close-requested", ());
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            output::capture(move |text, err| {
                let _ = handle.emit("output", OutputText { text, err });
            })?;
            let engine = engine::start(app.handle().clone(), data.toolchain.clone());
            let webview_dir = data.dir.join("webview");
            if let Some(dir) = &check {
                limit_check(dir.clone());
            }
            let is_check = check.is_some();
            app.manage(Editor::new(data, engine, check));
            build_window(app.handle(), webview_dir, is_check)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("the Tauri app runs");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_path_rejects_paths_outside_dist() {
        assert_eq!(safe_path("/"), Some("index.html"));
        assert_eq!(safe_path("/assets/index-1.js"), Some("assets/index-1.js"));
        for bad in ["/../Cargo.toml", "/assets/../../x", "/C:/x", "/a\\b", "//etc/passwd", "/./x"] {
            assert_eq!(safe_path(bad), None, "{bad}");
        }
    }
}
