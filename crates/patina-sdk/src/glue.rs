//! The support code of the clusters that Patina generates. Node code does not use it.
//!
//! A cluster keeps its parameters in one closure, so the generated code never names a type of a
//! node crate. Rust infers each type from the node calls.
//!
//! Each call into node code has its own `catch_unwind`, so a panic never crosses the C ABI
//! (spec 4.7).

use std::any::Any;
use std::cell::Cell;
use std::ffi::c_void;
use std::fmt::{self, Write as _};
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{Lanes, Param, Text, Viewer};

/// The version of the C ABI between the engine and a cluster.
pub const ABI_VERSION: u32 = 8;

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

    /// Sends the time since `start` as the run time of node `node` (15.7). It does not allocate.
    pub fn time(&self, node: u32, start: std::time::Instant) {
        let mut buf = [0; 39 + 3];
        let mut out = Text::new(&mut buf);
        let _ = write!(out, "{}", start.elapsed().as_nanos());
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
/// Patina generates in a cluster calls it. It does not send a record outside a node call.
pub fn log_line(level: &str, message: &fmt::Arguments<'_>) {
    with_run(|rep| {
        let (node, Some(control)) = (NODE.get(), rep.control()) else { return };
        if node == IDLE {
            return;
        }
        // The first check stops the count near the limit, so it cannot overflow.
        if control.lines.load(Relaxed) > LOG_LINES {
            return;
        }
        let lines = control.lines.fetch_add(1, Relaxed);
        if lines > LOG_LINES {
            return;
        }
        let mut buf = [0; TEXT_LEN + 3];
        let mut out = Text::new(&mut buf);
        let _ = if lines < LOG_LINES {
            write!(out, "{level} {message}")
        } else {
            write!(out, "WARN the run has more than {LOG_LINES} log lines. Patina shows the first {LOG_LINES}")
        };
        rep.send(node, LOG, out.finish());
    });
}

/// A command to a cluster.
pub enum Cmd<'a> {
    /// Sets the parameter with this index from its text form (6.4).
    Set(u32, &'a str),
    /// Runs the task with this index (9.2). Returns false when a node failed.
    Task(u32),
    /// Ends a run of the domain with this index (9.1): drops each value of the domain that no
    /// node took. Returns false when a drop failed.
    End(u32),
}

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

pub fn default_value<T: Param>() -> T {
    T::default_value()
}

pub fn set_param<T: Param>(slot: &mut T, text: &str, rep: &Reporter, node: u32) -> bool {
    let caught = catch_unwind(AssertUnwindSafe(|| T::from_text(text).map(|value| *slot = value)));
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

/// Reports the result of one node call. An `Err` holds the `Debug` text of the node error.
pub fn finish(caught: std::thread::Result<Result<(), String>>, rep: &Reporter, node: u32) -> bool {
    match caught {
        Ok(Ok(())) => true,
        Ok(Err(message)) => {
            rep.send(node, ERROR, &message);
            false
        }
        Err(payload) => {
            rep.send(node, PANIC, &panic_text(&*payload));
            false
        }
    }
}

/// Reports the result of a call of a stateful node. A panic can break the state, so the state
/// drops, and the next call makes a new state with `init` (spec 4.7).
pub fn finish_state<T>(caught: std::thread::Result<Result<(), String>>, state: &mut Option<T>, rep: &Reporter, node: u32) -> bool {
    let panicked = caught.is_err();
    let ok = finish(caught, rep, node);
    if panicked {
        drop_value(state.take(), rep, node);
    }
    ok
}

/// Drops a value that no node took, with its own catch (spec 4.7).
pub fn drop_value<T>(value: Option<T>, rep: &Reporter, node: u32) -> bool {
    rep.set_node(node);
    let caught = catch_unwind(AssertUnwindSafe(move || {
        drop(value);
        Ok(())
    }));
    finish(caught, rep, node)
}

/// A held value for its text. The generated code calls `(&&&Probe(v)).text(out)`, so Rust uses
/// `ViewerText` when the type implements `Viewer`, then `DebugText` for `Debug`, and `NameText`
/// if not (5.3).
pub struct Probe<'a, T>(pub &'a T);

pub trait ViewerText {
    fn text(&self, out: &mut Text<'_>);
}

impl<T: Viewer> ViewerText for &&Probe<'_, T> {
    fn text(&self, out: &mut Text<'_>) {
        let _ = Viewer::text(self.0, out);
    }
}

pub trait DebugText {
    fn text(&self, out: &mut Text<'_>);
}

impl<T: fmt::Debug> DebugText for &Probe<'_, T> {
    fn text(&self, out: &mut Text<'_>) {
        let _ = write!(out, "{:?}", self.0);
    }
}

pub trait NameText {
    fn text(&self, out: &mut Text<'_>);
}

impl<T> NameText for Probe<'_, T> {
    fn text(&self, out: &mut Text<'_>) {
        let _ = write!(out, "{} ({} bytes)", std::any::type_name::<T>(), size_of::<T>());
    }
}

/// Sends the text of held value `index` (15.9), with its own catch.
// shortcut: each run formats and sends each held value, send them at the display rate (15.6) when a graph runs faster than the display.
pub fn send_value(rep: &Reporter, index: u32, text: impl FnOnce(&mut Text<'_>)) {
    let mut buf = [0; TEXT_LEN + 3];
    let mut out = Text::new(&mut buf);
    if catch_unwind(AssertUnwindSafe(|| text(&mut out))).is_err() {
        out.clear();
        let _ = out.write_str("the text of the value panicked");
    }
    rep.send(index, VALUE, out.finish());
}

/// Sends the lanes of held value `index` (6.3), with its own catch. A panic sends no lanes.
pub fn send_lanes(rep: &Reporter, index: u32, lanes: impl FnOnce() -> Lanes) {
    let Ok(lanes) = catch_unwind(AssertUnwindSafe(lanes)) else { return };
    let mut buf = [0; 64 + 1 + 64 + 3];
    let mut out = Text::new(&mut buf);
    for i in (0..lanes.count).rev() {
        let _ = out.write_char(if lanes.bits >> i & 1 == 1 { '1' } else { '0' });
    }
    let _ = write!(out, " {}", lanes.label);
    rep.send(index, LANES, out.finish());
}

fn panic_text(payload: &(dyn Any + Send)) -> String {
    let text = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("no message");
    format!("panic: {text}")
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
/// `cluster` comes from `new_cluster` and is not freed. `text` points to `len` bytes, and `rep`
/// points to a valid reporter.
pub unsafe fn set(
    cluster: *mut Cluster,
    index: u32,
    text: *const u8,
    len: usize,
    rep: *const Reporter,
) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep, bytes) = unsafe { (&*cluster, &*rep, std::slice::from_raw_parts(text, len)) };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| (cluster.f)(Cmd::Set(index, text), rep))).unwrap_or(false)
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

#[cfg(test)]
mod tests {
    use super::*;

    struct NoDebug;

    #[derive(Debug)]
    struct Hex(u8);

    impl Viewer for Hex {
        fn text(&self, out: &mut Text<'_>) -> fmt::Result {
            write!(out, "0x{:02x}", self.0)
        }
    }

    unsafe extern "C" fn keep(ctx: *mut c_void, node: u32, kind: u32, text: *const u8, len: usize) {
        // SAFETY: the test gives a `Vec` as the context, and a text that is valid during the call.
        let (sent, bytes) = unsafe { (&mut *ctx.cast::<Vec<(u32, u32, String)>>(), std::slice::from_raw_parts(text, len)) };
        sent.push((node, kind, String::from_utf8(bytes.to_vec()).unwrap()));
    }

    #[test]
    #[allow(clippy::needless_borrow, reason = "the calls are the same as in the generated code")]
    fn values_use_the_viewer_then_debug_then_the_type_name() {
        let mut sent: Vec<(u32, u32, String)> = Vec::new();
        let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
        send_value(&rep, 0, |out| (&&&Probe(&2.5_f64)).text(out));
        send_value(&rep, 1, |out| (&&&Probe(&NoDebug)).text(out));
        send_value(&rep, 2, |out| (&&&Probe(&Hex(10))).text(out));
        let long = "\u{e9}".repeat(TEXT_LEN);
        send_value(&rep, 3, |out| (&&&Probe(&long)).text(out));
        send_value(&rep, 4, |_| panic!("no text"));
        assert_eq!(sent[0], (0, VALUE, "2.5".to_owned()));
        assert!(sent[1].2.ends_with("NoDebug (0 bytes)"), "{}", sent[1].2);
        assert_eq!(sent[2].2, "0x0a");
        // The quote and 2047 two-byte characters fill 4095 bytes. The next character does not fit.
        assert_eq!(sent[3].2, format!("\"{}...", "\u{e9}".repeat(TEXT_LEN / 2 - 1)));
        assert_eq!(sent[4].2, "the text of the value panicked");
    }

    #[test]
    fn a_panic_drops_the_state_and_an_error_keeps_it() {
        let mut sent: Vec<(u32, u32, String)> = Vec::new();
        let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
        let state = State::new();
        *state.lock() = Some(5);
        assert!(!finish_state(Ok(Err("bad".to_owned())), &mut *state.lock(), &rep, 1));
        assert_eq!(*state.lock(), Some(5));
        assert!(!finish_state(catch_unwind(|| -> Result<(), String> { panic!("broken") }), &mut *state.lock(), &rep, 1));
        assert_eq!(*state.lock(), None);
        assert_eq!(sent, [(1, ERROR, "bad".to_owned()), (1, PANIC, "panic: broken".to_owned())]);
    }

    #[test]
    fn lanes_send_the_bits_msb_first_and_the_label() {
        let mut sent: Vec<(u32, u32, String)> = Vec::new();
        let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: std::ptr::null() };
        send_lanes(&rep, 2, || Lanes::bits(4, 0b0110).label("a b"));
        send_lanes(&rep, 3, || Lanes::bits(0, 1));
        send_lanes(&rep, 4, || panic!("no lanes"));
        send_lanes(&rep, 5, || Lanes::bits(99, u64::MAX).label("x"));
        assert_eq!(sent[..2], [(2, LANES, "0110 a b".to_owned()), (3, LANES, "1 ".to_owned())]);
        assert_eq!(sent[2], (5, LANES, format!("{} x", "1".repeat(64))));
    }

    /// Runs task 0 and ends the run.
    ///
    /// # Safety
    ///
    /// The same as for `task`.
    unsafe fn run(state: *mut Cluster, rep: &Reporter) -> bool {
        // SAFETY: the caller keeps the rules of `task`, and they include the rules of `end`.
        unsafe { task(state, 0, rep) & end(state, 0, rep) }
    }

    #[test]
    #[allow(clippy::needless_borrow, reason = "the calls are the same as in the generated code")]
    fn only_sync_values_let_a_cluster_run_in_parallel() {
        assert!((&&check(&Shared::new(String::new()))).sync());
        assert!(!(&&check(&Shared::new(Cell::new(1)))).sync());
        assert!(!(&&check(&Shared::new(std::rc::Rc::new(1)))).sync());
        let shared = Shared::new(Some(1));
        *shared.write() = None;
        assert_eq!(*shared.read(), None);
    }

    #[test]
    fn stop_and_missing_inputs_start_no_node_call() {
        fn make() -> Cluster {
            let f = typed(|cmd, rep| {
                let Cmd::Task(0) = cmd else { return matches!(cmd, Cmd::End(_)) };
                // SAFETY: the test gives a control that is valid during the run.
                let node = || unsafe { &*rep.control }.node();
                let skipped = !rep.enter(8, false) && node().is_none();
                let started = rep.enter(7, true);
                if started {
                    rep.time(7, std::time::Instant::now());
                }
                skipped && started != stop_requested() && node() == started.then_some(7)
            });
            cluster(f, true)
        }
        let mut sent: Vec<(u32, u32, String)> = Vec::new();
        let control = Control::default();
        let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: &raw const control };
        let state = new_cluster(make);
        // SAFETY: the cluster comes from `new_cluster` and is freed one time. The reporter and the
        // control are valid during each run.
        unsafe {
            assert!(parallel(state));
            assert!(!task(state, 1, &rep));
            assert!(run(state, &rep));
            assert_eq!(control.node(), None);
            control.set_stop(true);
            assert!(run(state, &rep));
            free(state);
        }
        let time = sent.remove(1);
        assert_eq!((time.0, time.1), (7, TIME));
        assert!(time.2.parse::<u64>().is_ok(), "{}", time.2);
        assert_eq!(sent, [(8, SKIPPED, String::new()), (8, STOPPED, String::new()), (7, STOPPED, String::new())]);
        assert!(!stop_requested(), "the flag is not visible outside a run");
    }

    #[test]
    fn log_lines_go_to_the_node_that_runs() {
        fn make() -> Cluster {
            let f = typed(|cmd, rep| {
                if let Cmd::Task(_) = cmd {
                    log_line("INFO", &format_args!("before the first node"));
                    rep.enter(3, true);
                    for i in 0..=LOG_LINES {
                        log_line("WARN", &format_args!("line {i}"));
                    }
                }
                true
            });
            cluster(f, false)
        }
        let mut sent: Vec<(u32, u32, String)> = Vec::new();
        let control = Control::default();
        let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: &raw const control };
        let state = new_cluster(make);
        // SAFETY: the same as in `stop_and_missing_inputs_start_no_node_call`.
        unsafe {
            assert!(run(state, &rep));
            assert!(run(state, &rep));
            free(state);
        }
        log_line("INFO", &format_args!("outside a run"));
        let max = LOG_LINES as usize;
        assert_eq!(sent.len(), 2 * (max + 1));
        assert_eq!(sent[0], (3, LOG, "WARN line 0".to_owned()));
        assert_eq!(sent[max - 1].2, format!("WARN line {}", max - 1));
        assert!(sent[max].2.starts_with("WARN the run has more than"), "{}", sent[max].2);
        assert_eq!(sent[max + 1].2, "WARN line 0", "each run counts its own lines");
    }
}
