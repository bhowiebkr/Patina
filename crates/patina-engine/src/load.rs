//! Loads the cluster libraries of a graph and calls them through the C ABI of `patina_sdk::glue`.

use std::collections::{BTreeMap, HashMap};
use std::ffi::c_void;
use std::fmt;
use std::path::{Path, PathBuf};

use patina_sdk::audio::{BLOCK, Mix};
use patina_sdk::glue::{self, Control, Reporter, SlotRef};
use serde::Serialize;

use crate::events::{EventRun, Events, Trigger};
use crate::library::{Library, Sink, collect};
use crate::node::Domain;
use crate::plan::Plan;
use crate::pool;
use crate::project::GraphFile;
use crate::run::{Audio, Calls, Frames, Nodes, Run};
use crate::{Error, generate};

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

/// The loaded clusters of a graph, one for each clock domain (8.2), with their parameters and
/// their node states.
pub struct Cluster {
    /// The cluster of each domain, by `Domain` index. A domain with no step has none.
    libraries: [Option<Library>; 4],
    /// The slot library and the reference that the clusters call it with (7.1). The box keeps the
    /// address of the reference while the clusters keep it.
    slot: Option<(Library, Box<SlotRef>)>,
    /// True for each node in the slot, by node index.
    slotted: Vec<bool>,
    /// The shared part of the state keys of the last full build (`cargo::Built::common`).
    pub(crate) common: u64,
    /// A hash of the code of the clusters and of the signature of the node in the slot. A build
    /// of the slot alone is correct only while it does not change (7.1).
    pub(crate) base: u64,
    /// The build mode of the clusters (8.3).
    pub(crate) mode: crate::Mode,
    /// The domain of each node, by node index.
    domains: Vec<Domain>,
    /// The domain that gives, the domain that reads and the index of each link (8.2).
    links: Vec<(Domain, Domain, u32)>,
    /// The state key of each node, by node index (7.2). A node with no state has none.
    keys: Vec<Option<u64>>,
    /// The sum of the node run times of the last run of each domain, in ms.
    work: [f64; 4],
    /// The ID and the folder name of each node, by node index.
    names: Vec<(String, String)>,
    /// The ID of each node with a GUI, and true when the node runs again after batch (4.3).
    guis: Vec<(String, bool)>,
    /// The node index and the input key of each parameter slot.
    slots: Vec<(usize, String)>,
    /// The output of each held value, as "<node id>.<output key>" (`Plan::held`).
    probes: Vec<String>,
    /// True for each slot that got a text at the last `set_params`.
    has_text: Vec<bool>,
    /// The reports and the outcome of the last call of the batch and the frame domains.
    calls: Calls,
    /// The reports and the outcome of the last block, apart, so that an audio thread fills them (9.3).
    block_calls: Calls,
    /// The mix that the audio nodes of the project play into (9.3).
    mix: &'static Mix,
    /// The mix of the last block.
    sound: [f32; BLOCK],
    /// The trigger nodes of the event domain and their settings (9.4).
    triggers: Vec<Trigger>,
    /// The project root, where a relative path of a `file_change` trigger starts (9.4).
    root: PathBuf,
}

impl Cluster {
    /// Loads the cluster of each domain and the slot library. If one fails, none stays loaded (7.2).
    /// `root` is the project root of the nodes (10.3).
    pub(crate) fn load(
        libraries: &[(Domain, PathBuf)],
        slot: Option<&Path>,
        root: &Path,
        plan: &Plan,
        keys: Vec<Option<u64>>,
    ) -> Result<Cluster, Error> {
        let mut loaded: [Option<Library>; 4] = [None, None, None, None];
        for &(domain, ref path) in libraries {
            loaded[domain as usize] = Some(Library::load(path, root, &plan.tasks, domain)?);
        }
        let slot = slot.map(|path| slot_library(path, root)).transpose()?;
        let names = plan.steps.iter().map(|s| (s.id.clone(), plan.nodes[s.node].0.clone())).collect();
        let slots: Vec<(usize, String)> = plan.slots.iter().map(|s| (s.step, s.key.clone())).collect();
        let probes = plan
            .held()
            .map(|(n, k)| format!("{}.{}", plan.steps[n].id, plan.nodes[plan.steps[n].node].1.output_keys()[k]))
            .collect();
        let guis = (plan.steps.iter())
            .filter(|s| plan.nodes[s.node].1.ui && plan.nodes[s.node].1.state.is_some())
            .map(|s| (s.id.clone(), s.domain != Domain::Batch))
            .collect();
        let mut links = Vec::new();
        for (index, (n, k)) in (0u32..).zip(generate::links(plan)) {
            let from = plan.steps[n].domain;
            for to in Domain::ALL.into_iter().filter(|&d| d != from && generate::reads(plan, n, k, d)) {
                links.push((from, to, index));
            }
        }
        let mut sink = Sink::default();
        if !link(&loaded, &links, slot.as_ref().map(|(_, r)| &**r), &mut sink) {
            return Err(Error::Load("a cluster did not link a value of another cluster. Build it again".to_owned()));
        }
        if loaded.iter().flatten().any(|l| l.parallel) {
            pool::prepare(plan.tasks.len());
        }
        Ok(Cluster {
            libraries: loaded,
            slot,
            slotted: (0..plan.steps.len()).map(|n| plan.in_slot(n)).collect(),
            common: 0,
            base: 0,
            mode: crate::Mode::Edit,
            domains: plan.steps.iter().map(|s| s.domain).collect(),
            links,
            keys,
            work: [f64::INFINITY; 4],
            names,
            guis,
            has_text: vec![false; slots.len()],
            slots,
            probes,
            calls: Calls { sink, out: Outcome::default() },
            block_calls: Calls::default(),
            mix: glue::shared_in(&root.to_string_lossy(), Mix::default),
            sound: [0.0; BLOCK],
            triggers: (0..plan.steps.len())
                .filter(|&n| plan.is_trigger(n))
                .filter_map(|n| Some(Trigger::new(n, plan.steps[n].task as u32, plan.builtin(n)?)))
                .collect(),
            root: root.to_path_buf(),
        })
    }

    /// The build mode of the clusters (8.3). The slot library is always in edit mode.
    pub fn mode(&self) -> crate::Mode {
        self.mode
    }

    /// True when a node is in the slot (7.1).
    pub fn slotted(&self) -> bool {
        self.slot.is_some()
    }

    /// The cluster of the domain of node `n`.
    fn library(&self, n: usize) -> Option<&Library> {
        self.libraries[*self.domains.get(n)? as usize].as_ref()
    }

    /// The library that keeps the state of node `n`: the slot library for a node in the slot.
    fn owner(&self, n: usize) -> Option<&Library> {
        match self.slotted.get(n)? {
            true => self.slot.as_ref().map(|(library, _)| library),
            false => self.library(n),
        }
    }

    /// Puts the slot library at `path` in place of the active one (7.1). The nodes in the slot
    /// get new states and the state keys `keys`. On a failure, the active slot library stays.
    pub(crate) fn swap_slot(&mut self, path: &Path, root: &Path, keys: Vec<Option<u64>>) -> Result<(), Error> {
        let slot = slot_library(path, root)?;
        if !link(&self.libraries, &[], Some(&slot.1), &mut self.calls.sink) {
            if let Some((_, old)) = &self.slot {
                link(&self.libraries, &[], Some(old), &mut self.calls.sink);
            }
            return Err(Error::Load("a cluster did not take the slot library. Build it again".to_owned()));
        }
        self.slot = Some(slot);
        for (n, key) in keys.into_iter().enumerate().filter(|&(n, _)| self.slotted.get(n) == Some(&true)) {
            self.keys[n] = key;
        }
        Ok(())
    }

    /// Sets each parameter from its text in the graph (4.8). An input that lost its text gets its
    /// default value again.
    pub fn set_params(&mut self, graph: &GraphFile) -> Outcome {
        let params: HashMap<&str, &BTreeMap<String, String>> =
            graph.node.iter().map(|n| (n.id.as_str(), &n.params)).collect();
        let texts: Vec<Option<&String>> =
            self.slots.iter().map(|(n, key)| params.get(self.names[*n].0.as_str()).and_then(|p| p.get(key))).collect();
        if self.has_text.iter().zip(&texts).any(|(had, text)| *had && text.is_none()) {
            // All clusters get new states, so that the links stay valid (8.2).
            let states: Vec<*mut c_void> = self.libraries.iter().flatten().map(Library::make).collect();
            if states.iter().any(|s| s.is_null()) {
                self.libraries.iter().flatten().zip(&states).for_each(|(library, &state)| library.free_state(state));
                return Outcome::default();
            }
            self.libraries.iter_mut().flatten().zip(states).for_each(|(library, state)| library.replace(state));
            if !link(&self.libraries, &self.links, self.slot.as_ref().map(|(_, r)| &**r), &mut self.calls.sink) {
                return Outcome::default();
            }
        }
        let mut all = Outcome { ok: true, ..Outcome::default() };
        self.triggers.iter_mut().for_each(Trigger::reset);
        for (slot, text) in (0u32..).zip(&texts) {
            let Some(text) = text else { continue };
            let (n, key) = &self.slots[slot as usize];
            // The engine reads the settings of a trigger (9.4).
            if let Some(trigger) = self.triggers.iter_mut().find(|t| t.node == *n) {
                if let Err(text) = trigger.set(key, text) {
                    let (id, folder) = self.names[*n].clone();
                    all.reports.push(Report { id, folder, kind: ReportKind::BadParam, text: format!("input `{key}`: {text}") });
                    all.ok = false;
                }
                continue;
            }
            let Some(library) = self.library(self.slots[slot as usize].0) else { continue };
            let (state, set) = (library.state, library.set);
            self.calls.sink.clear();
            // SAFETY: `state` is live, `text` is valid during the call, and so is the reporter.
            self.call(std::ptr::null(), |rep| unsafe { set(state, slot, text.as_ptr(), text.len(), rep) });
            let key = &self.slots[slot as usize].1;
            all.ok &= self.calls.out.ok;
            all.reports.extend(self.calls.out.reports.drain(..).map(|r| Report { text: format!("input `{key}`: {}", r.text), ..r }));
        }
        self.has_text = texts.iter().map(Option::is_some).collect();
        all
    }

    /// Runs the batch domain of the graph one time. Another thread can ask the run to stop with
    /// `control` (15.2). The reports come in task order (9.2).
    pub fn run(&mut self, control: &Control) -> Outcome {
        self.run_domain(Domain::Batch, control);
        // The batch run is not a hot path, so it gives its lists to the caller.
        std::mem::take(&mut self.calls.out)
    }

    /// Runs the frame domain of the graph one time: one frame (9.1). After the first frames, it
    /// does not allocate, because it fills the lists of the last outcome again.
    pub fn frame(&mut self, control: &Control) -> &Outcome {
        self.run_domain(Domain::Frame, control);
        &self.calls.out
    }

    /// Runs the audio domain of the graph one time: one block of `patina_sdk::audio::BLOCK`
    /// samples (9.3).
    pub fn block(&mut self, control: &Control) -> &Outcome {
        self.run_domain(Domain::Audio, control);
        self.mix.take(&mut self.sound);
        &self.block_calls.out
    }

    /// The samples that the nodes played in the last block, or `None` before the first node played.
    pub fn sound(&self) -> Option<&[f32; BLOCK]> {
        self.mix.played().then_some(&self.sound)
    }

    /// True when the graph has nodes in the frame domain.
    pub fn frames(&self) -> bool {
        self.has(Domain::Frame)
    }

    /// True when the graph has nodes in the audio domain.
    pub fn blocks(&self) -> bool {
        self.has(Domain::Audio)
    }

    /// True when the graph has nodes in the event domain (9.4).
    pub fn events(&self) -> bool {
        self.has(Domain::Event)
    }

    /// Runs the event domain (9.4) until `count` runs ended, until no trigger can fire again, or
    /// until `control` asks to stop. Each `trigger` node fires one time at the start with an
    /// empty text. `done` gets each run when it ends. Returns false when a run failed.
    pub fn run_events(&mut self, count: u64, control: &Control, done: impl FnMut(&EventRun)) -> bool {
        self.parts(true, true).3.is_none_or(|events| events.run(count, control, None, done))
    }

    fn has(&self, domain: Domain) -> bool {
        self.libraries[domain as usize].as_ref().is_some_and(|l| !l.tasks.ids.is_empty())
    }

    fn run_domain(&mut self, domain: Domain, control: &Control) {
        let audio = domain == Domain::Audio;
        // One thread runs all audio nodes (9.3), so the audio domain never uses the pool.
        let pool = !audio && self.slot.as_ref().is_none_or(|(slot, _)| slot.parallel);
        let calls = if audio { &mut self.block_calls } else { &mut self.calls };
        let (names, probes, work) = (&self.names, &self.probes, &mut self.work[domain as usize]);
        Run { library: self.libraries[domain as usize].as_ref(), pool, names, probes, calls, work }.domain(domain, control);
    }

    /// Splits the cluster into its frame domain and its audio domain, so that the audio domain
    /// runs on its own thread at the same time as the frames (9.3). `None` when the graph has no
    /// audio nodes, or when the audio library or the slot library keeps a value that is not `Sync`.
    pub fn split(&mut self) -> Option<(Frames<'_>, Audio<'_>)> {
        let audio = self.libraries[Domain::Audio as usize].as_ref();
        if !audio.is_some_and(|audio| audio.parallel) || self.slot.as_ref().is_some_and(|(slot, _)| !slot.parallel) {
            return None;
        }
        let mix = self.mix;
        let (frames, run, sound, _) = self.parts(false, true);
        Some((frames, Audio { run, mix, sound }))
    }

    /// Splits the cluster for a live run of the editor (9.3, 9.4): the frame part, the audio
    /// domain for its own thread when the graph has audio nodes, and the event domain for its own
    /// threads. `None` when the graph has no event nodes, or when the slot library or the audio
    /// library keeps a value that is not `Sync`.
    pub fn live(&mut self) -> Option<(Frames<'_>, Option<Audio<'_>>, Events<'_>)> {
        let audio = self.libraries[Domain::Audio as usize].as_ref();
        if !self.events() || audio.is_some_and(|audio| !audio.parallel) || self.slot.as_ref().is_some_and(|(slot, _)| !slot.parallel) {
            return None;
        }
        let (blocks, mix) = (self.blocks(), self.mix);
        let (frames, run, sound, events) = self.parts(false, false);
        Some((frames, blocks.then_some(Audio { run, mix, sound }), events?))
    }

    /// The frame part of the cluster: the frames, the node GUIs and the reads of the views,
    /// between two runs of the other domains.
    pub fn frame_part(&mut self) -> Frames<'_> {
        self.parts(true, true).0
    }

    /// The frame part, the run of the audio domain, the samples of the last block and the event
    /// domain. `audio` and `events` are false when other threads run these domains, so the frame
    /// part refuses their nodes.
    fn parts(&mut self, audio: bool, events: bool) -> (Frames<'_>, Run<'_>, &mut [f32; BLOCK], Option<Events<'_>>) {
        let [_, frame_work, audio_work, _] = &mut self.work;
        let [_, frame, audio_library, event_library] = &self.libraries;
        let (names, probes) = (&self.names, &self.probes);
        // The frames use the pool only when the slot library allows it (9.2).
        let pool = self.slot.as_ref().is_none_or(|(slot, _)| slot.parallel);
        let nodes = Nodes {
            libraries: &self.libraries,
            slot: self.slot.as_ref().map(|(library, _)| library),
            slotted: &self.slotted,
            domains: &self.domains,
            guis: &self.guis,
            audio,
            events,
        };
        let run = Run { library: frame.as_ref(), pool, names, probes, calls: &mut self.calls, work: frame_work };
        let blocks = Run { library: audio_library.as_ref(), pool: false, names, probes, calls: &mut self.block_calls, work: audio_work };
        let events = event_library.as_ref().map(|library| Events::new(library, pool, &self.triggers, names, probes, &self.root));
        (Frames { run, nodes }, blocks, &mut self.sound, events)
    }

    /// Calls the `ui` function of node `id` between two runs, as `Frames::ui` (4.3).
    pub fn ui(&mut self, id: &str, mode: u32, events: &str) -> (Option<String>, Outcome) {
        self.frame_part().ui(id, mode, events)
    }

    /// The kept copy of output `key` of node `id` for a view, as `Frames::read` (15.8).
    pub fn read(&mut self, id: &str, key: &str) -> *const c_void {
        self.frame_part().read(id, key)
    }

    /// The ID of each node with a GUI, and true when the node runs again after batch (4.3).
    pub fn guis(&self) -> &[(String, bool)] {
        &self.guis
    }

    /// Moves the state of each node from `old`, when the node ID and the state key did not change
    /// (7.2). Returns the number of states that moved. A node of `old` with no state yet gives none.
    pub fn adopt(&mut self, old: &mut Cluster) -> usize {
        let pairs: Vec<(usize, usize)> = (self.keys.iter().zip(&self.names).enumerate())
            .filter_map(|(n, (key, (id, _)))| {
                let from = old.keys.iter().zip(&old.names).position(|(k, (i, _))| key.is_some() && k == key && i == id)?;
                Some((from, n))
            })
            .collect();
        let mut moved = 0;
        for (from, to) in pairs {
            let (Some(source), Ok(from)) = (old.owner(from), u32::try_from(from)) else { continue };
            let (old_state, take, mut value) = (source.state, source.take, std::ptr::null_mut());
            old.call(std::ptr::null(), |rep| {
                // SAFETY: `old_state` is live, `&mut old` shows that no run is active, and the
                // reporter is valid during the call.
                value = unsafe { take(old_state, from, rep) };
                true
            });
            if value.is_null() {
                continue;
            }
            // shortcut: a node with no cluster or index loses its state. Only a defective plan gives this, so no free.
            let (Some(target), Ok(to)) = (self.owner(to), u32::try_from(to)) else { continue };
            let (state, put, mut ok) = (target.state, target.put, false);
            self.call(std::ptr::null(), |rep| {
                // SAFETY: `state` is live and no run is active. `value` came from `take` of a node
                // with the same state key, so it has the state type of node `to` (7.2). It is
                // given one time.
                ok = unsafe { put(state, to, value, rep) };
                ok
            });
            moved += usize::from(ok);
        }
        moved
    }

    /// The ID of each node, by the node index of `Control::node`.
    pub fn node_ids(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(|(id, _)| id.as_str())
    }

    /// Calls `f` with a reporter that adds to the sink. Then fills the outcome from the sink.
    fn call(&mut self, control: *const Control, f: impl FnOnce(*const Reporter) -> bool) {
        self.calls.call(&self.names, &self.probes, control, f);
    }
}

/// Loads a slot library (7.1) and makes the reference that the clusters call it with.
fn slot_library(path: &Path, root: &Path) -> Result<(Library, Box<SlotRef>), Error> {
    let library = Library::load(path, root, &[], Domain::Batch)?;
    let reference = Box::new(SlotRef { state: library.state, call: library.call });
    Ok((library, reference))
}

/// Gives each value that a cluster makes to each cluster of another domain that reads it (8.2),
/// and gives each cluster the slot library. False when a cluster has no such link. Each new
/// state needs its links again.
fn link(libraries: &[Option<Library>; 4], links: &[(Domain, Domain, u32)], slot: Option<&SlotRef>, sink: &mut Sink) -> bool {
    let rep = Reporter { ctx: std::ptr::from_mut(sink).cast(), report: collect, control: std::ptr::null() };
    let slotted = slot.is_none_or(|slot| {
        // SAFETY: the state is live, no run is active, and the reporter is valid during the call.
        // `slot` and the state of its library stay valid while the cluster keeps them.
        libraries.iter().flatten().all(|l| unsafe { (l.slot)(l.state, slot, &rep) })
    });
    slotted && links.iter().all(|&(from, to, index)| {
        let (Some(source), Some(target)) = (&libraries[from as usize], &libraries[to as usize]) else { return false };
        // SAFETY: the state is live, no run is active, and the reporter is valid during the call.
        let value = unsafe { (source.export)(source.state, index, &rep) };
        // SAFETY: the same as for the export. `value` came from the export of the same link index,
        // so it has the type that the reader expects (the witness of generate.rs). It is given one time.
        !value.is_null() && unsafe { (target.import)(target.state, index, value, &rep) }
    })
}


/// Splits the reports of one call into the problems of nodes and the live values. It fills the
/// lists of `out` again and keeps their texts, so it does not allocate after the first frames.
pub(crate) fn fill(out: &mut Outcome, ok: bool, sink: &Sink, names: &[(String, String)], probes: &[String]) {
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
            // Lanes with no value before them come only from a defective cluster. `Cluster::ui`
            // reads the widget lines.
            glue::LANES | glue::UI => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 18, question 12: the cost of a live value, from its text in the cluster to the
    /// outcome. Run with `--release --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement, not a check"]
    fn measure_live_values() {
        use std::fmt::Write;
        let n = 1000u32;
        let names: Vec<(String, String)> = (0..n).map(|i| (format!("n{i}"), "number".to_owned())).collect();
        let probes: Vec<String> = (0..n).map(|i| format!("n{i}.out")).collect();
        let (mut sink, mut out) = (Sink::default(), Outcome::default());
        let frame = |sink: &mut Sink, out: &mut Outcome| {
            sink.clear();
            let rep = Reporter { ctx: (&raw mut *sink).cast(), report: collect, control: std::ptr::null() };
            for i in 0..n {
                glue::send_value(&rep, i, &(f64::from(i) * 0.1), |v, t| {
                    let _ = write!(t, "{v}");
                });
            }
            fill(out, true, sink, &names, &probes);
        };
        for _ in 0..3 {
            frame(&mut sink, &mut out);
        }
        let start = std::time::Instant::now();
        for _ in 0..100 {
            frame(&mut sink, &mut out);
        }
        assert_eq!(out.values.len(), n as usize);
        println!("{n} live values: {:.0} ns each", start.elapsed().as_secs_f64() * 1e9 / f64::from(100 * n));
    }

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
