//! The engine thread. It owns the loaded cluster, which stays on one thread, and builds and runs
//! the graph (spec 7.2, 15.10).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SendError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use patina_engine::clock::{self, Clock};
use patina_engine::{Checked, Cluster, Control, Diagnostic, Error, Mode, NodeLog, NodeTime, Report, Toolchain, Value, package};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::views::{ViewInput, Views};

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
    /// Fuses the clusters now: a bake build with no slot (7.2). **Bake** sends it, and the engine
    /// makes it after `FUSE_IDLE` with no change.
    Bake,
    /// Makes a package of the saved project `root` in `out` (14.1). `reply` gets the warnings.
    Package { root: PathBuf, out: PathBuf, reply: Sender<Result<Vec<String>, String>> },
    /// User input to a view (15.8). The first input builds the view.
    View(Box<ViewInput>),
    /// Drops a view, so the next input builds it again.
    CloseView(String),
    /// Drops one view instance, by its dock slot. The reply comes after the drop.
    DropView(String, Sender<Result<(), String>>),
}

/// Sends jobs to the engine thread. It counts the runs that wait for a build, so the engine
/// drops a build when a newer build waits (7.2).
#[derive(Clone)]
pub struct Jobs {
    send: Sender<Job>,
    shared: Arc<Shared>,
}

impl Jobs {
    pub fn send(&self, job: Job) -> Result<(), SendError<Job>> {
        if matches!(&job, Job::Run(run) if run.build) {
            self.shared.builds.fetch_add(1, Ordering::SeqCst);
        }
        self.send.send(job)
    }
}

/// The time with no change after which the engine fuses the clusters (7.2, roadmap question 7).
const FUSE_IDLE: Duration = Duration::from_secs(30);

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
    pub build: bool,
    /// The node folder of the code file that this change wrote, which goes into the slot (7.1).
    pub slot: Option<String>,
}

/// What a run builds: the mode and the node folder for the slot (7.2).
struct Build {
    mode: Mode,
    slot: Option<String>,
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
    /// when the frame domain runs after this run of the batch domain (9.1). `fused` is true for
    /// clusters in bake mode with no slot, and `slot` has the node folder in the slot (7.3).
    /// `slot_only` is true when the build made only the slot library.
    Ran {
        serial: u64,
        built_ms: Option<f64>,
        slot_only: bool,
        fused: bool,
        slot: Option<String>,
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
    /// The node folder in the slot (7.1).
    slot: Option<String>,
    /// True after a build of the clusters failed, so the base is older than the code. Then a
    /// save does not build the slot alone.
    stale: bool,
    /// The output pins of the subgraph uses, with the inner outputs that give their values (6.4).
    aliases: Vec<(String, String)>,
}

impl Loaded {
    /// True when the clusters are in edit mode or a node is in the slot, so a fuse changes them.
    fn needs_fuse(&self) -> bool {
        self.cluster.mode() == Mode::Edit || self.cluster.slotted()
    }
}

/// The state that the engine thread shares with the commands.
#[derive(Default)]
pub struct Shared {
    /// The full texts of the live values of the last run. The **Inspector** reads them (15.7), so
    /// the event of each run carries only the short texts of the badges.
    pub values: Mutex<Vec<Value>>,
    /// The node errors of the last run or frame. A view reads them (15.8).
    pub reports: Mutex<Vec<Report>>,
    /// The stop flag and the node that runs (15.2).
    pub control: Control,
    /// The node ID of each node index of the loaded cluster.
    ids: Mutex<Vec<String>>,
    /// The number of sent runs with a build that the engine did not take yet.
    builds: AtomicUsize,
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
pub fn start(app: AppHandle, toolchain: PathBuf) -> (Jobs, Arc<Shared>) {
    let (send, jobs) = std::sync::mpsc::channel();
    let shared = Arc::new(Shared::default());
    let kept = shared.clone();
    std::thread::Builder::new()
        .name("patina-engine".to_owned())
        .spawn(move || run_jobs(&app, &jobs, &kept, &toolchain))
        .expect("the engine thread starts");
    (Jobs { send, shared: shared.clone() }, shared)
}

fn run_jobs(app: &AppHandle, jobs: &Receiver<Job>, shared: &Shared, folder: &Path) {
    let mut toolchain = None;
    let mut views = Views::default();
    let mut loaded: Option<Loaded> = None;
    let mut next = None;
    // The last run, which a GUI change runs again, and the nodes of the Inspector and the full size.
    let (mut last, mut shown) = (None::<Box<Run>>, Shown::default());
    // The time of the fuse while the graph is live and the clusters are not fused (7.2).
    let mut fuse_at = None;
    while let Some(first) = next.take().or_else(|| wait(jobs, fuse_at)) {
        // Only the newest run starts. A build that a skipped run asked for still happens.
        let mut newest = None;
        let (mut build, mut rerun, mut fuse) = (false, false, false);
        // The slot holds a node only when all the changes that need a build are in its code.
        let mut slot = None;
        for job in std::iter::once(first).chain(jobs.try_iter()) {
            match job {
                Job::Unload => {
                    (loaded, newest, build, last, fuse_at) = (None, None, false, None, None);
                    views = Views::default();
                    lock(&shared.values).clear();
                    lock(&shared.reports).clear();
                }
                Job::Run(run) => {
                    if run.build {
                        shared.builds.fetch_sub(1, Ordering::SeqCst);
                        slot = if build && slot != run.slot { None } else { run.slot.clone() };
                        build = true;
                    }
                    newest = Some(run);
                }
                Job::Pause => (newest, build, rerun, fuse, fuse_at) = (None, false, false, false, None),
                Job::Bake => fuse = true,
                Job::Gui(input) => {
                    if let (Some(loaded), Some(serial)) = (loaded.as_mut(), last.as_ref().map(|r| r.serial)) {
                        rerun |= gui(app, &mut loaded.cluster, serial, &input, &mut shown).is_some();
                    }
                }
                Job::Rerun => rerun = true,
                // The engine thread makes the package, so no other build uses the build folder.
                Job::Package { root, out, reply } => {
                    let _ = reply.send(make_package(folder, &mut toolchain, &root, &out));
                }
                Job::View(input) => views.ui(folder, &mut toolchain, *input, loaded.as_mut().map(|l| &mut l.cluster)),
                Job::CloseView(name) => views.close(&name),
                Job::DropView(instance, reply) => {
                    views.drop_instance(&instance);
                    let _ = reply.send(Ok(()));
                }
            }
        }
        fuse &= build || loaded.as_ref().is_none_or(Loaded::needs_fuse);
        if (rerun || fuse) && newest.is_none() {
            newest = last.take();
        }
        let Some(mut run) = newest else { continue };
        let build = match (fuse, build || loaded.is_none()) {
            (true, _) => Some(Build { mode: Mode::Bake, slot: None }),
            (false, true) => Some(Build { mode: Mode::Edit, slot }),
            (false, false) => None,
        };
        if build.is_some() {
            let _ = app.emit("engine", Event::Building { serial: run.serial });
        }
        // shortcut: the fuse builds on the engine thread, so a change waits for it; build in a background thread when bake builds get slow.
        let event = match run_one(app, (folder, &mut toolchain), &mut loaded, &mut run, build.as_ref(), shared) {
            Ok(Some(event)) => event,
            // A newer build waits, so this build does not run and its errors do not show (7.2).
            Ok(None) => continue,
            Err(e) => {
                lock(&shared.values).clear();
                lock(&shared.reports).clear();
                let text = e.to_string();
                Event::Failed { serial: run.serial, text, errors: if let Error::Compile(errors) = e { errors } else { Vec::new() } }
            }
        };
        // An edit build fuses after a time with no change. A failed fuse waits for the next change.
        if let Some(build) = &build {
            let edit = build.mode == Mode::Edit && loaded.as_ref().is_some_and(Loaded::needs_fuse);
            fuse_at = edit.then(|| Instant::now() + FUSE_IDLE);
        }
        let frames = matches!(event, Event::Ran { frames: true, .. });
        let ran = matches!(event, Event::Ran { .. });
        let _ = app.emit("engine", event);
        if let (true, Some(loaded)) = (ran, loaded.as_mut()) {
            draw(app, &mut loaded.cluster, run.serial, &shown);
            if frames {
                next = run_frames(app, jobs, (&mut loaded.cluster, &mut shown, &loaded.aliases), (&mut views, folder, &mut toolchain), shared, (run.serial, fuse_at));
            }
        }
        last = Some(run);
    }
}

/// Waits for the next job. At `fuse_at`, gives `Job::Bake` (7.2).
fn wait(jobs: &Receiver<Job>, fuse_at: Option<Instant>) -> Option<Job> {
    let Some(at) = fuse_at else { return jobs.recv().ok() };
    match jobs.recv_timeout(at.saturating_duration_since(Instant::now())) {
        Ok(job) => Some(job),
        Err(RecvTimeoutError::Timeout) => Some(Job::Bake),
        Err(RecvTimeoutError::Disconnected) => None,
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

/// Runs the frame domain at 60 FPS until a job comes, the user stops the run, a frame fails or
/// the time of the fuse comes (9.1, 7.2). Returns the job that ended the run.
fn run_frames(app: &AppHandle, jobs: &Receiver<Job>, (cluster, shown, aliases): (&mut Cluster, &mut Shown, &[(String, String)]), (views, folder, toolchain): (&mut Views, &Path, &mut Option<Toolchain>), shared: &Shared, (serial, fuse_at): (u64, Option<Instant>)) -> Option<Job> {
    let (mut clock, mut frames) = (Clock::new(clock::FRAME), 0);
    let job = loop {
        clock.wait();
        if fuse_at.is_some_and(|at| Instant::now() >= at) {
            break Some(Job::Bake);
        }
        match jobs.try_recv() {
            // A change of a frame node applies at the next frame. A batch node needs a batch run.
            Ok(Job::Gui(input)) => {
                if gui(app, cluster, serial, &input, shown) == Some(false) {
                    break Some(Job::Rerun);
                }
            }
            // A view reads the live values, so the frames go on.
            Ok(Job::View(input)) => views.ui(folder, toolchain, *input, Some(&mut *cluster)),
            Ok(Job::CloseView(name)) => views.close(&name),
            Ok(Job::DropView(instance, reply)) => {
                views.drop_instance(&instance);
                let _ = reply.send(Ok(()));
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
        let extra = alias(&outcome.values, aliases);
        let mut values = badges(&outcome.values);
        values.extend(badges(&extra));
        keep(&mut lock(&shared.values), &outcome.values);
        keep(&mut lock(&shared.values), &extra);
        lock(&shared.reports).clone_from(&outcome.reports);
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

/// A copy of the value of each inner output for the output pin of its subgraph use (6.4).
fn alias(values: &[Value], aliases: &[(String, String)]) -> Vec<Value> {
    let value = |(outer, inner): &(String, String)| Some(Value { from: outer.clone(), ..values.iter().find(|v| &v.from == inner)?.clone() });
    aliases.iter().filter_map(value).collect()
}

/// The values with the badge texts (15.9).
fn badges(values: &[Value]) -> Vec<Value> {
    values.iter().map(|v| Value { from: v.from.clone(), text: badge(&v.text), lanes: v.lanes.clone() }).collect()
}

/// Builds the clusters for a run (7.2). A save of the code of one node puts the node into the
/// slot, and builds the slot library alone when the base can stay. Gives None when only the slot
/// library changed.
fn compile(checked: &mut Checked, toolchain: &Toolchain, build: &Build, loaded: Option<&mut Loaded>) -> Result<Option<Cluster>, Error> {
    // A generic node or the delay node does not go into the slot, so its change builds all (7.1).
    let slotted = build.slot.is_some() && checked.set_slot(build.slot.as_deref()).is_ok();
    let swapped = match loaded.filter(|l| slotted && !l.stale && l.slot == build.slot) {
        Some(loaded) => checked.build_slot(toolchain, &mut loaded.cluster),
        None => Ok(false),
    };
    let built = match swapped {
        Ok(true) => Ok(None),
        Ok(false) => checked.build(toolchain, build.mode).map(Some),
        Err(e) => Err(e),
    };
    // The run stays for a rerun and for the fuse, which builds with no slot.
    checked.set_slot(None).expect("an empty slot always works");
    built
}

/// The toolchain of the builds. After an install, the toolchain folder replaces the Rust of the
/// system.
pub(crate) fn find_toolchain<'a>(folder: &Path, toolchain: &'a mut Option<Toolchain>) -> Result<&'a Toolchain, Error> {
    if toolchain.as_ref().is_none_or(|t| t.own.is_none() && Toolchain::installed(folder)) {
        *toolchain = Some(Toolchain::find(folder)?);
    }
    Ok(toolchain.as_ref().expect("the toolchain is set above"))
}

/// Makes a package with a bake build. The runner of the package is the `patina-cli` next to the
/// editor (14.1).
fn make_package(folder: &Path, toolchain: &mut Option<Toolchain>, root: &Path, out: &Path) -> Result<Vec<String>, String> {
    let toolchain = find_toolchain(folder, toolchain).map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let runner = exe.with_file_name(format!("patina-cli{}", std::env::consts::EXE_SUFFIX));
    if !runner.is_file() {
        return Err(format!("Patina cannot make a package, because {} is not there.", runner.display()));
    }
    package::make(root, toolchain, Mode::Bake, &runner, out).map_err(|e| e.to_string())
}

fn run_one(app: &AppHandle, (folder, toolchain): (&Path, &mut Option<Toolchain>), loaded: &mut Option<Loaded>, run: &mut Run, build: Option<&Build>, shared: &Shared) -> Result<Option<Event>, Error> {
    let mut built_ms = None;
    let mut slot_only = false;
    let mut errors = Vec::new();
    if let Some(build) = build {
        let start = Instant::now();
        let wiring = run.checked.wiring();
        let toolchain = find_toolchain(folder, toolchain)?;
        let built = compile(&mut run.checked, toolchain, build, loaded.as_mut());
        if shared.builds.load(Ordering::SeqCst) > 0 {
            return Ok(None);
        }
        match built {
            // The slot library changed in place, and the nodes in it got new states (7.1).
            Ok(None) => slot_only = true,
            Ok(Some(mut cluster)) => {
                *lock(&shared.ids) = cluster.node_ids().map(str::to_owned).collect();
                // The node states with the same key go on in the new cluster (7.2).
                if let Some(old) = loaded.as_mut() {
                    cluster.adopt(&mut old.cluster);
                }
                let slot = build.slot.clone().filter(|_| cluster.slotted());
                *loaded = Some(Loaded { cluster, wiring, slot, stale: false, aliases: run.checked.aliases().to_vec() });
            }
            // The last build that compiled keeps running only with the same wiring (7.2, 15.10).
            Err(Error::Compile(list)) if loaded.as_ref().is_some_and(|l| l.wiring == wiring) => {
                // shortcut: a failed build of the slot alone also marks the base as old, so the next save builds all; keep the slot build when saves after an error get slow.
                loaded.as_mut().expect("the cluster is checked above").stale = true;
                errors = list;
            }
            Err(e) => {
                *loaded = None;
                return Err(e);
            }
        }
        built_ms = Some(start.elapsed().as_secs_f64() * 1000.0);
    }
    let loaded = loaded.as_mut().expect("the cluster is loaded above");
    let (fused, slot) = (!loaded.needs_fuse(), loaded.slot.clone());
    let cluster = &mut loaded.cluster;
    let params = cluster.set_params(run.checked.graph());
    if params.ok {
        let _ = app.emit("engine", Event::Started { serial: run.serial });
    }
    let start = Instant::now();
    // A node with a bad parameter does not run, so the graph does not run (4.8).
    let outcome = params.ok.then(|| cluster.run(&shared.control));
    let run_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut outcome = outcome.unwrap_or_default();
    let extra = alias(&outcome.values, &loaded.aliases);
    outcome.values.extend(extra);
    let reports: Vec<Report> = params.reports.into_iter().chain(outcome.reports).collect();
    lock(&shared.reports).clone_from(&reports);
    let values = badges(&outcome.values);
    *lock(&shared.values) = outcome.values;
    let frames = params.ok && outcome.ok && cluster.frames() && !shared.control.stopped();
    Ok(Some(Event::Ran { serial: run.serial, built_ms, slot_only, fused, slot, run_ms, ok: outcome.ok, frames, skipped: outcome.skipped, stopped: outcome.stopped, times: outcome.times, logs: outcome.logs, reports, values, errors }))
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
        let extra = alias(&kept, &[("u.0000000a".to_owned(), "u/b.out".to_owned()), ("v.0000000a".to_owned(), "b.out".to_owned())]);
        assert_eq!((extra.len(), extra[0].from.as_str(), extra[0].text.as_str()), (1, "v.0000000a", "3"));
    }
}
