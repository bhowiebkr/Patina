//! The engine thread. It owns the loaded cluster, which stays on one thread, and builds and runs
//! the graph (spec 7.2, 15.10).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use patina_engine::project::GraphFile;
use patina_engine::{Checked, Cluster, Control, Diagnostic, Error, NodeLog, NodeTime, Report, Toolchain, Value};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub enum Job {
    Run(Box<Run>),
    /// Drops the cluster, for example when the project closes.
    Unload,
}

/// Builds the graph when `build` is true or no cluster is loaded. Then sets the parameters and
/// runs the graph one time.
pub struct Run {
    /// The number of the open project, so that the page ignores the result for an old project.
    pub serial: u64,
    pub checked: Checked,
    pub graph: GraphFile,
    pub build: bool,
}

#[derive(Serialize, Clone)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    Building { serial: u64 },
    /// A run starts, so **Stop** can stop it (15.2).
    Started { serial: u64 },
    /// When the build failed, `errors` has its compile errors, and the last build that compiled ran.
    /// `values` has the badge text of each output that a wire reads (15.9). `skipped` and `stopped`
    /// have the IDs of the nodes that did not run (15.3, 15.2). `times` has the run time of each
    /// node call that started, and `logs` has the log lines of the nodes (15.7).
    Ran {
        serial: u64,
        built_ms: Option<f64>,
        run_ms: f64,
        ok: bool,
        skipped: Vec<String>,
        stopped: Vec<String>,
        times: Vec<NodeTime>,
        logs: Vec<NodeLog>,
        reports: Vec<Report>,
        values: Vec<Value>,
        errors: Vec<Diagnostic>,
    },
    Failed { serial: u64, text: String, errors: Vec<Diagnostic> },
}

/// A loaded cluster and the generated code that it was built from.
struct Loaded {
    cluster: Cluster,
    wiring: String,
}

/// The state that the engine thread shares with the commands.
#[derive(Default)]
pub struct Shared {
    /// The full texts of the live values of the last run. The **Inspector** reads them (15.7), so
    /// the event of each run carries only the short texts of the badges.
    pub values: Mutex<Vec<Value>>,
    /// The stop flag and the node that runs (15.2).
    pub control: Control,
    /// The node ID of each node index of the loaded cluster.
    ids: Mutex<Vec<String>>,
}

impl Shared {
    /// The ID of the node that runs, or that ran last in the active run. None between runs.
    pub fn running(&self) -> Option<String> {
        let node = self.control.node()?;
        lock(&self.ids).get(node as usize).cloned()
    }
}

/// The longest text of a badge, in bytes. A longer text ends with "...".
const BADGE_LEN: usize = 64;

/// A panic in a command does not stop the editor, so the shared state stays in use after it.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// `toolchain` is the toolchain folder of Patina (13.3).
pub fn start(app: AppHandle, toolchain: PathBuf) -> (Sender<Job>, Arc<Shared>) {
    let (send, jobs) = std::sync::mpsc::channel();
    let shared = Arc::new(Shared::default());
    let kept = shared.clone();
    std::thread::Builder::new()
        .name("patina-engine".to_owned())
        .spawn(move || run_jobs(&app, &jobs, &kept, &toolchain))
        .expect("the engine thread starts");
    (send, shared)
}

fn run_jobs(app: &AppHandle, jobs: &Receiver<Job>, shared: &Shared, folder: &Path) {
    let mut toolchain = None;
    let mut loaded = None;
    while let Ok(first) = jobs.recv() {
        // Only the newest run starts. A build that a skipped run asked for still happens.
        let mut newest = None;
        let mut build = false;
        for job in std::iter::once(first).chain(jobs.try_iter()) {
            match job {
                Job::Unload => {
                    (loaded, newest, build) = (None, None, false);
                    lock(&shared.values).clear();
                }
                Job::Run(run) => {
                    build |= run.build;
                    newest = Some(run);
                }
            }
        }
        let Some(run) = newest else { continue };
        if build || loaded.is_none() {
            let _ = app.emit("engine", Event::Building { serial: run.serial });
        }
        let event = match run_one(app, (folder, &mut toolchain), &mut loaded, &run, build, shared) {
            Ok(event) => event,
            Err(e) => {
                lock(&shared.values).clear();
                let text = e.to_string();
                Event::Failed { serial: run.serial, text, errors: if let Error::Compile(errors) = e { errors } else { Vec::new() } }
            }
        };
        let _ = app.emit("engine", event);
    }
}

fn run_one(app: &AppHandle, (folder, toolchain): (&Path, &mut Option<Toolchain>), loaded: &mut Option<Loaded>, run: &Run, build: bool, shared: &Shared) -> Result<Event, Error> {
    let mut built_ms = None;
    let mut errors = Vec::new();
    if build || loaded.is_none() {
        let start = Instant::now();
        let wiring = run.checked.wiring();
        // After an install, the toolchain folder replaces the Rust of the system.
        if toolchain.as_ref().is_none_or(|t| t.own.is_none() && Toolchain::installed(folder)) {
            *toolchain = Some(Toolchain::find(folder)?);
        }
        let toolchain = toolchain.as_ref().expect("the toolchain is set above");
        match run.checked.build(toolchain) {
            Ok(cluster) => {
                *lock(&shared.ids) = cluster.node_ids().map(str::to_owned).collect();
                *loaded = Some(Loaded { cluster, wiring });
            }
            // The last build that compiled keeps running only with the same wiring (7.2, 15.10).
            Err(Error::Compile(list)) if loaded.as_ref().is_some_and(|l| l.wiring == wiring) => errors = list,
            Err(e) => {
                *loaded = None;
                return Err(e);
            }
        }
        built_ms = Some(start.elapsed().as_secs_f64() * 1000.0);
    }
    let cluster = &mut loaded.as_mut().expect("the cluster is loaded above").cluster;
    let params = cluster.set_params(&run.graph);
    if params.ok {
        let _ = app.emit("engine", Event::Started { serial: run.serial });
    }
    let start = Instant::now();
    // A node with a bad parameter does not run, so the graph does not run (4.8).
    let outcome = params.ok.then(|| cluster.run(&shared.control));
    let run_ms = start.elapsed().as_secs_f64() * 1000.0;
    let outcome = outcome.unwrap_or_default();
    let reports = params.reports.into_iter().chain(outcome.reports).collect();
    let badges = outcome.values.iter().map(|v| Value { from: v.from.clone(), text: badge(&v.text), lanes: v.lanes.clone() }).collect();
    *lock(&shared.values) = outcome.values;
    Ok(Event::Ran { serial: run.serial, built_ms, run_ms, ok: outcome.ok, skipped: outcome.skipped, stopped: outcome.stopped, times: outcome.times, logs: outcome.logs, reports, values: badges, errors })
}

/// Cuts a text for its badge at a character boundary.
fn badge(text: &str) -> String {
    if text.len() <= BADGE_LEN {
        return text.to_owned();
    }
    let end = (0..=BADGE_LEN).rev().find(|&i| text.is_char_boundary(i)).unwrap_or(0);
    format!("{}...", &text[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badges_cut_long_texts_at_a_character_boundary() {
        assert_eq!(badge("2.5"), "2.5");
        let full = "a".repeat(BADGE_LEN);
        assert_eq!(badge(&full), full);
        // The 33rd two-byte character ends at byte 66, so the badge keeps 32 of them.
        assert_eq!(badge(&"\u{e9}".repeat(40)), format!("{}...", "\u{e9}".repeat(32)));
    }
}
