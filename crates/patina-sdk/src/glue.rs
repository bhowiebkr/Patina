//! The support code of the clusters that Patina generates. Node code does not use it.
//!
//! A cluster keeps its parameters in one closure, so the generated code never names a type of a
//! node crate. Rust infers each type from the node calls.
//!
//! Each call into node code has its own `catch_unwind`, so a panic never crosses the C ABI
//! (spec 4.7).

use std::any::Any;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::Param;

/// The version of the C ABI between the engine and a cluster.
pub const ABI_VERSION: u32 = 1;

/// The kinds of a report.
pub const ERROR: u32 = 1;
pub const PANIC: u32 = 2;
pub const BAD_PARAM: u32 = 3;

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
