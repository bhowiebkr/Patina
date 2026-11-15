//! The engine thread. It owns the loaded cluster, which stays on one thread, and builds and runs
//! the graph (spec 7.2, 15.10).

use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

use patina_engine::project::GraphFile;
use patina_engine::{Checked, Cluster, Diagnostic, Error, Report, Toolchain, Value};
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
    /// When the build failed, `errors` has its compile errors, and the last build that compiled ran.
    /// `values` has the live value of each output that a wire reads (15.9).
    Ran {
        serial: u64,
        built_ms: Option<f64>,
        run_ms: f64,
        ok: bool,
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

pub fn start(app: AppHandle) -> Sender<Job> {
    let (send, jobs) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("patina-engine".to_owned())
        .spawn(move || run_jobs(&app, &jobs))
        .expect("the engine thread starts");
    send
}

fn run_jobs(app: &AppHandle, jobs: &Receiver<Job>) {
    let mut toolchain = None;
    let mut loaded = None;
    while let Ok(first) = jobs.recv() {
        // Only the newest run starts. A build that a skipped run asked for still happens.
        let mut newest = None;
        let mut build = false;
        for job in std::iter::once(first).chain(jobs.try_iter()) {
            match job {
                Job::Unload => (loaded, newest, build) = (None, None, false),
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
        let event = match run_one(&mut toolchain, &mut loaded, &run, build) {
            Ok(event) => event,
            Err(e) => {
                let text = e.to_string();
                Event::Failed { serial: run.serial, text, errors: if let Error::Compile(errors) = e { errors } else { Vec::new() } }
            }
        };
        let _ = app.emit("engine", event);
    }
}

fn run_one(toolchain: &mut Option<Toolchain>, loaded: &mut Option<Loaded>, run: &Run, build: bool) -> Result<Event, Error> {
    let mut built_ms = None;
    let mut errors = Vec::new();
    if build || loaded.is_none() {
        let start = Instant::now();
        let wiring = run.checked.wiring();
        if toolchain.is_none() {
            *toolchain = Some(Toolchain::detect()?);
        }
        let toolchain = toolchain.as_ref().expect("the toolchain is set above");
        match run.checked.build(toolchain) {
            Ok(cluster) => *loaded = Some(Loaded { cluster, wiring }),
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
    let start = Instant::now();
    // A node with a bad parameter does not run, so the graph does not run (4.8).
    let outcome = params.ok.then(|| cluster.run());
    let run_ms = start.elapsed().as_secs_f64() * 1000.0;
    let outcome = outcome.unwrap_or_default();
    let reports = params.reports.into_iter().chain(outcome.reports).collect();
    Ok(Event::Ran { serial: run.serial, built_ms, run_ms, ok: outcome.ok, reports, values: outcome.values, errors })
}
