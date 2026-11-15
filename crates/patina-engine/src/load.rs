//! Loads a cluster library and calls it through the C ABI of `patina_sdk::glue`.

use std::collections::{BTreeMap, HashMap};
use std::ffi::c_void;
use std::fmt;
use std::ops::Range;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use patina_sdk::glue::{self, Control, Reporter};
use serde::Serialize;

use crate::Error;
use crate::node::Domain;
use crate::plan::Task;
use crate::pool::{self, Work};
use crate::project::GraphFile;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ReportKind {
    /// The node returned an `Err` (4.7).
    #[default]
    Error,
    /// The node panicked (4.7).
    Panic,
    /// A parameter text did not read back (4.8).
    BadParam,
}

/// A problem of one node during a call into the cluster.
#[derive(Serialize, Clone, Debug, Default)]
pub struct Report {
    pub id: String,
    /// The node folder name.
    pub folder: String,
    pub kind: ReportKind,
    pub text: String,
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            ReportKind::Error => "error: ",
            ReportKind::Panic => "",
            ReportKind::BadParam => "bad parameter: ",
        };
        write!(f, "Node {} ({}): {kind}{}", self.id, self.folder, self.text)
    }
}

/// The live value of an output after a run (15.9).
#[derive(Serialize, Clone, Debug, Default)]
pub struct Value {
    /// The output, as "<node id>.<output key>".
    pub from: String,
    /// The `Debug` text of the value, or its type name and size (5.3).
    pub text: String,
    /// The lanes from the connection kind of the type (6.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lanes: Option<Lanes>,
}

/// The lanes of a live value (6.3).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Lanes {
    /// One '0' or '1' for each lane, the most significant lane first.
    pub bits: String,
    /// The name of the bus, or an empty text.
    pub label: String,
}

impl Lanes {
    /// Reads the text of a `LANES` report into `lanes`, and keeps the texts of old lanes. A
    /// defective cluster can send a bad text.
    fn read(text: Option<&str>, lanes: &mut Option<Lanes>) {
        let valid = |bits: &str| (1..=64).contains(&bits.len()) && bits.bytes().all(|b| b == b'0' || b == b'1');
        match (text.and_then(|t| t.split_once(' ')).filter(|(bits, _)| valid(bits)), lanes.as_mut()) {
            (Some((bits, label)), Some(old)) => {
                put(&mut old.bits, bits);
                put(&mut old.label, label);
            }
            (Some((bits, label)), None) => *lanes = Some(Lanes { bits: bits.to_owned(), label: label.to_owned() }),
            (None, _) => *lanes = None,
        }
    }
}

/// The run time of one node call (15.7).
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct NodeTime {
    pub id: String,
    pub ms: f64,
}

/// A log line of a node, from the `log` crate (15.7).
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct NodeLog {
    pub id: String,
    /// The node folder name.
    pub folder: String,
    /// "ERROR", "WARN" or "INFO".
    pub level: String,
    pub text: String,
}

impl fmt::Display for NodeLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node {} ({}): {} {}", self.id, self.folder, self.level, self.text)
    }
}

/// The result of one call: false when a node failed or the cluster failed with no report.
#[derive(Default)]
pub struct Outcome {
    pub ok: bool,
    pub reports: Vec<Report>,
    pub values: Vec<Value>,
    /// The ID of each node that did not run, because a wired input had no value (15.3).
    pub skipped: Vec<String>,
    /// The ID of each node that did not start, because the user clicked **Stop** (15.2).
    pub stopped: Vec<String>,
    /// The run time of each node call that started (15.7).
    pub times: Vec<NodeTime>,
    /// The log lines of the nodes, in the order of the run (15.7).
    pub logs: Vec<NodeLog>,
}

type VersionFn = unsafe extern "C" fn() -> u32;
type NewFn = unsafe extern "C" fn() -> *mut c_void;
type SetFn = unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *const Reporter) -> bool;
type ParallelFn = unsafe extern "C" fn(*const c_void) -> bool;
type TaskFn = unsafe extern "C" fn(*const c_void, u32, *const Reporter) -> bool;
type EndFn = unsafe extern "C" fn(*const c_void, u32, *const Reporter) -> bool;
type FreeFn = unsafe extern "C" fn(*mut c_void);

/// A loaded cluster with its parameters and its node states.
pub struct Cluster {
    state: *mut c_void,
    new: NewFn,
    set: SetFn,
    end: EndFn,
    free: FreeFn,
    /// True when the tasks of a run can run on many threads at a time (9.2).
    parallel: bool,
    /// The tasks of the batch and of the frame domain (9.1).
    domains: [Arc<Tasks>; 2],
    /// The ID and the folder name of each node, by node index.
    names: Vec<(String, String)>,
    /// The node ID and the input key of each parameter slot.
    slots: Vec<(String, String)>,
    /// The output of each held value, as "<node id>.<output key>" (`Plan::held`).
    probes: Vec<String>,
    /// True for each slot that got a text at the last `set_params`.
    has_text: Vec<bool>,
    /// The reports of the last call.
    sink: Sink,
    /// The outcome of the last call. A frame fills it again, so it does not allocate (9.1).
    out: Outcome,
}

/// The reports of one call: the node, the kind and the text range of each report, and the texts
/// in one buffer. It is the context of the reporter. The buffers keep their capacity.
#[derive(Default)]
struct Sink {
    items: Vec<(u32, u32, Range<usize>)>,
    text: String,
}

impl Sink {
    fn push(&mut self, node: u32, kind: u32, text: &str) {
        let start = self.text.len();
        self.text.push_str(text);
        self.items.push((node, kind, start..self.text.len()));
    }

    fn clear(&mut self) {
        self.items.clear();
        self.text.clear();
    }

    /// Moves the reports of `other` to the end of this sink.
    fn append(&mut self, other: &mut Sink) {
        let start = self.text.len();
        self.text.push_str(&other.text);
        self.items.extend(other.items.drain(..).map(|(node, kind, r)| (node, kind, r.start + start..r.end + start)));
        other.text.clear();
    }

    fn iter(&self) -> impl Iterator<Item = (u32, u32, &str)> {
        self.items.iter().map(|(node, kind, r)| (*node, *kind, &self.text[r.clone()]))
    }
}

unsafe extern "C" fn collect(ctx: *mut c_void, node: u32, kind: u32, text: *const u8, len: usize) {
    // SAFETY: `ctx` is the sink of `Cluster::call`, and `text` points to `len` bytes during the
    // call (glue::Reporter).
    let (sink, bytes) = unsafe { (&mut *ctx.cast::<Sink>(), std::slice::from_raw_parts(text, len)) };
    sink.push(node, kind, &String::from_utf8_lossy(bytes));
}

/// The tasks of a run of one domain (9.2), and a report list for each task. The worker pool
/// calls them.
struct Tasks {
    state: AtomicPtr<c_void>,
    control: AtomicPtr<Control>,
    task: TaskFn,
    /// The cluster task index of each task.
    ids: Vec<usize>,
    inputs: Vec<u32>,
    next: Vec<Vec<usize>>,
    sinks: Vec<Mutex<Sink>>,
    ok: AtomicBool,
}

// SAFETY: `Cluster::run_domain` sets the state and the control before the tasks start, and keeps
// them valid until all tasks are done. Many threads call tasks at a time only when the cluster says
// that each value that it keeps is `Sync` (`patina_cluster_parallel`).
unsafe impl Send for Tasks {}
// SAFETY: the same as for `Send`.
unsafe impl Sync for Tasks {}

impl Work for Tasks {
    fn inputs(&self) -> &[u32] {
        &self.inputs
    }

    fn next(&self, task: usize) -> &[usize] {
        &self.next[task]
    }

    fn run(&self, task: usize) {
        let index = self.ids.get(task).and_then(|&id| u32::try_from(id).ok());
        let (Some(sink), Some(index)) = (self.sinks.get(task), index) else { return };
        let mut sink = lock(sink);
        let rep = Reporter { ctx: (&raw mut *sink).cast(), report: collect, control: self.control.load(Relaxed) };
        // SAFETY: the same as for `unsafe impl Send for Tasks`. The reporter and its sink are valid
        // during the call.
        if !unsafe { (self.task)(self.state.load(Relaxed), index, &rep) } {
            self.ok.store(false, Relaxed);
        }
    }
}

impl Tasks {
    /// The tasks of `all` in `domain`, in the same order. A link to another domain is not a wait.
    fn new(state: *mut c_void, task: TaskFn, all: &[Task], domain: Domain) -> Tasks {
        let ids: Vec<usize> = (0..all.len()).filter(|&t| all[t].domain == domain).collect();
        let next: Vec<Vec<usize>> =
            ids.iter().map(|&t| all[t].next.iter().filter_map(|n| ids.iter().position(|i| i == n)).collect()).collect();
        let mut inputs = vec![0; ids.len()];
        for &n in next.iter().flatten() {
            inputs[n] += 1;
        }
        let sinks = ids.iter().map(|_| Mutex::new(Sink::default())).collect();
        let (state, control) = (AtomicPtr::new(state), AtomicPtr::new(std::ptr::null_mut()));
        Tasks { state, control, task, ids, inputs, next, sinks, ok: AtomicBool::new(true) }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Cluster {
    pub(crate) fn load(
        path: &Path,
        names: Vec<(String, String)>,
        slots: Vec<(String, String)>,
        probes: Vec<String>,
        tasks: &[Task],
    ) -> Result<Cluster, Error> {
        let fail = |text: String| Error::Load(format!("{}: {text}", path.display()));
        let missing = |e: libloading::Error| fail(e.to_string());
        let library = open(path).map_err(missing)?;
        // SAFETY: Patina generated the cluster with these symbols and signatures (generate.rs).
        let cluster = unsafe {
            let version: VersionFn = symbol(&library, b"patina_abi_version\0").map_err(missing)?;
            if version() != glue::ABI_VERSION {
                return Err(fail("the cluster has another ABI version. Build it again".to_owned()));
            }
            let new: NewFn = symbol(&library, b"patina_cluster_new\0").map_err(missing)?;
            let set: SetFn = symbol(&library, b"patina_cluster_set\0").map_err(missing)?;
            let parallel: ParallelFn = symbol(&library, b"patina_cluster_parallel\0").map_err(missing)?;
            let task: TaskFn = symbol(&library, b"patina_cluster_task\0").map_err(missing)?;
            let end: EndFn = symbol(&library, b"patina_cluster_end\0").map_err(missing)?;
            let free: FreeFn = symbol(&library, b"patina_cluster_free\0").map_err(missing)?;
            let state = new();
            if state.is_null() {
                return Err(fail("a parameter default panicked".to_owned()));
            }
            let domains = [Domain::Batch, Domain::Frame].map(|d| Arc::new(Tasks::new(state, task, tasks, d)));
            let (parallel, has_text) = (parallel(state), vec![false; slots.len()]);
            let (sink, out) = (Sink::default(), Outcome::default());
            Cluster { state, new, set, end, free, parallel, domains, names, slots, probes, has_text, sink, out }
        };
        // Rust libraries do not unload safely, so the library stays loaded (8.5).
        std::mem::forget(library);
        Ok(cluster)
    }

    /// Sets each parameter from its text in the graph (4.8). An input that lost its text gets its
    /// default value again.
    pub fn set_params(&mut self, graph: &GraphFile) -> Outcome {
        let params: HashMap<&str, &BTreeMap<String, String>> =
            graph.node.iter().map(|n| (n.id.as_str(), &n.params)).collect();
        let texts: Vec<Option<&String>> =
            self.slots.iter().map(|(id, key)| params.get(id.as_str()).and_then(|p| p.get(key))).collect();
        if self.has_text.iter().zip(&texts).any(|(had, text)| *had && text.is_none()) {
            // SAFETY: the functions came from the loaded cluster, and `self.state` is live.
            let state = unsafe { (self.new)() };
            if state.is_null() {
                return Outcome::default();
            }
            // SAFETY: the old state came from `patina_cluster_new` and is not used again.
            unsafe { (self.free)(self.state) };
            self.state = state;
        }
        let mut all = Outcome { ok: true, ..Outcome::default() };
        for (slot, text) in (0u32..).zip(&texts) {
            let Some(text) = text else { continue };
            let (state, set) = (self.state, self.set);
            self.sink.clear();
            // SAFETY: `state` is live, `text` is valid during the call, and so is the reporter.
            self.call(std::ptr::null(), |rep| unsafe { set(state, slot, text.as_ptr(), text.len(), rep) });
            let key = &self.slots[slot as usize].1;
            all.ok &= self.out.ok;
            all.reports.extend(self.out.reports.drain(..).map(|r| Report { text: format!("input `{key}`: {}", r.text), ..r }));
        }
        self.has_text = texts.iter().map(Option::is_some).collect();
        all
    }

    /// Runs the batch domain of the graph one time. Another thread can ask the run to stop with
    /// `control` (15.2). The reports come in task order (9.2).
    pub fn run(&mut self, control: &Control) -> Outcome {
        self.run_domain(Domain::Batch, control);
        // The batch run is not a hot path, so it gives its lists to the caller.
        std::mem::take(&mut self.out)
    }

    /// Runs the frame domain of the graph one time: one frame (9.1). After the first frames, it
    /// does not allocate, because it fills the lists of the last outcome again.
    pub fn frame(&mut self, control: &Control) -> &Outcome {
        self.run_domain(Domain::Frame, control);
        &self.out
    }

    /// True when the graph has nodes in the frame domain.
    pub fn frames(&self) -> bool {
        !self.domains[Domain::Frame as usize].ids.is_empty()
    }

    fn run_domain(&mut self, domain: Domain, control: &Control) {
        let tasks = &self.domains[domain as usize];
        tasks.state.store(self.state, Relaxed);
        tasks.control.store(std::ptr::from_ref(control).cast_mut(), Relaxed);
        tasks.ok.store(true, Relaxed);
        if self.parallel && tasks.inputs.len() > 1 {
            pool::run(tasks.clone());
        } else {
            (0..tasks.inputs.len()).for_each(|task| tasks.run(task));
        }
        tasks.control.store(std::ptr::null_mut(), Relaxed);
        self.sink.clear();
        for task in &tasks.sinks {
            self.sink.append(&mut lock(task));
        }
        let (state, end, ok) = (self.state, self.end, tasks.ok.load(Relaxed));
        // SAFETY: `state` is live, no task runs, and the reporter and the control are valid
        // during the call.
        self.call(control, |rep| unsafe { end(state, domain as u32, rep) });
        self.out.ok &= ok;
    }

    /// The ID of each node, by the node index of `Control::node`.
    pub fn node_ids(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(|(id, _)| id.as_str())
    }

    /// Calls `f` with a reporter that adds to the sink. Then fills the outcome from the sink.
    fn call(&mut self, control: *const Control, f: impl FnOnce(*const Reporter) -> bool) {
        let reporter = Reporter { ctx: (&raw mut self.sink).cast(), report: collect, control };
        let ok = f(&reporter);
        fill(&mut self.out, ok, &self.sink, &self.names, &self.probes);
    }
}

/// Splits the reports of one call into the problems of nodes and the live values. It fills the
/// lists of `out` again and keeps their texts, so it does not allocate after the first frames.
fn fill(out: &mut Outcome, ok: bool, sink: &Sink, names: &[(String, String)], probes: &[String]) {
    let (mut reports, mut values, mut skipped, mut stopped, mut times, mut logs) = (0, 0, 0, 0, 0, 0);
    let mut items = sink.iter().peekable();
    while let Some((index, kind, text)) = items.next() {
        let name = names.get(index as usize);
        match kind {
            glue::SKIPPED | glue::STOPPED => {
                let (list, used) = if kind == glue::SKIPPED { (&mut out.skipped, &mut skipped) } else { (&mut out.stopped, &mut stopped) };
                if let Some((id, _)) = name {
                    put(reuse(list, used), id);
                }
            }
            glue::VALUE => {
                // Only a defective cluster sends an unknown index, and its value has no wire to show on.
                let Some(from) = probes.get(index as usize) else { continue };
                let value = reuse(&mut out.values, &mut values);
                put(&mut value.from, from);
                put(&mut value.text, text);
                // The lanes come after the text of the same value.
                let lanes = items.next_if(|&(i, k, _)| (i, k) == (index, glue::LANES));
                Lanes::read(lanes.map(|(_, _, text)| text), &mut value.lanes);
            }
            // Lanes with no value before them come only from a defective cluster.
            glue::LANES => {}
            glue::TIME => {
                // A defective cluster can send a bad text or index. Its time has no node to show on.
                if let (Some((id, _)), Ok(ns)) = (name, text.parse::<u64>()) {
                    let time = reuse(&mut out.times, &mut times);
                    put(&mut time.id, id);
                    time.ms = ns as f64 / 1e6;
                }
            }
            glue::LOG => {
                // A defective cluster can send a bad text or index. Its line has no node to show on.
                if let (Some((id, folder)), Some((level, text))) = (name, text.split_once(' ')) {
                    let log = reuse(&mut out.logs, &mut logs);
                    put(&mut log.id, id);
                    put(&mut log.folder, folder);
                    put(&mut log.level, level);
                    put(&mut log.text, text);
                }
            }
            _ => {
                let report = reuse(&mut out.reports, &mut reports);
                let (id, folder) = name.map_or(("", ""), |(id, folder)| (id.as_str(), folder.as_str()));
                put(&mut report.id, id);
                put(&mut report.folder, folder);
                put(&mut report.text, text);
                report.kind = match kind {
                    glue::PANIC => ReportKind::Panic,
                    glue::BAD_PARAM => ReportKind::BadParam,
                    _ => ReportKind::Error,
                };
            }
        }
    }
    out.ok = ok;
    out.reports.truncate(reports);
    out.values.truncate(values);
    out.skipped.truncate(skipped);
    out.stopped.truncate(stopped);
    out.times.truncate(times);
    out.logs.truncate(logs);
}

/// The next item of `list` to fill: an old item that keeps its texts, or a new item.
fn reuse<'a, T: Default>(list: &'a mut Vec<T>, used: &mut usize) -> &'a mut T {
    if *used == list.len() {
        list.push(T::default());
    }
    *used += 1;
    &mut list[*used - 1]
}

/// Puts `text` in `to`, and keeps the capacity of `to`.
fn put(to: &mut String, text: &str) {
    to.clear();
    to.push_str(text);
}

impl Drop for Cluster {
    fn drop(&mut self) {
        // SAFETY: `state` came from `patina_cluster_new` and is freed only here.
        unsafe { (self.free)(self.state) }
    }
}

/// Reads a symbol of the library as a function pointer.
///
/// # Safety
///
/// The symbol has the type `T`.
unsafe fn symbol<T: Copy>(library: &libloading::Library, name: &[u8]) -> Result<T, libloading::Error> {
    // SAFETY: the caller gives the type of the symbol.
    unsafe { library.get::<T>(name).map(|s| *s) }
}

#[cfg(windows)]
fn open(path: &Path) -> Result<libloading::Library, libloading::Error> {
    use libloading::os::windows::{LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, Library};
    // Only the folder of the library and the system folders give other libraries (8.5).
    let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS;
    // SAFETY: the load runs the init code of the library. Patina built it from a project that
    // the owner trusts (11).
    unsafe { Library::load_with_flags(path, flags) }.map(libloading::Library::from)
}

#[cfg(not(windows))]
fn open(path: &Path) -> Result<libloading::Library, libloading::Error> {
    // SAFETY: the load runs the init code of the library. Patina built it from a project that
    // the owner trusts (11).
    unsafe { libloading::Library::new(path) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_go_apart_from_the_reports() {
        let names = [("a1".to_owned(), "number".to_owned())];
        let probes = ["a1.out".to_owned()];
        let sink = |items: &[(u32, u32, &str)]| {
            let mut sink = Sink::default();
            items.iter().for_each(|&(node, kind, text)| sink.push(node, kind, text));
            sink
        };
        let run = sink(&[
            (0, glue::VALUE, "2.5"),
            (0, glue::LANES, "0110 data bus"),
            (7, glue::VALUE, "x"),
            (0, glue::PANIC, "p"),
            (0, glue::TIME, "2500000"),
            (0, glue::TIME, "x"),
            (5, glue::TIME, "1"),
            (0, glue::LOG, "INFO two words"),
            (0, glue::LOG, "INFO"),
            (5, glue::LOG, "INFO x"),
        ]);
        let mut out = Outcome::default();
        fill(&mut out, false, &run, &names, &probes);
        assert!(out.skipped.is_empty() && out.stopped.is_empty());
        assert_eq!(out.logs.iter().map(ToString::to_string).collect::<Vec<_>>(), ["Node a1 (number): INFO two words"]);
        assert_eq!(out.times, [NodeTime { id: "a1".to_owned(), ms: 2.5 }]);
        assert_eq!(out.values.len(), 1);
        assert_eq!((out.values[0].from.as_str(), out.values[0].text.as_str()), ("a1.out", "2.5"));
        assert_eq!(out.values[0].lanes, Some(Lanes { bits: "0110".to_owned(), label: "data bus".to_owned() }));
        for bad in [None, Some("0110"), Some("012 x"), Some(" x"), Some(&format!("{} x", "1".repeat(65)))] {
            let mut lanes = out.values[0].lanes.clone();
            Lanes::read(bad, &mut lanes);
            assert_eq!(lanes, None, "{bad:?}");
        }
        assert_eq!(out.reports.len(), 1);
        assert_eq!((out.reports[0].folder.as_str(), out.reports[0].kind), ("number", ReportKind::Panic));

        // A second fill keeps the texts of the first fill, and a shorter list drops the rest.
        let text = out.values[0].text.as_ptr();
        fill(&mut out, false, &run, &names, &probes);
        assert_eq!(out.values[0].text.as_ptr(), text, "the text of the value keeps its buffer");
        fill(&mut out, true, &sink(&[(0, glue::SKIPPED, ""), (0, glue::STOPPED, ""), (9, glue::STOPPED, "")]), &names, &probes);
        assert!(out.ok && out.values.is_empty() && out.reports.is_empty() && out.times.is_empty() && out.logs.is_empty());
        assert_eq!((&out.skipped[..], &out.stopped[..]), (&["a1".to_owned()][..], &["a1".to_owned()][..]));
    }
}
