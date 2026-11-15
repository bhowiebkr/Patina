//! Milestone 0 prototype for spec 15.9: live wires and buses in a Tauri graph view.
//!
//! `connections` opens the graph view. `connections --bench` runs the measured scenarios in an
//! off-screen window and writes a report to `artifacts/connections/`.

mod bench;
mod feed;
mod graph;
#[cfg(windows)]
mod shot;

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tauri::http::{header::CONTENT_TYPE, Request, Response, StatusCode};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use bench::Bench;
use feed::{Feed, Sink};

#[cfg(windows)]
const ORIGIN: &str = "http://patina.localhost";
#[cfg(not(windows))]
const ORIGIN: &str = "patina://localhost";

/// The wry defaults, and the flags that keep an off-screen window rendering at the full rate.
const BENCH_BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion \
    --disable-background-timer-throttling --disable-renderer-backgrounding --disable-backgrounding-occluded-windows";

struct Shared {
    feed: Arc<Feed>,
    bench: Option<Arc<Bench>>,
}

#[tauri::command]
fn start_feed(app: AppHandle, state: State<'_, Shared>, mode: String, channel: Channel<InvokeResponseBody>) {
    let sink = if mode == "json" { Sink::Event } else { Sink::Channel(channel) };
    state.feed.start(app, sink);
}

#[tauri::command(async)]
fn scenario_begin(state: State<'_, Shared>) {
    state.feed.take_send_times();
    if let Some(bench) = &state.bench {
        bench.scenario_begin();
    }
}

#[tauri::command(async)]
fn scenario_end(state: State<'_, Shared>, stats: Value) {
    if let Some(bench) = &state.bench {
        bench.scenario_end(stats, state.feed.take_send_times(), state.feed.frame_bytes());
    }
}

/// Returns the page of the next run. The page loads it itself: a navigation from here stops the
/// IPC request, and the IPC script then sends `run_done` a second time.
#[tauri::command(async)]
fn run_done(app: AppHandle, window: WebviewWindow, state: State<'_, Shared>, info: Value) -> Option<String> {
    let bench = state.bench.as_ref()?;
    state.feed.stop();
    if bench.run_done(info) {
        let run = bench.current()?;
        set_run_zoom(&window, run);
        return Some(format!("index.html?{}", bench.query(run)));
    }
    match bench.finish() {
        Ok(path) => println!("bench: report written to {}", path.display()),
        Err(e) => eprintln!("bench: cannot write the report: {e}"),
    }
    app.exit(0);
    None
}

/// Writes a screenshot of the page to `artifacts/connections/shot-<name>.png`.
#[tauri::command(async)]
fn capture(window: WebviewWindow, state: State<'_, Shared>, name: String) -> Result<(), String> {
    let Some(bench) = &state.bench else { return Err("capture runs only in the bench".into()) };
    bench.beat();
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("bad screenshot name {name}"));
    }
    #[cfg(windows)]
    {
        let png = shot::capture(&window)?;
        bench::write_artifact(&format!("shot-{name}.png"), &png).map_err(|e| e.to_string())?;
    }
    // shortcut: screenshots work on Windows only; add a path for Linux when the bench runs there.
    #[cfg(not(windows))]
    let _ = window;
    Ok(())
}

#[tauri::command]
fn page_error(app: AppHandle, state: State<'_, Shared>, message: String) {
    eprintln!("page error: {message}");
    if state.bench.is_some() {
        app.exit(1);
    }
}

/// Emulates the display scale of the run with the page zoom.
fn set_run_zoom(window: &WebviewWindow, run: &bench::Run) {
    let native = window.scale_factor().unwrap_or(1.0);
    let zoom = if run.dpr > 0.0 { run.dpr / native } else { 1.0 };
    if let Err(e) = window.set_zoom(zoom) {
        eprintln!("bench: cannot set the zoom: {e}");
        std::process::exit(1);
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
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

fn serve(graph_json: &[u8], request: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let reply = |status: StatusCode, kind: &str, body: Vec<u8>| {
        Response::builder()
            .status(status)
            .header(CONTENT_TYPE, kind)
            .body(Cow::Owned(body))
            .unwrap()
    };
    let Some(path) = safe_path(request.uri().path()) else {
        return reply(StatusCode::FORBIDDEN, "text/plain", Vec::new());
    };
    if path == "graph.json" {
        return reply(StatusCode::OK, "application/json", graph_json.to_vec());
    }
    match std::fs::read(dist_dir().join(path)) {
        Ok(body) => reply(StatusCode::OK, content_type(path), body),
        Err(_) => reply(StatusCode::NOT_FOUND, "text/plain", Vec::new()),
    }
}

struct Args {
    bench: Option<Arc<Bench>>,
    query: String,
    /// Asks the browser for the integrated GPU of a computer with two GPUs.
    low_power_gpu: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut bench = false;
    // The default runs compare the two renderers with the live feed. The other rows need --run.
    let mut runs = vec![0, 1];
    let mut seconds = 3;
    let mut shots = false;
    let mut low_power_gpu = false;
    let (mut renderer, mut feed) = ("webgl".to_string(), "raw".to_string());
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--bench" => bench = true,
            "--shots" => shots = true,
            "--low-power-gpu" => low_power_gpu = true,
            "--run" => {
                let list = value()?;
                runs = list.split(',').map(|r| r.parse().map_err(|_| format!("bad run {r}"))).collect::<Result<_, _>>()?;
                if runs.iter().any(|&r| r >= bench::RUNS.len()) {
                    return Err(format!("the runs are 0 to {}", bench::RUNS.len() - 1));
                }
            }
            "--seconds" => seconds = value()?.parse().map_err(|_| "bad --seconds")?,
            "--renderer" => renderer = value()?,
            "--feed" => feed = value()?,
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    let bench = bench.then(|| Arc::new(Bench::new(runs, seconds, shots)));
    let query = match &bench {
        Some(b) => b.query(b.current().ok_or("no runs")?),
        None => format!("renderer={renderer}&feed={feed}"),
    };
    Ok(Args { bench, query, low_power_gpu })
}

fn build_window(app: &AppHandle, bench: bool, query: &str, low_power_gpu: bool) -> tauri::Result<WebviewWindow> {
    let url = format!("{ORIGIN}/index.html?{query}").parse().unwrap();
    let data_dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/connections-webview"));
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::CustomProtocol(url))
        .title("Patina connections prototype")
        .data_directory(data_dir)
        .inner_size(1536.0, 864.0);
    let builder = if bench {
        // Off-screen and not focused: the bench must not take the desktop (AGENTS.md).
        builder
            .position(-20000.0, 0.0)
            .focused(false)
            .focusable(false)
            .skip_taskbar(true)
            .decorations(false)
            .additional_browser_args(&if low_power_gpu {
                format!("{BENCH_BROWSER_ARGS} --force_low_power_gpu")
            } else {
                BENCH_BROWSER_ARGS.to_string()
            })
    } else {
        builder.maximized(true)
    };
    builder.build()
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("connections: {e}");
            std::process::exit(2);
        }
    };
    let graph = graph::generate(7);
    let feed = Arc::new(Feed::new(graph.lane_count));
    let graph_json: Arc<[u8]> = serde_json::to_vec(&graph).expect("the graph serializes").into();
    if let Some(bench) = &args.bench {
        bench.watch();
    }
    let is_bench = args.bench.is_some();
    let query = args.query;
    let low_power_gpu = args.low_power_gpu;
    tauri::Builder::default()
        .manage(Shared { feed, bench: args.bench })
        .register_uri_scheme_protocol("patina", move |_ctx, request| serve(&graph_json, &request))
        .invoke_handler(tauri::generate_handler![start_feed, scenario_begin, scenario_end, run_done, capture, page_error])
        .on_window_event(|window, event| {
            if let (WindowEvent::Focused(true), Some(bench)) = (event, &window.state::<Shared>().bench) {
                bench.focus_events.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        })
        .setup(move |app| {
            let window = build_window(app.handle(), is_bench, &query, low_power_gpu)?;
            if let Some(run) = app.state::<Shared>().bench.as_ref().and_then(|b| b.current()) {
                set_run_zoom(&window, run);
            }
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
