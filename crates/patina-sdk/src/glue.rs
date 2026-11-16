//! The support code of the clusters that Patina generates. Node code does not use it.
//!
//! A cluster keeps its parameters in one closure, so the generated code never names a type of a
//! node crate. Rust infers each type from the node calls.
//!
//! Each call into node code has its own `catch_unwind`, so a panic never crosses the C ABI
//! (spec 4.7).

use std::cell::Cell;
use std::ffi::c_void;
use std::fmt::{self, Write as _};
use std::marker::PhantomData;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{Param, Text};

/// The version of the C ABI between the engine and a cluster.
pub const ABI_VERSION: u32 = 18;

/// The kinds of a report.
pub const ERROR: u32 = 1;
pub const PANIC: u32 = 2;
pub const BAD_PARAM: u32 = 3;
/// A live value (15.9). Its node index is the index of the held value, in the order of `Plan::held`.
pub const VALUE: u32 = 4;
/// A node call that did not start, because the user clicked **Stop** (15.2). It has no text.
pub const STOPPED: u32 = 5;
/// A node call that did not start, because a wired input had no value (15.3). It has no text.
pub const SKIPPED: u32 = 6;
/// The lanes of a held value (6.3), after its `VALUE`. The text has one '0' or '1' for each lane,
/// the most significant lane first, then a space and the label.
pub const LANES: u32 = 7;
/// The run time of a node call (15.7). The text is the nanoseconds in decimal digits.
pub const TIME: u32 = 8;
/// A log line of a node (15.7). The text is the level, a space and the message.
pub const LOG: u32 = 9;
/// The widget lines of a call of `ui` (4.3), from `Ui::tree`.
pub const UI: u32 = 10;
/// A write of a view to the graph (15.8), from `Graph::set_param`.
pub const EDIT: u32 = 11;

/// The most log lines of one run (15.7). The cluster does not send the lines after them.
pub const LOG_LINES: u32 = 1000;

/// The longest text of a live value, in bytes. A longer text ends with "...".
pub const TEXT_LEN: usize = 4096;

/// The engine function that receives a report: a node index, a kind and a UTF-8 text.
pub type ReportFn = unsafe extern "C" fn(*mut c_void, u32, u32, *const u8, usize);

/// The node index of a `Control` when no run is active.
pub const IDLE: u32 = u32::MAX;

/// The engine side of **Stop** (15.2): the stop flag, the node index of the call that started
/// last, and the number of log lines of the run (15.7).
#[repr(C)]
pub struct Control {
    stop: AtomicBool,
    node: AtomicU32,
    lines: AtomicU32,
}

impl Default for Control {
    fn default() -> Control {
        Control { stop: AtomicBool::new(false), node: AtomicU32::new(IDLE), lines: AtomicU32::new(0) }
    }
}

impl Control {
    /// Asks the active run to stop, or removes the request.
    pub fn set_stop(&self, stop: bool) {
        self.stop.store(stop, Relaxed);
    }

    /// True when the user asked the runs to stop (15.2).
    pub fn stopped(&self) -> bool {
        self.stop.load(Relaxed)
    }

    /// The index of the node that started last in the active run. None between runs.
    pub fn node(&self) -> Option<u32> {
        let node = self.node.load(Relaxed);
        (node != IDLE).then_some(node)
    }
}

/// The engine gives a reporter to each call. It stays valid during the call.
#[repr(C)]
pub struct Reporter {
    pub ctx: *mut c_void,
    pub report: ReportFn,
    /// The control of a run, or null for a call that is not a run.
    pub control: *const Control,
}

impl Reporter {
    pub fn send(&self, node: u32, kind: u32, text: &str) {
        // SAFETY: the engine gives a function and a context that are valid during the call.
        unsafe { (self.report)(self.ctx, node, kind, text.as_ptr(), text.len()) }
    }

    /// Records that node `node` starts, and returns true. `ready` is true when each wired input
    /// has a value. Else it reports `STOPPED` after **Stop** (15.2), or `SKIPPED` (15.3).
    pub fn enter(&self, node: u32, ready: bool) -> bool {
        let kind = if self.control().is_some_and(|c| c.stop.load(Relaxed)) {
            STOPPED
        } else if !ready {
            SKIPPED
        } else {
            self.set_node(node);
            return true;
        };
        self.send(node, kind, "");
        false
    }

    /// Sends the time since `start` as the run time of node `node` (15.7). It does not allocate. The
    /// text has a fixed width, so a slow run does not make the report buffers of the engine grow.
    pub fn time(&self, node: u32, start: std::time::Instant) {
        let mut buf = [0; 39 + 3];
        let mut out = Text::new(&mut buf);
        let _ = write!(out, "{:020}", start.elapsed().as_nanos());
        self.send(node, TIME, out.finish());
    }

    fn set_node(&self, node: u32) {
        NODE.set(node);
        if let Some(control) = self.control() {
            control.node.store(node, Relaxed);
        }
    }

    fn control(&self) -> Option<&Control> {
        // SAFETY: the engine gives null or a control that is valid during the call.
        unsafe { self.control.as_ref() }
    }
}

thread_local! {
    /// The reporter of the task on this thread, for `stop_requested` and `log_line`. Null outside
    /// a task.
    static RUN: Cell<*const Reporter> = const { Cell::new(std::ptr::null()) };
    /// The node that runs on this thread, for `log_line`.
    static NODE: Cell<u32> = const { Cell::new(IDLE) };
}

/// Calls `f` with the reporter of the run on this thread. Gives None outside a run.
fn with_run<R>(f: impl FnOnce(&Reporter) -> R) -> Option<R> {
    // SAFETY: `run` sets the pointer only during the call, and the engine keeps the reporter
    // valid during the call.
    unsafe { RUN.with(Cell::get).as_ref() }.map(f)
}

pub(crate) fn stop_requested() -> bool {
    with_run(|rep| rep.control().is_some_and(|c| c.stop.load(Relaxed))).unwrap_or(false)
}

/// Sends a record of the `log` crate as a log line of the node that runs (15.7). The logger that
/// Patina generates in a cluster calls it. A record from another thread or from outside a node
/// call goes to the standard error.
pub fn log_line(level: &str, message: &fmt::Arguments<'_>) {
    if !log_to_run(level, message) {
        use std::io::Write as _;
        // A logger must not panic, so a failed write is ignored.
        let _ = writeln!(std::io::stderr().lock(), "{level} {message}");
    }
}

/// Sends the record to the node that runs on this thread. Gives false when no node runs here.
fn log_to_run(level: &str, message: &fmt::Arguments<'_>) -> bool {
    with_run(|rep| {
        let (node, Some(control)) = (NODE.get(), rep.control()) else { return false };
        if node == IDLE {
            return false;
        }
        // The first check stops the count near the limit, so it cannot overflow.
        if control.lines.load(Relaxed) > LOG_LINES {
            return true;
        }
        let lines = control.lines.fetch_add(1, Relaxed);
        if lines > LOG_LINES {
            return true;
        }
        let mut buf = [0; TEXT_LEN + 3];
        let mut out = Text::new(&mut buf);
        let _ = if lines < LOG_LINES {
            write!(out, "{level} {message}")
        } else {
            write!(out, "WARN the run has more than {LOG_LINES} log lines. Patina shows the first {LOG_LINES}")
        };
        rep.send(node, LOG, out.finish());
        true
    })
    .unwrap_or(false)
}

/// A command to a cluster.
pub enum Cmd<'a> {
    /// Sets the parameter with this index from its text form, or to its default value for
    /// `None` (4.8, 6.4).
    Set(u32, Option<&'a str>),
    /// Starts a run of the domain with this index: gets the newest values of the latest-value
    /// wires without a lock (9.3).
    Begin(u32),
    /// Runs the task with this index (9.2). Returns false when a node failed.
    Task(u32),
    /// Runs the event task with this index for one fire of its trigger (9.4), with the text of
    /// the event. Returns false when a node failed.
    Fire(u32, &'a str),
    /// Ends a run of the domain with this index (9.1): drops each value of the domain that no
    /// node took. Returns false when a drop failed.
    End(u32),
    /// Calls the `ui` function of the node with this index (4.3), with a mode and the events.
    Ui(u32, u32, &'a str),
    /// Takes the state of the stateful node with this index out of the cluster (7.2), as a box.
    /// Writes null when the node has no state.
    Take(u32, &'a mut *mut c_void),
    /// Gives a state from `Take` of an old cluster to the node with this index (7.2).
    Put(u32, Moved),
    /// Writes a new reference to the value of the link with this index (8.2), for `Import` in the
    /// cluster of the other domain. Writes null when this cluster does not make the value.
    Export(u32, &'a mut *mut c_void),
    /// Links the link with this index to a value from `Export` of a cluster of the same build.
    Import(u32, Moved),
    /// Gives the base the slot library (7.1). The engine keeps it valid until the next `Slot`.
    Slot(*const SlotRef),
    /// Calls the node with this index in the slot library (7.1). The arguments point to the
    /// inputs, in the order of the node inputs, then to an `Option` for the result.
    Call(u32, &'a [*mut c_void]),
    /// Writes the kept copy of the held value with this index, for a view (15.8). See `Keeps::read`.
    Read(u32, &'a mut *mut c_void),
}

/// A node state that moves from an old cluster to a new cluster (7.2), or a value of a link
/// (8.2). Only `put` and `import` make it, from `take_state` or `export` with the same type.
pub struct Moved(*mut c_void);

type ClusterFn = dyn Fn(Cmd<'_>, &Reporter) -> bool;

/// The closure of a cluster, and true when the engine can call it from many threads at a time.
pub struct Cluster {
    f: Box<ClusterFn>,
    parallel: bool,
}

/// Gives the closure of a cluster its argument types.
pub fn typed<F: Fn(Cmd<'_>, &Reporter) -> bool + 'static>(f: F) -> F {
    f
}

/// `parallel` is true only when each value that the closure keeps is `Sync` (see `Check`).
pub fn cluster<F: Fn(Cmd<'_>, &Reporter) -> bool + 'static>(f: F, parallel: bool) -> Cluster {
    Cluster { f: Box::new(f), parallel }
}

/// The type of a value, for its thread rule (9.2). The generated code calls `(&&check(&v)).sync()`
/// after the closure, when Rust knows the type. Rust then uses `SyncCheck` when the type is
/// `Sync`, and `NotSyncCheck` if not. A check of the closure type itself does not work, because
/// Rust knows the types that a closure keeps only at the end of the function.
pub struct Check<T>(PhantomData<fn() -> T>);

pub fn check<T>(_: &T) -> Check<T> {
    Check(PhantomData)
}

pub trait SyncCheck {
    fn sync(&self) -> bool;
}

impl<T: Sync> SyncCheck for &Check<T> {
    fn sync(&self) -> bool {
        true
    }
}

pub trait NotSyncCheck {
    fn sync(&self) -> bool;
}

impl<T> NotSyncCheck for Check<T> {
    fn sync(&self) -> bool {
        false
    }
}

/// A parameter, or a value that goes from one task to another (9.2). Many readers can share it.
/// The lock ignores poison: a panic in a node call never leaves a value half written.
pub struct Shared<T>(RwLock<T>);

impl<T> Shared<T> {
    pub fn new(value: T) -> Shared<T> {
        Shared(RwLock::new(value))
    }

    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        self.0.read().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.0.write().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A value that a node of this cluster reads from a cluster of another domain: a latest-value
/// wire (8.2, 9.1). It is empty until the engine links it to the value of the other cluster.
pub struct Link<T> {
    own: Shared<Option<T>>,
    link: OnceLock<Arc<Shared<Option<T>>>>,
}

impl<T> Link<T> {
    #[allow(clippy::new_without_default, reason = "the generated code calls only `new`")]
    pub fn new() -> Link<T> {
        Link { own: Shared::new(None), link: OnceLock::new() }
    }

    /// Gives Rust the type of the value. The generated code calls it in a block that never runs.
    pub fn witness(&self, _value: T) {}
}

impl<T> std::ops::Deref for Link<T> {
    type Target = Shared<Option<T>>;

    fn deref(&self) -> &Shared<Option<T>> {
        self.link.get().map_or(&self.own, |link| link)
    }
}

/// A value of the type in `witness`, for a block that never runs (see `Link::witness`).
pub fn like<T>(_witness: &Option<T>) -> T {
    unreachable!("the witness block never runs")
}

/// Gives two values the same type, in a block that never runs.
pub fn same<T>(_a: &T, _b: &T) {}

/// The value of a delay node (9.2), with the type of its input. The first frame gives the default.
pub fn delay_for<T: Default>(_input: &Shared<Option<T>>) -> Shared<T> {
    Shared::new(T::default())
}

/// The state of one use of a stateful node (4.2). It is empty until the first call. Only the task
/// of the node uses it, so a state that is `Send` lets the cluster run in parallel.
pub struct State<T>(Mutex<Option<T>>);

impl<T> State<T> {
    #[allow(clippy::new_without_default, reason = "the generated code calls only `new`")]
    pub fn new() -> State<T> {
        State(Mutex::new(None))
    }

    pub fn lock(&self) -> MutexGuard<'_, Option<T>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Calls the `ui` function of a stateful node between two runs (4.3), and sends its widget lines.
/// A panic drops the state, as in `process` (4.7).
pub fn draw<T>(state: &State<T>, init: impl FnOnce() -> T, ui: impl Fn(&mut T, &mut crate::Ui), cmd: (u32, &str), rep: &Reporter, node: u32) -> bool {
    let mut st = state.lock();
    let mut gui = crate::Ui::new(cmd.0, cmd.1);
    let caught = catch_quiet(AssertUnwindSafe(|| {
        let value = st.get_or_insert_with(init);
        ui(value, &mut gui);
        // A widget writes its value before it applies its event, so draw again with the new state.
        if !cmd.1.is_empty() {
            gui = crate::Ui::new(cmd.0, "");
            ui(value, &mut gui);
        }
        Ok(())
    }));
    let ok = finish_state(caught, &mut st, rep, node);
    if ok {
        rep.send(node, UI, gui.tree());
    }
    ok
}

/// Takes the state out of `state` as a raw box, for `Cmd::Take`.
pub fn take_state<T>(state: &State<T>, out: &mut *mut c_void) -> bool {
    *out = state.lock().take().map_or(std::ptr::null_mut(), |value| Box::into_raw(Box::new(value)).cast());
    true
}

/// Puts a state from an old cluster into `state`, for `Cmd::Put`. A state that the node had drops.
pub fn put_state<T>(state: &State<T>, moved: Moved, rep: &Reporter, node: u32) -> bool {
    // SAFETY: `put` makes `moved` only from a box of `take_state`, and its caller keeps the rule
    // that the state type is `T`.
    let value = unsafe { Box::from_raw(moved.0.cast::<T>()) };
    let old = state.lock().replace(*value);
    drop_value(old, rep, node)
}

/// Writes a new reference to `value` for `import_link` in another cluster (8.2).
pub fn export_link<T>(value: &Arc<Shared<Option<T>>>, out: &mut *mut c_void) -> bool {
    *out = Arc::into_raw(Arc::clone(value)).cast_mut().cast();
    true
}

/// Links `link` to a value from `export_link`. Returns false when it has a link already.
pub fn import_link<T>(link: &Link<T>, moved: Moved) -> bool {
    // SAFETY: `moved` came from `export` of the same link in a cluster of the same build, so it
    // is an `Arc` of the same type (see `Moved`). The engine gives it one time.
    let value = unsafe { Arc::from_raw(moved.0.cast_const().cast::<Shared<Option<T>>>()) };
    link.link.set(value).is_ok()
}

pub fn default_value<T: Param>() -> T {
    T::default_value()
}

pub fn set_param<T: Param>(slot: &mut T, text: Option<&str>, rep: &Reporter, node: u32) -> bool {
    let value = || text.map_or_else(|| Ok(T::default_value()), T::from_text);
    let caught = catch_quiet(AssertUnwindSafe(|| value().map(|value| *slot = value)));
    match caught {
        Ok(Ok(())) => true,
        Ok(Err(message)) => {
            rep.send(node, BAD_PARAM, &message);
            false
        }
        Err(payload) => {
            rep.send(node, PANIC, &panic_text(&*payload));
            false
        }
    }
}

/// The engine function that keeps one value for each key in the process (10.2). When the key has
/// no value and `value` is not null, it keeps `value`. It gives the value of the key, or null.
pub type ShareFn = unsafe extern "C" fn(key: *const u8, len: usize, value: *mut c_void) -> *mut c_void;

/// The share function of the engine and the project root.
static HOST: OnceLock<(ShareFn, String)> = OnceLock::new();

/// The body of `patina_cluster_host`. The engine calls it before `patina_cluster_new`.
///
/// # Safety
///
/// `root` points to `len` bytes.
pub unsafe fn set_host(share: ShareFn, root: *const u8, len: usize) {
    // SAFETY: the caller gives `len` bytes at `root`.
    let root = unsafe { std::slice::from_raw_parts(root, len) };
    let _ = HOST.set((share, String::from_utf8_lossy(root).into_owned()));
}

/// The body of `patina_sdk::asset!`. With no engine, as in a test, the file is in the crate folder.
pub fn asset(crate_dir: &str, file: &str) -> std::path::PathBuf {
    asset_in(HOST.get().map(|(_, root)| root.as_str()), crate_dir, file)
}

fn asset_in(root: Option<&str>, crate_dir: &str, file: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(crate_dir);
    let Some((root, mut parts)) = root.map(|root| (root, dir.iter().rev())) else {
        return dir.join("assets").join(file);
    };
    // A node crate is in `<group>/<name>` of the project, for example `nodes/blur` (6.2).
    let (name, group) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
    std::path::Path::new(root).join(group).join(name).join("assets").join(file)
}

/// The body of `patina_sdk::project_path`. An absolute path stays the same.
pub fn project_path(path: &str) -> std::path::PathBuf {
    match HOST.get() {
        Some((_, root)) => std::path::Path::new(root).join(path),
        None => path.into(),
    }
}

/// The `ShareFn` of the engine. A cluster with no engine, as in a test, uses its own copy.
///
/// # Safety
///
/// `key` points to `len` bytes. `value` is null or a value that stays until the process ends.
pub unsafe extern "C" fn share(key: *const u8, len: usize, value: *mut c_void) -> *mut c_void {
    static VALUES: Mutex<Vec<(Box<[u8]>, usize)>> = Mutex::new(Vec::new());
    // SAFETY: the caller gives `len` bytes at `key`.
    let key = unsafe { std::slice::from_raw_parts(key, len) };
    let mut values = VALUES.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(&(_, kept)) = values.iter().find(|(k, _)| **k == *key) {
        return kept as *mut c_void;
    }
    if !value.is_null() {
        values.push((key.into(), value as usize));
    }
    value
}

/// The body of `patina_sdk::shared`. The clusters and the slot of one project have the same
/// toolchain and the same source, so a type has the same layout in each (7.2). The key holds the
/// project root, so the projects of one editor process do not share.
pub fn shared<T: Send + Sync + 'static>(make: impl FnOnce() -> T) -> &'static T {
    let (share, root) = HOST.get().map_or((share as ShareFn, ""), |(f, root)| (*f, root.as_str()));
    shared_with(share, root, make)
}

/// The shared value of the project `root` for the engine, which gives `share` to the clusters.
pub fn shared_in<T: Send + Sync + 'static>(root: &str, make: impl FnOnce() -> T) -> &'static T {
    shared_with(share, root, make)
}

fn shared_with<T: Send + Sync + 'static>(share: ShareFn, root: &str, make: impl FnOnce() -> T) -> &'static T {
    let key = format!("{root} {} {} {}", std::any::type_name::<T>(), size_of::<T>(), align_of::<T>());
    // SAFETY: the key is valid during the calls, and a kept value is never freed.
    let value = unsafe {
        let mut value = share(key.as_ptr(), key.len(), std::ptr::null_mut());
        if value.is_null() {
            let mine = Box::into_raw(Box::new(make())).cast::<c_void>();
            value = share(key.as_ptr(), key.len(), mine);
            if value != mine {
                // Another thread kept its value first.
                drop(Box::from_raw(mine.cast::<T>()));
            }
        }
        value
    };
    // SAFETY: the key holds the `TypeId` of `T`, so the value of the key is a `T`.
    unsafe { &*value.cast::<T>() }
}

/// The body of `patina_cluster_new`. Returns null when the parameter defaults panic.
pub fn new_cluster(make: fn() -> Cluster) -> *mut Cluster {
    match catch_unwind(make) {
        Ok(cluster) => Box::into_raw(Box::new(cluster)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// The body of `patina_cluster_set`.
///
/// # Safety
///
/// `cluster` comes from `new_cluster` and is not freed. `text` is null for the default value, or
/// points to `len` bytes. `rep` points to a valid reporter.
pub unsafe fn set(
    cluster: *mut Cluster,
    index: u32,
    text: *const u8,
    len: usize,
    rep: *const Reporter,
) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    let text = match text.is_null() {
        true => None,
        // SAFETY: the caller keeps the rules of this function.
        false => match std::str::from_utf8(unsafe { std::slice::from_raw_parts(text, len) }) {
            Ok(text) => Some(text),
            Err(_) => return false,
        },
    };
    catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Set(index, text), rep))).unwrap_or(false)
}

/// The body of `patina_cluster_ui`.
///
/// # Safety
///
/// The same as for `set`, and no run of the cluster is active.
pub unsafe fn ui(cluster: *const Cluster, node: u32, mode: u32, text: *const u8, len: usize, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep, bytes) = unsafe { (&*cluster, &*rep, std::slice::from_raw_parts(text, len)) };
    let Ok(events) = std::str::from_utf8(bytes) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Ui(node, mode, events), rep))).unwrap_or(false)
}

/// The body of `patina_cluster_parallel`: true when the engine can run the tasks of a run on many
/// threads at a time.
///
/// # Safety
///
/// `cluster` comes from `new_cluster` and is not freed.
pub unsafe fn parallel(cluster: *const Cluster) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    unsafe { (*cluster).parallel }
}

/// The body of `patina_cluster_task`.
///
/// # Safety
///
/// `cluster` comes from `new_cluster`, and is not freed or set during the call. `rep` points to a
/// valid reporter, and its control is valid during the call. Other tasks of the same run can run
/// on other threads at the same time only when `parallel` is true.
pub unsafe fn task(cluster: *const Cluster, task: u32, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    in_run(rep, || (cluster.f)(Cmd::Task(task), rep))
}

/// The body of `patina_cluster_fire`: one event run of task `task` with the event text (9.4).
///
/// # Safety
///
/// The same as for `task`. `text` points to `len` bytes. Other event runs of the same cluster can
/// run on other threads at the same time only when `parallel` is true.
pub unsafe fn fire(cluster: *const Cluster, task: u32, text: *const u8, len: usize, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep, bytes) = unsafe { (&*cluster, &*rep, std::slice::from_raw_parts(text, len)) };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let ok = in_run(rep, || (cluster.f)(Cmd::Fire(task, text), rep));
    rep.set_node(IDLE);
    NODE.set(IDLE);
    if let Some(control) = rep.control() {
        control.lines.store(0, Relaxed);
    }
    let _ = std::io::Write::flush(&mut std::io::stdout());
    ok
}

/// The body of `patina_cluster_begin`, before the first task of a run of domain `domain`.
///
/// # Safety
///
/// The same as for `end`.
pub unsafe fn begin(cluster: *const Cluster, domain: u32, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    in_run(rep, || (cluster.f)(Cmd::Begin(domain), rep))
}

/// The body of `patina_cluster_end`, after the last task of a run of domain `domain`.
///
/// # Safety
///
/// The same as for `task`, and no task of the cluster runs during the call.
pub unsafe fn end(cluster: *const Cluster, domain: u32, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    let ok = in_run(rep, || (cluster.f)(Cmd::End(domain), rep));
    rep.set_node(IDLE);
    NODE.set(IDLE);
    if let Some(control) = rep.control() {
        control.lines.store(0, Relaxed);
    }
    // The cluster has its own standard library, which does not flush its stdout at exit.
    let _ = std::io::Write::flush(&mut std::io::stdout());
    ok
}

/// The body of `patina_cluster_take`: the state of node `node` as a box, or null (7.2).
///
/// # Safety
///
/// The same as for `ui`.
pub unsafe fn take(cluster: *const Cluster, node: u32, rep: *const Reporter) -> *mut c_void {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    let mut out = std::ptr::null_mut();
    let _ = catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Take(node, &mut out), rep)));
    out
}

/// The body of `patina_cluster_put`: gives `value` to node `node` (7.2). Returns false when the
/// cluster did not take it. Then the value leaks.
///
/// # Safety
///
/// The same as for `ui`. `value` comes from `take` of a cluster, and is not given again. Its state
/// type is the same type as the state type of node `node`.
pub unsafe fn put(cluster: *const Cluster, node: u32, value: *mut c_void, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Put(node, Moved(value)), rep))).unwrap_or(false)
}

/// The body of `patina_cluster_export`: a new reference to the value of link `index` (8.2), or
/// null when this cluster does not make it.
///
/// # Safety
///
/// The same as for `ui`.
pub unsafe fn export(cluster: *const Cluster, index: u32, rep: *const Reporter) -> *mut c_void {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    let mut out = std::ptr::null_mut();
    let _ = catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Export(index, &mut out), rep)));
    out
}

/// The body of `patina_cluster_import`: links link `index` to `value` (8.2). Returns false when
/// the cluster did not take it. Then the reference leaks.
///
/// # Safety
///
/// The same as for `ui`. `value` comes from `export` of link `index` of a cluster of the same
/// build, and is not given again.
pub unsafe fn import(cluster: *const Cluster, index: u32, value: *mut c_void, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&*cluster, &*rep) };
    catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Import(index, Moved(value)), rep))).unwrap_or(false)
}

/// Calls `f` with `rep` as the reporter of the task on this thread.
fn in_run(rep: &Reporter, f: impl FnOnce() -> bool) -> bool {
    let outer = RUN.replace(rep);
    NODE.set(IDLE);
    let ok = catch_unwind(AssertUnwindSafe(f)).unwrap_or(false);
    RUN.set(outer);
    NODE.set(IDLE);
    ok
}

/// The body of `patina_cluster_free`. If a drop panics, the rest of the cluster leaks (4.7).
///
/// # Safety
///
/// `cluster` is null, or comes from `new_cluster` and is not freed.
pub unsafe fn free(cluster: *mut Cluster) {
    if !cluster.is_null() {
        // SAFETY: the caller gives a cluster from `new_cluster`, and frees it one time.
        let cluster = unsafe { Box::from_raw(cluster) };
        let _ = catch_unwind(AssertUnwindSafe(move || drop(cluster)));
    }
}

mod call;
pub use call::*;
mod catch;
pub use catch::*;
mod keep;
pub use keep::*;
mod latest;
pub use latest::*;
mod probe;
pub use probe::*;
mod slot;
pub use slot::*;
mod view;
pub use view::*;

#[cfg(test)]
mod tests;
