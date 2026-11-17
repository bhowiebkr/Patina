//! The fuse of the clusters: a bake build with no slot in a background thread (spec 7.2).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use patina_engine::{Checked, Compiled, Error, Mode, Toolchain};

use super::{BUILDING, find_toolchain, lock};

/// The stop flag of the last fuse that started.
static FUSING: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// Stops the fuse that builds now, so a view build does not wait for it. The fuse starts again
/// after `FUSE_IDLE`. A **Package** builds the same clusters, so it waits for the fuse and uses
/// its crates.
pub(crate) fn stop_fuse() {
    if let Some(stop) = lock(&FUSING).as_ref() {
        stop.store(true, Ordering::Relaxed);
    }
}

/// The time with no change after which the engine fuses the clusters (7.2, roadmap question 7).
pub(super) const FUSE_IDLE: Duration = Duration::from_secs(30);

/// The fuse of the clusters: a bake build with no slot (7.2). It builds in a background thread,
/// so the graph runs on until the fused clusters load.
#[derive(Default)]
pub(super) struct Fuse {
    /// The time of the fuse while the graph is live and the clusters are not fused.
    pub(super) at: Option<Instant>,
    /// The number of builds, unloads and pauses. A fuse that started before the last one is old.
    pub(super) changes: u64,
    /// The fuse that builds now, with the value of `changes` at its start and its stop flag.
    pub(super) thread: Option<FuseThread>,
}

type FuseThread = (JoinHandle<Result<Compiled, Error>>, u64, Arc<AtomicBool>);

impl Fuse {
    /// Counts a build, an unload or a pause. A fuse that builds now stops, because its result is
    /// old, so the next build does not wait for it.
    pub(super) fn change(&mut self) {
        self.changes += 1;
        if let Some((_, _, stop)) = &self.thread {
            stop.store(true, Ordering::Relaxed);
        }
    }

    /// Starts a fuse of `checked` in a background thread, unless a fuse of the same graph builds.
    pub(super) fn start(&mut self, checked: &Checked, folder: &Path, toolchain: &mut Option<Toolchain>) {
        self.at = None;
        if self.thread.as_ref().is_some_and(|(_, changes, _)| *changes == self.changes) {
            return;
        }
        let (checked, toolchain) = (checked.clone(), find_toolchain(folder, toolchain).cloned());
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        *lock(&FUSING) = Some(stop.clone());
        let thread = std::thread::Builder::new().name("patina-fuse".to_owned()).spawn(move || {
            let toolchain = toolchain?;
            let _build = lock(&BUILDING);
            checked.compile(&toolchain, Mode::Bake, &|| {}, &flag)
        });
        self.thread = thread.ok().map(|thread| (thread, self.changes, stop));
    }

    /// Gives the result of a fuse that ended, when no change came after its start.
    pub(super) fn done(&mut self) -> Option<Result<Compiled, Error>> {
        if !self.thread.as_ref().is_some_and(|(thread, _, _)| thread.is_finished()) {
            return None;
        }
        let (thread, changes, stop) = self.thread.take()?;
        let result = thread.join().unwrap_or_else(|_| Err(Error::Build("the fuse build panicked".to_owned())));
        if changes != self.changes {
            return None;
        }
        // A view build stopped the fuse, so it starts again later. A fuse that ended first loads.
        if result.is_err() && stop.load(Ordering::Relaxed) {
            self.at = Some(Instant::now() + FUSE_IDLE);
            return None;
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ended(result: Result<Compiled, Error>) -> JoinHandle<Result<Compiled, Error>> {
        let thread = std::thread::spawn(move || result);
        while !thread.is_finished() {
            std::thread::sleep(Duration::from_millis(1));
        }
        thread
    }

    #[test]
    fn a_fuse_that_a_view_build_stopped_starts_again_later() {
        let mut fuse = Fuse::default();
        let stopped = || Err(Error::Build("the build stopped".to_owned()));
        fuse.thread = Some((ended(stopped()), 0, Arc::new(AtomicBool::new(true))));
        assert!(fuse.done().is_none());
        assert!(fuse.at.is_some_and(|at| at > Instant::now()));
        // A failed fuse with no stop gives its error, so the run shows it.
        fuse.at = None;
        fuse.thread = Some((ended(Err(Error::Build("bad".to_owned()))), 0, Arc::new(AtomicBool::new(false))));
        assert!(matches!(fuse.done(), Some(Err(_))) && fuse.at.is_none());
        // A change makes the running fuse old, so its result does not load.
        fuse.thread = Some((ended(stopped()), 0, Arc::new(AtomicBool::new(false))));
        fuse.change();
        assert!(fuse.done().is_none() && fuse.at.is_none());
    }
}
