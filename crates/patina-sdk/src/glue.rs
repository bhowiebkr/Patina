//! The support code of the clusters that Patina generates. Node code does not use it.
//!
//! A cluster keeps its parameters in one closure, so the generated code never names a type of a
//! node crate. Rust infers each type from the node calls.
//!
//! Each call into node code has its own `catch_unwind`, so a panic never crosses the C ABI
//! (spec 4.7).

use std::any::Any;
use std::ffi::c_void;
use std::fmt::{self, Write as _};
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::Param;

/// The version of the C ABI between the engine and a cluster.
pub const ABI_VERSION: u32 = 2;

/// The kinds of a report.
pub const ERROR: u32 = 1;
pub const PANIC: u32 = 2;
pub const BAD_PARAM: u32 = 3;
/// A live value (15.9). Its node index is the index of the held value, in the order of `Plan::held`.
pub const VALUE: u32 = 4;

/// The longest text of a live value, in bytes. A longer text ends with "...".
pub const VALUE_LEN: usize = 64;

/// The engine function that receives a report: a node index, a kind and a UTF-8 text.
pub type ReportFn = unsafe extern "C" fn(*mut c_void, u32, u32, *const u8, usize);

/// The engine gives a reporter to each call. It stays valid during the call.
#[repr(C)]
pub struct Reporter {
    pub ctx: *mut c_void,
    pub report: ReportFn,
}

impl Reporter {
    pub fn send(&self, node: u32, kind: u32, text: &str) {
        // SAFETY: the engine gives a function and a context that are valid during the call.
        unsafe { (self.report)(self.ctx, node, kind, text.as_ptr(), text.len()) }
    }
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
    let caught = catch_unwind(AssertUnwindSafe(move || {
        drop(value);
        Ok(())
    }));
    finish(caught, rep, node)
}

/// The text of a live value. It does not allocate, and a full text stops the formatting.
pub struct Text {
    buf: [u8; VALUE_LEN + 3],
    len: usize,
    cut: bool,
}

impl Text {
    fn new() -> Text {
        Text { buf: [0; VALUE_LEN + 3], len: 0, cut: false }
    }

    fn finish(&mut self) -> &str {
        if self.cut {
            self.buf[self.len..self.len + 3].copy_from_slice(b"...");
            self.len += 3;
            self.cut = false;
        }
        std::str::from_utf8(&self.buf[..self.len]).unwrap_or_default()
    }
}

impl fmt::Write for Text {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.cut {
            return Err(fmt::Error);
        }
        let mut end = s.len().min(VALUE_LEN - self.len);
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        self.buf[self.len..self.len + end].copy_from_slice(&s.as_bytes()[..end]);
        self.len += end;
        self.cut = end < s.len();
        if self.cut { Err(fmt::Error) } else { Ok(()) }
    }
}

/// A held value for its text. The generated code calls `(&&Probe(v)).text(out)`, so Rust uses
/// `DebugText` when the type implements `Debug`, and `NameText` if not (5.3).
pub struct Probe<'a, T>(pub &'a T);

pub trait DebugText {
    fn text(&self, out: &mut Text);
}

impl<T: fmt::Debug> DebugText for &Probe<'_, T> {
    fn text(&self, out: &mut Text) {
        let _ = write!(out, "{:?}", self.0);
    }
}

pub trait NameText {
    fn text(&self, out: &mut Text);
}

impl<T> NameText for Probe<'_, T> {
    fn text(&self, out: &mut Text) {
        let _ = write!(out, "{} ({} bytes)", std::any::type_name::<T>(), size_of::<T>());
    }
}

/// Sends the text of held value `index` (15.9), with its own catch.
// shortcut: each run formats and sends each held value, send them at the display rate (15.6) when a graph runs faster than the display.
pub fn send_value(rep: &Reporter, index: u32, text: impl FnOnce(&mut Text)) {
    let mut out = Text::new();
    if catch_unwind(AssertUnwindSafe(|| text(&mut out))).is_err() {
        out = Text::new();
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
/// `cluster` comes from `new_cluster` and is not freed, and `rep` points to a valid reporter.
pub unsafe fn run(cluster: *mut Cluster, rep: *const Reporter) -> bool {
    // SAFETY: the caller keeps the rules of this function.
    let (cluster, rep) = unsafe { (&mut *cluster, &*rep) };
    let ok = catch_unwind(AssertUnwindSafe(|| cluster(Cmd::Run, rep))).unwrap_or(false);
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

    unsafe extern "C" fn keep(ctx: *mut c_void, node: u32, kind: u32, text: *const u8, len: usize) {
        // SAFETY: the test gives a `Vec` as the context, and a text that is valid during the call.
        let (sent, bytes) = unsafe { (&mut *ctx.cast::<Vec<(u32, u32, String)>>(), std::slice::from_raw_parts(text, len)) };
        sent.push((node, kind, String::from_utf8(bytes.to_vec()).unwrap()));
    }

    #[test]
    #[allow(clippy::needless_borrow, reason = "the calls are the same as in the generated code")]
    fn values_use_debug_or_the_type_name_and_stay_short() {
        let mut sent: Vec<(u32, u32, String)> = Vec::new();
        let rep = Reporter { ctx: (&raw mut sent).cast(), report: keep };
        send_value(&rep, 0, |out| (&&Probe(&2.5_f64)).text(out));
        send_value(&rep, 1, |out| (&&Probe(&NoDebug)).text(out));
        let long = "\u{e9}".repeat(40);
        send_value(&rep, 2, |out| (&&Probe(&long)).text(out));
        send_value(&rep, 3, |_| panic!("no text"));
        assert_eq!(sent[0], (0, VALUE, "2.5".to_owned()));
        assert!(sent[1].2.ends_with("NoDebug (0 bytes)"), "{}", sent[1].2);
        // The quote and 31 two-byte characters fill 63 bytes. The next character does not fit.
        assert_eq!(sent[2].2, format!("\"{}...", "\u{e9}".repeat(31)));
        assert_eq!(sent[3].2, "the text of the value panicked");
    }
}
