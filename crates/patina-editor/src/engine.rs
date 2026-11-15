//! The engine thread. It owns the loaded cluster, which stays on one thread, and builds and runs
//! the graph (spec 7.2, 15.10).

use std::sync::mpsc::{Receiver, Sender};
use std::time::Instant;

use patina_engine::project::GraphFile;
use patina_engine::{Checked, Cluster, Report, Toolchain};
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
    Ran { serial: u64, built_ms: Option<f64>, run_ms: f64, ok: bool, reports: Vec<Report> },
    Failed { serial: u64, text: String },
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
    let mut cluster = None;
    while let Ok(first) = jobs.recv() {
        // Only the newest run starts. A build that a skipped run asked for still happens.
        let mut newest = None;
        let mut build = false;
        for job in std::iter::once(first).chain(jobs.try_iter()) {
            match job {
                Job::Unload => (cluster, newest, build) = (None, None, false),
                Job::Run(run) => {
                    build |= run.build;
                    newest = Some(run);
                }
            }
        }
        let Some(run) = newest else { continue };
        if build || cluster.is_none() {
            let _ = app.emit("engine", Event::Building { serial: run.serial });
        }
        let event = match run_one(&mut toolchain, &mut cluster, &run, build) {
            Ok(event) => event,
            Err(text) => Event::Failed { serial: run.serial, text },
        };
        let _ = app.emit("engine", event);
    }
}

fn run_one(toolchain: &mut Option<Toolchain>, cluster: &mut Option<Cluster>, run: &Run, build: bool) -> Result<Event, String> {
    let mut built_ms = None;
    if build || cluster.is_none() {
        // A failed build leaves no cluster, so the old wiring never runs with the new graph.
        *cluster = None;
        let start = Instant::now();
        if toolchain.is_none() {
            *toolchain = Some(Toolchain::detect().map_err(|e| e.to_string())?);
        }
        let toolchain = toolchain.as_ref().expect("the toolchain is set above");
        *cluster = Some(run.checked.build(toolchain).map_err(|e| e.to_string())?);
        built_ms = Some(start.elapsed().as_secs_f64() * 1000.0);
    }
    let cluster = cluster.as_mut().expect("the cluster is loaded above");
    let params = cluster.set_params(&run.graph);
    let start = Instant::now();
    // A node with a bad parameter does not run, so the graph does not run (4.8).
    let outcome = params.ok.then(|| cluster.run());
    let run_ms = start.elapsed().as_secs_f64() * 1000.0;
    let ok = outcome.as_ref().is_some_and(|o| o.ok);
    let reports = params.reports.into_iter().chain(outcome.into_iter().flat_map(|o| o.reports)).collect();
    Ok(Event::Ran { serial: run.serial, built_ms, run_ms, ok, reports })
}
