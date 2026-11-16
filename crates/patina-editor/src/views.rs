//! The open views of the editor (15.8). The engine thread owns them, as it owns the cluster, so
//! they run in the node process (16.1).

use std::path::{Path, PathBuf};

use patina_engine::project::GraphFile;
use patina_engine::view::{Frame, View, snapshot};
use patina_engine::{Edit, Frames, Project, Report, Toolchain};
use serde::{Deserialize, Serialize};

use crate::engine::{Shared, find_toolchain, lock};
use crate::node_process::Out;

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

/// The loaded view instances: the view name, the instance and the view. An instance builds and
/// loads on its first input.
#[derive(Default)]
pub struct Views(Vec<(String, String, View)>);

impl Views {
    /// Calls the view of `input`, and sends the reply to `out`. The view reads the typed outputs
    /// of `frames`, when a cluster is loaded.
    pub fn ui(&mut self, out: &Out, shared: &Shared, folder: (&Path, &mut Option<Toolchain>), input: ViewInput, frames: Option<&mut Frames>) {
        out.reply(input.ask, self.reply(shared, folder, &input, frames));
    }

    fn reply(&mut self, shared: &Shared, folder: (&Path, &mut Option<Toolchain>), input: &ViewInput, frames: Option<&mut Frames>) -> Result<ViewReply, String> {
        let graph = {
            let reports: Vec<Report> = lock(&shared.reports).iter().chain(lock(&shared.event_reports).iter()).cloned().collect();
            snapshot(&input.graph, &lock(&shared.values), &reports, &input.selected)
        };
        let frame = self.frame(folder, input, &graph, frames)?;
        // The fires come before the edits, because an edit ends the event domain (9.4).
        let remote = lock(&shared.remote).clone();
        for (id, text) in frame.fires {
            if !remote.as_ref().is_some_and(|remote| remote.fire(&id, text)) {
                return Err(format!("The view fired `{id}`, but the event domain does not run. Click **Run** to start it."));
            }
        }
        Ok(ViewReply { tree: frame.tree, edits: frame.edits, select: frame.select })
    }

    fn frame(&mut self, (folder, toolchain): (&Path, &mut Option<Toolchain>), input: &ViewInput, graph: &str, frames: Option<&mut Frames>) -> Result<Frame, String> {
        // A slot that shows another view now drops the old instance.
        self.0.retain(|(name, instance, _)| *instance != input.instance || *name == input.name);
        let index = match self.0.iter().position(|(_, instance, _)| *instance == input.instance) {
            Some(index) => index,
            None => {
                let toolchain = find_toolchain(folder, toolchain).map_err(|e| e.to_string())?;
                let project = Project::open(&input.root).map_err(|e| e.to_string())?;
                let library = project.build_view(toolchain, &input.name).map_err(|e| e.to_string())?;
                self.0.push((input.name.clone(), input.instance.clone(), View::load(&library, &input.root)?));
                self.0.len() - 1
            }
        };
        self.0[index].2.ui(0, &input.events, graph, frames)
    }

    /// Drops each instance of view `name`, so the next input builds it again, for example after a
    /// code change.
    pub fn close(&mut self, name: &str) {
        self.0.retain(|(open, _, _)| open != name);
    }

    /// Drops one instance, for example when its panel closes.
    pub fn drop_instance(&mut self, instance: &str) {
        self.0.retain(|(_, open, _)| open != instance);
    }
}
