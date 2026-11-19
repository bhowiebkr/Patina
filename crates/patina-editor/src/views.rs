//! The open views of the editor (15.8). The engine thread owns them, as it owns the cluster, so
//! they run in the node process (16.1).

use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};

use patina_engine::project::GraphFile;
use patina_engine::view::{Frame, View, snapshot};
use patina_engine::{Edit, Frames, Mode, Project, Report, Toolchain};
use serde::{Deserialize, Serialize};

use crate::engine::{BUILDING, Shared, find_toolchain, lock, stop_fuse};
use crate::node_process::Out;

/// Project views can choose a first dock without adding domain names to the editor.
pub fn preferred_docks(root: &Path, names: &[String]) -> std::collections::BTreeMap<String, String> {
    names.iter().filter_map(|name| {
        let text = std::fs::read_to_string(root.join("views").join(name).join("Cargo.toml")).ok()?;
        Some((name.clone(), preferred_dock(&text)?))
    }).collect()
}

fn preferred_dock(text: &str) -> Option<String> {
    let manifest: toml::Value = toml::from_str(text).ok()?;
    let dock = manifest.get("package")?.get("metadata")?.get("patina")?.get("dock")?.as_str()?;
    matches!(dock, "left" | "right" | "bottom" | "center" | "split").then(|| dock.to_owned())
}

#[cfg(test)]
mod dock_tests {
    use super::preferred_dock;
    #[test]
    fn only_known_docks_are_accepted() {
        assert_eq!(preferred_dock("[package.metadata.patina]\ndock = 'bottom'"), Some("bottom".into()));
        assert_eq!(preferred_dock("[package.metadata.patina]\ndock = 'split'"), Some("split".into()));
        assert_eq!(preferred_dock("[package.metadata.patina]\ndock = 'unknown'"), None);
        assert_eq!(preferred_dock("invalid toml"), None);
    }
}

/// User input to view `name`, or a request for its widget lines when `events` is empty.
#[derive(Serialize, Deserialize)]
pub struct ViewInput {
    pub root: PathBuf,
    pub name: String,
    /// The dock slot of the instance. Each slot has its own state.
    pub instance: String,
    pub events: String,
    /// The open graph, and the selected nodes in it, for the graph lines of the view.
    pub graph: GraphFile,
    pub selected: Vec<String>,
    /// The ID of the reply.
    pub ask: u64,
}

/// The widget lines of a view call, and its writes to the graph and to the selection.
#[derive(Serialize, Deserialize, Default)]
pub struct ViewReply {
    pub tree: String,
    pub edits: Vec<Edit>,
    pub select: Option<Vec<String>>,
}

/// The loaded view instances: the view name, the instance, the view and its build mode. An
/// instance builds on its first input, in a background thread, so the frames go on (15.8).
#[derive(Default)]
pub struct Views {
    open: Vec<(String, String, View, Mode)>,
    building: Vec<Building>,
}

/// The build of a view instance, and the inputs that wait for it.
struct Building {
    name: String,
    instance: String,
    root: PathBuf,
    toolchain: Toolchain,
    mode: Mode,
    thread: JoinHandle<Result<PathBuf, String>>,
    inputs: Vec<ViewInput>,
    /// True when the code of the view changed during the build, so it builds again.
    again: bool,
}

impl Views {
    /// Calls the view of `input`, and sends the reply to `out`. The view reads the typed outputs
    /// of `frames`, when a cluster is loaded. The first input of an instance gets its reply after
    /// the build, from `poll`.
    pub fn ui(&mut self, out: &Out, shared: &Shared, (folder, toolchain): (&Path, &mut Option<Toolchain>), input: ViewInput, frames: Option<&mut Frames>) {
        // A slot that shows another view now drops the old instance. A view links the type crates
        // of one build mode, so an instance of another mode than the cluster builds again (15.8).
        let mode = frames.as_ref().map(|frames| frames.mode());
        self.open.retain(|(name, instance, _, built)| *instance != input.instance || (*name == input.name && mode.is_none_or(|m| m == *built)));
        self.cancel(out, |b| b.instance == input.instance && b.name != input.name, "The panel shows another view.");
        if self.open.iter().any(|(_, instance, _, _)| *instance == input.instance) {
            return out.reply(input.ask, self.reply(shared, &input, frames));
        }
        if let Some(waits) = self.building.iter_mut().find(|b| b.instance == input.instance) {
            return waits.inputs.push(input);
        }
        let mode = mode.unwrap_or(Mode::Edit);
        let started = find_toolchain(folder, toolchain).map_err(|e| e.to_string()).and_then(|toolchain| Ok((toolchain.clone(), build(toolchain.clone(), input.root.clone(), input.name.clone(), mode)?)));
        match started {
            Ok((toolchain, thread)) => self.building.push(Building { name: input.name.clone(), instance: input.instance.clone(), root: input.root.clone(), toolchain, mode, thread, inputs: vec![input], again: false }),
            Err(e) => out.reply(input.ask, Err::<ViewReply, _>(e)),
        }
    }

    /// True while a view builds.
    pub fn building(&self) -> bool {
        !self.building.is_empty()
    }

    /// True when a view build ended, so `poll` has work.
    pub fn built(&self) -> bool {
        self.building.iter().any(|b| b.thread.is_finished())
    }

    /// Loads each view whose build ended, and answers the inputs that wait for it.
    pub fn poll(&mut self, out: &Out, shared: &Shared, mut frames: Option<&mut Frames>) {
        while let Some(at) = self.building.iter().position(|b| b.thread.is_finished()) {
            let done = self.building.swap_remove(at);
            let built = done.thread.join().unwrap_or_else(|_| Err("The view build panicked.".to_owned()));
            let fail = |inputs: Vec<ViewInput>, e: String| inputs.into_iter().for_each(|input| out.reply(input.ask, Err::<ViewReply, _>(e.clone())));
            if done.again {
                let mode = frames.as_ref().map_or(done.mode, |frames| frames.mode());
                match build(done.toolchain.clone(), done.root.clone(), done.name.clone(), mode) {
                    Ok(thread) => self.building.push(Building { thread, mode, again: false, ..done }),
                    Err(e) => fail(done.inputs, e),
                }
                continue;
            }
            match built.and_then(|library| View::load(&library, &done.root)) {
                Ok(view) => {
                    self.open.push((done.name, done.instance, view, done.mode));
                    for input in done.inputs {
                        out.reply(input.ask, self.reply(shared, &input, frames.as_deref_mut()));
                    }
                }
                Err(e) => fail(done.inputs, e),
            }
        }
    }

    fn reply(&mut self, shared: &Shared, input: &ViewInput, frames: Option<&mut Frames>) -> Result<ViewReply, String> {
        let graph = {
            let reports: Vec<Report> = lock(&shared.reports).iter().chain(lock(&shared.event_reports).iter()).cloned().collect();
            snapshot(&input.graph, &lock(&shared.values), &reports, &input.selected)
        };
        let view = self.open.iter_mut().find(|(_, instance, _, _)| *instance == input.instance).ok_or("The view is not loaded.")?;
        let frame: Frame = view.2.ui(0, &input.events, &graph, frames)?;
        // The fires come before the edits, because an edit ends the event domain (9.4).
        let remote = lock(&shared.remote).clone();
        for (id, text) in frame.fires {
            if !remote.as_ref().is_some_and(|remote| remote.fire(&id, text)) {
                return Err(format!("The view fired `{id}`, but the event domain does not run. Click **Run** to start it."));
            }
        }
        Ok(ViewReply { tree: frame.tree, edits: frame.edits, select: frame.select })
    }

    /// Drops the builds that `drop` picks, and answers their inputs with `why`.
    fn cancel(&mut self, out: &Out, drop: impl Fn(&Building) -> bool, why: &str) {
        for gone in self.building.extract_if(.., |b| drop(b)) {
            for input in gone.inputs {
                out.reply(input.ask, Err::<ViewReply, _>(why.to_owned()));
            }
        }
    }

    /// Drops each instance of view `name`, so the next input builds it again, for example after a
    /// code change. A build of the view that runs builds again.
    pub fn close(&mut self, name: &str) {
        self.open.retain(|(open, _, _, _)| open != name);
        self.building.iter_mut().filter(|b| b.name == name).for_each(|b| b.again = true);
    }

    /// Drops one instance, for example when its panel closes.
    pub fn drop_instance(&mut self, out: &Out, instance: &str) {
        self.open.retain(|(_, open, _, _)| open != instance);
        self.cancel(out, |b| b.instance == instance, "The panel closed.");
    }

    /// Drops all instances, so that their surfaces free the editor windows before the node process
    /// ends (15.8).
    pub fn drop_all(&mut self, out: &Out) {
        self.open.clear();
        self.cancel(out, |_| true, "The project closed.");
    }
}

/// Builds view `name` of the project `root` in `mode`, in a background thread. Builds run one at a
/// time.
fn build(toolchain: Toolchain, root: PathBuf, name: String, mode: Mode) -> Result<JoinHandle<Result<PathBuf, String>>, String> {
    let work = move || {
        let project = Project::open(&root).map_err(|e| e.to_string())?;
        stop_fuse();
        let _build = lock(&BUILDING);
        project.build_view(&toolchain, &name, mode).map_err(|e| e.to_string())
    };
    thread::Builder::new().name("patina-view".to_owned()).spawn(work).map_err(|e| format!("The view build cannot start: {e}"))
}
