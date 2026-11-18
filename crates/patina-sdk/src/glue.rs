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
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};

use crate::{Param, Text};

/// The version of the C ABI between the engine and a cluster.
pub const ABI_VERSION: u32 = 23;

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
/// A write of a view to the graph (15.8), from `Graph::set_param`, or a parameter write of a node
/// GUI (4.3), from `Ui::set_param`: the key, a tab and the text.
pub const EDIT: u32 = 11;
/// The thumbnail of a held value (5.3), after its `VALUE`: the width, a space, the height, a space
/// and the RGBA pixels in base64.
pub const THUMB: u32 = 12;
/// The widget lines of `Viewer::show` for a held value (5.3), after its `THUMB`.
pub const VIEW: u32 = 13;

/// The most log lines of one run (15.7). The cluster does not send the lines after them.
pub const LOG_LINES: u32 = 1000;

/// The longest text of a live value, in bytes. A longer text ends with "...".
pub const TEXT_LEN: usize = 4096;

/// The engine function that receives a report: a node index, a kind and a UTF-8 text.
pub type ReportFn = unsafe extern "C" fn(*mut c_void, u32, u32, *const u8, usize);

/// The node index of a `Control` when no run is active.
pub const IDLE: u32 = u32::MAX;

/// The engine side of **Stop** (15.2): the stop flag, the node index of the call that started
/// last, the number of log lines of the run (15.7), and the quiet flag of a run that nobody shows
/// (9.1).
#[repr(C)]
pub struct Control {
    stop: AtomicBool,
    node: AtomicU32,
    lines: AtomicU32,
    quiet: AtomicBool,
}

impl Default for Control {
    fn default() -> Control {
        Control { stop: AtomicBool::new(false), node: AtomicU32::new(IDLE), lines: AtomicU32::new(0), quiet: AtomicBool::new(false) }
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

    /// While `quiet` is true, the runs send no values and no run times, because nobody shows them
    /// (9.1). The logs, the reports and the failures still come.
    pub fn set_quiet(&self, quiet: bool) {
        self.quiet.store(quiet, Relaxed);
    }

    /// True while the runs send no values and no run times.
    pub fn quiet(&self) -> bool {
        self.quiet.load(Relaxed)
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

    /// The start time of a call for `time`. A quiet run sends no times (9.1), so it does not read
    /// the clock.
    pub fn start(&self) -> Option<std::time::Instant> {
        (!self.quiet()).then(std::time::Instant::now)
    }

    /// Sends the time since `start`, if any, as the run time of node `node` (15.7). It does not
    /// allocate. The text has a fixed width, so a slow run does not make the report buffers of the
    /// engine grow.
    pub fn time(&self, node: u32, start: Option<std::time::Instant>) {
        if let Some(start) = start {
            self.send_time(node, start.elapsed().as_nanos());
        }
    }

    fn send_time(&self, node: u32, nanos: u128) {
        let mut buf = [0; 39 + 3];
        let mut out = Text::new(&mut buf);
        let _ = write!(out, "{nanos:020}");
        self.send(node, TIME, out.finish());
    }

    fn set_node(&self, node: u32) {
        NODE.set(node);
        if let Some(control) = self.control() {
            control.node.store(node, Relaxed);
        }
    }

    /// True when the control of the run is quiet: the run sends no values and no run times.
    pub fn quiet(&self) -> bool {
        self.control().is_some_and(Control::quiet)
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

/// The body of `patina_cluster_node`: the node that runs on this thread in this cluster, or `IDLE`,
/// for the crash handler of the engine (16.1). It never allocates and never waits.
pub fn thread_node() -> u32 {
    NODE.try_with(Cell::get).unwrap_or(IDLE)
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

    /// The lock, or `None` while another call holds it.
    pub fn try_lock(&self) -> Option<MutexGuard<'_, Option<T>>> {
        match self.0.try_lock() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(e)) => Some(e.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    }
}

/// Calls the `ui` function of a stateful node between two runs, or during an event run (4.3), and
/// sends its widget lines. While a run holds the state, it sends nothing, so the GUI keeps its last
/// lines. A panic drops the state, as in `process` (4.7). The control keeps the node during the call (15.2).
pub fn draw<T>(state: &State<T>, init: impl FnOnce() -> T, ui: impl Fn(&mut T, &mut crate::Ui), cmd: (u32, &str), rep: &Reporter, node: u32) -> bool {
    // A slow call of an event node holds its state, so the GUI must not wait for it.
    let Some(mut st) = state.try_lock() else { return true };
    rep.set_node(node);
    let mut gui = crate::Ui::new(cmd.0, cmd.1);
    let mut params = Vec::new();
    let caught = catch_quiet(AssertUnwindSafe(|| {
        let value = st.get_or_insert_with(init);
        ui(value, &mut gui);
        // A widget writes its value before it applies its event, so draw again with the new state.
        // Only the call with the events writes parameters.
        if !cmd.1.is_empty() {
            params = std::mem::replace(&mut gui, crate::Ui::new(cmd.0, "")).params;
            ui(value, &mut gui);
        }
        Ok(())
    }));
    let ok = finish_state(caught, &mut st, rep, node);
    if ok {
        params.iter().for_each(|(key, text)| rep.send(node, EDIT, &format!("{key}\t{text}")));
        rep.send(node, UI, gui.tree());
    }
    rep.set_node(IDLE);
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

mod abi;
pub use abi::*;
mod atom;
pub use atom::*;
mod call;
pub use call::*;
mod catch;
pub use catch::*;
mod host;
pub use host::*;
mod keep;
pub use keep::*;
mod latest;
pub use latest::*;
mod net;
pub use net::*;
mod probe;
pub use probe::*;
mod slot;
pub use slot::*;
mod view;
pub use view::*;

#[cfg(test)]
mod tests;
