//! The builds of a run (spec 7.2) and of a package (14.1), and the batch run after a build.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use patina_engine::{Checked, Cluster, Error, Mode, Report, Toolchain, package};

use super::{BUILDING, Build, Event, Loaded, Run, Shared, alias, badges, by_inner, lock};
use crate::node_process::Out;

/// Builds the clusters for a run (7.2). A save of the code of one node puts the node into the
/// slot, and builds the slot library alone when the base can stay. Gives None when only the slot
/// library changed. `building` gets the layers of each build before it starts.
fn compile(checked: &mut Checked, toolchain: &Toolchain, build: &mut Build, mut loaded: Option<&mut Loaded>, building: impl Fn(bool, Option<&str>)) -> Result<Option<Cluster>, Error> {
    if let Some(fused) = build.fused.take() {
        return fused.and_then(|compiled| checked.load(compiled)).map(Some);
    }
    // A generic node or the delay node does not go into the slot, so its change builds all (7.1).
    let slotted = build.slot.is_some() && checked.set_slot(build.slot.as_deref()).is_ok();
    let swapped = match loaded.as_deref_mut().filter(|l| slotted && !l.stale && l.slot == build.slot) {
        Some(loaded) => {
            building(false, build.slot.as_deref());
            checked.build_slot(toolchain, &mut loaded.cluster)
        }
        None => Ok(false),
    };
    let built = match swapped {
        Ok(true) => Ok(None),
        Ok(false) => {
            // Until a full build loads, the base can be older than the code. A failed slot build keeps the base.
            if let Some(loaded) = loaded {
                loaded.stale = true;
            }
            // A build with all crates fresh compiles nothing, so the nodes do not show Compiling.
            checked.build_with(toolchain, build.mode, &|| building(true, build.slot.as_deref().filter(|_| slotted))).map(Some)
        }
        Err(e) => Err(e),
    };
    // The run stays for a rerun and for the fuse, which builds with no slot.
    checked.set_slot(None).expect("an empty slot always works");
    built
}

/// The toolchain of the builds. After an install, the toolchain folder replaces the Rust of the
/// system.
pub(crate) fn find_toolchain<'a>(folder: &Path, toolchain: &'a mut Option<Toolchain>) -> Result<&'a Toolchain, Error> {
    if toolchain.as_ref().is_none_or(|t| t.own.is_none() && Toolchain::installed(folder)) {
        *toolchain = Some(Toolchain::find(folder)?);
    }
    Ok(toolchain.as_ref().expect("the toolchain is set above"))
}

/// Makes a package with a bake build. The runner of the package is the `patina-cli` next to the
/// editor (14.1).
/// Makes the package of the top graph `graph` of project `root` in folder `to`, in a background
/// thread, so the graph runs on. The thread sends the reply to `ask`. Builds run one at a time.
pub(super) fn make_package(out: &Arc<Out>, (folder, toolchain): (&Path, &mut Option<Toolchain>), (root, graph): (PathBuf, String), to: PathBuf, ask: u64) {
    let toolchain = match find_toolchain(folder, toolchain) {
        Ok(toolchain) => toolchain.clone(),
        Err(e) => return out.reply(ask, Err::<Vec<String>, _>(e.to_string())),
    };
    let reply = out.clone();
    let work = move || {
        let made = (|| {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let runner = exe.with_file_name(format!("patina-cli{}", std::env::consts::EXE_SUFFIX));
            if !runner.is_file() {
                return Err(format!("Patina cannot make a package, because {} is not there.", runner.display()));
            }
            let _build = lock(&BUILDING);
            package::make(&root, &graph, &toolchain, Mode::Bake, &runner, &to).map_err(|e| e.to_string())
        })();
        reply.reply(ask, made);
    };
    if let Err(e) = std::thread::Builder::new().name("patina-package".to_owned()).spawn(work) {
        out.reply(ask, Err::<Vec<String>, _>(format!("The package build cannot start: {e}")));
    }
}

pub(super) fn run_one(out: &Out, (folder, toolchain): (&Path, &mut Option<Toolchain>), loaded: &mut Option<Loaded>, run: &mut Run, build: Option<&mut Build>, shared: &Shared) -> Result<Option<Event>, Error> {
    let mut built_ms = None;
    let mut slot_only = false;
    let mut errors = Vec::new();
    if let Some(build) = build {
        let start = Instant::now();
        let wiring = run.checked.wiring();
        let toolchain = find_toolchain(folder, toolchain)?;
        let serial = run.serial;
        let building = |base, slot: Option<&str>| {
            out.emit(&Event::Building { serial, base, slot: slot.map(str::to_owned) });
        };
        let built = {
            let _build = lock(&BUILDING);
            compile(&mut run.checked, toolchain, build, loaded.as_mut(), building)
        };
        if shared.builds.load(Ordering::SeqCst) > 0 {
            return Ok(None);
        }
        match built {
            // The slot library changed in place, and the nodes in it got new states (7.1).
            Ok(None) => slot_only = true,
            Ok(Some(mut cluster)) => {
                *lock(&shared.ids) = cluster.node_ids().map(str::to_owned).collect();
                // The node states with the same key go on in the new cluster (7.2).
                if let Some(old) = loaded.as_mut() {
                    cluster.adopt(&mut old.cluster);
                }
                let slot = build.slot.clone().filter(|_| cluster.slotted());
                *loaded = Some(Loaded { cluster, wiring, slot, stale: false, aliases: by_inner(run.checked.aliases()) });
            }
            // The last build that compiled keeps running only with the same wiring (7.2, 15.10).
            Err(Error::Compile(list)) if loaded.as_ref().is_some_and(|l| l.wiring == wiring) => errors = list,
            Err(e) => {
                *loaded = None;
                return Err(e);
            }
        }
        built_ms = Some(start.elapsed().as_secs_f64() * 1000.0);
    }
    let loaded = loaded.as_mut().expect("the cluster is loaded above");
    let (fused, slot) = (!loaded.needs_fuse(), loaded.slot.clone());
    let cluster = &mut loaded.cluster;
    let params = cluster.set_params(run.checked.graph());
    if !run.start {
        return Ok(Some(Event::Loaded { serial: run.serial, built_ms, fused, slot, errors }));
    }
    if params.ok {
        out.emit(&Event::Started { serial: run.serial });
    }
    let start = Instant::now();
    // A node with a bad parameter does not run, so the graph does not run (4.8).
    let outcome = params.ok.then(|| cluster.run(&shared.control));
    let run_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut outcome = outcome.unwrap_or_default();
    let extra = alias(&outcome.values, &loaded.aliases);
    outcome.values.extend(extra);
    let reports: Vec<Report> = params.reports.into_iter().chain(outcome.reports).collect();
    lock(&shared.reports).clone_from(&reports);
    lock(&shared.event_reports).clear();
    let values = badges(&outcome.values);
    *lock(&shared.values) = outcome.values;
    let goes_on = params.ok && outcome.ok && !shared.control.stopped();
    let (frames, events) = (goes_on && (cluster.frames() || cluster.blocks()), goes_on && cluster.events());
    Ok(Some(Event::Ran { serial: run.serial, built_ms, slot_only, fused, slot, run_ms, ok: outcome.ok, frames, events, skipped: outcome.skipped, stopped: outcome.stopped, times: outcome.times, logs: outcome.logs, reports, values, errors }))
}
