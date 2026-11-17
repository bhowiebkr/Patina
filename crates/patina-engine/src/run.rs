//! The run of one domain on one thread, and the parts of a cluster for the frames and for the
//! audio thread (9.3).

use std::ffi::c_void;
use std::sync::atomic::Ordering::Relaxed;

use patina_sdk::audio::{BLOCK, Mix};
use patina_sdk::glue::{self, Control, Reporter};

use crate::library::{Calling, Library, Sink, collect, lock};
use crate::load::{Outcome, fill};
use crate::node::Domain;
use crate::pool::{self, Work};

/// The reports and the outcome of the calls of one thread. A run fills them again, so it does
/// not allocate (9.1).
#[derive(Default)]
pub(crate) struct Calls {
    pub(crate) sink: Sink,
    pub(crate) out: Outcome,
}

impl Calls {
    /// Calls `f` with a reporter that adds to the sink. Then fills the outcome from the sink.
    pub(crate) fn call(&mut self, names: &[(String, String)], probes: &[String], control: *const Control, f: impl FnOnce(*const Reporter) -> bool) {
        let reporter = Reporter { ctx: (&raw mut self.sink).cast(), report: collect, control };
        let _calling = Calling::new(control);
        let ok = f(&reporter);
        fill(&mut self.out, ok, &self.sink, names, probes);
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
    /// The sum of the node run times of the last run, in ms.
    pub(crate) work: &'a mut f64,
}

impl Run<'_> {
    pub(crate) fn domain(&mut self, domain: Domain, control: &Control) {
        let Some(library) = self.library else {
            self.calls.sink.clear();
            self.calls.call(self.names, self.probes, control, |_| true);
            return;
        };
        let rep = Reporter { ctx: (&raw mut self.calls.sink).cast(), report: collect, control };
        // SAFETY: `state` is live, no task runs, and the reporter is valid during the call.
        let begun = unsafe { (library.begin)(library.state, domain as u32, &rep) };
        let tasks = &library.tasks;
        tasks.state.store(library.state, Relaxed);
        tasks.control.store(std::ptr::from_ref(control).cast_mut(), Relaxed);
        tasks.ok.store(true, Relaxed);
        if use_pool(self.pool && library.parallel, tasks.inputs.len(), *self.work) {
            pool::run(tasks.clone());
        } else {
            (0..tasks.inputs.len()).for_each(|task| tasks.run(task));
        }
        tasks.control.store(std::ptr::null_mut(), Relaxed);
        self.calls.sink.clear();
        for task in &tasks.sinks {
            self.calls.sink.append(&mut lock(task));
        }
        let (state, end, ok) = (library.state, library.end, tasks.ok.load(Relaxed));
        // SAFETY: `state` is live, no task runs, and the reporter and the control are valid
        // during the call.
        self.calls.call(self.names, self.probes, control, |rep| unsafe { end(state, domain as u32, rep) });
        self.calls.out.ok &= ok && begun;
        *self.work = self.calls.out.times.iter().map(|t| t.ms).sum();
    }
}

/// The frame part of a cluster (`Cluster::frame_part`, `Cluster::split`): the frames, the node
/// GUIs and the reads of the views.
pub struct Frames<'a> {
    pub(crate) run: Run<'a>,
    pub(crate) nodes: Nodes<'a>,
}

impl Frames<'_> {
    /// Runs one frame, as `Cluster::frame`.
    pub fn frame(&mut self, control: &Control) -> &Outcome {
        self.run.domain(Domain::Frame, control);
        &self.run.calls.out
    }

    /// Calls the `ui` function of node `id` (4.3), in mode `mode` (0 compact, 1 large, 2 full).
    /// `events` has a line for each user input: the widget index and the value. Returns the
    /// widget lines, or `None` when the node has no GUI or failed (see the outcome). While the
    /// audio thread runs, an audio node gives `None` (4.3). During the call, `control` keeps the
    /// node, so a second **Stop** can end a GUI that does not return (15.2).
    pub fn ui(&mut self, control: &Control, id: &str, mode: u32, events: &str) -> (Option<String>, Outcome) {
        let (names, probes) = (self.run.names, self.run.probes);
        let Some(n) = names.iter().position(|(n, _)| n == id) else { return (None, Outcome::default()) };
        let (Some(library), Ok(node)) = (self.nodes.owner(n), u32::try_from(n)) else { return (None, Outcome::default()) };
        let (state, ui) = (library.state, library.ui);
        let calls = &mut *self.run.calls;
        calls.sink.clear();
        // SAFETY: `state` is live, `&mut self` shows that no run of this thread is active, the
        // audio thread never calls the library of a node that is not an audio node, and `events`
        // and the reporter are valid during the call.
        calls.call(names, probes, control, |rep| unsafe { ui(state, node, mode, events.as_ptr(), events.len(), rep) });
        let tree = calls.sink.iter().find(|&(_, kind, _)| kind == glue::UI).map(|(_, _, text)| text.to_owned());
        (tree, std::mem::take(&mut calls.out))
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
        let (Some(library), Ok(index)) = (self.nodes.library(n), u32::try_from(index)) else { return std::ptr::null() };
        // A sink of its own, so the outcome of the last run stays.
        let mut sink = Sink::default();
        let rep = Reporter { ctx: (&raw mut sink).cast(), report: collect, control: std::ptr::null() };
        // SAFETY: the state is live, `&mut self` shows that no run of this thread is active, the
        // audio thread never calls the library of a node that is not an audio node, and the
        // reporter is valid during the call.
        unsafe { (library.read)(library.state, index, &rep) }
    }

    /// The ID of each node with a GUI, and true when the node runs again after batch (4.3).
    pub fn guis(&self) -> &[(String, bool)] {
        self.nodes.guis
    }
}

/// What the node GUIs and the views of the frame part need to find the library of a node.
pub(crate) struct Nodes<'a> {
    pub(crate) libraries: &'a [Option<Library>; 4],
    pub(crate) slot: Option<&'a Library>,
    pub(crate) slotted: &'a [bool],
    pub(crate) domains: &'a [Domain],
    pub(crate) guis: &'a [(String, bool)],
    /// False while the audio thread runs the audio domain (`Cluster::split`).
    pub(crate) audio: bool,
    /// False while the event runs call the event domain (`Cluster::live`).
    pub(crate) events: bool,
}

impl Nodes<'_> {
    /// False for an audio node while the audio thread runs, for an event node while the event
    /// runs go on, and for an unknown node.
    fn callable(&self, n: usize) -> bool {
        self.domains.get(n).is_some_and(|&domain| match domain {
            Domain::Audio => self.audio,
            Domain::Event => self.events,
            _ => true,
        })
    }

    /// The cluster of the domain of node `n`.
    fn library(&self, n: usize) -> Option<&Library> {
        self.callable(n).then(|| self.libraries[self.domains[n] as usize].as_ref()).flatten()
    }

    /// The library that keeps the state of node `n`: the slot library for a node in the slot.
    fn owner(&self, n: usize) -> Option<&Library> {
        match self.slotted.get(n)? {
            true => self.slot.filter(|_| self.callable(n)),
            false => self.library(n),
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
// `Frames` never calls it: its `ui` and `read` refuse the audio nodes. `Cluster::split` allows a split only when the audio library and the
// slot library are parallel, so each value that the audio thread keeps or reads is `Sync`. The
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

/// Spec 9.2: the pool runs the tasks only when the nodes of the last run took 0.1 ms or more.
/// Under that, the wake of the workers costs more time than the pool saves.
fn use_pool(parallel: bool, tasks: usize, last_ms: f64) -> bool {
    parallel && tasks > 1 && last_ms >= 0.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_heavy_runs_use_the_pool() {
        assert!(use_pool(true, 2, f64::INFINITY), "the first run");
        assert!(!use_pool(true, 2, 0.05) && !use_pool(true, 1, 1.0) && !use_pool(false, 2, 1.0));
    }
}
