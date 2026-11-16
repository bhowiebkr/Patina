//! The open views of the editor (15.8). The engine thread owns them, as it owns the cluster.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use patina_engine::view::{Frame, View};
use patina_engine::{Project, Toolchain};

use crate::engine::find_toolchain;

/// User input to view `name`, or a request for its widget lines when `events` is empty.
pub struct ViewInput {
    pub root: PathBuf,
    pub name: String,
    /// The dock slot of the instance. Each slot has its own state.
    pub instance: String,
    pub events: String,
    /// The graph lines of `patina_engine::view::snapshot`.
    pub graph: String,
    pub reply: Sender<Result<Frame, String>>,
}

/// The loaded view instances: the view name, the instance and the view. An instance builds and
/// loads on its first input.
#[derive(Default)]
pub struct Views(Vec<(String, String, View)>);

impl Views {
    pub fn ui(&mut self, folder: &Path, toolchain: &mut Option<Toolchain>, input: ViewInput) {
        let _ = input.reply.send(self.frame(folder, toolchain, &input));
    }

    fn frame(&mut self, folder: &Path, toolchain: &mut Option<Toolchain>, input: &ViewInput) -> Result<Frame, String> {
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
        self.0[index].2.ui(0, &input.events, &input.graph)
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
