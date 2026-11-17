//! The runs of the event domain (spec 9.4): the triggers, their settings and a thread for each
//! active run.

mod watch;
mod webhook;

use std::collections::VecDeque;
use std::ffi::c_void;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed, Ordering::SeqCst};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use patina_sdk::glue::{Control, Reporter};

use crate::library::{Calling, FireFn, Library, Sink, collect, lock};
use crate::load::{Outcome, Report, ReportKind, fill};
use crate::node::Builtin;

/// The most waiting and active runs of one trigger (9.4).
const MOST: usize = 16;
/// The longest wait of the dispatcher, so that it sees **Stop** in time.
const CHECK: Duration = Duration::from_millis(20);

/// What a fire does while a run of the same trigger is active (9.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Overlap {
    Queue,
    Both,
    Skip,
}

/// A trigger node of the graph and its settings.
#[derive(Clone, Debug)]
pub(crate) struct Trigger {
    /// The node index.
    pub(crate) node: usize,
    /// The index of the task of its runs.
    pub(crate) task: u32,
    /// `schedule`, `trigger`, `file_change` or `webhook`.
    kind: Builtin,
    every: Duration,
    overlap: Overlap,
    /// The file or the folder of `file_change`, or the URL path of `webhook`.
    path: String,
    port: u16,
}

impl Trigger {
    pub(crate) fn new(node: usize, task: u32, kind: Builtin) -> Trigger {
        let mut trigger = Trigger { node, task, kind, every: Duration::ZERO, overlap: Overlap::Queue, path: String::new(), port: 0 };
        trigger.reset();
        trigger
    }

    /// Sets the defaults again, before the parameters of the graph.
    pub(crate) fn reset(&mut self) {
        self.every = Duration::from_millis(if self.kind == Builtin::FileChange { 500 } else { 1000 });
        self.overlap = Overlap::Queue;
        self.path = if self.kind == Builtin::Webhook { "/".to_owned() } else { String::new() };
        self.port = 8686;
    }

    /// Sets input `key` from its parameter text (4.8).
    pub(crate) fn set(&mut self, key: &str, text: &str) -> Result<(), String> {
        match key {
            "every_ms" => match text.trim().parse::<u64>() {
                Ok(ms) if ms > 0 => self.every = Duration::from_millis(ms),
                _ => return Err(format!("{text:?} is not a number of milliseconds, 1 or more")),
            },
            "overlap" => {
                self.overlap = match text.trim() {
                    "queue" => Overlap::Queue,
                    "both" => Overlap::Both,
                    "skip" => Overlap::Skip,
                    _ => return Err(format!("{text:?} is not queue, both or skip")),
                }
            }
            "path" if self.kind == Builtin::Webhook && !text.trim().starts_with('/') => {
                return Err(format!("{text:?} is not a URL path that starts with /"));
            }
            "path" => self.path = text.trim().to_owned(),
            "port" => match text.trim().parse::<u16>() {
                Ok(port) if port > 0 => self.port = port,
                _ => return Err(format!("{text:?} is not a port from 1 to 65535")),
            },
            _ => {}
        }
        Ok(())
    }
}

/// The result of one event run.
#[derive(Default)]
pub struct EventRun {
    /// The run ID, from 1 at each start of the event domain. 0 for a report of the trigger, for
    /// example a fire that it dropped because it had the most runs.
    pub id: u64,
    /// The node ID of the trigger.
    pub trigger: String,
    pub out: Outcome,
}

/// The event function and the state of the event cluster, for the run threads.
#[derive(Clone, Copy)]
struct Entry {
    fire: FireFn,
    state: *mut c_void,
}

// SAFETY: the cluster lets other threads call `patina_cluster_fire` (glue::fire). When it keeps a
// value that is not `Sync`, the gate of `Events::run` lets one run call it at a time. While
// `Cluster::live` lasts, the frame part refuses the event nodes, and the event domain shares only
// the batch values with the other domains (plan::reaches), which do not change.
unsafe impl Send for Entry {}
// SAFETY: as for `Send`. The entry is only read.
unsafe impl Sync for Entry {}

/// The live runs of one trigger.
#[derive(Default)]
struct Live {
    active: usize,
    waiting: VecDeque<(u64, String)>,
    /// The time of the next fire or scan. None when the trigger does not fire again by time.
    next: Option<Instant>,
}

/// What the run threads read.
struct Shared<'a> {
    entry: Entry,
    /// Held for each call when the cluster keeps a value that is not `Sync`.
    gate: Option<Mutex<()>>,
    names: &'a [(String, String)],
    probes: &'a [String],
}

impl Shared<'_> {
    /// Runs one event run on this thread and fills its outcome.
    fn run(&self, task: u32, text: &str, control: &Control) -> Outcome {
        let mut sink = Sink::default();
        let rep = Reporter { ctx: (&raw mut sink).cast(), report: collect, control };
        let _calling = Calling::new(control);
        let gate = self.gate.as_ref().map(lock);
        // SAFETY: the state is live while the cluster is borrowed, `text` and the reporter are
        // valid during the call, and the gate keeps the rule of `parallel` (Entry).
        let ok = unsafe { (self.entry.fire)(self.entry.state, task, text.as_ptr(), text.len(), &rep) };
        drop(gate);
        let mut out = Outcome::default();
        fill(&mut out, ok, &sink, self.names, self.probes);
        out
    }
}

/// A message to the dispatcher of `Events::run`.
enum Msg {
    /// Run `id` of trigger `t` ended.
    Done(usize, u64, Outcome),
    /// Trigger `t` got an event with this text.
    Fire(usize, String),
    /// The editor or a view fires the `trigger` node with this ID, with this text.
    Outside(String, String),
    /// No new run starts. The active runs finish (15.2).
    End,
}

/// Fires the `trigger` nodes of a live event domain from other threads (9.4).
#[derive(Clone)]
pub struct Remote {
    send: mpsc::Sender<Msg>,
    runs: Runs,
}

/// A run for a thread of the pool: the trigger, the run ID, the text and the control of the run.
type Job = (usize, u64, String, Arc<Control>);

/// The ID and the control of each active event run.
type Runs = Arc<Mutex<Vec<(u64, Arc<Control>)>>>;

impl Remote {
    /// Fires the `trigger` node `id` with `text`. Returns false when the event domain does not run.
    pub fn fire(&self, id: &str, text: String) -> bool {
        self.send.send(Msg::Outside(id.to_owned(), text)).is_ok()
    }

    /// Ends the event domain: no new run starts, and the active runs finish.
    pub fn end(&self) {
        let _ = self.send.send(Msg::End);
    }

    /// The index of the node that runs in an active event run, so the editor can name a node that
    /// does not stop (15.2). None when no event run is in a node.
    pub fn running(&self) -> Option<u32> {
        lock(&self.runs).iter().find_map(|(_, control)| control.node())
    }

    /// As `running`, but it never waits for a lock, for a crash handler.
    pub fn try_running(&self) -> Option<u32> {
        self.runs.try_lock().ok()?.iter().find_map(|(_, control)| control.node())
    }
}

/// The messages of a `Remote`, for `Events::run`.
pub struct Inbox {
    send: mpsc::Sender<Msg>,
    receive: mpsc::Receiver<Msg>,
    runs: Runs,
}

/// A remote and the inbox that gets its messages.
pub fn remote() -> (Remote, Inbox) {
    let ((send, receive), runs) = (mpsc::channel(), Runs::default());
    (Remote { send: send.clone(), runs: Arc::clone(&runs) }, Inbox { send, receive, runs })
}

/// The event domain of a cluster (9.4), ready to run on its own threads.
#[derive(Clone, Copy)]
pub struct Events<'a> {
    entry: Entry,
    /// False when the event library or the slot library keeps a value that is not `Sync`, so one
    /// run calls the cluster at a time.
    parallel: bool,
    triggers: &'a [Trigger],
    names: &'a [(String, String)],
    probes: &'a [String],
    /// The start of a relative path of a `file_change` trigger.
    root: &'a Path,
}

impl<'a> Events<'a> {
    pub(crate) fn new(library: &Library, slot_parallel: bool, triggers: &'a [Trigger], names: &'a [(String, String)], probes: &'a [String], root: &'a Path) -> Events<'a> {
        let entry = Entry { fire: library.fire, state: library.state };
        Events { entry, parallel: library.parallel && slot_parallel, triggers, names, probes, root }
    }

    /// Runs the event domain until `count` runs ended, until no trigger can fire again, or until
    /// `control` asks to stop. With no `inbox`, each `trigger` node fires one time at the start with
    /// an empty text. With an `inbox`, its remote fires them, and the run ends only at
    /// `Remote::end` or a stop. With no `inbox` and a quiet `control`, nobody shows the values, so
    /// the runs send no values and no run times (9.1). `done` gets each run when it ends. Returns false when a run failed
    /// or a trigger could not start.
    pub fn run(self, count: u64, control: &Control, inbox: Option<Inbox>, mut done: impl FnMut(&EventRun)) -> bool {
        let Events { entry, parallel, triggers, names, probes, root } = self;
        let report = |t: usize, text: String| {
            let (id, folder) = &names[triggers[t].node];
            let report = Report { id: id.clone(), folder: folder.clone(), kind: ReportKind::Error, text };
            EventRun { id: 0, trigger: id.clone(), out: Outcome { reports: vec![report], ..Outcome::default() } }
        };
        // The watches take their first scan, and the webhooks listen, before the first run.
        let mut watches = Vec::new();
        let mut listeners = Vec::new();
        for (t, trigger) in triggers.iter().enumerate() {
            let problem = match trigger.kind {
                Builtin::FileChange if trigger.path.is_empty() => Some("input `path`: give the file or the folder to watch".to_owned()),
                Builtin::Webhook => webhook::bind(trigger.port).map(|l| listeners.push((t, l))).err().map(|e| format!("cannot listen on port {}: {e}", trigger.port)),
                _ => None,
            };
            if let Some(text) = problem {
                done(&report(t, text));
                return false;
            }
            watches.push((trigger.kind == Builtin::FileChange).then(|| watch::Watch::new(root.join(&trigger.path))));
        }
        let shared = Shared { entry, gate: (!parallel).then(|| Mutex::new(())), names, probes };
        let start = Instant::now();
        let remote = inbox.is_some();
        let first = |trigger: &Trigger| match trigger.kind {
            Builtin::FileChange => Some(start + trigger.every),
            Builtin::Webhook => None,
            Builtin::Trigger if remote => None,
            _ => Some(start),
        };
        let mut live: Vec<Live> = triggers.iter().map(|t| Live { next: first(t), ..Live::default() }).collect();
        let (endless, quit) = (remote || !listeners.is_empty(), AtomicBool::new(false));
        let Inbox { send, receive, runs: controls } = inbox.unwrap_or_else(|| {
            let ((send, receive), runs) = (mpsc::channel(), Runs::default());
            Inbox { send, receive, runs }
        });
        let (mut started, mut next_id, mut all_ok, mut ended) = (0u64, 1u64, true, false);
        // The text of each fire. None gives the run ID, for a schedule.
        let mut fires: Vec<(usize, Option<String>)> = Vec::new();
        // A finished run gives its thread to the next run. `idle` counts the threads that wait for a job.
        let (jobs, queue) = mpsc::channel::<Job>();
        let (queue, idle) = (Mutex::new(queue), AtomicUsize::new(0));
        std::thread::scope(|scope| {
            for (t, listener) in listeners {
                let (back, quit, path) = (send.clone(), &quit, triggers[t].path.as_str());
                scope.spawn(move || webhook::serve(&listener, path, quit, |text| back.send(Msg::Fire(t, text)).is_ok()));
            }
            let spawn = |t: usize, id: u64, text: String| {
                let run_control = Arc::new(Control::default());
                run_control.set_quiet(!remote && control.quiet());
                lock(&controls).push((id, Arc::clone(&run_control)));
                let job = (t, id, text, run_control);
                // The count goes down before the send, so each job in the queue has a thread. Only
                // this thread takes from the count, so the load and the sub do not race.
                if idle.load(SeqCst) > 0 {
                    idle.fetch_sub(1, SeqCst);
                    let _ = jobs.send(job);
                    return;
                }
                let (shared, back, queue, idle) = (&shared, send.clone(), &queue, &idle);
                let thread = std::thread::Builder::new().name("patina-run".to_owned()).spawn_scoped(scope, move || {
                    let mut job = job;
                    loop {
                        let (t, id, text, run_control) = job;
                        let out = shared.run(triggers[t].task, &text, &run_control);
                        idle.fetch_add(1, SeqCst);
                        let _ = back.send(Msg::Done(t, id, out));
                        match lock(queue).recv() {
                            Ok(next) => job = next,
                            Err(_) => break,
                        }
                    }
                });
                if let Err(e) = thread {
                    let report = Report { id: names[triggers[t].node].0.clone(), text: format!("run {id} did not start: {e}"), ..Report::default() };
                    let _ = send.send(Msg::Done(t, id, Outcome { reports: vec![report], ..Outcome::default() }));
                }
            };
            loop {
                let now = Instant::now();
                let stopped = control.stopped();
                let halt = stopped || ended || started >= count;
                for (t, trigger) in triggers.iter().enumerate() {
                    let Some(next) = live[t].next.filter(|&next| next <= now && !halt) else { continue };
                    let again = matches!(trigger.kind, Builtin::Schedule | Builtin::FileChange);
                    live[t].next = again.then(|| (next + trigger.every).max(now));
                    match &mut watches[t] {
                        Some(watch) => fires.extend(watch.changes().into_iter().map(|path| (t, Some(path)))),
                        None => fires.push((t, (trigger.kind != Builtin::Schedule).then(String::new))),
                    }
                }
                for (t, text) in fires.drain(..) {
                    if stopped || ended || started >= count {
                        break;
                    }
                    let busy = live[t].active + live[t].waiting.len();
                    if busy > 0 && triggers[t].overlap == Overlap::Skip {
                        continue;
                    }
                    if busy >= MOST {
                        done(&report(t, format!("the trigger has {MOST} runs, so it dropped a fire (spec 9.4)")));
                        all_ok = false;
                        continue;
                    }
                    let id = next_id;
                    let text = text.unwrap_or_else(|| id.to_string());
                    next_id += 1;
                    started += 1;
                    if busy > 0 && triggers[t].overlap == Overlap::Queue {
                        live[t].waiting.push_back((id, text));
                    } else {
                        live[t].active += 1;
                        spawn(t, id, text);
                    }
                }
                if stopped {
                    lock(&controls).iter().for_each(|(_, c)| c.set_stop(true));
                }
                if stopped || ended {
                    live.iter_mut().for_each(|l| l.waiting.clear());
                }
                let idle = live.iter().all(|l| l.active == 0 && l.waiting.is_empty());
                let firing = !stopped && !ended && started < count && (endless || live.iter().any(|l| l.next.is_some()));
                if idle && !firing {
                    break;
                }
                let soonest = live.iter().filter_map(|l| l.next).min().filter(|_| firing);
                let wait = soonest.map_or(CHECK, |next| next.saturating_duration_since(now).min(CHECK));
                match receive.recv_timeout(wait) {
                    Ok(Msg::Done(t, id, out)) => {
                        live[t].active -= 1;
                        lock(&controls).retain(|(c, _)| *c != id);
                        all_ok &= out.ok;
                        done(&EventRun { id, trigger: names[triggers[t].node].0.clone(), out });
                        if let Some((id, text)) = live[t].waiting.pop_front() {
                            live[t].active += 1;
                            spawn(t, id, text);
                        }
                    }
                    Ok(Msg::Fire(t, text)) => fires.push((t, Some(text))),
                    Ok(Msg::Outside(id, text)) => match triggers.iter().position(|t| t.kind == Builtin::Trigger && names[t.node].0 == id) {
                        Some(t) => fires.push((t, Some(text))),
                        None => {
                            let text = "the running graph has no trigger node with this ID, so run the graph again".to_owned();
                            let report = Report { id: id.clone(), folder: "trigger".to_owned(), kind: ReportKind::Error, text };
                            done(&EventRun { id: 0, trigger: id, out: Outcome { reports: vec![report], ..Outcome::default() } });
                        }
                    },
                    Ok(Msg::End) => ended = true,
                    Err(_) => {}
                }
            }
            quit.store(true, Relaxed);
            // The threads of the pool end when the queue closes.
            drop(jobs);
        });
        all_ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_settings_read_their_text() {
        let mut trigger = Trigger::new(0, 0, Builtin::Schedule);
        assert!(trigger.set("every_ms", " 250 ").is_ok() && trigger.set("overlap", "skip").is_ok());
        assert_eq!((trigger.every, trigger.overlap), (Duration::from_millis(250), Overlap::Skip));
        for (key, bad) in [("every_ms", "0"), ("every_ms", "x"), ("overlap", "later"), ("port", "0"), ("port", "70000")] {
            assert!(trigger.set(key, bad).is_err(), "{key} {bad}");
        }
        trigger.reset();
        assert_eq!((trigger.every, trigger.overlap), (Duration::from_millis(1000), Overlap::Queue));
        let mut hook = Trigger::new(0, 0, Builtin::Webhook);
        assert_eq!((hook.path.as_str(), hook.port), ("/", 8686));
        assert!(hook.set("path", "hook").is_err() && hook.set("path", "/hook").is_ok() && hook.set("port", "9000").is_ok());
        assert_eq!((hook.path.as_str(), hook.port), ("/hook", 9000));
        assert_eq!(Trigger::new(0, 0, Builtin::FileChange).every, Duration::from_millis(500));
    }

    /// Gives a run that fails when the text is empty.
    unsafe extern "C" fn fire(_: *const c_void, _: u32, _: *const u8, len: usize, _: *const Reporter) -> bool {
        len > 0
    }

    /// Gives a run that fails when its control is not quiet.
    unsafe extern "C" fn quiet(_: *const c_void, _: u32, _: *const u8, _: usize, rep: *const Reporter) -> bool {
        // SAFETY: the engine gives a reporter that is valid during the call.
        unsafe { (*rep).quiet() }
    }

    #[test]
    fn the_runs_with_no_inbox_follow_the_quiet_control() {
        let triggers = [Trigger::new(0, 0, Builtin::Trigger)];
        let names = [("t".to_owned(), "trigger".to_owned())];
        let entry = Entry { fire: quiet, state: std::ptr::null_mut() };
        let events = Events { entry, parallel: true, triggers: &triggers, names: &names, probes: &[], root: Path::new(".") };
        let control = Control::default();
        assert!(!events.run(1, &control, None, |_| {}), "the C API reads the values");
        control.set_quiet(true);
        assert!(events.run(1, &control, None, |_| {}), "the CLI shows no values");
    }

    #[test]
    fn a_remote_fires_triggers_until_it_ends_the_runs() {
        let triggers = [Trigger::new(0, 0, Builtin::Trigger)];
        let names = [("t".to_owned(), "trigger".to_owned())];
        let entry = Entry { fire, state: std::ptr::null_mut() };
        let events = Events { entry, parallel: true, triggers: &triggers, names: &names, probes: &[], root: Path::new(".") };
        let (remote, inbox) = remote();
        // The trigger does not fire at the start. An unknown ID gives a report, and a fire after
        // the end does not start a run.
        assert!(remote.fire("t", "go".to_owned()) && remote.fire("x", String::new()));
        remote.end();
        assert!(remote.fire("t", "late".to_owned()));
        let mut runs = Vec::new();
        let ok = events.run(u64::MAX, &Control::default(), Some(inbox), |run| runs.push((run.id, run.trigger.clone(), run.out.ok, run.out.reports.len())));
        runs.sort();
        assert!(ok);
        assert_eq!(runs, [(0, "x".to_owned(), false, 1), (1, "t".to_owned(), true, 0)]);
        assert!(!remote.fire("t", String::new()), "the inbox is gone after the run");
    }

    static THREADS: Mutex<Vec<std::thread::ThreadId>> = Mutex::new(Vec::new());

    /// Gives a run that keeps the ID of its thread.
    unsafe extern "C" fn keep_thread(_: *const c_void, _: u32, _: *const u8, _: usize, _: *const Reporter) -> bool {
        lock(&THREADS).push(std::thread::current().id());
        true
    }

    #[test]
    fn the_runs_use_the_threads_of_the_pool_again() {
        let triggers = [Trigger::new(0, 0, Builtin::Trigger)];
        let names = [("t".to_owned(), "trigger".to_owned())];
        let entry = Entry { fire: keep_thread, state: std::ptr::null_mut() };
        let events = Events { entry, parallel: true, triggers: &triggers, names: &names, probes: &[], root: Path::new(".") };
        let (remote, inbox) = remote();
        // The fires queue, so one run is active at a time, and each run uses the thread of the last.
        // The domain ends after the count of runs, because an end clears the queue.
        for _ in 0..MOST {
            assert!(remote.fire("t", "go".to_owned()));
        }
        let mut count = 0;
        assert!(events.run(MOST as u64, &Control::default(), Some(inbox), |run| count += u64::from(run.out.ok)));
        let mut threads = lock(&THREADS).clone();
        threads.dedup();
        assert_eq!((count, threads.len()), (MOST as u64, 1));
    }

    static HOLD: AtomicBool = AtomicBool::new(true);

    /// Gives a run that is in node 0 until the test clears `HOLD`.
    unsafe extern "C" fn hold(_: *const c_void, _: u32, _: *const u8, _: usize, rep: *const Reporter) -> bool {
        // SAFETY: the run gives a reporter that is valid during the call.
        unsafe { &*rep }.enter(0, true);
        while HOLD.load(Relaxed) {
            std::thread::sleep(Duration::from_millis(5));
        }
        true
    }

    #[test]
    fn the_remote_names_the_node_of_an_active_run() {
        let triggers = [Trigger::new(0, 0, Builtin::Trigger)];
        let names = [("t".to_owned(), "trigger".to_owned())];
        let entry = Entry { fire: hold, state: std::ptr::null_mut() };
        let events = Events { entry, parallel: true, triggers: &triggers, names: &names, probes: &[], root: Path::new(".") };
        let (remote, inbox) = remote();
        std::thread::scope(|s| {
            s.spawn(|| events.run(u64::MAX, &Control::default(), Some(inbox), |_| {}));
            assert_eq!(remote.running(), None);
            assert!(remote.fire("t", "go".to_owned()));
            let end = Instant::now() + Duration::from_secs(10);
            while remote.running().is_none() {
                assert!(Instant::now() < end, "the remote names the node of the run");
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(remote.running(), Some(0));
            assert_eq!(remote.try_running(), Some(0));
            HOLD.store(false, Relaxed);
            remote.end();
        });
        assert_eq!(remote.running(), None);
    }
}
