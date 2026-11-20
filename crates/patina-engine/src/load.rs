//! Loads the cluster libraries of a graph and calls them through the C ABI of `patina_sdk::glue`.

use std::collections::{BTreeMap, HashMap};
use std::ffi::c_void;
use std::path::{Path, PathBuf};

use patina_sdk::audio::{Mix, OUTPUT_SAMPLES};
use patina_sdk::glue::{self, Control, Reporter, SlotRef};

use crate::events::{EventRun, Events, Trigger};
use crate::library::{Library, Sink, collect};
use crate::node::{Domain, ParamAttr, out_of_range};
use crate::outcome::{Outcome, Report, ReportKind};
use crate::plan::Plan;
use crate::pool;
use crate::project::GraphFile;
use crate::run::{Audio, Calls, Frames, Gui, Nodes, Pace, Run};
use crate::{Error, generate};

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
    /// A hash of the code of the clusters and of the node in the slot, or None when its folder
    /// cannot be read. A build of the slot alone is correct only while it does not change (7.1).
    pub(crate) base: Option<u64>,
    /// The build mode of the clusters (8.3).
    pub(crate) mode: crate::Mode,
    /// The path package folders of the last full build, with a hash of their files.
    pub(crate) sources: crate::cargo::Sources,
    /// The domain of each node, by node index.
    domains: Vec<Domain>,
    /// True for each audio node with a GUI, by node index (`generate::shows`).
    shown: Vec<bool>,
    /// The state key of each node, by node index (7.2). A node with no state has none.
    keys: Vec<Option<u64>>,
    /// The times of the last runs of each domain (9.2).
    pace: [Pace; 4],
    /// The ID and the folder name of each node, by node index.
    names: Vec<(String, String)>,
    /// The ID of each node with a GUI, and true when the node runs again after batch (4.3).
    guis: Vec<(String, bool)>,
    /// The node index of each node of `guis`, in the same order.
    gui_nodes: Vec<usize>,
    /// The node index, the input key and the `#[param]` of each parameter slot (4.8).
    slots: Vec<(usize, String, ParamAttr)>,
    /// The output of each held value, as "<node id>.<output key>" (`Plan::held`).
    probes: Vec<String>,
    /// True for each slot that got a text at the last `set_params`.
    has_text: Vec<bool>,
    /// The reports and the outcome of the last call of the batch and the frame domains.
    calls: Calls,
    /// The reports and the outcome of the last block, apart, so that an audio thread fills them (9.3).
    block_calls: Calls,
    /// The reports and the outcome of the last call of `ui`, apart, so that the frame outcome stays.
    gui_calls: Gui,
    /// The mix that the audio nodes of the project play into (9.3).
    mix: &'static Mix,
    /// The mix of the last block.
    sound: [f32; OUTPUT_SAMPLES],
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
        let param = |s: &crate::plan::Slot| plan.nodes[plan.steps[s.step].node].1.inputs.iter().find(|i| i.key == s.key).map(|i| i.param.clone());
        let slots: Vec<(usize, String, ParamAttr)> = plan.slots.iter().map(|s| (s.step, s.key.clone(), param(s).unwrap_or_default())).collect();
        let probes: Vec<String> = plan
            .held()
            .map(|(n, k)| format!("{}.{}", plan.steps[n].id, plan.nodes[plan.steps[n].node].1.output_keys()[k]))
            .collect();
        let has_gui = |n: &usize| plan.nodes[plan.steps[*n].node].1.ui && plan.nodes[plan.steps[*n].node].1.state.is_some();
        let mut gui_calls = Gui::default();
        gui_calls.reads = vec![false; probes.len()];
        let gui_nodes: Vec<usize> = (0..plan.steps.len()).filter(has_gui).collect();
        let guis = gui_nodes.iter().map(|&n| (plan.steps[n].id.clone(), plan.steps[n].domain != Domain::Batch)).collect();
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
            base: None,
            mode: crate::Mode::Edit,
            sources: Vec::new(),
            domains: plan.steps.iter().map(|s| s.domain).collect(),
            shown: (0..plan.steps.len()).map(|n| crate::generate::shows(plan, n)).collect(),
            keys,
            pace: [Pace::default(); 4],
            names,
            guis,
            gui_nodes,
            has_text: vec![false; slots.len()],
            slots,
            gui_calls,
            probes,
            calls: Calls { sink, ..Calls::default() },
            block_calls: Calls::default(),
            mix: glue::shared_in(&root.to_string_lossy(), Mix::default),
            sound: [0.0; OUTPUT_SAMPLES],
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

    /// True when a file of a crate folder of the last full build changed, for example a code change
    /// from outside the editor (8.3).
    pub fn sources_changed(&self) -> bool {
        (self.sources.iter()).any(|(folder, hash)| crate::cargo::folder_hash(folder).ok() != Some(*hash))
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
    /// default value again: the default of `#[param]` or of its type. The states of the nodes stay.
    pub fn set_params(&mut self, graph: &GraphFile) -> Outcome {
        let params: HashMap<&str, &BTreeMap<String, String>> =
            graph.node.iter().map(|n| (n.id.as_str(), &n.params)).collect();
        let texts: Vec<Option<&String>> = self
            .slots
            .iter()
            .map(|(n, key, param)| params.get(self.names[*n].0.as_str()).and_then(|p| p.get(key)).or(param.default.as_ref()))
            .collect();
        let mut all = Outcome { ok: true, ..Outcome::default() };
        self.triggers.iter_mut().for_each(Trigger::reset);
        for (slot, text) in (0u32..).zip(&texts) {
            if text.is_none() && !self.has_text[slot as usize] {
                continue;
            }
            let (n, key, param) = &self.slots[slot as usize];
            let bad = |text: String| Report { id: self.names[*n].0.clone(), folder: self.names[*n].1.clone(), kind: ReportKind::BadParam, text: format!("input `{key}`: {text}") };
            // The engine reads the settings of a trigger (9.4). `reset` gave the defaults.
            if let Some(trigger) = self.triggers.iter_mut().find(|t| t.node == *n) {
                if let Some(Err(text)) = text.map(|text| trigger.set(key, text)) {
                    all.reports.push(bad(text));
                    all.ok = false;
                }
                continue;
            }
            if let Some(value) = text.and_then(|t| t.trim().parse::<f64>().ok()).filter(|&v| out_of_range(param, v)) {
                let (min, max) = (param.min.map_or("-".to_owned(), |m| m.to_string()), param.max.map_or("-".to_owned(), |m| m.to_string()));
                all.reports.push(bad(format!("{value} is out of the range {min} to {max} of #[param] (spec 4.8)")));
                all.ok = false;
                continue;
            }
            let Some(library) = self.library(*n) else { continue };
            let (state, set) = (library.state, library.set);
            self.calls.sink.clear();
            // A null text gives the default value.
            let (ptr, len) = text.map_or((std::ptr::null(), 0), |text| (text.as_ptr(), text.len()));
            // SAFETY: `state` is live, `ptr` is null or valid during the call, and so is the reporter.
            self.calls.call(&self.names, &self.probes, std::ptr::null(), None, |rep| unsafe { set(state, slot, ptr, len, rep) });
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

    /// The interleaved stereo samples that the nodes played in the last block, or `None` before the first node played.
    pub fn sound(&self) -> Option<&[f32; OUTPUT_SAMPLES]> {
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
        self.parts(true, true).2.is_none_or(|events| events.run(count, control, None, done))
    }

    fn has(&self, domain: Domain) -> bool {
        self.libraries[domain as usize].as_ref().is_some_and(|l| !l.tasks.ids.is_empty())
    }

    fn run_domain(&mut self, domain: Domain, control: &Control) {
        let audio = domain == Domain::Audio;
        // One thread runs all audio nodes (9.3), so the audio domain never uses the pool.
        let pool = !audio && self.slot.as_ref().is_none_or(|(slot, _)| slot.parallel);
        let calls = if audio { &mut self.block_calls } else { &mut self.calls };
        let (names, probes, pace) = (&self.names, &self.probes, &mut self.pace[domain as usize]);
        Run { library: self.libraries[domain as usize].as_ref(), pool, names, probes, calls, pace }.domain(domain, control);
    }

    /// True when the audio domain can run on its own thread: the graph has audio nodes, and the
    /// audio library and the slot library keep only `Sync` values (9.3).
    fn apart(&self) -> bool {
        let audio = self.libraries[Domain::Audio as usize].as_ref();
        self.blocks() && audio.is_some_and(|audio| audio.parallel) && self.slot.as_ref().is_none_or(|(slot, _)| slot.parallel)
    }

    /// Splits the cluster into its frame domain and its audio domain, so that the audio domain
    /// runs on its own thread at the same time as the frames (9.3). `None` when the graph has no
    /// audio nodes, or when the audio library or the slot library keeps a value that is not `Sync`.
    pub fn split(&mut self) -> Option<(Frames<'_>, Audio<'_>)> {
        if !self.apart() {
            return None;
        }
        let (frames, audio, _) = self.parts(false, true);
        Some((frames, audio?))
    }

    /// Splits the cluster for a live run (9.3, 9.4): the frame part, the audio domain for its own
    /// thread, and the event domain for its own threads. When the audio domain cannot have its own
    /// thread, the frame part runs the blocks (`Frames::block`). `None` when the graph has no event
    /// nodes, or when the slot library keeps a value that is not `Sync` and holds an event node and
    /// a node of another domain.
    pub fn live(&mut self) -> Option<(Frames<'_>, Option<Audio<'_>>, Events<'_>)> {
        if !self.events() || (self.slot.as_ref().is_some_and(|(slot, _)| !slot.parallel) && mixed(&self.slotted, &self.domains)) {
            return None;
        }
        let apart = self.apart();
        let (frames, audio, events) = self.parts(!apart, false);
        Some((frames, audio, events?))
    }

    /// The frame part of the cluster: the frames, the blocks, the node GUIs and the reads of the
    /// views, between two runs of the other domains.
    pub fn frame_part(&mut self) -> Frames<'_> {
        self.parts(true, true).0
    }

    /// The frame part, the audio domain for its own thread and the event domain. With `audio`, the
    /// frame part runs the blocks. Else the audio domain is for another thread, and the frame part
    /// refuses its nodes. `events` is false when other threads run the event domain, so the frame
    /// part refuses their nodes.
    fn parts(&mut self, audio: bool, events: bool) -> (Frames<'_>, Option<Audio<'_>>, Option<Events<'_>>) {
        let (has_blocks, mix) = (self.blocks(), self.mix);
        let [_, frame_pace, audio_pace, _] = &mut self.pace;
        let [_, frame, audio_library, event_library] = &self.libraries;
        let (names, probes) = (&self.names, &self.probes);
        // The frames use the pool only when the slot library allows it (9.2).
        let pool = self.slot.as_ref().is_none_or(|(slot, _)| slot.parallel);
        let nodes = Nodes {
            libraries: &self.libraries,
            slot: self.slot.as_ref().map(|(library, _)| library),
            slotted: &self.slotted,
            domains: &self.domains,
            shown: &self.shown,
            guis: &self.guis,
            gui_nodes: &self.gui_nodes,
            audio,
            events,
            event_guis: event_library.as_ref().is_some_and(|library| library.parallel) && pool,
        };
        let run = Run { library: frame.as_ref(), pool, names, probes, calls: &mut self.calls, pace: frame_pace };
        let blocks = Run { library: audio_library.as_ref(), pool: false, names, probes, calls: &mut self.block_calls, pace: audio_pace };
        let blocks = has_blocks.then_some(Audio { run: blocks, mix, sound: &mut self.sound });
        let events = event_library.as_ref().map(|library| Events::new(library, pool, &self.triggers, names, probes, &self.root));
        let (here, apart) = if audio { (blocks, None) } else { (None, blocks) };
        (Frames { run, gui: &mut self.gui_calls, nodes, audio: here, mode: self.mode }, apart, events)
    }

    /// Calls the `ui` function of node `id` between two runs, as `Frames::ui` (4.3).
    pub fn ui(&mut self, control: &Control, id: &str, mode: u32, events: &str) -> (Option<String>, Outcome) {
        self.frame_part().ui(control, id, mode, events)
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
        // Copy requests by output name because a new plan can change the probe order.
        for (probe, requested) in old.probes.iter().zip(&old.gui_calls.reads) {
            if *requested && let Some((id, key)) = probe.split_once('.') {
                self.read(id, key);
            }
        }
        // The batch run of the new cluster makes the copies, so it asks for no other run.
        self.gui_calls.asked = false;
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
        self.calls.call(&self.names, &self.probes, control, None, f);
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

/// True when the nodes in the slot include an event node and a node of another domain, so that
/// the event threads and the other threads can call the slot library at the same time.
fn mixed(slotted: &[bool], domains: &[Domain]) -> bool {
    let events = || slotted.iter().zip(domains).filter(|&(&slot, _)| slot).map(|(_, &domain)| domain == Domain::Event);
    events().any(|event| event) && events().any(|event| !event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_needs_an_event_node_and_another_node_in_the_slot() {
        let domains = [Domain::Event, Domain::Frame, Domain::Event];
        assert!(mixed(&[true, true, false], &domains));
        assert!(!mixed(&[true, false, true], &domains));
        assert!(!mixed(&[false, true, false], &domains));
        assert!(!mixed(&[false, false, false], &domains));
    }
}
