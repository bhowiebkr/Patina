//! The runs of the event domain (spec 9.4): the triggers, their settings and a thread for each run.

use std::collections::VecDeque;
use std::ffi::c_void;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use patina_sdk::glue::{Control, Reporter};

use crate::library::{FireFn, Library, Sink, collect, lock};
use crate::load::{Outcome, Report, ReportKind, fill};

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
    /// True for `schedule`, false for `trigger`.
    pub(crate) schedule: bool,
    every: Duration,
    overlap: Overlap,
}

impl Trigger {
    pub(crate) fn new(node: usize, task: u32, schedule: bool) -> Trigger {
        Trigger { node, task, schedule, every: Duration::from_millis(1000), overlap: Overlap::Queue }
    }

    /// Sets the defaults again, before the parameters of the graph.
    pub(crate) fn reset(&mut self) {
        *self = Trigger::new(self.node, self.task, self.schedule);
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
            _ => {}
        }
        Ok(())
    }
}

/// The result of one event run.
#[derive(Default)]
pub struct EventRun {
    /// The run ID, from 1 at each start of the event domain. 0 for a fire that the trigger
    /// dropped, because it had the most runs.
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
    /// The time of the next fire. None when the trigger does not fire again.
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

/// Runs the event domain of `library` (9.4) until `count` runs ended, until no trigger can fire
/// again, or until `control` asks to stop. Each `trigger` node fires one time at the start with
/// an empty text. `done` gets each run when it ends. Returns false when a run failed.
pub(crate) fn run_all(
    library: &Library,
    triggers: &[Trigger],
    names: &[(String, String)],
    probes: &[String],
    count: u64,
    control: &Control,
    mut done: impl FnMut(&EventRun),
) -> bool {
    let shared = Shared {
        entry: Entry { fire: library.fire, state: library.state },
        gate: (!library.parallel).then(|| Mutex::new(())),
        names,
        probes,
    };
    let start = Instant::now();
    let mut live: Vec<Live> = triggers.iter().map(|_| Live { next: Some(start), ..Live::default() }).collect();
    let (send, receive) = mpsc::channel::<(usize, u64, Outcome)>();
    let (mut started, mut next_id, mut all_ok) = (0u64, 1u64, true);
    let mut controls: Vec<(u64, Arc<Control>)> = Vec::new();
    std::thread::scope(|scope| {
        let spawn = |t: usize, id: u64, text: String, controls: &mut Vec<(u64, Arc<Control>)>| {
            let (shared, back, task) = (&shared, send.clone(), triggers[t].task);
            let run_control = Arc::new(Control::default());
            controls.push((id, Arc::clone(&run_control)));
            // shortcut: one thread for each run (9.4), change it to a pool when runs are short and many.
            let thread = std::thread::Builder::new().name(format!("patina-run-{id}")).spawn_scoped(scope, move || {
                let out = shared.run(task, &text, &run_control);
                let _ = back.send((t, id, out));
            });
            if let Err(e) = thread {
                let report = Report { id: names[triggers[t].node].0.clone(), text: format!("run {id} did not start: {e}"), ..Report::default() };
                let _ = send.send((t, id, Outcome { reports: vec![report], ..Outcome::default() }));
            }
        };
        loop {
            let now = Instant::now();
            let stopped = control.stopped();
            for (t, trigger) in triggers.iter().enumerate() {
                let Some(next) = live[t].next.filter(|&next| next <= now && !stopped && started < count) else { continue };
                live[t].next = trigger.schedule.then(|| (next + trigger.every).max(now));
                let busy = live[t].active + live[t].waiting.len();
                if busy > 0 && trigger.overlap == Overlap::Skip {
                    continue;
                }
                if busy >= MOST {
                    let (id, folder) = &names[trigger.node];
                    let text = format!("the trigger has {MOST} runs, so it dropped a fire (spec 9.4)");
                    let report = Report { id: id.clone(), folder: folder.clone(), kind: ReportKind::Error, text };
                    let out = Outcome { reports: vec![report], ..Outcome::default() };
                    done(&EventRun { id: 0, trigger: id.clone(), out });
                    all_ok = false;
                    continue;
                }
                let (id, text) = (next_id, if trigger.schedule { next_id.to_string() } else { String::new() });
                next_id += 1;
                started += 1;
                if busy > 0 && trigger.overlap == Overlap::Queue {
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
            let firing = !stopped && started < count && live.iter().any(|l| l.next.is_some());
            if idle && !firing {
                break;
            }
            let soonest = live.iter().filter_map(|l| l.next).min().filter(|_| firing);
            let wait = soonest.map_or(CHECK, |next| next.saturating_duration_since(now).min(CHECK));
            let Ok((t, id, out)) = receive.recv_timeout(wait) else { continue };
            live[t].active -= 1;
            controls.retain(|(c, _)| *c != id);
            all_ok &= out.ok;
            done(&EventRun { id, trigger: names[triggers[t].node].0.clone(), out });
            if let Some((id, text)) = live[t].waiting.pop_front() {
                live[t].active += 1;
                spawn(t, id, text, &mut controls);
            }
        }
    });
    all_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_settings_read_their_text() {
        let mut trigger = Trigger::new(0, 0, true);
        assert!(trigger.set("every_ms", " 250 ").is_ok() && trigger.set("overlap", "skip").is_ok());
        assert_eq!((trigger.every, trigger.overlap), (Duration::from_millis(250), Overlap::Skip));
        for (key, bad) in [("every_ms", "0"), ("every_ms", "x"), ("overlap", "later")] {
            assert!(trigger.set(key, bad).is_err(), "{key} {bad}");
        }
        trigger.reset();
        assert_eq!((trigger.every, trigger.overlap), (Duration::from_millis(1000), Overlap::Queue));
    }
}
