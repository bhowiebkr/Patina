//! The engine thread. It owns the loaded cluster, and builds and runs the graph (spec 7.2,
//! 15.10). During a frame run, an audio thread can run the audio domain of the cluster (9.3), and
//! the event domain runs on its own threads (9.4). In the editor, the engine thread runs in the
//! node process (16.1), and its events go to the editor as lines of JSON.

mod blocks;
mod events;
mod frame;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SendError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use patina_engine::clock::{self, Clock, frame_due};
use patina_engine::device::{Pace, raise_priority};
use patina_engine::{Audio, Checked, Cluster, Compiled, Control, Diagnostic, Error, Frames, Mode, NodeLog, NodeTime, Remote, Report, Toolchain, Value, package};
use serde::{Deserialize, Serialize};

use crate::node_process::Out;
use crate::views::{ViewInput, Views};
use blocks::{Blocks, Part};
use frame::{Shown, alias, badges, changed, draw, frame, gui};

#[derive(Serialize, Deserialize)]
pub enum Job {
    /// Only the node process makes it, from the parts of the checked graph (16.1).
    #[serde(skip)]
    Run(Box<Run>),
    /// Drops the cluster, for example when the project closes.
    Unload,
    /// Ends the frame run (9.1). **Pause** and **Stop** send it.
    Pause,
    /// Ends the frame run, and runs one frame of the last run (9.1). **Step** sends it.
    Step,
    /// User input to the GUI of a node, or a request for its widget lines (4.3).
    Gui(GuiInput),
    /// Runs the batch domain again after a GUI change of a batch node. Only the frame run sends it.
    Rerun,
    /// Fuses the clusters now: a bake build with no slot (7.2). **Bake** sends it, and the engine
    /// makes it after `FUSE_IDLE` with no change.
    Bake,
    /// The result of a fuse that built in a background thread. Only the engine makes it.
    #[serde(skip)]
    Fused(Box<Result<Compiled, Error>>),
    /// Makes a package of the saved project `root` in `out` (14.1). The reply to `ask` has the
    /// warnings.
    Package { root: PathBuf, out: PathBuf, ask: u64 },
    /// User input to a view (15.8). The first input builds the view.
    View(Box<ViewInput>),
    /// Drops a view, so the next input builds it again.
    CloseView(String),
    /// Drops one view instance, by its dock slot. The reply to the ask comes after the drop.
    DropView(String, u64),
    /// The nodes in the view of the graph. The frames draw only their GUIs (4.3 rule 3).
    Visible(Vec<String>),
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

/// How often the engine looks for the end of a fuse while it waits for a job.
const FUSE_POLL: Duration = Duration::from_millis(100);

/// One build at a time: the builds of the project share its `build` folder (7.2).
pub(crate) static BUILDING: Mutex<()> = Mutex::new(());

/// The fuse of the clusters: a bake build with no slot (7.2). It builds in a background thread,
/// so the graph runs on until the fused clusters load.
#[derive(Default)]
struct Fuse {
    /// The time of the fuse while the graph is live and the clusters are not fused.
    at: Option<Instant>,
    /// The number of builds, unloads and pauses. A fuse that started before the last one is old.
    changes: u64,
    /// The fuse that builds now, with the value of `changes` at its start.
    thread: Option<(JoinHandle<Result<Compiled, Error>>, u64)>,
}

impl Fuse {
    /// Starts a fuse of `checked` in a background thread, unless a fuse of the same graph builds.
    fn start(&mut self, checked: &Checked, folder: &Path, toolchain: &mut Option<Toolchain>) {
        self.at = None;
        if self.thread.as_ref().is_some_and(|(_, changes)| *changes == self.changes) {
            return;
        }
        let (checked, toolchain) = (checked.clone(), find_toolchain(folder, toolchain).cloned());
        let thread = std::thread::Builder::new().name("patina-fuse".to_owned()).spawn(move || {
            let toolchain = toolchain?;
            let _build = lock(&BUILDING);
            checked.compile(&toolchain, Mode::Bake, &|| {})
        });
        self.thread = thread.ok().map(|thread| (thread, self.changes));
    }

    /// Gives the result of a fuse that ended, when no change came after its start.
    fn done(&mut self) -> Option<Result<Compiled, Error>> {
        if !self.thread.as_ref().is_some_and(|(thread, _)| thread.is_finished()) {
            return None;
        }
        let (thread, changes) = self.thread.take()?;
        let result = thread.join().unwrap_or_else(|_| Err(Error::Build("the fuse build panicked".to_owned())));
        (changes == self.changes).then_some(result)
    }
}

#[derive(Serialize, Deserialize)]
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

/// A parameter write of a node GUI (4.3). The page applies it as a `SetParam` edit, so undo works.
#[derive(Serialize, Clone)]
pub struct ParamWrite {
    id: String,
    key: String,
    text: String,
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
    /// False for the load when a project opens: the clusters load and the node GUIs draw, but
    /// the graph does not run (15.10).
    pub start: bool,
}

/// What a run builds: the mode and the node folder for the slot (7.2).
struct Build {
    mode: Mode,
    slot: Option<String>,
    /// The result of a fuse that built in a background thread, which loads with no build.
    fused: Option<Result<Compiled, Error>>,
}

#[derive(Serialize, Clone)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    /// A build starts. `base` is true when it builds the base, and `slot` has the node folder of
    /// the slot library that it builds (7.3).
    Building { serial: u64, base: bool, slot: Option<String> },
    /// A run starts, so **Stop** can stop it (15.2).
    Started { serial: u64 },
    /// When the build failed, `errors` has its compile errors, and the last build that compiled ran.
    /// `values` has the badge text of each output that a wire reads (15.9). `skipped` and `stopped`
    /// have the IDs of the nodes that did not run (15.3, 15.2). `times` has the run time of each
    /// node call that started, and `logs` has the log lines of the nodes (15.7). `frames` is true
    /// when the frame domain or the audio domain runs after this run of the batch domain (9.1,
    /// 9.3), and `events` is true when the triggers listen after it (9.4). `fused` is true for
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
        events: bool,
        skipped: Vec<String>,
        stopped: Vec<String>,
        times: Vec<NodeTime>,
        logs: Vec<NodeLog>,
        reports: Vec<Report>,
        values: Vec<Value>,
        errors: Vec<Diagnostic>,
    },
    Failed { serial: u64, text: String, errors: Vec<Diagnostic> },
    /// The clusters loaded for an open project, and the graph did not run (15.10). The fields
    /// are as in `Ran`.
    Loaded { serial: u64, built_ms: Option<f64>, fused: bool, slot: Option<String>, errors: Vec<Diagnostic> },
    /// The node process ended (16.1). The build did not fail, but the node states are lost.
    Lost { serial: u64, text: String },
    /// The result of one frame (9.1). The fields are as in `Ran`, for the nodes of the frame domain
    /// and for the blocks of the audio domain since the last frame (9.3).
    /// The last of `frames` frames (9.1). `run_ms` is the time of all the frames.
    Frame {
        serial: u64,
        frames: u64,
        run_ms: f64,
        ok: bool,
        skipped: Vec<String>,
        stopped: Vec<String>,
        times: Vec<NodeTime>,
        logs: Vec<NodeLog>,
        reports: Vec<Report>,
        values: Vec<Value>,
    },
    /// One event run ended (9.4). `id` is the run ID, from 1 at each start, and 0 for a report of
    /// a trigger or of Patina. The other fields are as in `Frame`, for the nodes of the run.
    Fired {
        serial: u64,
        id: u64,
        trigger: String,
        ok: bool,
        skipped: Vec<String>,
        stopped: Vec<String>,
        times: Vec<NodeTime>,
        logs: Vec<NodeLog>,
        reports: Vec<Report>,
        values: Vec<Value>,
    },
    /// The live run ended after `frames` frames: by **Pause**, **Stop**, a new run or a failed frame.
    Ended { serial: u64, frames: u64 },
    /// The widget lines of the node GUIs (4.3), the problems of their `ui` calls, and the parameter
    /// writes of a GUI that got user input.
    Gui { serial: u64, trees: Vec<Tree>, reports: Vec<Report>, params: Vec<ParamWrite> },
}

/// A loaded cluster and the generated code that it was built from.
struct Loaded {
    cluster: Cluster,
    wiring: String,
    /// The node folder in the slot (7.1).
    slot: Option<String>,
    /// True from the start of a full build until a full build loads, so the base can be older
    /// than the code. Then a save does not build the slot alone. A failed slot build keeps it.
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
    /// The node errors of the last event run of each node since the last run (9.4).
    pub event_reports: Mutex<Vec<Report>>,
    /// Fires the `trigger` nodes while the event domain runs (9.4).
    pub remote: Mutex<Option<Remote>>,
    /// The stop flag and the node that runs (15.2).
    pub control: Control,
    /// The node ID of each node index of the loaded cluster.
    ids: Mutex<Vec<String>>,
    /// The number of sent runs with a build that the engine did not take yet.
    builds: AtomicUsize,
}

impl Shared {
    /// The ID of the node that runs, or that ran last in the active run, on the engine thread or in
    /// an event run. An empty ID while project code runs that names no node (16.1). None between runs.
    pub fn running(&self) -> Option<String> {
        // `control` keeps the last node of a run after its task, so an unnamed call comes first.
        if patina_engine::in_unnamed_call() {
            return Some(String::new());
        }
        let node = self.control.node().or_else(|| lock(&self.remote).as_ref()?.running())?;
        lock(&self.ids).get(node as usize).cloned()
    }

    /// Gives the ID of the node that runs, on this thread or else on the engine thread or in an event
    /// run, to `f`, for a crash handler. It never waits for a lock and never allocates.
    #[cfg(windows)]
    pub fn try_running(&self, f: impl FnOnce(&str)) {
        let node = patina_engine::thread_node().or_else(|| match patina_engine::in_unnamed_call() {
            // The last node of the run is not the code that crashed.
            true => None,
            false => self.control.node().or_else(|| self.remote.try_lock().ok()?.as_ref()?.try_running()),
        });
        if let (Some(node), Ok(ids)) = (node, self.ids.try_lock())
            && let Some(id) = ids.get(node as usize)
        {
            f(id);
        }
    }
}

/// A panic in a command does not stop the editor, so the shared state stays in use after it.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// `toolchain` is the toolchain folder of Patina (13.3). The events go to `out`. When the engine
/// thread ends, the process ends: with code 101 after a panic of the engine.
pub fn start(out: Arc<Out>, toolchain: PathBuf) -> (Jobs, Arc<Shared>) {
    let (send, jobs) = std::sync::mpsc::channel();
    let shared = Arc::new(Shared::default());
    let kept = shared.clone();
    std::thread::Builder::new()
        .name("patina-engine".to_owned())
        .spawn(move || {
            let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_jobs(&out, &jobs, &kept, &toolchain))).is_ok();
            std::process::exit(if ok { 0 } else { 101 });
        })
        .expect("the engine thread starts");
    (Jobs { send, shared: shared.clone() }, shared)
}

fn run_jobs(out: &Out, jobs: &Receiver<Job>, shared: &Shared, folder: &Path) {
    let mut toolchain = None;
    let mut views = Views::default();
    let mut loaded: Option<Loaded> = None;
    let mut next = None;
    // The last run, which a GUI change runs again, and the nodes of the Inspector and the full size.
    let (mut last, mut shown) = (None::<Box<Run>>, Shown::default());
    let mut fuse = Fuse::default();
    // True when the last run has frame nodes, so a step can run one frame.
    let mut framed = false;
    while let Some(first) = next.take().or_else(|| wait(jobs, &mut fuse)) {
        // Only the newest run starts. A build that a skipped run asked for still happens.
        let (mut newest, mut fused) = (None, None);
        let (mut build, mut rerun, mut bake, mut step) = (false, false, false, false);
        // The slot holds a node only when all the changes that need a build are in its code.
        let mut slot = None;
        for job in std::iter::once(first).chain(jobs.try_iter()) {
            match job {
                Job::Unload => {
                    (loaded, newest, build, last, fuse.at, framed) = (None, None, false, None, None, false);
                    fuse.changes += 1;
                    views = Views::default();
                    lock(&shared.values).clear();
                    lock(&shared.reports).clear();
                    lock(&shared.event_reports).clear();
                }
                Job::Run(run) => {
                    if run.build {
                        shared.builds.fetch_sub(1, Ordering::SeqCst);
                        fuse.changes += 1;
                        slot = if build && slot != run.slot { None } else { run.slot.clone() };
                        build = true;
                    }
                    newest = Some(run);
                }
                Job::Pause | Job::Step => {
                    (newest, build, rerun, bake, fuse.at, step) = (None, false, false, false, None, matches!(job, Job::Step));
                    fuse.changes += 1;
                }
                Job::Bake => bake = true,
                Job::Fused(done) => fused = Some(*done),
                Job::Gui(input) => {
                    if let (Some(loaded), Some(serial)) = (loaded.as_mut(), last.as_ref().map(|r| r.serial)) {
                        rerun |= gui(out, &shared.control, &mut loaded.cluster.frame_part(), serial, &input, &mut shown).is_some();
                    }
                }
                Job::Rerun => rerun = true,
                // The engine thread makes the package, so no other build uses the build folder.
                Job::Package { root, out: to, ask } => {
                    let _build = lock(&BUILDING);
                    out.reply(ask, make_package(folder, &mut toolchain, &root, &to));
                }
                Job::View(input) => views.ui(out, shared, (folder, &mut toolchain), *input, loaded.as_mut().map(|l| l.cluster.frame_part()).as_mut()),
                Job::CloseView(name) => views.close(&name),
                Job::DropView(instance, ask) => {
                    views.drop_instance(&instance);
                    out.reply(ask, Ok(()));
                }
                Job::Visible(ids) => shown.visible = Some(ids.into_iter().collect()),
            }
        }
        if let (true, true, Some(loaded), Some(serial)) = (step && newest.is_none(), framed, loaded.as_mut(), last.as_ref().map(|r| r.serial)) {
            frame(out, (&mut loaded.cluster.frame_part(), &shown, &loaded.aliases), shared, (serial, 1, 1), &mut Blocks::default());
            out.emit(&Event::Ended { serial, frames: 1 });
        }
        bake &= build || loaded.as_ref().is_none_or(Loaded::needs_fuse);
        // A fuse with no other build builds in a background thread, so the graph runs on (7.2).
        if bake
            && !build
            && loaded.is_some()
            && let Some(run) = newest.as_ref().or(last.as_ref())
        {
            fuse.start(&run.checked, folder, &mut toolchain);
            bake = false;
        }
        // The fused clusters load only when no build came after the fuse.
        let fused = fused.filter(|_| !build);
        let opened = loaded.is_none() && last.is_none();
        if (rerun || bake || fused.is_some()) && newest.is_none() {
            newest = last.take();
        }
        let Some(mut run) = newest else { continue };
        let mut build = if fused.is_some() {
            Some(Build { mode: Mode::Bake, slot: None, fused })
        } else if bake {
            Some(Build { mode: Mode::Bake, slot: None, fused: None })
        } else if opened {
            // The first build of a project has the mode of its last build, so cargo builds nothing when nothing changed (7.2).
            Some(Build { mode: run.checked.built_mode().unwrap_or(Mode::Bake), slot: None, fused: None })
        } else if build || loaded.is_none() {
            // The same clusters keep their mode, so a build with no change compiles nothing (7.2).
            // A changed crate builds in edit mode and fuses later, also after a bake load (8.3).
            let same = loaded.as_ref().filter(|l| slot.is_none() && !l.stale && l.wiring == run.checked.wiring());
            let mode = |l: &Loaded| match l.cluster.mode() {
                Mode::Bake if !l.cluster.sources_changed() => Mode::Bake,
                _ => Mode::Edit,
            };
            Some(same.map_or(Build { mode: Mode::Edit, slot, fused: None }, |l| Build { mode: mode(l), slot: None, fused: None }))
        } else {
            None
        };
        let event = match run_one(out, (folder, &mut toolchain), &mut loaded, &mut run, build.as_mut(), shared) {
            Ok(Some(event)) => event,
            // A newer build waits, so this build does not run and its errors do not show (7.2).
            Ok(None) => continue,
            Err(e) => {
                lock(&shared.values).clear();
                lock(&shared.reports).clear();
                lock(&shared.event_reports).clear();
                let text = e.to_string();
                Event::Failed { serial: run.serial, text, errors: if let Error::Compile(errors) = e { errors } else { Vec::new() } }
            }
        };
        // An edit build of a live graph fuses after a time with no change. A failed fuse waits for the next change.
        if let Some(build) = &build {
            let edit = run.start && build.mode == Mode::Edit && loaded.as_ref().is_some_and(Loaded::needs_fuse);
            fuse.at = edit.then(|| Instant::now() + FUSE_IDLE);
        }
        framed = matches!(event, Event::Ran { frames: true, .. });
        let goes_on = matches!(event, Event::Ran { frames: true, .. } | Event::Ran { events: true, .. });
        let drawn = matches!(event, Event::Ran { .. } | Event::Loaded { .. });
        out.emit(&event);
        if let (true, Some(loaded)) = (drawn, loaded.as_mut()) {
            draw(out, &shared.control, &mut loaded.cluster.frame_part(), run.serial, &shown, true);
            if goes_on {
                let fusable = loaded.needs_fuse();
                let live = Live { out, jobs, shown: &mut shown, aliases: &loaded.aliases, views: &mut views, folder, toolchain: &mut toolchain, shared, serial: run.serial, fuse: &mut fuse, checked: &run.checked, fusable, frames: 0, shows: 0, rate: run.checked.frame_rate() };
                next = run_frames(live, &mut loaded.cluster);
            }
        }
        last = Some(run);
    }
}

/// Waits for the next job. Gives `Job::Bake` at the time of the fuse, and `Job::Fused` when a fuse
/// ends (7.2).
fn wait(jobs: &Receiver<Job>, fuse: &mut Fuse) -> Option<Job> {
    loop {
        if let Some(done) = fuse.done() {
            return Some(Job::Fused(Box::new(done)));
        }
        let poll = fuse.thread.as_ref().map(|_| Instant::now() + FUSE_POLL);
        let Some(until) = fuse.at.into_iter().chain(poll).min() else { return jobs.recv().ok() };
        match jobs.recv_timeout(until.saturating_duration_since(Instant::now())) {
            Ok(job) => return Some(job),
            Err(RecvTimeoutError::Timeout) if fuse.at.is_some_and(|at| Instant::now() >= at) => return Some(Job::Bake),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
}

/// What a frame run needs besides the cluster.
struct Live<'a> {
    out: &'a Out,
    jobs: &'a Receiver<Job>,
    shown: &'a mut Shown,
    aliases: &'a [(String, String)],
    views: &'a mut Views,
    folder: &'a Path,
    toolchain: &'a mut Option<Toolchain>,
    shared: &'a Shared,
    serial: u64,
    fuse: &'a mut Fuse,
    /// The graph of the run, which a fuse builds.
    checked: &'a Checked,
    /// True when the clusters are not fused yet.
    fusable: bool,
    /// The number of frames of the run.
    frames: u64,
    /// The number of frames that the page showed, at most 60 each second.
    shows: u64,
    /// The frames each second (9.1).
    rate: u32,
}

impl Live<'_> {
    /// Takes the next job. Returns `Some` when the job ends the run, with the job for the engine.
    fn job(&mut self, part: &mut Frames) -> Option<Option<Job>> {
        if let Some(done) = self.fuse.done() {
            return Some(Some(Job::Fused(Box::new(done))));
        }
        if self.fuse.at.is_some_and(|at| Instant::now() >= at) {
            self.fuse.start(self.checked, self.folder, self.toolchain);
        }
        match self.jobs.try_recv() {
            // The fuse builds in a background thread, so the frames go on (7.2).
            Ok(Job::Bake) => {
                if self.fusable {
                    self.fuse.start(self.checked, self.folder, self.toolchain);
                }
            }
            // A change of a frame node applies at the next frame. A batch node needs a batch run.
            Ok(Job::Gui(input)) => {
                if gui(self.out, &self.shared.control, part, self.serial, &input, self.shown) == Some(false) {
                    return Some(Some(Job::Rerun));
                }
            }
            // A view reads the live values, so the frames go on.
            Ok(Job::View(input)) => self.views.ui(self.out, self.shared, (self.folder, self.toolchain), *input, Some(part)),
            Ok(Job::CloseView(name)) => self.views.close(&name),
            Ok(Job::DropView(instance, ask)) => {
                self.views.drop_instance(&instance);
                self.out.reply(ask, Ok(()));
            }
            Ok(Job::Visible(ids)) => self.shown.visible = Some(ids.into_iter().collect()),
            Ok(job) => return Some(Some(job)),
            Err(TryRecvError::Disconnected) => return Some(None),
            Err(TryRecvError::Empty) if self.shared.control.stopped() => return Some(None),
            Err(TryRecvError::Empty) => {}
        }
        None
    }

    /// Runs `count` frames and shows the last, with the results of `blocks`. Returns false when a
    /// frame or a block failed.
    fn frame(&mut self, part: &mut Frames, blocks: &mut Blocks, count: u64) -> bool {
        self.shows += 1;
        let (ok, ran) = frame(self.out, (part, self.shown, self.aliases), self.shared, (self.serial, self.shows, count), blocks);
        self.frames += ran;
        ok
    }
}

/// Runs the frame domain at the frame rate of the project, the audio domain at each block (9.3) and the event domain at
/// each fire (9.4) until a job comes, the user stops the run, a frame or a block fails or the time
/// of the fuse comes (9.1, 7.2). Returns the job that ended the run.
fn run_frames(mut live: Live, cluster: &mut Cluster) -> Option<Job> {
    let (frames, blocks, events) = (cluster.frames(), cluster.blocks(), cluster.events());
    let job = match events.then(|| cluster.live()).flatten() {
        Some((mut part, audio, events)) => events::run(&mut live, &mut part, audio, events, frames),
        None => {
            if events {
                // shortcut: no event runs when a node of the slot or the audio domain keeps a value that is not Sync; give such a graph one thread for all domains when this gets common.
                events::warn(&live, "The event runs do not start, because a node in the slot or in the audio domain keeps a value that is not Sync (9.4).".to_owned());
            }
            match (frames, blocks) {
                (false, false) => None,
                (true, false) => frames_only(&mut live, &mut cluster.frame_part()),
                (_, true) => match cluster.split() {
                    Some((mut part, audio)) => split(&mut live, &mut part, audio),
                    None => between(&mut live, cluster),
                },
            }
        }
    };
    live.out.emit(&Event::Ended { serial: live.serial, frames: live.frames });
    job
}

/// The frames of a graph with no audio nodes.
fn frames_only(live: &mut Live, part: &mut Frames) -> Option<Job> {
    let mut clock = Clock::new(clock::FRAME);
    for tick in 1.. {
        clock.wait();
        if let Some(end) = live.job(part) {
            return end;
        }
        let count = clock::frames_at(tick, live.rate);
        if count > 0 && !live.frame(part, &mut Blocks::default(), count) {
            return None;
        }
    }
    None
}

/// The frames, while an audio thread runs the blocks (9.3), so a slow frame or node GUI does not
/// make the blocks late. Each frame shows the results of the blocks since the last frame.
fn split(live: &mut Live, part: &mut Frames, audio: Audio) -> Option<Job> {
    let (stop, out) = (&AtomicBool::new(false), &Mutex::new(Blocks::default()));
    let shared = live.shared;
    std::thread::scope(|s| {
        s.spawn(move || play(audio, shared, stop, out));
        let (mut clock, mut blocks, mut tick) = (Clock::new(clock::FRAME), Blocks::default(), 0);
        let job = loop {
            clock.wait();
            if let Some(end) = live.job(part) {
                break end;
            }
            tick += 1;
            let count = clock::frames_at(tick, live.rate);
            if count == 0 {
                continue;
            }
            // The swap gives the audio thread the emptied blocks of the last frame, with their texts.
            std::mem::swap(&mut *lock(out), &mut blocks);
            if !live.frame(part, &mut blocks, count) {
                break None;
            }
        };
        stop.store(true, Ordering::Relaxed);
        job
    })
}

/// The audio thread: runs the blocks until `stop`, **Stop** or a failed block. It gives their
/// results to the frames in `out`, and never waits for the lock while the blocks run (4.3). It
/// does not allocate after the first blocks.
fn play(mut audio: Audio, shared: &Shared, stop: &AtomicBool, out: &Mutex<Blocks>) {
    raise_priority();
    // Each thread has its own control: the control keeps the node that runs (15.2).
    let (control, mut pace, mut blocks) = (Control::default(), Pace::default(), Blocks::default());
    while !stop.load(Ordering::Relaxed) && !shared.control.stopped() {
        pace.wait();
        let block = audio.block(&control);
        let ok = block.ok;
        blocks.gather(&Part::from(block));
        if let Some(notice) = pace.play(audio.sound().map(|s| s.as_slice())) {
            blocks.device(notice);
        }
        if let Ok(mut out) = out.try_lock() {
            out.gather(&blocks.part());
            blocks.clear();
        }
        if !ok {
            break;
        }
    }
    lock(out).gather(&blocks.part());
}

/// The frames and the blocks on the engine thread, when the cluster cannot split: a library keeps
/// a value that is not `Sync` (9.3).
fn between(live: &mut Live, cluster: &mut Cluster) -> Option<Job> {
    // shortcut: the blocks run between the frames when a library is not parallel, so a slow frame or node GUI makes them late; give such a graph its own thread for all domains when this gets common.
    let (mut pace, mut blocks, mut block) = (Pace::default(), Blocks::default(), 0);
    loop {
        pace.wait();
        if let Some(end) = live.job(&mut cluster.frame_part()) {
            return end;
        }
        blocks.gather(&Part::from(cluster.block(&live.shared.control)));
        block += 1;
        if let Some(notice) = pace.play(cluster.sound().map(|s| s.as_slice())) {
            blocks.device(notice);
        }
        // A failed block ends the run at once, with a frame that shows its results.
        let due = (live.frames..).take_while(|&frame| frame_due(frame, block, live.rate)).count() as u64;
        if (!blocks.part().ok || due > 0) && !live.frame(&mut cluster.frame_part(), &mut blocks, due.max(1)) {
            return None;
        }
    }
}

/// Builds the clusters for a run (7.2). A save of the code of one node puts the node into the
/// slot, and builds the slot library alone when the base can stay. Gives None when only the slot
/// library changed. `building` gets the layers of each build before it starts.
fn compile(checked: &mut Checked, toolchain: &Toolchain, build: &mut Build, mut loaded: Option<&mut Loaded>, building: impl Fn(bool, Option<&str>)) -> Result<Option<Cluster>, Error> {
    if let Some(fused) = build.fused.take() {
        return fused.and_then(|compiled| checked.load(compiled)).map(Some);
    }
    // A generic node or the delay node does not go into the slot, so its change builds all (7.1).
    let slotted = build.slot.is_some() && checked.set_slot(build.slot.as_deref()).is_ok();
    let swapped = match loaded.as_deref_mut().filter(|l| slotted && !l.stale && l.slot == build.slot) {
        Some(loaded) => {
            building(false, build.slot.as_deref());
            checked.build_slot(toolchain, &mut loaded.cluster)
        }
        None => Ok(false),
    };
    let built = match swapped {
        Ok(true) => Ok(None),
        Ok(false) => {
            // Until a full build loads, the base can be older than the code. A failed slot build keeps the base.
            if let Some(loaded) = loaded {
                loaded.stale = true;
            }
            // A build with all crates fresh compiles nothing, so the nodes do not show Compiling.
            checked.build_with(toolchain, build.mode, &|| building(true, build.slot.as_deref().filter(|_| slotted))).map(Some)
        }
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

fn run_one(out: &Out, (folder, toolchain): (&Path, &mut Option<Toolchain>), loaded: &mut Option<Loaded>, run: &mut Run, build: Option<&mut Build>, shared: &Shared) -> Result<Option<Event>, Error> {
    let mut built_ms = None;
    let mut slot_only = false;
    let mut errors = Vec::new();
    if let Some(build) = build {
        let start = Instant::now();
        let wiring = run.checked.wiring();
        let toolchain = find_toolchain(folder, toolchain)?;
        let serial = run.serial;
        let building = |base, slot: Option<&str>| {
            out.emit(&Event::Building { serial, base, slot: slot.map(str::to_owned) });
        };
        let built = {
            let _build = lock(&BUILDING);
            compile(&mut run.checked, toolchain, build, loaded.as_mut(), building)
        };
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
            Err(Error::Compile(list)) if loaded.as_ref().is_some_and(|l| l.wiring == wiring) => errors = list,
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
    if !run.start {
        return Ok(Some(Event::Loaded { serial: run.serial, built_ms, fused, slot, errors }));
    }
    if params.ok {
        out.emit(&Event::Started { serial: run.serial });
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
    lock(&shared.event_reports).clear();
    let values = badges(&outcome.values);
    *lock(&shared.values) = outcome.values;
    let goes_on = params.ok && outcome.ok && !shared.control.stopped();
    let (frames, events) = (goes_on && (cluster.frames() || cluster.blocks()), goes_on && cluster.events());
    Ok(Some(Event::Ran { serial: run.serial, built_ms, slot_only, fused, slot, run_ms, ok: outcome.ok, frames, events, skipped: outcome.skipped, stopped: outcome.stopped, times: outcome.times, logs: outcome.logs, reports, values, errors }))
}
