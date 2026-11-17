//! The jobs of one wake of the engine thread, and the build that they ask for (spec 7.2).

use std::sync::atomic::Ordering;

use patina_engine::{Compiled, Error, Mode};

use super::fuse::Fuse;
use super::{Build, Loaded, Run, Shared};

/// What the jobs of one wake ask for. Only the newest run starts. A build that a skipped run
/// asked for still happens.
#[derive(Default)]
pub(super) struct Batch {
    pub newest: Option<Box<Run>>,
    pub fused: Option<Result<Compiled, Error>>,
    pub build: bool,
    pub rerun: bool,
    pub bake: bool,
    pub step: bool,
    /// The slot holds a node only when all the changes that need a build are in its code.
    slot: Option<String>,
}

impl Batch {
    /// Takes a run, and keeps the build that it asks for.
    pub fn run(&mut self, run: Box<Run>, shared: &Shared, fuse: &mut Fuse) {
        if run.build {
            shared.builds.fetch_sub(1, Ordering::SeqCst);
            fuse.change();
            self.slot = if self.build && self.slot != run.slot { None } else { run.slot.clone() };
            self.build = true;
        }
        self.newest = Some(run);
    }

    /// Drops the runs, the builds and the bakes before **Pause** or **Step**.
    pub fn pause(&mut self, step: bool) {
        (self.newest, self.build, self.rerun, self.bake, self.step) = (None, false, false, false, step);
    }

    /// The build for `run`, or None when the loaded clusters run as they are. `fused` is the fuse
    /// to load, and `opened` is true for the first run of a project.
    pub fn pick_build(self, fused: Option<Result<Compiled, Error>>, opened: bool, loaded: Option<&Loaded>, run: &Run) -> Option<Build> {
        if fused.is_some() {
            Some(Build { mode: Mode::Bake, slot: None, fused })
        } else if self.bake {
            Some(Build { mode: Mode::Bake, slot: None, fused: None })
        } else if opened {
            // The first build of a project has the mode of its last build, so cargo builds nothing when nothing changed (7.2).
            Some(Build { mode: run.checked.built_mode().unwrap_or(Mode::Bake), slot: None, fused: None })
        } else if self.build || loaded.is_none() {
            // The same clusters keep their mode, so a build with no change compiles nothing (7.2).
            // A changed crate builds in edit mode and fuses later, also after a bake load (8.3).
            let same = loaded.filter(|l| self.slot.is_none() && !l.stale && l.wiring == run.checked.wiring());
            let mode = |l: &Loaded| match l.cluster.mode() {
                Mode::Bake if !l.cluster.sources_changed() => Mode::Bake,
                _ => Mode::Edit,
            };
            Some(same.map_or(Build { mode: Mode::Edit, slot: self.slot, fused: None }, |l| Build { mode: mode(l), slot: None, fused: None }))
        } else {
            None
        }
    }
}
