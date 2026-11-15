//! The engine thread. It owns the loaded cluster, which stays on one thread, and builds and runs
//! the graph (spec 7.2, 15.10).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use patina_engine::clock::{self, Clock};
use patina_engine::project::GraphFile;
use patina_engine::{Checked, Cluster, Control, Diagnostic, Error, NodeLog, NodeTime, Report, Toolchain, Value};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub enum Job {
    Run(Box<Run>),
    /// Drops the cluster, for example when the project closes.
    Unload,
    /// Ends the frame run (9.1). **Pause** and **Stop** send it.
    Pause,
    /// User input to the GUI of a node, or a request for its widget lines (4.3).
    Gui(GuiInput),
    /// Runs the batch domain again after a GUI change of a batch node. Only the frame run sends it.
    Rerun,
}

pub struct GuiInput {
    pub id: String,
    /// 0 for the node body, 1 for the **Inspector**, 2 for the full size. After each run, the
    /// engine draws the node of the last mode 1 and mode 2 input in that size. An empty ID ends it.
    pub mode: u32,
    /// A line for each user input: the widget index and the value.
    pub events: String,
    /// True when the graph is live, so a change runs the graph again (4.3).
    pub rerun: bool,
}

/// The widget lines of a node GUI in one size (4.3).
#[derive(Serialize, Clone)]
pub struct Tree {
    id: String,
    mode: u32,
    tree: String,
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
    /// node call that started, and `logs` has the log lines of the nodes (15.7). `frames` is true
    /// when the frame domain runs after this run of the batch domain (9.1).
    Ran {
        serial: u64,
        built_ms: Option<f64>,
        run_ms: f64,
        ok: bool,
        frames: bool,
        skipped: Vec<String>,
        stopped: Vec<String>,
        times: Vec<NodeTime>,
        logs: Vec<NodeLog>,
        reports: Vec<Report>,
        values: Vec<Value>,
        errors: Vec<Diagnostic>,
    },
    Failed { serial: u64, text: String, errors: Vec<Diagnostic> },
    /// The result of one frame (9.1). The fields are as in `Ran`, for the nodes of the frame domain.
    Frame {
        serial: u64,
        run_ms: f64,
        ok: bool,
        skipped: Vec<String>,
        stopped: Vec<String>,
        times: Vec<NodeTime>,
        logs: Vec<NodeLog>,
        reports: Vec<Report>,
        values: Vec<Value>,
    },
    /// The frame run ended after `frames` frames: by **Pause**, **Stop**, a new run or a failed frame.
    Ended { serial: u64, frames: u64 },
    /// The widget lines of the node GUIs (4.3), and the problems of their `ui` calls.
    Gui { serial: u64, trees: Vec<Tree>, reports: Vec<Report> },
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
    let mut loaded: Option<Loaded> = None;
    let mut next = None;
    // The last run, which a GUI change runs again, and the nodes of the Inspector and the full size.
    let (mut last, mut shown) = (None::<Box<Run>>, Shown::default());
    while let Some(first) = next.take().or_else(|| jobs.recv().ok()) {
        // Only the newest run starts. A build that a skipped run asked for still happens.
        let mut newest = None;
        let mut build = false;
        let mut rerun = false;
        for job in std::iter::once(first).chain(jobs.try_iter()) {
            match job {
                Job::Unload => {
                    (loaded, newest, build, last) = (None, None, false, None);
                    lock(&shared.values).clear();
                }
                Job::Run(run) => {
                    build |= run.build;
                    newest = Some(run);
                }
                Job::Pause => (newest, build, rerun) = (None, false, false),
                Job::Gui(input) => {
                    if let (Some(loaded), Some(serial)) = (loaded.as_mut(), last.as_ref().map(|r| r.serial)) {
                        rerun |= gui(app, &mut loaded.cluster, serial, &input, &mut shown).is_some();
                    }
                }
                Job::Rerun => rerun = true,
            }
        }
        if rerun && newest.is_none() {
            newest = last.take();
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
        let frames = matches!(event, Event::Ran { frames: true, .. });
        let ran = matches!(event, Event::Ran { .. });
        let _ = app.emit("engine", event);
        if let (true, Some(loaded)) = (ran, loaded.as_mut()) {
            draw(app, &mut loaded.cluster, run.serial, &shown);
            if frames {
                next = run_frames(app, jobs, (&mut loaded.cluster, &mut shown), shared, run.serial);
            }
        }
        last = Some(run);
    }
}

/// Gives the GUI input to node `input.id`, and sends its new widget lines. Returns `Some` when
/// the change must run the graph again: `Some(true)` when the node is in the frame domain.
fn gui(app: &AppHandle, cluster: &mut Cluster, serial: u64, input: &GuiInput, shown: &mut Shown) -> Option<bool> {
    if let Some(node) = (input.mode as usize).checked_sub(1).and_then(|i| shown.get_mut(i)) {
        *node = (!input.id.is_empty()).then(|| input.id.clone());
    }
    let (tree, out) = cluster.ui(&input.id, input.mode, &input.events);
    let trees = tree.map(|tree| Tree { id: input.id.clone(), mode: input.mode, tree }).into_iter().collect();
    let _ = app.emit("engine", Event::Gui { serial, trees, reports: out.reports });
    let frame = cluster.guis().iter().find(|(id, _)| *id == input.id).map(|&(_, frame)| frame)?;
    (input.rerun && !input.events.is_empty()).then_some(frame)
}

/// The node of the **Inspector** size and the node of the full size (4.3).
type Shown = [Option<String>; 2];

/// Sends the widget lines of each node GUI (4.3): the node body size of each node, and the
/// larger sizes of the nodes in `shown`.
fn draw(app: &AppHandle, cluster: &mut Cluster, serial: u64, shown: &Shown) {
    let (mut trees, mut reports) = (Vec::new(), Vec::new());
    // shortcut: the engine draws all node GUIs, also the nodes out of view (4.3 rule 3). Send the visible nodes from the page when graphs get large.
    for n in 0..cluster.guis().len() {
        let id = cluster.guis()[n].0.clone();
        for mode in [0, 1, 2].into_iter().filter(|&m| m == 0 || shown[m as usize - 1].as_ref() == Some(&id)) {
            let (tree, out) = cluster.ui(&id, mode, "");
            reports.extend(out.reports);
            trees.extend(tree.map(|tree| Tree { id: id.clone(), mode, tree }));
        }
    }
    if !trees.is_empty() || !reports.is_empty() {
        let _ = app.emit("engine", Event::Gui { serial, trees, reports });
    }
}

/// Runs the frame domain at 60 FPS until a job comes, the user stops the run, or a frame fails
/// (9.1). Returns the job that ended the run.
fn run_frames(app: &AppHandle, jobs: &Receiver<Job>, (cluster, shown): (&mut Cluster, &mut Shown), shared: &Shared, serial: u64) -> Option<Job> {
    let (mut clock, mut frames) = (Clock::new(clock::FRAME), 0);
    let job = loop {
        clock.wait();
        match jobs.try_recv() {
            // A change of a frame node applies at the next frame. A batch node needs a batch run.
            Ok(Job::Gui(input)) => {
                if gui(app, cluster, serial, &input, shown) == Some(false) {
                    break Some(Job::Rerun);
                }
            }
            Ok(job) => break Some(job),
            Err(TryRecvError::Disconnected) => break None,
            Err(TryRecvError::Empty) if shared.control.stopped() => break None,
            Err(TryRecvError::Empty) => {}
        }
        let start = Instant::now();
        let outcome = cluster.frame(&shared.control);
        let run_ms = start.elapsed().as_secs_f64() * 1000.0;
        frames += 1;
        let values = badges(&outcome.values);
        keep(&mut lock(&shared.values), &outcome.values);
        let ok = outcome.ok;
        // shortcut: the editor copies each frame into a JSON event, so its frame loop allocates. Send a binary frame when the web view needs more FPS.
        let (skipped, stopped, times) = (outcome.skipped.clone(), outcome.stopped.clone(), outcome.times.clone());
        let (logs, reports) = (outcome.logs.clone(), outcome.reports.clone());
        let _ = app.emit("engine", Event::Frame { serial, run_ms, ok, skipped, stopped, times, logs, reports, values });
        draw(app, cluster, serial, shown);
        if !ok {
            break None;
        }
    };
    let _ = app.emit("engine", Event::Ended { serial, frames });
    job
}

/// Puts the values of a frame in place of the old values of the same outputs. The values of the
/// batch domain stay.
fn keep(kept: &mut Vec<Value>, values: &[Value]) {
    for value in values {
        match kept.iter_mut().find(|v| v.from == value.from) {
            Some(old) => old.clone_from(value),
            None => kept.push(value.clone()),
        }
    }
}

/// The values with the badge texts (15.9).
fn badges(values: &[Value]) -> Vec<Value> {
    values.iter().map(|v| Value { from: v.from.clone(), text: badge(&v.text), lanes: v.lanes.clone() }).collect()
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
            Ok(mut cluster) => {
                *lock(&shared.ids) = cluster.node_ids().map(str::to_owned).collect();
                // The node states with the same key go on in the new cluster (7.2).
                if let Some(old) = loaded.as_mut() {
                    cluster.adopt(&mut old.cluster);
                }
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
    let values = badges(&outcome.values);
    *lock(&shared.values) = outcome.values;
    let frames = params.ok && outcome.ok && cluster.frames() && !shared.control.stopped();
    Ok(Event::Ran { serial: run.serial, built_ms, run_ms, ok: outcome.ok, frames, skipped: outcome.skipped, stopped: outcome.stopped, times: outcome.times, logs: outcome.logs, reports, values, errors })
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

    #[test]
    fn a_frame_replaces_only_the_values_of_its_outputs() {
        let value = |from: &str, text: &str| Value { from: from.to_owned(), text: text.to_owned(), lanes: None };
        let mut kept = vec![value("a.out", "1"), value("b.out", "2")];
        keep(&mut kept, &[value("b.out", "3"), value("c.out", "4")]);
        let texts: Vec<_> = kept.iter().map(|v| (v.from.as_str(), v.text.as_str())).collect();
        assert_eq!(texts, [("a.out", "1"), ("b.out", "3"), ("c.out", "4")]);
    }
}
