//! The run of one domain on one thread, and the parts of a cluster for the frames and for the
//! audio thread (9.3).

use std::ffi::c_void;
use std::sync::atomic::Ordering::Relaxed;

use patina_sdk::audio::{BLOCK, Mix};
use patina_sdk::glue::{self, Control, Reporter};

use crate::library::{Calling, Library, NodeFn, Sink, clock, collect, lock};
use crate::outcome::{Outcome, Spare, fill};
use crate::node::Domain;
use crate::pool::{self, Work};

/// The reports and the outcome of the calls of one thread. A run fills them again, so it does
/// not allocate (9.1).
#[derive(Default)]
pub(crate) struct Calls {
    pub(crate) sink: Sink,
    pub(crate) out: Outcome,
    pub(crate) spare: Spare,
}

impl Calls {
    /// Calls `f` with a reporter that adds to the sink. Then fills the outcome from the sink.
    /// `node` is the node function of the called cluster, for a crash handler (16.1).
    pub(crate) fn call(&mut self, names: &[(String, String)], probes: &[String], control: *const Control, node: Option<NodeFn>, f: impl FnOnce(*const Reporter) -> bool) {
        let reporter = Reporter { ctx: (&raw mut self.sink).cast(), report: collect, control };
        let _calling = Calling::new(control, node);
        let ok = f(&reporter);
        fill(&mut self.out, &mut self.spare, ok, &self.sink, names, probes);
    }
}

/// The most bytes of user input that wait for the node GUIs, so that a node that a run holds for
/// a long time does not grow the list without end.
const MAX_WAITING: usize = 64 * 1024;

/// The calls of the node GUIs, and the user input that a GUI could not take yet (4.3).
#[derive(Default)]
pub(crate) struct Gui {
    calls: Calls,
    /// The node index, the mode and the event lines of each input that came while a run held the
    /// node.
    waiting: Vec<(usize, u32, String)>,
}

impl Gui {
    /// Keeps `events` for the next call of node `n` in mode `mode`. Drops them past the cap.
    fn keep(&mut self, n: usize, mode: u32, events: &str) {
        let size: usize = self.waiting.iter().map(|w| w.2.len()).sum();
        if !events.is_empty() && size + events.len() <= MAX_WAITING {
            self.waiting.push((n, mode, events.to_owned()));
        }
    }
}

/// What one thread needs for a run of one domain.
pub(crate) struct Run<'a> {
    pub(crate) library: Option<&'a Library>,
    /// False when the worker pool must not run the tasks.
    pub(crate) pool: bool,
    pub(crate) names: &'a [(String, String)],
    pub(crate) probes: &'a [String],
    pub(crate) calls: &'a mut Calls,
    pub(crate) pace: &'a mut Pace,
}

impl Run<'_> {
    pub(crate) fn domain(&mut self, domain: Domain, control: &Control) {
        let Some(library) = self.library else {
            self.calls.sink.clear();
            self.calls.call(self.names, self.probes, control, None, |_| true);
            return;
        };
        let begin = clock();
        let rep = Reporter { ctx: (&raw mut self.calls.sink).cast(), report: collect, control };
        // SAFETY: `state` is live, no task runs, and the reporter is valid during the call.
        let begun = unsafe { (library.begin)(library.state, domain as u32, &rep) };
        let tasks = &library.tasks;
        tasks.state.store(library.state, Relaxed);
        tasks.control.store(std::ptr::from_ref(control).cast_mut(), Relaxed);
        tasks.ok.store(true, Relaxed);
        let start = std::time::Instant::now();
        let pooled = self.pace.pool(self.pool && library.parallel, tasks.inputs.len());
        if pooled {
            pool::run(tasks.clone());
        } else {
            (0..tasks.inputs.len()).for_each(|task| tasks.run(task));
        }
        self.pace.took(pooled, start.elapsed().as_secs_f64() * 1000.0);
        tasks.control.store(std::ptr::null_mut(), Relaxed);
        self.calls.sink.clear();
        self.calls.sink.begin = begin;
        for task in &tasks.sinks {
            self.calls.sink.append(&mut lock(task));
        }
        let (state, end, ok) = (library.state, library.end, tasks.ok.load(Relaxed));
        // SAFETY: `state` is live, no task runs, and the reporter and the control are valid
        // during the call.
        self.calls.call(self.names, self.probes, control, Some(library.node), |rep| unsafe { end(state, domain as u32, rep) });
        self.calls.out.ok &= ok && begun;
        // A quiet run sends no times (9.1), so the last measure stays.
        if !control.quiet() {
            self.pace.work = self.calls.out.times.iter().map(|t| t.ms).sum();
        }
    }
}

/// The frame part of a cluster (`Cluster::frame_part`, `Cluster::split`): the frames, the node
/// GUIs and the reads of the views. It also runs the blocks when no other thread runs them.
pub struct Frames<'a> {
    pub(crate) run: Run<'a>,
    /// The calls of `ui`, apart from the frames, so that the outcome of the last frame stays.
    pub(crate) gui: &'a mut Gui,
    pub(crate) nodes: Nodes<'a>,
    /// The audio domain, when this thread runs the blocks between the frames.
    pub(crate) audio: Option<Audio<'a>>,
    pub(crate) mode: crate::Mode,
}

impl Frames<'_> {
    /// Runs one frame, as `Cluster::frame`.
    pub fn frame(&mut self, control: &Control) -> &Outcome {
        self.run.domain(Domain::Frame, control);
        &self.run.calls.out
    }

    /// Runs one block on this thread, as `Cluster::block`. `None` when the graph has no audio
    /// nodes, or when the audio thread runs the blocks.
    pub fn block(&mut self, control: &Control) -> Option<&Outcome> {
        self.audio.as_mut().map(|audio| audio.block(control))
    }

    /// The samples of the last block of `block`, as `Cluster::sound`.
    pub fn sound(&self) -> Option<&[f32; BLOCK]> {
        self.audio.as_ref()?.sound()
    }

    /// The outcome of the last frame.
    pub fn outcome(&self) -> &Outcome {
        &self.run.calls.out
    }

    /// Calls the `ui` function of node `id` (4.3), in mode `mode` (0 compact, 1 large, 2 full).
    /// `events` has a line for each user input: the widget index and the value. Returns the
    /// widget lines, or `None` when the node has no GUI or failed (see the outcome), and the
    /// parameter writes of the GUI in the outcome. While the audio thread runs, an audio node
    /// gives `None` (4.3). While the event runs go on, an event node gives `None` when the cluster
    /// is not parallel, or while a run holds its state. Then the input waits, and the next call of
    /// the node in the same mode applies it first. During the call, `control` keeps the node, so a
    /// second **Stop** can end a GUI that does not return (15.2).
    pub fn ui(&mut self, control: &Control, id: &str, mode: u32, events: &str) -> (Option<String>, Outcome) {
        if let Some(g) = self.nodes.guis.iter().position(|(g, _)| g == id) {
            return self.ui_at(control, g, mode, events);
        }
        let Some(n) = self.run.names.iter().position(|(n, _)| n == id) else { return (None, Outcome::default()) };
        self.node_ui(control, n, false, mode, events)
    }

    /// Calls the `ui` function of the node at place `g` of `guis`, as `ui` does, with no search for the node.
    pub fn ui_at(&mut self, control: &Control, g: usize, mode: u32, events: &str) -> (Option<String>, Outcome) {
        match self.nodes.gui_nodes.get(g) {
            Some(&n) => self.node_ui(control, n, true, mode, events),
            None => (None, Outcome::default()),
        }
    }

    /// The `ui` calls of node `n`. A node with a GUI state (`keep`) keeps an input that waits.
    fn node_ui(&mut self, control: &Control, n: usize, keep: bool, mode: u32, events: &str) -> (Option<String>, Outcome) {
        // One call for each waiting input, in order, so that each click counts.
        let mut earlier = Outcome::default();
        while let Some(i) = self.gui.waiting.iter().position(|w| w.0 == n && w.1 == mode) {
            let text = self.gui.waiting.remove(i).2;
            let (tree, out, wait) = self.call(control, n, mode, &text);
            earlier = join(earlier, out);
            if wait {
                self.gui.waiting.insert(i, (n, mode, text));
                if keep {
                    self.gui.keep(n, mode, events);
                }
                return (None, earlier);
            }
            if tree.is_none() {
                return (None, earlier);
            }
        }
        let (tree, out, wait) = self.call(control, n, mode, events);
        if wait && keep {
            self.gui.keep(n, mode, events);
        }
        (tree, join(earlier, out))
    }

    /// One call of the `ui` function of node `n`, and true when the node cannot take the input
    /// now: a run holds its state, or its cluster is not callable.
    fn call(&mut self, control: &Control, n: usize, mode: u32, events: &str) -> (Option<String>, Outcome, bool) {
        let (names, probes) = (self.run.names, self.run.probes);
        let (Some(library), Ok(node)) = (self.nodes.owner(n), u32::try_from(n)) else { return (None, Outcome::default(), true) };
        let (state, ui, own) = (library.state, library.ui, library.node);
        let calls = &mut self.gui.calls;
        calls.sink.clear();
        // SAFETY: `state` is live, `&mut self` shows that no run of this thread is active, the
        // audio thread never calls the library of a node that is not an audio node, the event runs
        // share only a parallel library with this call (`Nodes::event_guis`), and `events` and the
        // reporter are valid during the call.
        calls.call(names, probes, control, Some(own), |rep| unsafe { ui(state, node, mode, events.as_ptr(), events.len(), rep) });
        let tree = calls.sink.iter().find(|&(_, kind, _)| kind == glue::UI).map(|(_, _, text)| text.to_owned());
        // A GUI call reports little, so the copy is small, and `calls.out` keeps its buffers.
        let mut out = calls.out.clone();
        let edits = calls.sink.iter().filter(|&(_, kind, _)| kind == glue::EDIT).filter_map(|(_, _, text)| text.split_once('\t'));
        out.params = edits.map(|(key, text)| (key.to_owned(), text.to_owned())).collect();
        // A call that succeeds with no widget lines found the state held by a run (`glue::draw`).
        let wait = out.ok && tree.is_none();
        (tree, out, wait)
    }

    /// The kept copy of output `key` of node `id` for a view (15.8 rule 3), as a pointer to a
    /// `Box<dyn Any + Send>`, or null. The first read of an output only asks the cluster to keep
    /// copies. The pointer is valid until the next run. While the audio thread runs, an output of
    /// an audio node gives null.
    pub fn read(&mut self, id: &str, key: &str) -> *const c_void {
        let is = |probe: &String| probe.strip_prefix(id).and_then(|rest| rest.strip_prefix('.')) == Some(key);
        let (Some(index), Some(n)) = (self.run.probes.iter().position(is), self.run.names.iter().position(|(n, _)| n == id)) else {
            return std::ptr::null();
        };
        let (Some(library), Ok(index)) = (self.nodes.library(n, false), u32::try_from(index)) else { return std::ptr::null() };
        // A sink of its own, so the outcome of the last run stays.
        let mut sink = Sink::default();
        let rep = Reporter { ctx: (&raw mut sink).cast(), report: collect, control: std::ptr::null() };
        // SAFETY: the state is live, `&mut self` shows that no run of this thread is active, the
        // audio thread never calls the library of a node that is not an audio node, and the
        // reporter is valid during the call.
        unsafe { (library.read)(library.state, index, &rep) }
    }

    /// The build mode of the cluster. A view that reads typed outputs builds in the same mode, so
    /// it links the same build of each type crate (15.8 rule 3).
    pub fn mode(&self) -> crate::Mode {
        self.mode
    }

    /// The ID of each node with a GUI, and true when the node runs again after batch (4.3).
    pub fn guis(&self) -> &[(String, bool)] {
        self.nodes.guis
    }
}

/// `later` with the reports, the logs and the parameter writes of `earlier` before its own.
fn join(mut earlier: Outcome, mut later: Outcome) -> Outcome {
    prepend(&mut earlier.reports, &mut later.reports);
    prepend(&mut earlier.logs, &mut later.logs);
    prepend(&mut earlier.params, &mut later.params);
    later
}

/// Puts the items of `first` before the items of `list`. It does not allocate when `first` is empty.
fn prepend<T>(first: &mut Vec<T>, list: &mut Vec<T>) {
    if !first.is_empty() {
        first.append(list);
        std::mem::swap(first, list);
    }
}

/// What the node GUIs and the views of the frame part need to find the library of a node.
pub(crate) struct Nodes<'a> {
    pub(crate) libraries: &'a [Option<Library>; 4],
    pub(crate) slot: Option<&'a Library>,
    pub(crate) slotted: &'a [bool],
    pub(crate) domains: &'a [Domain],
    pub(crate) guis: &'a [(String, bool)],
    pub(crate) gui_nodes: &'a [usize],
    /// False while the audio thread runs the audio domain (`Cluster::split`).
    pub(crate) audio: bool,
    /// False while the event runs call the event domain (`Cluster::live`).
    pub(crate) events: bool,
    /// True when the event runs call the cluster from many threads at a time (`Events::new`), so the
    /// GUI of an event node can draw while they go on (4.3).
    pub(crate) event_guis: bool,
}

impl Nodes<'_> {
    /// False for an audio node while the audio thread runs, for an event node while the event
    /// runs go on, and for an unknown node. A `gui` call of an event node can run with parallel
    /// event runs.
    fn callable(&self, n: usize, gui: bool) -> bool {
        self.domains.get(n).is_some_and(|&domain| match domain {
            Domain::Audio => self.audio,
            Domain::Event => self.events || (gui && self.event_guis),
            _ => true,
        })
    }

    /// The cluster of the domain of node `n`.
    fn library(&self, n: usize, gui: bool) -> Option<&Library> {
        self.callable(n, gui).then(|| self.libraries[self.domains[n] as usize].as_ref()).flatten()
    }

    /// The library that keeps the state of node `n`, for its GUI: the slot library for a node in
    /// the slot.
    fn owner(&self, n: usize) -> Option<&Library> {
        match self.slotted.get(n)? {
            true => self.slot.filter(|_| self.callable(n, true)),
            false => self.library(n, true),
        }
    }
}

/// The audio domain of a split cluster, for the audio thread (9.3).
pub struct Audio<'a> {
    pub(crate) run: Run<'a>,
    pub(crate) mix: &'static Mix,
    pub(crate) sound: &'a mut [f32; BLOCK],
}

// SAFETY: while the split lasts, only the thread that holds `Audio` calls the audio cluster, and
// `Frames` never calls it: its `ui` and `read` refuse the audio nodes. `Cluster::split` and
// `Cluster::live` give an `Audio` only when the audio library and the slot library are parallel,
// so each value that the audio thread keeps or reads is `Sync`. Else `Frames` keeps the `Audio`,
// and `Frames` is not `Send`. The
// two domains share only the latest-value wires, which take one writer and one reader without a
// lock (9.3), the batch values, which do not change while the split lasts, and the slot.
unsafe impl Send for Audio<'_> {}

impl Audio<'_> {
    /// Runs one block, as `Cluster::block`.
    pub fn block(&mut self, control: &Control) -> &Outcome {
        self.run.domain(Domain::Audio, control);
        self.mix.take(self.sound);
        &self.run.calls.out
    }

    /// The samples of the last block, as `Cluster::sound`.
    pub fn sound(&self) -> Option<&[f32; BLOCK]> {
        self.mix.played().then_some(&*self.sound)
    }
}

/// The times of the last runs of a domain, to choose the pool or the calling thread (9.2).
#[derive(Clone, Copy)]
pub(crate) struct Pace {
    /// The sum of the node run times of the last run, in ms.
    work: f64,
    /// The mean wall time of the tasks with the pool and on the calling thread, in ms. 0 is not
    /// measured.
    taken: [f64; 2],
    runs: u32,
}

impl Default for Pace {
    fn default() -> Self {
        Pace { work: f64::INFINITY, taken: [0.0; 2], runs: 0 }
    }
}

impl Pace {
    /// Spec 9.2: the pool runs only when the nodes of the last run took 0.1 ms or more. Then the
    /// faster of the two ways runs, and each 64th run measures the other way again.
    fn pool(&self, parallel: bool, tasks: usize) -> bool {
        let [pool, alone] = self.taken;
        parallel && tasks > 1 && self.work >= 0.1 && ((pool <= alone) != (self.runs % 64 == 63))
    }

    fn took(&mut self, pooled: bool, ms: f64) {
        let mean = &mut self.taken[usize::from(!pooled)];
        *mean = if *mean == 0.0 { ms } else { (*mean + ms) / 2.0 };
        self.runs = self.runs.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pool_runs_when_it_is_faster() {
        let mut pace = Pace::default();
        assert!(pace.pool(true, 2), "the first run");
        assert!(!pace.pool(false, 2) && !pace.pool(true, 1));
        pace.took(true, 0.9);
        // The second run measures the calling thread.
        assert!(!pace.pool(true, 2));
        pace.took(false, 0.5);
        assert!(!pace.pool(true, 2), "the calling thread is faster");
        (2..63).for_each(|_| pace.took(false, 0.5));
        assert!(pace.pool(true, 2), "each 64th run measures the pool again");
        pace.took(true, 0.05);
        assert!(pace.pool(true, 2), "now the pool is faster");
        pace.work = 0.05;
        assert!(!pace.pool(true, 2), "a light run");
    }
}
