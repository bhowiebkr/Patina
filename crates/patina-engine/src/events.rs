//! The runs of the event domain (spec 9.4): the triggers, their settings and a thread for each run.

mod watch;
mod webhook;

use std::collections::VecDeque;
use std::ffi::c_void;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use patina_sdk::glue::{Control, Reporter};

use crate::library::{FireFn, Library, Sink, collect, lock};
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
// value that is not `Sync`, the gate of `run_all` lets one run call it at a time.
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

/// A message to the dispatcher of `run_all`.
enum Msg {
    /// Run `id` of trigger `t` ended.
    Done(usize, u64, Outcome),
    /// Trigger `t` got an event with this text.
    Fire(usize, String),
}

/// Runs the event domain of `library` (9.4) until `count` runs ended, until no trigger can fire
/// again, or until `control` asks to stop. Each `trigger` node fires one time at the start with
/// an empty text. A relative path of a `file_change` trigger starts at `root`. `done` gets each
/// run when it ends. Returns false when a run failed or a trigger could not start.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_all(
    library: &Library,
    triggers: &[Trigger],
    names: &[(String, String)],
    probes: &[String],
    root: &Path,
    count: u64,
    control: &Control,
    mut done: impl FnMut(&EventRun),
) -> bool {
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
    let shared = Shared {
        entry: Entry { fire: library.fire, state: library.state },
        gate: (!library.parallel).then(|| Mutex::new(())),
        names,
        probes,
    };
    let start = Instant::now();
    let first = |trigger: &Trigger| match trigger.kind {
        Builtin::FileChange => Some(start + trigger.every),
        Builtin::Webhook => None,
        _ => Some(start),
    };
    let mut live: Vec<Live> = triggers.iter().map(|t| Live { next: first(t), ..Live::default() }).collect();
    let (endless, quit) = (!listeners.is_empty(), AtomicBool::new(false));
    let (send, receive) = mpsc::channel::<Msg>();
    let (mut started, mut next_id, mut all_ok) = (0u64, 1u64, true);
    let mut controls: Vec<(u64, Arc<Control>)> = Vec::new();
    // The text of each fire. None gives the run ID, for a schedule.
    let mut fires: Vec<(usize, Option<String>)> = Vec::new();
    std::thread::scope(|scope| {
        for (t, listener) in listeners {
            let (back, quit, path) = (send.clone(), &quit, triggers[t].path.as_str());
            scope.spawn(move || webhook::serve(&listener, path, quit, |text| back.send(Msg::Fire(t, text)).is_ok()));
        }
        let spawn = |t: usize, id: u64, text: String, controls: &mut Vec<(u64, Arc<Control>)>| {
            let (shared, back, task) = (&shared, send.clone(), triggers[t].task);
            let run_control = Arc::new(Control::default());
            controls.push((id, Arc::clone(&run_control)));
            // shortcut: one thread for each run (9.4), change it to a pool when runs are short and many.
            let thread = std::thread::Builder::new().name(format!("patina-run-{id}")).spawn_scoped(scope, move || {
                let out = shared.run(task, &text, &run_control);
                let _ = back.send(Msg::Done(t, id, out));
            });
            if let Err(e) = thread {
                let report = Report { id: names[triggers[t].node].0.clone(), text: format!("run {id} did not start: {e}"), ..Report::default() };
                let _ = send.send(Msg::Done(t, id, Outcome { reports: vec![report], ..Outcome::default() }));
            }
        };
        loop {
            let now = Instant::now();
            let stopped = control.stopped();
            for (t, trigger) in triggers.iter().enumerate() {
                let Some(next) = live[t].next.filter(|&next| next <= now && !stopped && started < count) else { continue };
                let again = matches!(trigger.kind, Builtin::Schedule | Builtin::FileChange);
                live[t].next = again.then(|| (next + trigger.every).max(now));
                match &mut watches[t] {
                    Some(watch) => fires.extend(watch.changes().into_iter().map(|path| (t, Some(path)))),
                    None => fires.push((t, (trigger.kind != Builtin::Schedule).then(String::new))),
                }
            }
            for (t, text) in fires.drain(..) {
                if stopped || started >= count {
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
                    spawn(t, id, text, &mut controls);
                }
            }
            if stopped {
                controls.iter().for_each(|(_, c)| c.set_stop(true));
                live.iter_mut().for_each(|l| l.waiting.clear());
            }
            let idle = live.iter().all(|l| l.active == 0 && l.waiting.is_empty());
            let firing = !stopped && started < count && (endless || live.iter().any(|l| l.next.is_some()));
            if idle && !firing {
                break;
            }
            let soonest = live.iter().filter_map(|l| l.next).min().filter(|_| firing);
            let wait = soonest.map_or(CHECK, |next| next.saturating_duration_since(now).min(CHECK));
            match receive.recv_timeout(wait) {
                Ok(Msg::Done(t, id, out)) => {
                    live[t].active -= 1;
                    controls.retain(|(c, _)| *c != id);
                    all_ok &= out.ok;
                    done(&EventRun { id, trigger: names[triggers[t].node].0.clone(), out });
                    if let Some((id, text)) = live[t].waiting.pop_front() {
                        live[t].active += 1;
                        spawn(t, id, text, &mut controls);
                    }
                }
                Ok(Msg::Fire(t, text)) => fires.push((t, Some(text))),
                Err(_) => {}
            }
        }
        quit.store(true, Relaxed);
    });
    all_ok
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
}
