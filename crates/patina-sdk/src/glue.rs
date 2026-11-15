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
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

use crate::{Param, Text, Viewer};

/// The version of the C ABI between the engine and a cluster.
pub const ABI_VERSION: u32 = 3;

/// The kinds of a report.
pub const ERROR: u32 = 1;
pub const PANIC: u32 = 2;
pub const BAD_PARAM: u32 = 3;
/// A live value (15.9). Its node index is the index of the held value, in the order of `Plan::held`.
pub const VALUE: u32 = 4;
/// A node call that did not start, because the user clicked **Stop** (15.2). It has no text.
pub const STOPPED: u32 = 5;

/// The longest text of a live value, in bytes. A longer text ends with "...".
pub const TEXT_LEN: usize = 4096;

/// The engine function that receives a report: a node index, a kind and a UTF-8 text.
pub type ReportFn = unsafe extern "C" fn(*mut c_void, u32, u32, *const u8, usize);

/// The node index of a `Control` when no run is active.
pub const IDLE: u32 = u32::MAX;

/// The engine side of **Stop** (15.2): the stop flag, and the node index of the call that runs.
#[repr(C)]
pub struct Control {
    stop: AtomicBool,
    node: AtomicU32,
}

impl Default for Control {
    fn default() -> Control {
        Control { stop: AtomicBool::new(false), node: AtomicU32::new(IDLE) }
    }
}

impl Control {
    /// Asks the active run to stop, or removes the request.
    pub fn set_stop(&self, stop: bool) {
        self.stop.store(stop, Relaxed);
    }

    /// The index of the node that runs, or that ran last in the active run. None between runs.
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

    /// Records that node `node` starts. When the user clicked **Stop**, it reports `STOPPED` and
    /// returns false, and the node does not start (15.2).
    pub fn enter(&self, node: u32) -> bool {
        // SAFETY: the same as in `set_node`.
        if unsafe { self.control.as_ref() }.is_some_and(|c| c.stop.load(Relaxed)) {
            self.send(node, STOPPED, "");
            return false;
        }
        self.set_node(node);
        true
    }

    fn set_node(&self, node: u32) {
        // SAFETY: the engine gives null or a control that is valid during the call.
        if let Some(control) = unsafe { self.control.as_ref() } {
            control.node.store(node, Relaxed);
        }
    }
}

thread_local! {
    /// The control of the run on this thread, for `stop_requested`. Null outside a run.
    static CONTROL: Cell<*const Control> = const { Cell::new(std::ptr::null()) };
}

pub(crate) fn stop_requested() -> bool {
    let control = CONTROL.with(Cell::get);
    // SAFETY: `run` sets the pointer only during the call, and the engine keeps the control valid.
    unsafe { control.as_ref() }.is_some_and(|c| c.stop.load(Relaxed))
}

/// A command to a cluster.
pub enum Cmd<'a> {
    /// Sets the parameter with this index from its text form (6.4).
    Set(u32, &'a str),
    /// Runs the graph one time. Returns false when a node failed.
    Run,
}

pub type Cluster = Box<dyn FnMut(Cmd<'_>, &Reporter) -> bool>;

/// Boxes the closure of a cluster. The bound gives the closure its argument types.
pub fn cluster<F: FnMut(Cmd<'_>, &Reporter) -> bool + 'static>(f: F) -> Cluster {
    Box::new(f)
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
    let (cluster, rep, bytes) =
        unsafe { (&mut *cluster, &*rep, std::slice::from_raw_parts(text, len)) };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    catch_unwind(AssertUnwindSafe(|| cluster(Cmd::Set(index, text), rep))).unwrap_or(false)
}

/// The body of `patina_cluster_run`.
///
/// # Safety
///
/// `cluster` comes from `new_cluster` and is not freed. `rep` points to a valid reporter, and its
/// control is null or valid during the call.
pub unsafe fn run(cluster: *mut Cluster, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&mut *cluster, &*rep) };
    let outer = CONTROL.replace(rep.control);
    let ok = catch_unwind(AssertUnwindSafe(|| cluster(Cmd::Run, rep))).unwrap_or(false);
    CONTROL.set(outer);
    rep.set_node(IDLE);
    // The cluster has its own standard library, which does not flush its stdout at exit.
    let _ = std::io::Write::flush(&mut std::io::stdout());
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
    fn stop_starts_no_new_node_call() {
        fn make() -> Cluster {
            cluster(|_, rep| {
                // SAFETY: the test gives a control that is valid during the run.
                let node = || unsafe { &*rep.control }.node();
                if stop_requested() { !rep.enter(7) && node().is_none() } else { rep.enter(7) && node() == Some(7) }
            })
        }
        let mut sent: Vec<(u32, u32, String)> = Vec::new();
        let control = Control::default();
        let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep, control: &raw const control };
        let state = new_cluster(make);
        // SAFETY: the cluster comes from `new_cluster` and is freed one time. The reporter and the
        // control are valid during each run.
        unsafe {
            assert!(run(state, &rep));
            assert_eq!(control.node(), None);
            control.set_stop(true);
            assert!(run(state, &rep));
            free(state);
        }
        assert_eq!(sent, [(7, STOPPED, String::new())]);
        assert!(!stop_requested(), "the flag is not visible outside a run");
    }
}
