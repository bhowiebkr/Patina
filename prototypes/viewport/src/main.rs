//! Milestone 0 prototype for spec 15.8: a wgpu viewport in a panel of a Tauri window.
//!
//! `viewport` opens a small editor shell with two viewports. `viewport --bench` runs the
//! scenarios and the checks of each frame path in an off-screen window, and writes a report to
//! `artifacts/viewport/`.

mod bench;
#[cfg(windows)]
mod capture;
mod gpu;
mod render;
#[cfg(windows)]
mod shared;
mod stats;

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tauri::http::{header::CONTENT_TYPE, Request, Response, StatusCode};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use bench::{Bench, MODES};
use render::{Cmd, Sink};
use stats::Stats;

#[cfg(windows)]
const ORIGIN: &str = "http://patina.localhost";
#[cfg(not(windows))]
const ORIGIN: &str = "patina://localhost";

/// The wry defaults, and the flags that keep an off-screen window rendering at the full rate.
const BENCH_BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion \
    --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows";

struct Shared {
    cmds: Sender<Cmd>,
    stats: Arc<Mutex<Stats>>,
    bench: Option<Arc<Bench>>,
    low_power: bool,
}

impl Shared {
    fn send(&self, cmd: Cmd) {
        // The render thread stops only with the process, so a failed send needs no handling.
        let _ = self.cmds.send(cmd);
    }
}

#[tauri::command]
fn register(
    window: WebviewWindow,
    state: State<'_, Shared>,
    id: u32,
    mode: String,
    channel: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    let sink = match mode.as_str() {
        "native" => Sink::Native,
        "channel" => Sink::Channel { channel, busy: false },
        #[cfg(windows)]
        "shared" => Sink::Shared(Default::default()),
        _ => return Err(format!("the frame path {mode} is not available")),
    };
    state.send(Cmd::Viewport { id, window, sink });
    Ok(())
}

#[tauri::command]
fn unregister(state: State<'_, Shared>, id: u32) {
    state.send(Cmd::Remove { id });
}

#[tauri::command]
fn rect(state: State<'_, Shared>, id: u32, rect: Option<[u32; 4]>) {
    state.send(Cmd::Rect { id, rect });
}

#[tauri::command]
fn input(state: State<'_, Shared>, id: u32, dx: f32, dy: f32, wheel: f32, key: bool, t: f64) {
    state.send(Cmd::Input { id, dx, dy, wheel, key, t_ms: t });
}

#[tauri::command]
fn ack(state: State<'_, Shared>, id: u32) {
    state.send(Cmd::Ack { id });
}

#[tauri::command]
fn now() -> f64 {
    render::now_ms()
}

#[tauri::command(async)]
fn read_stats(state: State<'_, Shared>) -> Value {
    state.stats.lock().unwrap().summary()
}

#[tauri::command]
fn lose_device(state: State<'_, Shared>) {
    state.send(Cmd::LoseDevice);
}

/// Moves a viewport into its own window. A window must not be made in a sync command.
#[tauri::command(async)]
fn float_viewport(app: AppHandle, state: State<'_, Shared>, id: u32, mode: String) -> Result<(), String> {
    let label = format!("float-{id}");
    if app.get_webview_window(&label).is_some() {
        return Err(format!("viewport {id} floats already"));
    }
    let bench = if state.bench.is_some() { "&bench=1" } else { "" };
    let query = format!("float={id}&mode={mode}{bench}");
    build_window(&app, &label, &query, state.bench.is_some(), state.low_power).map_err(|e| e.to_string())?;
    Ok(())
}

/// Closes a float window. The render thread drops its surface before the window goes.
#[tauri::command]
fn dock(window: WebviewWindow, state: State<'_, Shared>) {
    state.send(Cmd::CloseWindow { window });
}

/// Captures a window and returns the part of each rectangle that shows the scene. With
/// `--shots`, it also writes the image to `artifacts/viewport/shot-<name>.bmp`.
#[tauri::command(async)]
fn probe(app: AppHandle, state: State<'_, Shared>, label: String, rects: Vec<[u32; 4]>, name: String) -> Result<Vec<f64>, String> {
    if let Some(bench) = &state.bench {
        bench.beat();
    }
    let window = app.get_webview_window(&label).ok_or(format!("no window {label}"))?;
    #[cfg(windows)]
    {
        let shot = capture::print_window(&window)?;
        if state.bench.as_ref().is_some_and(|b| b.shots) && !name.is_empty() {
            if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(format!("bad shot name {name}"));
            }
            bench::write_artifact(&format!("shot-{name}.bmp"), &shot.bmp()).map_err(|e| e.to_string())?;
        }
        Ok(rects.iter().map(|r| shot.scene_fraction(*r)).collect())
    }
    // shortcut: the probes work on Windows only; add a capture for Linux when the bench runs there.
    #[cfg(not(windows))]
    {
        let _ = (window, rects, name);
        Err("the probes need Windows".into())
    }
}

/// Calls a DevTools protocol method of the page, for the bench input and the focus emulation.
#[tauri::command(async)]
fn cdp(window: WebviewWindow, method: String, params: Value) -> Result<Value, String> {
    #[cfg(windows)]
    {
        let json = capture::cdp(&window, &method, &params.to_string())?;
        serde_json::from_str(&json).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (window, method, params);
        Err("the DevTools calls need Windows".into())
    }
}

/// Emulates a display scale with the page zoom. The system setting does not change.
#[tauri::command]
fn set_dpr(window: WebviewWindow, dpr: f64) -> Result<(), String> {
    let native = window.scale_factor().map_err(|e| e.to_string())?;
    window.set_zoom(dpr / native).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_size(window: WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    window.set_size(LogicalSize::new(width, height)).map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn scenario_begin(state: State<'_, Shared>) {
    state.stats.lock().unwrap().reset();
    if let Some(bench) = &state.bench {
        bench.scenario_begin();
    }
}

#[tauri::command(async)]
fn scenario_end(state: State<'_, Shared>, name: String, page: Value) {
    let render = state.stats.lock().unwrap().summary();
    if let Some(bench) = &state.bench {
        bench.scenario_end(&name, page, render);
    }
}

/// Returns the page of the next run. The page loads it itself: a navigation from here stops the
/// IPC request, and the IPC script then sends `run_done` a second time.
#[tauri::command(async)]
fn run_done(app: AppHandle, window: WebviewWindow, state: State<'_, Shared>, info: Value) -> Option<String> {
    let bench = state.bench.as_ref()?;
    let _ = window.set_zoom(1.0);
    if bench.run_done(info) {
        return Some(format!("index.html?{}", bench.query(bench.current()?)));
    }
    match bench.finish() {
        Ok(path) => println!("bench: report written to {}", path.display()),
        Err(e) => eprintln!("bench: cannot write the report: {e}"),
    }
    app.exit(0);
    None
}

#[tauri::command]
fn page_error(app: AppHandle, state: State<'_, Shared>, message: String) {
    eprintln!("page error: {message}");
    if state.bench.is_some() {
        app.exit(1);
    }
}

fn dist_dir() -> PathBuf {
    // shortcut: the page is read from the source tree; embed it when the editor ships (spec 13).
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/frontend/dist"))
}

/// Accepts only plain relative paths, so a request cannot read outside `frontend/dist`.
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
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn serve(request: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let reply = |status: StatusCode, kind: &str, body: Vec<u8>| {
        Response::builder().status(status).header(CONTENT_TYPE, kind).body(Cow::Owned(body)).unwrap()
    };
    let Some(path) = safe_path(request.uri().path()) else {
        return reply(StatusCode::FORBIDDEN, "text/plain", Vec::new());
    };
    match std::fs::read(dist_dir().join(path)) {
        Ok(body) => reply(StatusCode::OK, content_type(path), body),
        Err(_) => reply(StatusCode::NOT_FOUND, "text/plain", Vec::new()),
    }
}

struct Args {
    bench: Option<Arc<Bench>>,
    query: String,
    /// Uses the integrated GPU of a computer with two GPUs, for the GPU and the web view.
    low_power: bool,
}

fn parse_args() -> Result<Args, String> {
    let (mut bench, mut shots, mut low_power) = (false, false, false);
    let mut runs: Vec<usize> = (0..MODES.len()).collect();
    let mut seconds = 2;
    let mut mode = "native".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--bench" => bench = true,
            "--shots" => shots = true,
            "--low-power-gpu" => low_power = true,
            "--run" => {
                let list = value()?;
                let find = |m: &str| MODES.iter().position(|x| *x == m).ok_or(format!("the modes are {MODES:?}"));
                runs = list.split(',').map(find).collect::<Result<_, _>>()?;
            }
            "--seconds" => seconds = value()?.parse().map_err(|_| "bad --seconds")?,
            "--mode" => mode = value()?,
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    let bench = bench.then(|| Arc::new(Bench::new(runs, seconds, shots)));
    let query = match &bench {
        Some(b) => b.query(b.current().ok_or("no runs")?),
        None => format!("mode={mode}"),
    };
    Ok(Args { bench, query, low_power })
}

fn build_window(app: &AppHandle, label: &str, query: &str, bench: bool, low_power: bool) -> tauri::Result<WebviewWindow> {
    let url = format!("{ORIGIN}/index.html?{query}").parse().unwrap();
    let data_dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/viewport-webview"));
    let float = label != "main";
    let (width, height) = if float { (640.0, 400.0) } else { (1536.0, 864.0) };
    // The native frame path draws on the window under a transparent web view.
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::CustomProtocol(url))
        .title(if float { "Patina viewport" } else { "Patina viewport prototype" })
        .data_directory(data_dir)
        .inner_size(width, height)
        .transparent(true);
    let builder = if bench {
        // Off-screen and not focused: the bench must not take the desktop (AGENTS.md).
        let args = if low_power { format!("{BENCH_BROWSER_ARGS} --force_low_power_gpu") } else { BENCH_BROWSER_ARGS.into() };
        builder
            .position(-20000.0, if float { 1000.0 } else { 0.0 })
            .focused(false)
            .focusable(false)
            .skip_taskbar(true)
            .decorations(false)
            .additional_browser_args(&args)
    } else if float {
        builder
    } else {
        builder.maximized(true)
    };
    builder.build()
}

fn window_event(window: &tauri::Window, event: &WindowEvent) {
    let state = window.state::<Shared>();
    let label = window.label();
    match event {
        WindowEvent::Resized(size) => {
            state.send(Cmd::Resize { label: label.into(), width: size.width, height: size.height });
        }
        WindowEvent::CloseRequested { api, .. } if label.starts_with("float-") => {
            api.prevent_close();
            if let Some(webview) = window.app_handle().get_webview_window(label) {
                state.send(Cmd::CloseWindow { window: webview });
            }
        }
        WindowEvent::Destroyed => {
            state.send(Cmd::WindowGone { label: label.into() });
            if let Some(id) = label.strip_prefix("float-").and_then(|id| id.parse::<u32>().ok()) {
                let _ = window.app_handle().emit_to("main", "docked", id);
            }
        }
        WindowEvent::Focused(true) => {
            if let Some(bench) = &state.bench {
                bench.focus_events.fetch_add(1, Ordering::SeqCst);
            }
        }
        _ => {}
    }
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("viewport: {e}");
            std::process::exit(2);
        }
    };
    let stats = Arc::new(Mutex::new(Stats::new()));
    let (cmds, receiver) = std::sync::mpsc::channel();
    if let Err(e) = render::spawn(receiver, Arc::clone(&stats), args.low_power) {
        eprintln!("viewport: no GPU: {e}");
        std::process::exit(1);
    }
    if let Some(bench) = &args.bench {
        bench.watch();
    }
    let is_bench = args.bench.is_some();
    let (query, low_power) = (args.query, args.low_power);
    tauri::Builder::default()
        .manage(Shared { cmds, stats, bench: args.bench, low_power })
        .register_uri_scheme_protocol("patina", |_ctx, request| serve(&request))
        .invoke_handler(tauri::generate_handler![
            register, unregister, rect, input, ack, now, read_stats, lose_device, float_viewport, dock, probe, cdp, set_dpr,
            set_size, scenario_begin, scenario_end, run_done, page_error
        ])
        .on_window_event(window_event)
        .setup(move |app| {
            build_window(app.handle(), "main", &query, is_bench, low_power)?;
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
