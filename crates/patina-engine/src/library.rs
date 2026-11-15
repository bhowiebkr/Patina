//! Loads one cluster library and keeps its functions of the C ABI of `patina_sdk::glue` (8.2).

use std::ffi::c_void;
use std::ops::Range;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use patina_sdk::glue::{self, CallFn, Control, Reporter, SlotRef};

use crate::Error;
use crate::node::Domain;
use crate::plan::Task;
use crate::pool::Work;

type VersionFn = unsafe extern "C" fn() -> u32;
type NewFn = unsafe extern "C" fn() -> *mut c_void;
pub(crate) type SetFn = unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *const Reporter) -> bool;
pub(crate) type UiFn = unsafe extern "C" fn(*const c_void, u32, u32, *const u8, usize, *const Reporter) -> bool;
type HostFn = unsafe extern "C" fn(glue::ShareFn, *const u8, usize);
type ParallelFn = unsafe extern "C" fn(*const c_void) -> bool;
type TaskFn = unsafe extern "C" fn(*const c_void, u32, *const Reporter) -> bool;
pub(crate) type EndFn = unsafe extern "C" fn(*const c_void, u32, *const Reporter) -> bool;
type FreeFn = unsafe extern "C" fn(*mut c_void);
pub(crate) type TakeFn = unsafe extern "C" fn(*const c_void, u32, *const Reporter) -> *mut c_void;
pub(crate) type PutFn = unsafe extern "C" fn(*const c_void, u32, *mut c_void, *const Reporter) -> bool;
pub(crate) type SlotFn = unsafe extern "C" fn(*const c_void, *const SlotRef, *const Reporter) -> bool;

/// The cluster of one clock domain: its library functions and its state (8.2).
pub(crate) struct Library {
    pub state: *mut c_void,
    new: NewFn,
    pub set: SetFn,
    pub ui: UiFn,
    pub end: EndFn,
    free: FreeFn,
    pub take: TakeFn,
    pub put: PutFn,
    pub export: TakeFn,
    pub import: PutFn,
    /// Gives a base the slot library (7.1).
    pub slot: SlotFn,
    /// Calls a node of a slot library (7.1).
    pub call: CallFn,
    /// True when the tasks of a run can run on many threads at a time (9.2).
    pub parallel: bool,
    /// The tasks of the domain (9.1).
    pub tasks: Arc<Tasks>,
}

impl Library {
    /// Loads the cluster library of `domain` and makes its state. The nodes find their assets in
    /// the project `root` (10.3).
    pub fn load(path: &Path, root: &Path, all: &[Task], domain: Domain) -> Result<Library, Error> {
        let fail = |text: String| Error::Load(format!("{}: {text}", path.display()));
        let missing = |e: libloading::Error| fail(e.to_string());
        let library = open(path).map_err(missing)?;
        // SAFETY: Patina generated the cluster with these symbols and signatures (generate.rs).
        let loaded = unsafe {
            let version: VersionFn = symbol(&library, b"patina_abi_version\0").map_err(missing)?;
            if version() != glue::ABI_VERSION {
                return Err(fail("the cluster has another ABI version. Build it again".to_owned()));
            }
            let host: HostFn = symbol(&library, b"patina_cluster_host\0").map_err(missing)?;
            // shortcut: a root that is not UTF-8 changes, so its assets are not found; pass the OS bytes when a user needs it.
            let root = root.to_string_lossy();
            host(glue::share, root.as_ptr(), root.len());
            let new: NewFn = symbol(&library, b"patina_cluster_new\0").map_err(missing)?;
            let set: SetFn = symbol(&library, b"patina_cluster_set\0").map_err(missing)?;
            let ui: UiFn = symbol(&library, b"patina_cluster_ui\0").map_err(missing)?;
            let parallel: ParallelFn = symbol(&library, b"patina_cluster_parallel\0").map_err(missing)?;
            let task: TaskFn = symbol(&library, b"patina_cluster_task\0").map_err(missing)?;
            let end: EndFn = symbol(&library, b"patina_cluster_end\0").map_err(missing)?;
            let free: FreeFn = symbol(&library, b"patina_cluster_free\0").map_err(missing)?;
            let take: TakeFn = symbol(&library, b"patina_cluster_take\0").map_err(missing)?;
            let put: PutFn = symbol(&library, b"patina_cluster_put\0").map_err(missing)?;
            let export: TakeFn = symbol(&library, b"patina_cluster_export\0").map_err(missing)?;
            let import: PutFn = symbol(&library, b"patina_cluster_import\0").map_err(missing)?;
            let slot: SlotFn = symbol(&library, b"patina_cluster_slot\0").map_err(missing)?;
            let call: CallFn = symbol(&library, b"patina_cluster_call\0").map_err(missing)?;
            let state = new();
            if state.is_null() {
                return Err(fail("a parameter default panicked".to_owned()));
            }
            let tasks = Arc::new(Tasks::new(state, task, all, domain));
            Library { state, new, set, ui, end, free, take, put, export, import, slot, call, parallel: parallel(state), tasks }
        };
        // Rust libraries do not unload safely, so the library stays loaded (8.5).
        std::mem::forget(library);
        Ok(loaded)
    }

    /// A new state with the default parameters, or null when a default panicked.
    pub fn make(&self) -> *mut c_void {
        // SAFETY: the function came from the loaded cluster.
        unsafe { (self.new)() }
    }

    /// Frees a state of `make` that is not used. Null is not a state.
    pub fn free_state(&self, state: *mut c_void) {
        if !state.is_null() {
            // SAFETY: `state` came from `patina_cluster_new` of this library and is not used again.
            unsafe { (self.free)(state) }
        }
    }

    /// Puts a state of `make` in place of the active state, and frees the active state.
    pub fn replace(&mut self, state: *mut c_void) {
        let old = std::mem::replace(&mut self.state, state);
        self.free_state(old);
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        self.free_state(self.state);
    }
}

/// The reports of one call: the node, the kind and the text range of each report, and the texts
/// in one buffer. It is the context of the reporter. The buffers keep their capacity.
#[derive(Default)]
pub(crate) struct Sink {
    items: Vec<(u32, u32, Range<usize>)>,
    text: String,
}

impl Sink {
    pub fn push(&mut self, node: u32, kind: u32, text: &str) {
        let start = self.text.len();
        self.text.push_str(text);
        self.items.push((node, kind, start..self.text.len()));
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.text.clear();
    }

    /// Moves the reports of `other` to the end of this sink.
    pub fn append(&mut self, other: &mut Sink) {
        let start = self.text.len();
        self.text.push_str(&other.text);
        self.items.extend(other.items.drain(..).map(|(node, kind, r)| (node, kind, r.start + start..r.end + start)));
        other.text.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, u32, &str)> {
        self.items.iter().map(|(node, kind, r)| (*node, *kind, &self.text[r.clone()]))
    }
}

pub(crate) unsafe extern "C" fn collect(ctx: *mut c_void, node: u32, kind: u32, text: *const u8, len: usize) {
    // SAFETY: `ctx` is the sink of `Cluster::call`, and `text` points to `len` bytes during the
    // call (glue::Reporter).
    let (sink, bytes) = unsafe { (&mut *ctx.cast::<Sink>(), std::slice::from_raw_parts(text, len)) };
    sink.push(node, kind, &String::from_utf8_lossy(bytes));
}

/// The tasks of a run of one domain (9.2), and a report list for each task. The worker pool
/// calls them.
pub(crate) struct Tasks {
    pub state: AtomicPtr<c_void>,
    pub control: AtomicPtr<Control>,
    task: TaskFn,
    /// The cluster task index of each task.
    pub ids: Vec<usize>,
    pub inputs: Vec<u32>,
    next: Vec<Vec<usize>>,
    pub sinks: Vec<Mutex<Sink>>,
    pub ok: AtomicBool,
}

// SAFETY: `Cluster::run_domain` sets the state and the control before the tasks start, and keeps
// them valid until all tasks are done. Many threads call tasks at a time only when the cluster says
// that each value that it keeps is `Sync` (`patina_cluster_parallel`).
unsafe impl Send for Tasks {}
// SAFETY: the same as for `Send`.
unsafe impl Sync for Tasks {}

impl Work for Tasks {
    fn inputs(&self) -> &[u32] {
        &self.inputs
    }

    fn next(&self, task: usize) -> &[usize] {
        &self.next[task]
    }

    fn run(&self, task: usize) {
        let index = self.ids.get(task).and_then(|&id| u32::try_from(id).ok());
        let (Some(sink), Some(index)) = (self.sinks.get(task), index) else { return };
        let mut sink = lock(sink);
        let rep = Reporter { ctx: (&raw mut *sink).cast(), report: collect, control: self.control.load(Relaxed) };
        // SAFETY: the same as for `unsafe impl Send for Tasks`. The reporter and its sink are valid
        // during the call.
        if !unsafe { (self.task)(self.state.load(Relaxed), index, &rep) } {
            self.ok.store(false, Relaxed);
        }
    }
}

impl Tasks {
    /// The tasks of `all` in `domain`, in the same order. A link to another domain is not a wait.
    fn new(state: *mut c_void, task: TaskFn, all: &[Task], domain: Domain) -> Tasks {
        let ids: Vec<usize> = (0..all.len()).filter(|&t| all[t].domain == domain).collect();
        let next: Vec<Vec<usize>> =
            ids.iter().map(|&t| all[t].next.iter().filter_map(|n| ids.iter().position(|i| i == n)).collect()).collect();
        let mut inputs = vec![0; ids.len()];
        for &n in next.iter().flatten() {
            inputs[n] += 1;
        }
        let sinks = ids.iter().map(|_| Mutex::new(Sink::default())).collect();
        let (state, control) = (AtomicPtr::new(state), AtomicPtr::new(std::ptr::null_mut()));
        Tasks { state, control, task, ids, inputs, next, sinks, ok: AtomicBool::new(true) }
    }
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
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
