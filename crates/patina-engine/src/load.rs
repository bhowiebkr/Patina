//! Loads a cluster library and calls it through the C ABI of `patina_sdk::glue`.

use std::ffi::c_void;
use std::fmt;
use std::path::Path;

use patina_sdk::glue::{self, Reporter};

use crate::Error;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReportKind {
    /// The node returned an `Err` (4.7).
    Error,
    /// The node panicked (4.7).
    Panic,
    /// A parameter text did not read back (4.8).
    BadParam,
}

/// A problem of one node during a call into the cluster.
#[derive(Debug)]
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

/// The result of one call: false when a node failed or the cluster failed with no report.
pub struct Outcome {
    pub ok: bool,
    pub reports: Vec<Report>,
}

type VersionFn = unsafe extern "C" fn() -> u32;
type NewFn = unsafe extern "C" fn() -> *mut c_void;
type SetFn = unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *const Reporter) -> bool;
type RunFn = unsafe extern "C" fn(*mut c_void, *const Reporter) -> bool;
type FreeFn = unsafe extern "C" fn(*mut c_void);

/// A loaded cluster with its parameters and its node states.
pub struct Cluster {
    state: *mut c_void,
    set: SetFn,
    run: RunFn,
    free: FreeFn,
    /// The ID and the folder name of each node, by node index.
    names: Vec<(String, String)>,
}

/// The report list of one call. It is the context of the reporter.
type Sink = Vec<(u32, u32, String)>;

unsafe extern "C" fn collect(ctx: *mut c_void, node: u32, kind: u32, text: *const u8, len: usize) {
    // SAFETY: `ctx` is the sink of `Cluster::call`, and `text` points to `len` bytes during the
    // call (glue::Reporter).
    let (sink, bytes) = unsafe { (&mut *ctx.cast::<Sink>(), std::slice::from_raw_parts(text, len)) };
    sink.push((node, kind, String::from_utf8_lossy(bytes).into_owned()));
}

impl Cluster {
    pub(crate) fn load(path: &Path, names: Vec<(String, String)>) -> Result<Cluster, Error> {
        let fail = |text: String| Error::Load(format!("{}: {text}", path.display()));
        let missing = |e: libloading::Error| fail(e.to_string());
        let library = open(path).map_err(missing)?;
        // SAFETY: Patina generated the cluster with these symbols and signatures (generate.rs).
        let cluster = unsafe {
            let version: VersionFn = symbol(&library, b"patina_abi_version\0").map_err(missing)?;
            if version() != glue::ABI_VERSION {
                return Err(fail("the cluster has another ABI version. Build it again".to_owned()));
            }
            let new: NewFn = symbol(&library, b"patina_cluster_new\0").map_err(missing)?;
            let set: SetFn = symbol(&library, b"patina_cluster_set\0").map_err(missing)?;
            let run: RunFn = symbol(&library, b"patina_cluster_run\0").map_err(missing)?;
            let free: FreeFn = symbol(&library, b"patina_cluster_free\0").map_err(missing)?;
            let state = new();
            if state.is_null() {
                return Err(fail("a parameter default panicked".to_owned()));
            }
            Cluster { state, set, run, free, names }
        };
        // Rust libraries do not unload safely, so the library stays loaded (8.5).
        std::mem::forget(library);
        Ok(cluster)
    }

    /// Sets parameter slot `slot` from its text form (4.8).
    pub fn set(&mut self, slot: usize, text: &str) -> Outcome {
        let Ok(slot) = u32::try_from(slot) else {
            return Outcome { ok: false, reports: Vec::new() };
        };
        let (state, set) = (self.state, self.set);
        // SAFETY: `state` is live, `text` is valid during the call, and so is the reporter.
        self.call(|rep| unsafe { set(state, slot, text.as_ptr(), text.len(), rep) })
    }

    /// Runs the graph one time.
    pub fn run(&mut self) -> Outcome {
        let (state, run) = (self.state, self.run);
        // SAFETY: `state` is live, and the reporter is valid during the call.
        self.call(|rep| unsafe { run(state, rep) })
    }

    fn call(&self, f: impl FnOnce(*const Reporter) -> bool) -> Outcome {
        let mut sink: Sink = Vec::new();
        let reporter = Reporter { ctx: (&raw mut sink).cast(), report: collect };
        let ok = f(&reporter);
        let reports = sink
            .into_iter()
            .map(|(node, kind, text)| {
                let (id, folder) = self.names.get(node as usize).cloned().unwrap_or_default();
                let kind = match kind {
                    glue::PANIC => ReportKind::Panic,
                    glue::BAD_PARAM => ReportKind::BadParam,
                    _ => ReportKind::Error,
                };
                Report { id, folder, kind, text }
            })
            .collect();
        Outcome { ok, reports }
    }
}

impl Drop for Cluster {
    fn drop(&mut self) {
        // SAFETY: `state` came from `patina_cluster_new` and is freed only here.
        unsafe { (self.free)(self.state) }
    }
}

/// Reads a symbol of the library as a function pointer.
///
/// # Safety
///
/// The symbol has the type `T`.
unsafe fn symbol<T: Copy>(library: &libloading::Library, name: &[u8]) -> Result<T, libloading::Error> {
    // SAFETY: the caller gives the type of the symbol.
    unsafe { library.get::<T>(name).map(|s| *s) }
}

#[cfg(windows)]
fn open(path: &Path) -> Result<libloading::Library, libloading::Error> {
    use libloading::os::windows::{LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, Library};
    // Only the folder of the library and the system folders give other libraries (8.5).
    let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS;
    // SAFETY: the load runs the init code of the library. Patina built it from a project that
    // the owner trusts (11).
    unsafe { Library::load_with_flags(path, flags) }.map(libloading::Library::from)
}

#[cfg(not(windows))]
fn open(path: &Path) -> Result<libloading::Library, libloading::Error> {
    // SAFETY: the load runs the init code of the library. Patina built it from a project that
    // the owner trusts (11).
    unsafe { libloading::Library::new(path) }
}
